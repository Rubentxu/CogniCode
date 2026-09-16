//! U52 — the behavior authority boundary (M7.3, cycle e64, ADR-044).
//!
//! The point of this test is **not** that an `AgentBehavior` cannot write a fact
//! with LLM provenance. The kernel already refuses that (ADR-040), and
//! re-proving it would say nothing about the behavior class.
//!
//! The fact the agent attempts here is one that would be **perfectly valid** if a
//! `PureDerivation` had produced it: a registered predicate, the right snapshot,
//! deterministic-analyzer provenance. The only reason it is refused is the class
//! of the behavior that asked — which is what makes this a test of ADR-044.
//!
//! ```text
//! trigger event
//!       │ trigger_event
//!       ▼
//! ExecutionContext
//!       │
//!       ▼
//! BehaviorAdmission  source = AiGenerated, declared = PureDerivation
//!       │
//!       ▼
//! BehaviorPermit     effective = AgentBehavior
//!       │
//!       ▼
//! BehaviorRuntime ── behavior.started
//!       │
//!       ▼
//! attempt CommitCanonicalFact(valid draft)
//!       │
//!       ▼
//! BehaviorAuthorityPolicy ── DENY ── policy.behavior_output_rejected
//! ```
//!
//! Requires the `evidence-kernel` feature (the sink writes to a real fact store).

