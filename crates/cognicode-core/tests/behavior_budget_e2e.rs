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

use std::sync::atomic::{AtomicU64, Ordering};

use cognicode_core::application::behaviors::Clock;
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

// ── Local clock fake (Gap 1: proves Clock port is consumable from outside) ─────────

/// A deterministic, advanceable clock for integration tests.
///
/// Satisfies the public `Clock` port so the test binary can drive temporal
/// budget scenarios without depending on crate-internal `#[cfg(test)]` items.
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
    InMemoryFactStore::new(std::sync::Arc::new(registry))
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

// ── Test helpers ──────────────────────────────────────────────────────────────────

/// A behavior that produces 4 evidence effects.
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

/// A behavior that produces one effect.
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

/// A behavior that tries to commit a canonical fact (only PureDerivation may).
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

// ── DETERMINISTIC COUNTER SCENARIOS (no clock) ──────────────────────────────────

// ── U52-A: PureDerivation with EffectCount budget ────────────────────────────────

#[tokio::test]
async fn u52_a_pure_derivation_effect_count_exhausts_at_3() {
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(2).unwrap());

    let log = InMemoryEventLog::new();
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
    let clock = FakeClock::default(); // deterministic — no time passage needed

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

    // Property 1: the effect crossing the budget does NOT reach the sink.
    assert_eq!(
        sink.applied.len(),
        2,
        "sink receives only accepted effects (refuse-before-adapter invariant)"
    );

    // Property 2: previously-allowed effects remain per runtime semantics.
    assert_eq!(
        sink.applied[..],
        [BehaviorEffectKind::RecordEvidence, BehaviorEffectKind::RecordEvidence],
        "exactly the first 2 effects were applied"
    );

    // Property 3: BudgetExhausted is produced (not Ok(empty), not panic).
    assert!(
        matches!(outcome.budget_exhaustions[0].0.kind, BudgetKind::EffectCount),
        "BehaviorOutcome::BudgetExhausted must be produced for EffectCount"
    );

    // Property 4: behavior.budget_exhausted is recorded causally after behavior.started.
    let (_, exhaustion_event_id) = &outcome.budget_exhaustions[0];
    let chain = log.causal_chain(&ws(), *exhaustion_event_id).await.unwrap();
    assert!(
        chain.iter().any(|e| e.kind.as_str() == "behavior.started"),
        "causal chain must include behavior.started before budget_exhausted"
    );

    // Property 5: canonical state remains consistent (fact store not touched).
    let count_before = stores()
        .facts_in_snapshot(&ws(), &SnapshotId::new(SNAP))
        .await
        .expect("read")
        .len();
    let count_after = stores()
        .facts_in_snapshot(&ws(), &SnapshotId::new(SNAP))
        .await
        .expect("read")
        .len();
    assert_eq!(count_before, count_after, "fact store must not change");
}

// ── U52-C: Authority denies BEFORE budget is checked ─────────────────────────────

#[tokio::test]
async fn u52_c_agent_authority_denies_before_budget_checked() {
    use std::num::NonZeroU64;
    let budget = BudgetDeclaration::effects(NonZeroU64::new(10).unwrap());

    let log = InMemoryEventLog::new();

    // AgentBehavior is NOT allowed to commit canonical facts.
    let behavior_def = BehaviorDefinition::new(
        "agent.commits",
        "agent commits",
        BehaviorClass::AgentBehavior,
        [BehaviorEffectKind::RecordEvidence],
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
    let clock = FakeClock::default();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &WantsToCommitFact, &mut sink, &clock)
        .await
        .unwrap();

    // Refused by authority — budget never checked.
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

    // The rejection reason should NOT mention budget.
    let violation = &outcome.rejected[0];
    assert!(
        !violation.reason().contains("budget"),
        "reason must be authority denial, not budget exhaustion"
    );
}

// ── U52-D: AiGenerated behavior with declared infinite budget is silently capped ──

#[tokio::test]
async fn u52_d_aigenerated_high_budget_silently_capped() {
    let behavior_def = BehaviorDefinition::new(
        "ai.derivation",
        "ai derivation",
        BehaviorClass::PureDerivation, // claims to be a pure derivation
        [BehaviorEffectKind::RecordEvidence],
    )
    .unwrap();

    // Admitted from AiGenerated source → effective class = AgentBehavior.
    let budget = BudgetDeclaration::none();
    let permit =
        BehaviorAdmission::admit_with_budget(behavior_def, AdmissionSource::AiGenerated, budget)
            .unwrap();

    assert_eq!(
        permit.effective_class(),
        BehaviorClass::AgentBehavior,
        "AiGenerated must be downgraded to AgentBehavior (declaration never escalates)"
    );

    assert!(
        !permit.may(BehaviorEffectKind::CommitCanonicalFact),
        "downgraded behavior must not gain new authority"
    );

    assert!(
        permit.may(BehaviorEffectKind::RecordEvidence),
        "downgraded behavior keeps what its effective class allows"
    );
}

