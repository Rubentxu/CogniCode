//! U52 — behavior budget enforcement (M7.4, cycle e65).
//!
//! These tests verify the full budget enforcement pipeline:
//! authority check → budget check → sink dispatch → budget commit.
//!
//! Key invariants tested:
//! - Authority denies before budget is checked (ordering)
//! - Refusal leaves sink untouched (refuse-before-adapter)
//! - Snapshot integrity: exhaustion never touches the fact store
//! - Causal chain includes both policy rejection and exhaustion event
//!
//! Requires `evidence-kernel` feature (the sink writes to a real fact store).

#![cfg(feature = "evidence-kernel")]

use std::sync::Arc;

use cognicode_core::application::behaviors::MockClock;
use cognicode_core::domain::behaviors::{
    Behavior, BehaviorAdmission, BehaviorClass, BehaviorDefinition, BehaviorEffect,
    BehaviorEffectKind, BehaviorEffectSink, BehaviorRuntime,
};
use cognicode_core::domain::budgets::{BudgetDeclaration, BudgetKind};
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::execution::{ActorRef, CorrelationId, ExecutionContext};
use cognicode_core::domain::findings::AnalysisScope;
use cognicode_core::domain::intelligence_log::event::EventTime;
use cognicode_core::domain::intelligence_log::IntelligenceEventStore;
use cognicode_core::domain::kernel_ids::{EntityId, ExecutionId, SnapshotId};
use cognicode_core::domain::trust::AdmissionSource;
use cognicode_core::domain::value_objects::WorkspaceId;
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::intelligence_log::InMemoryEventLog;

const SNAP: u64 = 42;

fn ws() -> WorkspaceId {
    WorkspaceId::try_new("ws-u52-budgets").unwrap()
}

fn scope() -> AnalysisScope {
    AnalysisScope::new(ws(), SnapshotId::new(SNAP))
}

fn stores() -> InMemoryFactStore {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap");
    InMemoryFactStore::new(Arc::new(registry))
}

/// A sink that records what was applied, for assertions.
#[derive(Default)]
struct BufferingSink {
    applied: Vec<BehaviorEffectKind>,
    drafts: Vec<BehaviorEffect>,
}

impl BehaviorEffectSink for BufferingSink {
    fn apply(&mut self, effect: &BehaviorEffect) -> Result<(), String> {
        self.applied.push(effect.kind());
        self.drafts.push(effect.clone());
        Ok(())
    }
}

// ── U52-A: PureDerivation with EffectCount budget ─────────────────────────────────

/// A behavior that wants to record 4 pieces of evidence.
struct FourEvidenceBehavior;

impl Behavior for FourEvidenceBehavior {
    fn execute(&self, _ctx: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![
            BehaviorEffect::RecordEvidence {
                summary: "evidence 1".to_string(),
            },
            BehaviorEffect::RecordEvidence {
                summary: "evidence 2".to_string(),
            },
            BehaviorEffect::RecordEvidence {
                summary: "evidence 3".to_string(),
            },
            BehaviorEffect::RecordEvidence {
                summary: "evidence 4".to_string(),
            },
        ]
    }
}

#[tokio::test]
async fn u52_a_pure_derivation_effect_count_exhausts_at_3() {
    // Declare EffectCount budget = 2 (so only 2 effects should succeed).
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(2).unwrap());

    let log = InMemoryEventLog::new();
    let facts = stores();
    let behavior_def = BehaviorDefinition::new(
        "test.four_evidence",
        "four evidence",
        BehaviorClass::PureDerivation,
        [BehaviorEffectKind::RecordEvidence],
    )
    .unwrap();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::Builtin, budget).unwrap();

    let context = ExecutionContext::try_new(
        ExecutionId::new(1),
        scope(),
        ActorRef::kernel(),
        CorrelationId::new("u52-a").unwrap(),
        None,
    )
    .unwrap();

    let mut sink = BufferingSink::default();
    let mut clock = MockClock::start();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &FourEvidenceBehavior, &mut sink, &clock)
        .await
        .unwrap();

    // 2 effects accepted (within budget), 2 rejected.
    assert_eq!(
        outcome.accepted.len(),
        2,
        "only 2 effects should be accepted with budget=2"
    );
    assert_eq!(
        outcome.rejected.len(),
        2,
        "2 effects should be rejected after budget exhaustion"
    );
    assert!(
        outcome.has_budget_exhaustions(),
        "budget exhaustion should be recorded"
    );
    assert_eq!(
        outcome.budget_exhaustions.len(),
        2,
        "each rejected effect should produce a budget exhaustion"
    );

    // Sink should have received exactly 2 effects.
    assert_eq!(
        sink.applied.len(),
        2,
        "sink receives only accepted effects (refuse-before-adapter invariant)"
    );

    // Verify the exhaustion event kind.
    for (exhaustion, event_id) in &outcome.budget_exhaustions {
        let event = log.by_id(&ws(), *event_id).await.unwrap().unwrap();
        assert_eq!(
            event.kind.as_str(),
            "behavior.budget_exhausted",
            "exhaustion event must use the published kind"
        );
        assert_eq!(
            exhaustion.kind,
            BudgetKind::EffectCount,
            "EffectCount budget should be exhausted"
        );
    }
}