#![cfg(feature = "evidence-kernel")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::application::behaviors::Clock;
use cognicode_core::domain::behaviors::{
    Behavior, BehaviorAdmission, BehaviorAdmissionError, BehaviorClass, BehaviorDefinition,
    BehaviorEffect, BehaviorEffectKind, BehaviorEffectSink, BehaviorPermit, BehaviorRuntime,
    FactDraft,
};
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::execution::{ActorRef, CorrelationId, ExecutionContext};
use cognicode_core::domain::findings::AnalysisScope;
use cognicode_core::domain::intelligence_log::event::EventTime;
use cognicode_core::domain::intelligence_log::kind::EventKinds;
use cognicode_core::domain::intelligence_log::payload::EventPayloadRef;
use cognicode_core::domain::intelligence_log::{
    IntelligenceEvent, IntelligenceEventStore, NewIntelligenceEvent,
};
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, FactId, SnapshotId};
use cognicode_core::domain::trust::AdmissionSource;
use cognicode_core::domain::value_objects::{Provenance, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::intelligence_log::InMemoryEventLog;

// ── Local clock fake (same pattern as behavior_budget_e2e.rs) ───────────────────

#[derive(Default)]
struct FakeClock {
    millis: AtomicU64,
}

impl FakeClock {
    fn advance(&self, ms: u64) {
        self.millis.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_millis(&self) -> u64 {
        self.millis.load(Ordering::SeqCst)
    }
}

const SNAP: u64 = 1;

fn workspace() -> WorkspaceId {
    WorkspaceId::try_new("ws-u52").unwrap()
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(workspace(), SnapshotId::new(SNAP))
}

fn stores() -> InMemoryFactStore {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    InMemoryFactStore::new(Arc::new(registry))
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        Some("u52 fixture".to_string()),
    )
}

/// A draft that a `PureDerivation` would be entirely within its rights to commit.
fn valid_draft() -> FactDraft {
    FactDraft::new(
        EntityId::new(1),
        "core:calls",
        "entity 1001",
        SnapshotId::new(SNAP),
    )
}

/// The behavior under test: it wants to commit canonical truth.
struct WantsToCommit {
    draft: FactDraft,
}

impl Behavior for WantsToCommit {
    fn execute(&self, _context: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![BehaviorEffect::CommitCanonicalFact(self.draft.clone())]
    }
}

/// A behavior that only asks for what its class is allowed to do.
struct WantsToPropose;

impl Behavior for WantsToPropose {
    fn execute(&self, _context: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![BehaviorEffect::ProposeChange {
            summary: "rename the module".to_string(),
        }]
    }
}

/// An effect sink that buffers what it is given.
///
/// It buffers rather than writing, so the assertion "the store is unchanged" is
/// about the runtime having never reached the sink, and the positive control can
/// show the very same draft landing afterwards.
#[derive(Default)]
struct BufferingSink {
    applied: Vec<BehaviorEffectKind>,
    drafts: Vec<FactDraft>,
}

impl BehaviorEffectSink for BufferingSink {
    fn apply(&mut self, effect: &BehaviorEffect) -> Result<(), String> {
        self.applied.push(effect.kind());
        if let BehaviorEffect::CommitCanonicalFact(draft) = effect {
            self.drafts.push(draft.clone());
        }
        Ok(())
    }
}

/// Record the root trigger event and return its id.
async fn record_trigger(log: &InMemoryEventLog, correlation: &CorrelationId) -> u64 {
    let ids = log
        .append(
            &workspace(),
            vec![NewIntelligenceEvent {
                scope: scope(),
                actor: ActorRef::human("reviewer"),
                correlation: correlation.clone(),
                caused_by: None,
                kind: EventKinds::source_delta(),
                payload: EventPayloadRef::inline(
                    cognicode_core::domain::intelligence_log::BoundedEventPayload::new(
                        "a file changed",
                    )
                    .unwrap(),
                ),
                occurred_at: EventTime::from_millis(1),
            }],
        )
        .await
        .expect("trigger event");
    ids[0].get()
}

fn context(trigger: Option<u64>) -> ExecutionContext {
    ExecutionContext::try_new(
        ExecutionId::new(42),
        scope(),
        // Deliberately a *human* actor: the actor kind must not decide the class.
        ActorRef::human("reviewer"),
        CorrelationId::new("corr-u52").unwrap(),
        trigger.map(cognicode_core::domain::kernel_ids::EventId::new),
    )
    .unwrap()
}

/// A behavior that declares itself a pure derivation and asks to commit facts.
fn declaring_pure_derivation() -> BehaviorDefinition {
    BehaviorDefinition::new(
        "derivation.symbol_index",
        "symbol index",
        BehaviorClass::PureDerivation,
        [BehaviorEffectKind::CommitCanonicalFact],
    )
    .expect("a coherent definition")
}

// ============================================================================
// The exit gate
// ============================================================================

#[tokio::test]
async fn u52_an_agent_behavior_cannot_commit_a_valid_fact() {
    let facts = stores();
    let log = InMemoryEventLog::new();
    let correlation = CorrelationId::new("corr-u52").unwrap();
    let trigger = record_trigger(&log, &correlation).await;

    // The author claims a pure derivation; the source is an AI agent.
    let permit =
        BehaviorAdmission::admit(declaring_pure_derivation(), AdmissionSource::AiGenerated)
            .expect("admission never fails for a coherent definition");

    // The class the run actually carries is the downgraded one.
    assert_eq!(permit.effective_class(), BehaviorClass::AgentBehavior);
    assert!(permit.admitted().was_downgraded());

    let before = facts
        .facts_in_snapshot(&workspace(), &SnapshotId::new(SNAP))
        .await
        .expect("read");

    let mut sink = BufferingSink::default();
    let clock = FakeClock::default();
    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(2))
        .run(
            &permit,
            &context(Some(trigger)),
            &WantsToCommit {
                draft: valid_draft(),
            },
            &mut sink,
            &clock,
        )
        .await
        .expect("the run itself succeeds: a refusal is an outcome, not a crash");

    // ── The refusal is structural: the sink was never reached ─────────────
    assert_eq!(outcome.effective_class, BehaviorClass::AgentBehavior);
    assert!(outcome.has_violations());
    assert!(outcome.accepted.is_empty());
    assert_eq!(
        outcome.completed_event, None,
        "a refused run does not complete"
    );
    assert!(sink.applied.is_empty(), "no effect may reach an adapter");
    assert!(sink.drafts.is_empty());

    let after = facts
        .facts_in_snapshot(&workspace(), &SnapshotId::new(SNAP))
        .await
        .expect("read");
    assert_eq!(before, after, "FactStore before == FactStore after");

    // ── The violation is specific ─────────────────────────────────────────
    let violation = &outcome.rejected[0];
    assert_eq!(violation.effect, BehaviorEffectKind::CommitCanonicalFact);
    assert_eq!(violation.class, BehaviorClass::AgentBehavior);
    assert_eq!(violation.behavior_id, "derivation.symbol_index");
    assert_eq!(
        violation.reason(),
        "a agent_behavior behavior may not commit_canonical_fact"
    );

    // ── The causal chain is exactly the designed one ──────────────────────
    let rejection_event = outcome.rejection_events[0];
    let chain = log
        .causal_chain(&workspace(), rejection_event)
        .await
        .expect("chain");
    let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "kernel.source_delta",
            "behavior.started",
            "policy.behavior_output_rejected",
        ],
        "trigger → started → rejected"
    );
    assert_eq!(chain[0].id.get(), trigger);
    assert_eq!(chain[1].caused_by, chain[0].id.into());
    assert_eq!(chain[2].caused_by, Some(chain[1].id));

    // ── The violation is navigable ────────────────────────────────────────
    assert_eq!(
        chain[1].actor,
        ActorRef::behavior("derivation.symbol_index")
    );
    assert_eq!(
        chain[2].actor,
        ActorRef::behavior("derivation.symbol_index")
    );
    let fields = cognicode_core::domain::behaviors::runtime::event_fields(&chain[2]);
    assert_eq!(
        fields.get("behavior_id").map(String::as_str),
        Some("derivation.symbol_index")
    );
    assert_eq!(
        fields.get("class").map(String::as_str),
        Some("agent_behavior")
    );
    assert_eq!(
        fields.get("effect").map(String::as_str),
        Some("commit_canonical_fact")
    );
    assert_eq!(
        fields.get("execution_id").map(String::as_str),
        Some("exec:42")
    );
    assert!(fields.contains_key("reason"));

    // ── One correlation end to end ────────────────────────────────────────
    let correlated = log
        .by_correlation(&workspace(), &correlation)
        .await
        .expect("correlation");
    assert_eq!(correlated.len(), 3, "trigger, started, rejected");
    assert!(correlated.iter().all(|e| e.correlation == correlation));
}