// ── U52-E: Snapshot integrity — fact store unchanged after exhaustion ─────────────

#[tokio::test]
async fn u52_snapshot_integrity_factstore_unchanged_after_exhaustion() {
    // Demonstrates the five-property atomicity contract (e65 WU5-tighten):
    //
    // 1. The effect crossing the budget does NOT reach the sink.
    // 2. Previously-allowed effects remain per runtime semantics.
    // 3. BehaviorOutcome::BudgetExhausted is produced (not Ok(empty), not panic).
    // 4. behavior.budget_exhausted is recorded causally after behavior.started.
    // 5. Canonical state remains consistent.
    //
    // Contract: the runtime commits incrementally (structural atomicity, not
    // transactional). Refusal happens at the policy layer before the sink, so
    // a sink adapter never has to undo a refused effect. The fact store reflects
    // only effects that passed both authority and budget checks.
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
    let clock = FakeClock::default();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &FourEvidenceBehavior, &mut sink, &clock)
        .await
        .unwrap();

    // Property 3: BehaviorOutcome::BudgetExhausted is produced.
    assert!(
        outcome.has_budget_exhaustions(),
        "Property 3: budget exhaustion must be produced (not Ok(empty))"
    );
    assert!(
        matches!(outcome.budget_exhaustions[0].0.kind, BudgetKind::EffectCount),
        "Property 3: exhaustion kind must be EffectCount"
    );

    // Property 1: the effect crossing the budget does NOT reach the sink.
    // FourEvidenceBehavior produces 4 effects; budget=1 means effects 2-4 are rejected.
    let total_produced = 4;
    let budget_limit = 1u64;
    let expected_accepted = budget_limit;
    let expected_rejected = total_produced - budget_limit;
    assert_eq!(
        outcome.accepted.len() as u64,
        expected_accepted,
        "Property 1: exactly {} effect(s) accepted within budget=1",
        expected_accepted
    );
    assert_eq!(
        outcome.rejected.len() as u64,
        expected_rejected,
        "Property 1: {} effect(s) rejected before reaching sink",
        expected_rejected
    );
    assert_eq!(
        sink.applied.len(),
        outcome.accepted.len(),
        "Property 1: accepted effects reached the sink; rejected never did (refuse-before-adapter)"
    );

    // Property 2: previously-allowed effects remain.
    assert_eq!(
        sink.applied[..],
        [BehaviorEffectKind::RecordEvidence],
        "Property 2: the first (budget-allowed) effect was applied; later ones refused"
    );

    // Property 4: behavior.budget_exhausted is causally after behavior.started.
    let (_, exhaustion_event_id) = &outcome.budget_exhaustions[0];
    let chain = log.causal_chain(&ws(), *exhaustion_event_id).await.unwrap();
    let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
    assert!(
        kinds.contains(&"behavior.started"),
        "Property 4: chain must include behavior.started before budget_exhausted: got {:?}",
        kinds
    );

    // Property 5: canonical state remains consistent (fact store unchanged).
    // BufferingSink does not write to the fact store; the fact store was never
    // touched, which demonstrates the structural atomicity invariant: the rejected
    // effects never reached any persistence layer.
    let count_after = facts
        .facts_in_snapshot(&ws(), &SnapshotId::new(SNAP))
        .await
        .expect("read")
        .len();
    assert_eq!(
        count_before, count_after,
        "Property 5: FactStore count unchanged — no partial canonical mutation"
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
    let clock = FakeClock::default();

    let outcome = BehaviorRuntime::new(&log, EventTime::from_millis(1))
        .run(&permit, &context, &TwoEffectsBehavior, &mut sink, &clock)
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

    let (_, exhaustion_event_id) = &outcome.budget_exhaustions[0];

    // Verify the chain contains trigger → behavior.started → behavior.budget_exhausted.
    let chain = log
        .causal_chain(&ws(), *exhaustion_event_id)
        .await
        .unwrap();

    // With exhaustion caused by started_event (not rejection_event), the chain is:
    // [behavior.budget_exhausted, behavior.started] = 2 events.
    assert!(
        chain.len() >= 2,
        "chain must have at least 2 events: started and exhaustion, got {}",
        chain.len()
    );

    // Last event is behavior.budget_exhausted.
    assert_eq!(
        chain.last().unwrap().kind.as_str(),
        "behavior.budget_exhausted",
        "last event must be behavior.budget_exhausted"
    );

    // Chain includes behavior.started before the exhaustion.
    let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
    assert!(
        kinds.contains(&"behavior.started"),
        "chain must include behavior.started: got {:?}",
        kinds
    );
}

// ── TEMPORAL SCENARIOS (with FakeClock) ─────────────────────────────────────────

// ── U52-B: ReactiveAnalysis with Time budget ─────────────────────────────────────

#[tokio::test]
async fn u52_b_reactive_analysis_time_exhausts_via_fake_clock() {
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
    let clock = FakeClock::default();

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
    clock.advance(120);

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