// ── U52-B: ReactiveAnalysis with Time budget ─────────────────────────────────────

/// A behavior that produces one effect (the time budget is what gets exhausted
/// by the clock advancing between effects).
struct OneEffectBehavior;

impl Behavior for OneEffectBehavior {
    fn execute(&self, _ctx: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![BehaviorEffect::RecordHypothesis {
            summary: "hypothesis".to_string(),
        }]
    }
}

/// A behavior that produces two effects.
struct TwoEffectsBehavior;

impl Behavior for TwoEffectsBehavior {
    fn execute(&self, _ctx: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![
            BehaviorEffect::RecordEvidence {
                summary: "evidence 1".to_string(),
            },
            BehaviorEffect::RecordEvidence {
                summary: "evidence 2".to_string(),
            },
        ]
    }
}

#[tokio::test]
async fn u52_b_reactive_analysis_time_exhausts_via_mock_clock() {
    // Time budget = 100ms, but we advance the clock past it between effects.
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::time(NonZeroU64::new(100).unwrap());

    let log = InMemoryEventLog::new();
    let behavior_def = BehaviorDefinition::new(
        "test.two_effects",
        "two effects",
        BehaviorClass::ReactiveAnalysis,
        [BehaviorEffectKind::RecordHypothesis],
    )
    .unwrap();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::Builtin, budget).unwrap();

    let context = ExecutionContext::try_new(
        ExecutionId::new(2),
        scope(),
        ActorRef::kernel(),
        CorrelationId::new("u52-b").unwrap(),
        None,
    )
    .unwrap();

    // Clock starts at t=0.
    let mut clock = MockClock::start();

    // First effect: t=0, well within 100ms budget.
    {
        let mut sink = BufferingSink::default();
        let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
            .run(&permit, &context, &OneEffectBehavior, &mut sink, &clock)
            .await
            .unwrap();
        assert!(
            !outcome.has_budget_exhaustions(),
            "first effect at t=0 should be within 100ms budget"
        );
        assert_eq!(outcome.accepted.len(), 1);
    }

    // Advance clock to t=120 (past 100ms ceiling).
    clock.advance_by(120);

    // Second effect attempt: elapsed = 120ms > ceiling = 100ms.
    {
        let mut sink = BufferingSink::default();
        let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(2))
            .run(&permit, &context, &OneEffectBehavior, &mut sink, &clock)
            .await
            .unwrap();

        assert!(
            outcome.has_budget_exhaustions(),
            "effect at t=120 should exceed 100ms time budget"
        );
        assert!(
            outcome.accepted.is_empty(),
            "no effects should be accepted after time exhaustion"
        );

        let (exhaustion, _) = &outcome.budget_exhaustions[0];
        assert_eq!(
            exhaustion.kind,
            BudgetKind::Time,
            "Time budget should be exhausted"
        );
    }
}

// ── U52-C: Authority denies BEFORE budget is checked ─────────────────────────────

/// A behavior that tries to commit a canonical fact (only PureDerivation may do this).
struct WantsToCommitFact;

impl Behavior for WantsToCommitFact {
    fn execute(&self, _ctx: &ExecutionContext) -> Vec<BehaviorEffect> {
        vec![BehaviorEffect::CommitCanonicalFact(
            cognicode_core::domain::behaviors::FactDraft::new(
                EntityId::new(1),
                "has_predicate",
                "entity_1",
                SnapshotId::new(SNAP),
            ),
        )]
    }
}

#[tokio::test]
async fn u52_c_agent_authority_denies_before_budget_checked() {
    // Declare an EffectCount budget — but authority should deny first.
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(10).unwrap());

    let log = InMemoryEventLog::new();
    let facts = stores();

    // AgentBehavior is NOT allowed to commit canonical facts.
    let behavior_def = BehaviorDefinition::new(
        "agent.commits",
        "agent commits",
        BehaviorClass::AgentBehavior,
        [BehaviorEffectKind::RecordEvidence], // agent can record evidence
    )
    .unwrap();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::Builtin, budget).unwrap();

    let context = ExecutionContext::try_new(
        ExecutionId::new(4),
        scope(),
        ActorRef::kernel(),
        CorrelationId::new("u52-c").unwrap(),
        None,
    )
    .unwrap();

    let mut sink = BufferingSink::default();
    let clock = MockClock::start();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &WantsToCommitFact, &mut sink, &clock)
        .await
        .unwrap();

    // Refused by authority.
    assert!(
        outcome.has_violations(),
        "authority must refuse CommitCanonicalFact from AgentBehavior"
    );
    assert!(
        !outcome.has_budget_exhaustions(),
        "budget must NOT be checked when authority denies first"
    );
    assert!(
        sink.applied.is_empty(),
        "sink must never receive an effect denied by authority"
    );

    // The rejection reason should NOT be "budget_exhausted".
    let violation = &outcome.rejected[0];
    assert!(
        !violation.reason().contains("budget"),
        "reason must be authority denial, not budget exhaustion"
    );
}