/// The control. The *same* draft, requested by a behavior that is allowed to
/// commit facts, is accepted — so the refusal above is about the class and not
/// about the draft being unusable.
#[tokio::test]
async fn u52_the_same_fact_is_accepted_from_a_curated_pure_derivation() {
    let facts = stores();
    let log = InMemoryEventLog::new();
    let correlation = CorrelationId::new("corr-u52").unwrap();
    let trigger = record_trigger(&log, &correlation).await;

    let permit =
        BehaviorAdmission::admit(declaring_pure_derivation(), AdmissionSource::HumanCurated)
            .unwrap();
    assert_eq!(permit.effective_class(), BehaviorClass::PureDerivation);
    assert!(!permit.admitted().was_downgraded());

    let mut sink = BufferingSink::default();
    let clock = FakeClock::default();
    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(2))
        .run(
            &permit,
            &context(Some(trigger)),
            &WantsToCommit {
                draft: valid_draft(),
            },
            &mut sink,
            &clock,
        )
        .await
        .unwrap();

    assert!(!outcome.has_violations());
    assert_eq!(
        outcome.accepted,
        vec![BehaviorEffectKind::CommitCanonicalFact]
    );
    assert!(outcome.completed_event.is_some());
    assert_eq!(sink.drafts.len(), 1, "the sink received the effect");

    // And the draft really was committable: the same bytes land in the kernel.
    let draft = &sink.drafts[0];
    facts
        .commit(
            &workspace(),
            &SnapshotId::new(SNAP),
            vec![
                Fact::new(
                    FactId::new(1),
                    draft.subject,
                    RelationKind::try_new(&draft.predicate).expect("valid predicate"),
                    FactValue::Text(draft.object.clone()),
                    draft.snapshot,
                    provenance(),
                )
                .expect("valid fact"),
            ],
        )
        .await
        .expect("commit");
    assert_eq!(
        facts
            .facts_in_snapshot(&workspace(), &SnapshotId::new(SNAP))
            .await
            .unwrap()
            .len(),
        1,
        "a curated pure derivation can commit exactly this fact"
    );

    // The chain of an accepted run reads trigger → started → completed.
    let chain = log
        .causal_chain(&workspace(), outcome.completed_event.unwrap())
        .await
        .unwrap();
    let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
    assert_eq!(
        kinds,
        vec![
            "kernel.source_delta",
            "behavior.started",
            "behavior.completed"
        ]
    );
}

#[tokio::test]
async fn u52_a_behavior_keeps_what_its_downgraded_class_still_allows() {
    let log = InMemoryEventLog::new();
    let correlation = CorrelationId::new("corr-u52-allowed").unwrap();
    let trigger = record_trigger(&log, &correlation).await;

    // Same AI-generated downgrade, but it only asks to propose.
    let permit = BehaviorAdmission::admit(
        BehaviorDefinition::new(
            "agent.refactor_plan",
            "refactor plan",
            BehaviorClass::AgentBehavior,
            [BehaviorEffectKind::ProposeChange],
        )
        .unwrap(),
        AdmissionSource::AiGenerated,
    )
    .unwrap();

    let mut sink = BufferingSink::default();
    let clock = FakeClock::default();
    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(3))
        .run(
            &permit,
            &context(Some(trigger)),
            &WantsToPropose,
            &mut sink,
            &clock,
        )
        .await
        .unwrap();

    assert!(!outcome.has_violations());
    assert_eq!(outcome.accepted, vec![BehaviorEffectKind::ProposeChange]);
    assert_eq!(sink.applied, vec![BehaviorEffectKind::ProposeChange]);
}