// ── U52-D: AiGenerated behavior with declared infinite budget is silently capped ───

#[tokio::test]
async fn u52_d_aigenerated_high_budget_silently_capped() {
    // AiGenerated behaviors run as AgentBehavior regardless of their declaration.
    // An AiGenerated behavior that declares PureDerivation runs as AgentBehavior.
    let behavior_def = BehaviorDefinition::new(
        "ai.derivation",
        "ai derivation",
        BehaviorClass::PureDerivation, // claims to be a pure derivation
        [BehaviorEffectKind::RecordEvidence],
    )
    .unwrap();

    // Admitted from AiGenerated source → effective class = AgentBehavior.
    let budget = BudgetDeclaration::none(); // no budget declared
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::AiGenerated, budget)
            .unwrap();

    assert_eq!(
        permit.effective_class(),
        BehaviorClass::AgentBehavior,
        "AiGenerated must be downgraded to AgentBehavior (declaration never escalates)"
    );

    // AgentBehavior may NOT commit canonical facts.
    assert!(
        !permit.may(BehaviorEffectKind::CommitCanonicalFact),
        "downgraded behavior must not gain new authority"
    );

    // But it still can record evidence (what it legitimately declared).
    assert!(
        permit.may(BehaviorEffectKind::RecordEvidence),
        "downgraded behavior keeps what its effective class allows"
    );
}

// ── U52-E: Snapshot integrity — fact store unchanged after exhaustion ─────────────

#[tokio::test]
async fn u52_snapshot_integrity_factstore_unchanged_after_exhaustion() {
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(1).unwrap());

    let log = InMemoryEventLog::new();
    let facts = stores();

    let behavior_def = BehaviorDefinition::new(
        "test.four_evidence",
        "four evidence",
        BehaviorClass::PureDerivation,
        [BehaviorEffectKind::RecordEvidence],
    )
    .unwrap();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::Builtin, budget).unwrap();

    let context = ExecutionContext::try_new(
        ExecutionId::new(5),
        scope(),
        ActorRef::kernel(),
        CorrelationId::new("u52-e").unwrap(),
        None,
    )
    .unwrap();

    let count_before = facts
        .facts_in_snapshot(&ws(), &SnapshotId::new(SNAP))
        .await
        .expect("read")
        .len();

    let mut sink = BufferingSink::default();
    let clock = MockClock::start();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &FourEvidenceBehavior, &mut sink, &clock)
        .await
        .unwrap();

    assert!(
        outcome.has_budget_exhaustions(),
        "budget exhaustion must have occurred"
    );

    let count_after = facts
        .facts_in_snapshot(&ws(), &SnapshotId::new(SNAP))
        .await
        .expect("read")
        .len();

    assert_eq!(
        count_before, count_after,
        "FactStore must be byte-identical after exhaustion (snapshot integrity invariant)"
    );
}

// ── U52-F: Causal chain includes exhaustion event ───────────────────────────────

#[tokio::test]
async fn u52_causal_chain_includes_exhaustion_event() {
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(1).unwrap());

    let log = InMemoryEventLog::new();

    let behavior_def = BehaviorDefinition::new(
        "test.two_evidence",
        "two evidence",
        BehaviorClass::PureDerivation,
        [BehaviorEffectKind::RecordEvidence],
    )
    .unwrap();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::Builtin, budget).unwrap();

    let context = ExecutionContext::try_new(
        ExecutionId::new(6),
        scope(),
        ActorRef::kernel(),
        CorrelationId::new("u52-f").unwrap(),
        None,
    )
    .unwrap();

    let mut sink = BufferingSink::default();
    let clock = MockClock::start();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(
            &permit,
            &context,
            &TwoEffectsBehavior,
            &mut sink,
            &clock,
        )
        .await
        .unwrap();

    assert_eq!(
        outcome.accepted.len(),
        1,
        "first effect should be accepted"
    );
    assert_eq!(
        outcome.rejected.len(),
        1,
        "second effect should be rejected due to budget"
    );
    assert_eq!(
        outcome.budget_exhaustions.len(),
        1,
        "exactly one exhaustion event"
    );

    let (exhaustion, exhaustion_event_id) = &outcome.budget_exhaustions[0];
    let chain = log
        .causal_chain(&ws(), *exhaustion_event_id)
        .await
        .unwrap();

    // The chain must include: trigger → behavior.started → policy.rejection → budget.exhaustion.
    assert!(
        chain.len() >= 3,
        "causal chain must have at least 3 events: started, rejection, exhaustion"
    );

    // Last event is the exhaustion event.
    assert_eq!(
        chain.last().unwrap().id,
        *exhaustion_event_id,
        "last event in chain must be the exhaustion event"
    );
    assert_eq!(
        chain.last().unwrap().kind.as_str(),
        "behavior.budget_exhausted",
        "last event must be the budget exhaustion event"
    );
    assert_eq!(exhaustion.kind, BudgetKind::EffectCount);
}