/// A behavior cannot mint authority: it can only ask for an analysis that
/// admission already permitted, and the request carries no permit.
#[tokio::test]
async fn u52_requesting_an_analysis_grants_no_new_authority() {
    let log = InMemoryEventLog::new();
    let correlation = CorrelationId::new("corr-u52-exec").unwrap();
    let trigger = record_trigger(&log, &correlation).await;

    struct WantsAnalysis;
    impl Behavior for WantsAnalysis {
        fn execute(&self, _context: &ExecutionContext) -> Vec<BehaviorEffect> {
            vec![BehaviorEffect::ExecuteAdmittedAnalysis {
                detector_id: "security.weak_hash".to_string(),
            }]
        }
    }

    let permit = BehaviorAdmission::admit(
        BehaviorDefinition::new(
            "agent.analysis",
            "analysis",
            BehaviorClass::AgentBehavior,
            [BehaviorEffectKind::ExecuteAdmittedAnalysis],
        )
        .unwrap(),
        AdmissionSource::AiGenerated,
    )
    .unwrap();

    let mut sink = BufferingSink::default();
    let clock = FakeClock::default();
    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(4))
        .run(
            &permit,
            &context(Some(trigger)),
            &WantsAnalysis,
            &mut sink,
            &clock,
        )
        .await
        .unwrap();

    assert_eq!(
        outcome.accepted,
        vec![BehaviorEffectKind::ExecuteAdmittedAnalysis]
    );
    // The effect carries a detector *name*, not a permit: the only route to a
    // finding is still DetectorExecutor, which demands an ExecutionPermit that
    // only DetectorAdmission can mint.
    assert!(matches!(
        sink.applied.as_slice(),
        [BehaviorEffectKind::ExecuteAdmittedAnalysis]
    ));
}

#[tokio::test]
async fn u52_a_permit_cannot_be_obtained_without_admission() {
    // Compile-time boundary: there is no public constructor, no `Default`, no
    // `From<BehaviorDefinition>`, and no serde. The only route is `admit`.
    fn only_admission_can_mint(
        definition: BehaviorDefinition,
        source: AdmissionSource,
    ) -> Result<BehaviorPermit, BehaviorAdmissionError> {
        BehaviorAdmission::admit(definition, source)
    }
    let permit =
        only_admission_can_mint(declaring_pure_derivation(), AdmissionSource::AiGenerated).unwrap();
    assert_eq!(permit.effective_class(), BehaviorClass::AgentBehavior);
    assert!(!permit.may(BehaviorEffectKind::CommitCanonicalFact));
}

/// The log records the class the run actually carried, not the one it claimed.
#[tokio::test]
async fn u52_the_log_records_the_effective_class_and_the_claim() {
    let log = InMemoryEventLog::new();
    let correlation = CorrelationId::new("corr-u52-log").unwrap();
    let trigger = record_trigger(&log, &correlation).await;

    let permit =
        BehaviorAdmission::admit(declaring_pure_derivation(), AdmissionSource::AiGenerated)
            .unwrap();
    let mut sink = BufferingSink::default();
    let clock = FakeClock::default();
    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(5))
        .run(
            &permit,
            &context(Some(trigger)),
            &WantsToCommit {
                draft: valid_draft(),
            },
            &mut sink,
            &clock,
        )
        .await
        .unwrap();

    let started = log
        .by_id(&workspace(), outcome.started_event)
        .await
        .unwrap()
        .unwrap();
    let fields = cognicode_core::domain::behaviors::runtime::event_fields(&started);
    assert_eq!(
        fields.get("class").map(String::as_str),
        Some("agent_behavior")
    );
    assert_eq!(
        fields.get("declared_class").map(String::as_str),
        Some("pure_derivation"),
        "the claim is recorded, but it is not what the run ran as"
    );
}

/// Unused import guard: the event type is part of the public reporting surface.
#[allow(dead_code)]
fn _event_type_is_public(event: &IntelligenceEvent) -> String {
    event.kind.to_string()
}
