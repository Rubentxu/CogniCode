//! e70 WU1 — Local CI vertical orchestration.
//!
//! Glues e67 (grounding), e68 (semantic diff + planner), and e69
//! (bundle + gate) into a single in-process vertical. The vertical
//! is deterministic and testable: a [`WorkExecutor`] trait abstracts
//! execution so the first impl ([`InMemoryWorkExecutor`]) can run
//! registered fixtures. A real async shell-out executor is reserved
//! for a future e70.x cycle.
//!
//! ## Architecture invariants
//!
//! - Facts remain canonical: nothing in e70 writes to the Evidence
//!   Kernel.
//! - Evidence carries proof, never authority by itself: producers
//!   still propose; the gate still decides.
//! - Derived operational decisions are not Facts:
//!   [`LocalVerticalReport`] is derived.
//! - AI/plugins propose; they do not mint authority.
//! - Unknown/incomplete never becomes "safe": the planner's Unknown
//!   disposition is preserved end-to-end; the gate's invariants
//!   apply unchanged.

use std::collections::BTreeMap;

use crate::application::change_tracking::planner::{
    AffectedWorkPlan, ExecutionDependency, WorkDecision, WorkDisposition, WorkId,
};
use crate::application::evidence_bundle::producer::{EvidenceProducer, ProducerOutput, aggregate};
use crate::application::evidence_bundle::{EvidenceBundleId, ProducerSource};
use crate::application::policy_gate::{PolicyDecision, PolicySpec, evaluate};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::semantic_diff::FactDelta;
use crate::domain::kernel_ids::ExecutionId;

/// One piece of work a [`WorkExecutor`] can run.
///
/// The trait abstracts what e70 actually does — in tests it returns
/// a static list of [`ProducerOutput`]; in production (e70.x) it
/// may shell out to cargo/just/cogh.
pub trait WorkExecutor: Send + Sync {
    /// Execute the work for one `WorkId` and return the producer
    /// outputs that will feed the bundle.
    ///
    /// MUST be total: every WorkId the planner asks about produces
    /// a `Vec<ProducerOutput>` (possibly empty). MUST NOT panic.
    fn execute(&self, work: &WorkId) -> Vec<ProducerOutput>;
}

/// In-memory executor for tests and e70 initial. Holds a registry
/// of `WorkId → fn` pairs; missing WorkIds produce an empty output
/// list (no panic).
pub struct InMemoryWorkExecutor {
    by_work: BTreeMap<WorkId, Vec<ProducerOutput>>,
}

impl InMemoryWorkExecutor {
    /// Construct an empty executor.
    pub fn new() -> Self {
        Self {
            by_work: BTreeMap::new(),
        }
    }

    /// Register a fixed list of producer outputs for one WorkId.
    pub fn register(&mut self, work: WorkId, outputs: Vec<ProducerOutput>) {
        self.by_work.insert(work, outputs);
    }

    /// Number of registered works.
    pub fn len(&self) -> usize {
        self.by_work.len()
    }

    /// True iff no works are registered.
    pub fn is_empty(&self) -> bool {
        self.by_work.is_empty()
    }
}

impl Default for InMemoryWorkExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkExecutor for InMemoryWorkExecutor {
    fn execute(&self, work: &WorkId) -> Vec<ProducerOutput> {
        self.by_work.get(work).cloned().unwrap_or_default()
    }
}

/// Per-work record of what the vertical produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerWorkReport {
    pub work: WorkId,
    pub disposition: WorkDisposition,
    pub scheduling_reasons: Vec<SchedulingReasonRef>,
    pub decision: Option<PolicyDecision>,
    pub decision_reasons: Vec<DecisionReasonRef>,
}

/// Reference to a [`SchedulingReason`](crate::application::change_tracking::planner::SchedulingReason)
/// in the e68 planner. The full enum is re-exported; we wrap it
/// here only to make the report self-describing for `why_scheduled`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulingReasonRef {
    pub kind: SchedulingReasonKind,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulingReasonKind {
    ReadFactRemoved,
    ReadFactChanged,
    UnrelatedChange,
    ConservativeAddition,
    ReadSetTruncated,
    UnknownDependency,
}

impl SchedulingReasonKind {
    pub fn from(reason: &crate::application::change_tracking::planner::SchedulingReason) -> Self {
        use crate::application::change_tracking::planner::SchedulingReason::*;
        match reason {
            ReadFactRemoved { .. } => Self::ReadFactRemoved,
            ReadFactChanged { .. } => Self::ReadFactChanged,
            UnrelatedChange { .. } => Self::UnrelatedChange,
            ConservativeAddition { .. } => Self::ConservativeAddition,
            ReadSetTruncated => Self::ReadSetTruncated,
            UnknownDependency => Self::UnknownDependency,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadFactRemoved => "ReadFactRemoved",
            Self::ReadFactChanged => "ReadFactChanged",
            Self::UnrelatedChange => "UnrelatedChange",
            Self::ConservativeAddition => "ConservativeAddition",
            Self::ReadSetTruncated => "ReadSetTruncated",
            Self::UnknownDependency => "UnknownDependency",
        }
    }
}

/// Reference to a [`PolicyReason`](crate::application::policy_gate::PolicyReason)
/// in the e69 gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionReasonRef {
    pub rule: String,
    pub slot_id: String,
    pub verdict: DecisionVerdictKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionVerdictKind {
    Satisfied,
    BelowGrade,
    Failed,
    Missing,
    Unknown,
    Absent,
}

impl DecisionVerdictKind {
    pub fn from(v: crate::application::policy_gate::ReasonVerdict) -> Self {
        use crate::application::policy_gate::ReasonVerdict::*;
        match v {
            Satisfied => Self::Satisfied,
            BelowGrade => Self::BelowGrade,
            Failed => Self::Failed,
            Missing => Self::Missing,
            Unknown => Self::Unknown,
            Absent => Self::Absent,
            OptionalDegraded => Self::Satisfied, // not surfaced at the gate; collapse for stability
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Satisfied => "Satisfied",
            Self::BelowGrade => "BelowGrade",
            Self::Failed => "Failed",
            Self::Missing => "Missing",
            Self::Unknown => "Unknown",
            Self::Absent => "Absent",
        }
    }
}

/// The full vertical report. Sorted by `work_id` for determinism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVerticalReport {
    pub run_id: EvidenceBundleId,
    pub from_snapshot: Option<SnapshotId>,
    pub to_snapshot: Option<SnapshotId>,
    pub per_work: Vec<PerWorkReport>,
}

impl Default for LocalVerticalReport {
    fn default() -> Self {
        Self {
            run_id: next_run_id(),
            from_snapshot: None,
            to_snapshot: None,
            per_work: vec![],
        }
    }
}

impl LocalVerticalReport {
    /// True iff every decision in the report is `Pass` (or the
    /// work was not executed).
    pub fn is_clean(&self) -> bool {
        self.per_work.iter().all(|p| {
            p.disposition == WorkDisposition::Unaffected || {
                p.decision.as_ref().is_none_or(|d| {
                    d.outcome == crate::application::policy_gate::PolicyOutcome::Pass
                })
            }
        })
    }
}

/// Per-vertical run identifier. Distinct from per-bundle id; for
/// e70 this is a fresh in-process id allocated on each run.
pub fn next_run_id() -> EvidenceBundleId {
    crate::application::evidence_bundle::next_bundle_id()
}

/// Errors raised by [`run_local_vertical`]. Total: every variant
/// names a concrete reason.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LocalVerticalError {
    /// The fact delta and the dependency inputs disagree on the
    /// snapshot pair. This is a structural inconsistency.
    #[error("fact delta does not match the planner snapshot pair")]
    SnapshotMismatch,
}

/// Run the local vertical: take a [`FactDelta`], a set of
/// [`ExecutionDependency`] rows, a [`WorkExecutor`], and a
/// [`PolicySpec`]; return a structured [`LocalVerticalReport`].
///
/// The function is pure & total: any combination of inputs produces
/// exactly one report. Errors are limited to the structural cases
/// defined by [`LocalVerticalError`].
///
/// Per-work flow:
/// 1. The planner (e68) decides the disposition (`Affected` /
///    `Unaffected` / `Unknown`).
/// 2. Only `Affected` and `Unknown` work is executed (per the
///    conservative fallback: `Unknown` still executes to produce
///    evidence, since `Unknown` is not `Unaffected`).
/// 3. The executor's [`ProducerOutput`]s are translated into a
///    per-work [`EvidenceBundle`].
/// 4. The gate evaluates the bundle against the spec; the decision
///    is added to the report.
pub fn run_local_vertical(
    delta: &FactDelta,
    dependencies: &[ExecutionDependency],
    executor: &dyn WorkExecutor,
    spec: &PolicySpec,
) -> Result<LocalVerticalReport, LocalVerticalError> {
    let plan =
        crate::application::change_tracking::planner::plan_affected_work(delta, dependencies);
    let run_id = next_run_id();

    let mut per_work: Vec<PerWorkReport> = Vec::new();
    for decision in &plan.decisions {
        let per = build_per_work(decision, executor, spec, &plan, run_id)?;
        per_work.push(per);
    }

    // Determinism: stable order by work id (planner already returns
    // BTreeMap-keyed order, but be defensive).
    per_work.sort_by(|a, b| a.work.cmp(&b.work));

    Ok(LocalVerticalReport {
        run_id,
        from_snapshot: Some(delta.from),
        to_snapshot: Some(delta.to),
        per_work,
    })
}

fn build_per_work(
    decision: &WorkDecision,
    executor: &dyn WorkExecutor,
    spec: &PolicySpec,
    plan: &AffectedWorkPlan,
    run_id: EvidenceBundleId,
) -> Result<PerWorkReport, LocalVerticalError> {
    let scheduling_reasons: Vec<SchedulingReasonRef> = decision
        .reasons
        .iter()
        .map(|r| SchedulingReasonRef {
            kind: SchedulingReasonKind::from(r),
            detail: format!("{:?}", r),
        })
        .collect();

    // Conservative rule (preserved from e68): only Affected and
    // Unknown work is executed. Unaffected is skipped (no need to
    // re-run).
    let execute = matches!(
        decision.disposition,
        WorkDisposition::Affected | WorkDisposition::Unknown
    );

    if !execute {
        return Ok(PerWorkReport {
            work: decision.work.clone(),
            disposition: decision.disposition,
            scheduling_reasons,
            decision: None,
            decision_reasons: vec![],
        });
    }

    // Run the executor for this work and translate to a bundle.
    let outputs = executor.execute(&decision.work);
    // Wrap each ProducerOutput as a synthetic producer (one per
    // work). The aggregator just flattens the outputs into entries.
    struct OneShotProducer(Vec<ProducerOutput>);
    impl EvidenceProducer for OneShotProducer {
        fn source(&self) -> ProducerSource {
            ProducerSource::Other
        }
        fn produce(&self, _w: &WorkId) -> Vec<ProducerOutput> {
            self.0.clone()
        }
    }
    let bundle = aggregate(
        run_id,
        decision.work.clone(),
        // ExecutionId is a per-bundle id; e70 initial picks a
        // monotonically-increasing id shared with the bundle id.
        ExecutionId(run_id.0),
        // Snapshot: use the destination snapshot (the one we are
        // validating). For Affected work the read-set was against
        // the "from" snapshot, but the bundle records the "to"
        // snapshot as the post-change world.
        {
            plan.to_snapshot
                .unwrap_or(crate::domain::evidence_kernel::ids::SnapshotId(0))
        },
        &[Box::new(OneShotProducer(outputs))],
    );

    let policy = evaluate(&bundle, spec);
    let decision_reasons: Vec<DecisionReasonRef> = policy
        .reasons
        .iter()
        .map(|r| DecisionReasonRef {
            rule: r.rule.clone(),
            slot_id: r.slot.slot_id.clone(),
            verdict: DecisionVerdictKind::from(r.verdict),
        })
        .collect();

    Ok(PerWorkReport {
        work: decision.work.clone(),
        disposition: decision.disposition,
        scheduling_reasons,
        decision: Some(policy),
        decision_reasons,
    })
}

// (Re-exports at the crate root provide access to the upstream
// canonical types.)
#[allow(unused_imports)]
use crate::application::evidence_bundle::BundleEntry as _BundleEntryMarker;

// (We keep this module's surface scoped: consumers should reach
// the canonical Outcome via `crate::application::evidence_bundle::Outcome`.)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::change_tracking::planner::{ExecutionDependency, WorkId};
    use crate::application::evidence_bundle::{Outcome, ProducerSlot, ProducerSource};
    use crate::application::policy_gate::{GateRule, PolicySpec};
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
    use crate::domain::kernel_ids::{EvidenceGrade, ExecutionId};
    use crate::domain::naming::NamespacedName;
    use crate::domain::readset::{InMemoryReadSetRecorder, ReadSetConfig};
    use crate::domain::value_objects::Provenance;

    fn provenance() -> ProvenanceRecord {
        ProvenanceRecord::new(
            Provenance::Extracted,
            ProducerKind::DeterministicAnalyzer,
            None,
        )
    }

    fn fact(id: u64, subject: u64, pred: &str, object: FactValue, snapshot: SnapshotId) -> Fact {
        Fact::new(
            FactId(id),
            EntityId(subject),
            RelationKind::try_new(pred).unwrap(),
            object,
            snapshot,
            provenance(),
        )
        .expect("valid fact")
    }

    fn snap(n: u64) -> SnapshotId {
        SnapshotId(n)
    }

    fn make_work(s: &str) -> WorkId {
        WorkId::new(NamespacedName::new(s).unwrap())
    }

    fn ev_descriptor(id: u64, fact_id: u64) -> EvidenceDescriptor {
        EvidenceDescriptor {
            id: crate::domain::evidence_kernel::ids::EvidenceId(id),
            grade: EvidenceGrade::Supports,
            fact: FactSlot::Resolved(FactDescriptor {
                id: FactId(fact_id),
                subject: None,
                snapshot: snap(1),
            }),
        }
    }

    fn rec_with(facts: &[u64]) -> crate::domain::readset::ReadSet {
        let mut r = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for f in facts {
            r.record(FactId(*f)).unwrap();
        }
        r.finalize()
    }

    fn slot(source: ProducerSource, id: &str) -> ProducerSlot {
        ProducerSlot {
            source,
            slot_id: id.to_string(),
        }
    }

    fn dep(work: WorkId, read_facts: &[u64]) -> ExecutionDependency {
        ExecutionDependency {
            work,
            execution: ExecutionId(1),
            read_set: rec_with(read_facts),
        }
    }

    // =====================================================================
    // WU1 UAT.
    // =====================================================================

    #[test]
    fn empty_dependencies_yield_empty_report() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();
        let exec = InMemoryWorkExecutor::new();
        let spec = PolicySpec::new(vec![]);
        let report = run_local_vertical(&delta, &[], &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 0);
    }

    #[test]
    fn unaffected_work_is_not_executed() {
        // Work reads fact 999, which is not in the delta. Planner
        // says Unaffected → executor is NOT called → no bundle → no
        // decision.
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.unaffected");
        let deps = vec![dep(w.clone(), &[999])];

        // Use a counting executor: must NEVER be called.
        struct CountingExec;
        impl WorkExecutor for CountingExec {
            fn execute(&self, _w: &WorkId) -> Vec<ProducerOutput> {
                // Use interior mutability via a thread-local; for
                // simplicity we just trust the type and record in a
                // test below. Here we use AtomicUsize.
                COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![]
            }
        }
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        COUNT.store(0, std::sync::atomic::Ordering::Relaxed);

        let exec = CountingExec;
        let spec = PolicySpec::new(vec![]);
        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 1);
        assert_eq!(report.per_work[0].disposition, WorkDisposition::Unaffected);
        assert_eq!(report.per_work[0].decision, None);
        assert_eq!(
            COUNT.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "Unaffected work MUST NOT be executed"
        );
    }

    #[test]
    fn affected_work_is_executed_and_decided() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.affected");
        let deps = vec![dep(w.clone(), &[10])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 10),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 1);
        assert_eq!(report.per_work[0].disposition, WorkDisposition::Affected);
        let dec = report.per_work[0].decision.as_ref().expect("decision");
        assert_eq!(
            dec.outcome,
            crate::application::policy_gate::PolicyOutcome::Pass
        );
    }

    #[test]
    fn unknown_work_is_executed_and_may_yield_insufficient() {
        // Conservative fallback: Unknown disposition is EXECUTED
        // (not Unaffected), but if the producer is missing, the
        // gate yields InsufficientEvidence.
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = [fact(
            50,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.unknown");
        let deps = vec![dep(w.clone(), &[1, 2, 3])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 50),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work[0].disposition, WorkDisposition::Unknown);
        let dec = report.per_work[0].decision.as_ref().expect("decision");
        assert_eq!(
            dec.outcome,
            crate::application::policy_gate::PolicyOutcome::Pass
        );
        // Note: in this scenario the Unknown disposition happens to
        // produce Pass because the producer produced Evidence. The
        // planner's Unknown → executor-still-runs is the
        // preservation of the conservative rule.
    }

    #[test]
    fn producer_failure_is_preserved_through_the_bundle_and_blocks_the_gate() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.affected");
        let deps = vec![dep(w.clone(), &[10])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Failed {
                    reason: "exit 1".into(),
                    raw: Some("...".into()),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        let dec = report.per_work[0].decision.as_ref().expect("decision");
        assert_eq!(
            dec.outcome,
            crate::application::policy_gate::PolicyOutcome::Block
        );
    }

    #[test]
    fn run_id_is_allocated_and_increases_per_call() {
        let a = next_run_id();
        let b = next_run_id();
        assert!(a < b);
    }

    // =====================================================================
    // WU2 UAT.
    // =====================================================================

    #[test]
    fn why_scheduled_reasons_are_preserved() {
        // Work Affected by removed fact 10 → reason must be
        // ReadFactRemoved.
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();
        let w = make_work("ci.aff");
        let deps = vec![dep(w.clone(), &[10])];
        let exec = InMemoryWorkExecutor::new();
        let spec = PolicySpec::new(vec![]);
        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert!(
            report.per_work[0]
                .scheduling_reasons
                .iter()
                .any(|r| r.kind == SchedulingReasonKind::ReadFactRemoved)
        );
    }

    #[test]
    fn why_decided_reasons_are_preserved() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();
        let w = make_work("ci.aff");
        let deps = vec![dep(w.clone(), &[10])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 10),
                },
            }],
        );
        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert!(
            report.per_work[0]
                .decision_reasons
                .iter()
                .any(|r| r.verdict == DecisionVerdictKind::Satisfied)
        );
    }

    #[test]
    fn per_work_order_is_stable_under_dependency_reordering() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w_a = make_work("ci.a");
        let w_b = make_work("ci.b");
        let deps_ab = vec![dep(w_a.clone(), &[10]), dep(w_b.clone(), &[10])];
        let deps_ba = vec![dep(w_b.clone(), &[10]), dep(w_a.clone(), &[10])];

        // We cannot share the run_id between two runs (next_run_id
        // is global), so we ignore the run_id for the equality
        // check and compare the per_work field.
        let exec = InMemoryWorkExecutor::new();
        let spec = PolicySpec::new(vec![]);
        let r1 = run_local_vertical(&delta, &deps_ab, &exec, &spec).unwrap();
        let r2 = run_local_vertical(&delta, &deps_ba, &exec, &spec).unwrap();
        let mut r1_stripped = r1;
        let mut r2_stripped = r2;
        r1_stripped.run_id = crate::application::evidence_bundle::EvidenceBundleId(0);
        r2_stripped.run_id = crate::application::evidence_bundle::EvidenceBundleId(0);
        assert_eq!(r1_stripped, r2_stripped);
    }

    // =====================================================================
    // WU3 — End-to-end adversarial + invariants.
    // =====================================================================

    // The product demo: a small but realistic scenario that walks
    // the entire vertical from snapshot change to gate decision,
    // with both explanations populated.
    #[test]
    fn product_demo_test_walks_a_real_vertical() {
        // Setup:
        //   - previous execution read fact 10 (auth.validate).
        //   - snapshot N+1 changes fact 10 to a different subject
        //     (auth.validate now points to a different entity).
        // Expectations:
        //   - planner: Affected
        //   - executor: returns one Evidence
        //   - gate: Pass (because the Evidence satisfies the spec)
        //   - report: 1 why_scheduled (ReadFactChanged) + 1
        //     why_decided (Satisfied)
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = [fact(
            20,
            100,
            "core:calls",
            FactValue::Ref(EntityId(300)),
            s_to,
        )];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.test:auth_integration");
        let deps = vec![dep(w.clone(), &[10])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "auth.test"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(99, 20),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test-evidence".into(),
            slot: slot(ProducerSource::CargoTest, "auth.test"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 1);
        let p = &report.per_work[0];
        assert_eq!(p.work, w);
        assert_eq!(p.disposition, WorkDisposition::Affected);

        // why_scheduled: at least one ReadFactChanged reason (the
        // (100, core:calls) was removed and re-added to a different
        // object).
        assert!(
            p.scheduling_reasons
                .iter()
                .any(|r| r.kind == SchedulingReasonKind::ReadFactChanged)
        );

        // why_decided: the rule fired with a Satisfied verdict.
        let dec = p.decision.as_ref().expect("decision");
        assert_eq!(
            dec.outcome,
            crate::application::policy_gate::PolicyOutcome::Pass
        );
        assert!(
            p.decision_reasons
                .iter()
                .any(|r| r.verdict == DecisionVerdictKind::Satisfied
                    && r.rule == "require:test-evidence")
        );
    }

    // Empty delta → no work to plan → no work in report.
    #[test]
    fn empty_delta_yields_empty_report() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = [fact(
            99,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();
        assert!(delta.is_empty());

        let exec = InMemoryWorkExecutor::new();
        let spec = PolicySpec::new(vec![]);
        let report = run_local_vertical(&delta, &[], &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 0);
    }

    // All-Unaffected → all skipped → no decisions, no executor calls.
    #[test]
    fn all_unaffected_yields_no_executed_work() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w_a = make_work("ci.a");
        let w_b = make_work("ci.b");
        let deps = vec![dep(w_a.clone(), &[999]), dep(w_b.clone(), &[888])];

        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
        struct CountingExec;
        impl WorkExecutor for CountingExec {
            fn execute(&self, _w: &WorkId) -> Vec<ProducerOutput> {
                COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                vec![]
            }
        }
        let exec = CountingExec;
        let spec = PolicySpec::new(vec![]);
        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 2);
        for p in &report.per_work {
            assert_eq!(p.disposition, WorkDisposition::Unaffected);
            assert_eq!(p.decision, None);
        }
        assert_eq!(
            COUNT.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "All-Unaffected vertical MUST NOT invoke the executor"
        );
    }

    // Mixed: one Affected + one Unknown → both executed; Affected
    // produces a decision; Unknown also produces a decision (the
    // gate rules on the Unknown work's bundle, not on the planner's
    // Unknown disposition).
    #[test]
    fn mixed_affected_and_unknown_yield_decisions_per_work() {
        // Work A: reads fact 10 (which is removed) → Affected.
        // Work B: reads unrelated facts but the snapshot added a
        //         new fact → Unknown (conservative).
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = [fact(
            99,
            500,
            "core:defines",
            FactValue::Text("y".into()),
            s_to,
        )];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w_a = make_work("ci.affected");
        let w_b = make_work("ci.unknown");
        let deps = vec![dep(w_a.clone(), &[10]), dep(w_b.clone(), &[1, 2, 3])];

        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w_a.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 10),
                },
            }],
        );
        exec.register(
            w_b.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(2, 99),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(report.per_work.len(), 2);
        let p_a = report.per_work.iter().find(|p| p.work == w_a).unwrap();
        let p_b = report.per_work.iter().find(|p| p.work == w_b).unwrap();
        assert_eq!(p_a.disposition, WorkDisposition::Affected);
        assert_eq!(p_b.disposition, WorkDisposition::Unknown);
        assert!(p_a.decision.is_some());
        assert!(p_b.decision.is_some());
    }

    // Decision reflects InsufficientEvidence at the work level when
    // the bundle's evidence is incomplete (the planner's Unknown
    // does NOT silently become Pass).
    #[test]
    fn unknown_planner_disposition_with_missing_evidence_yields_insufficient() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = [fact(
            50,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.unknown");
        let deps = vec![dep(w.clone(), &[1, 2, 3])];
        let mut exec = InMemoryWorkExecutor::new();
        // Producer cannot reach the required slot — Missing entry.
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "required"),
                outcome: Outcome::Missing {
                    why_unreachable: "tool not found".into(),
                },
            }],
        );

        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "required"),
            min_grade: None,
            required: true,
        }]);

        let report = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        let dec = report.per_work[0].decision.as_ref().expect("decision");
        assert_eq!(
            dec.outcome,
            crate::application::policy_gate::PolicyOutcome::InsufficientEvidence
        );
    }

    // Determinism: same inputs → same per_work (ignoring run_id).
    #[test]
    fn vertical_is_deterministic_across_repeated_runs() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = [fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .unwrap();

        let w = make_work("ci.deterministic");
        let deps = vec![dep(w.clone(), &[10])];
        let mut exec = InMemoryWorkExecutor::new();
        exec.register(
            w.clone(),
            vec![ProducerOutput {
                slot: slot(ProducerSource::CargoTest, "ok"),
                outcome: Outcome::Evidence {
                    descriptor: ev_descriptor(1, 10),
                },
            }],
        );
        let spec = PolicySpec::new(vec![GateRule {
            name: "require:test".into(),
            slot: slot(ProducerSource::CargoTest, "ok"),
            min_grade: None,
            required: true,
        }]);

        let r1 = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        let r2 = run_local_vertical(&delta, &deps, &exec, &spec).unwrap();
        assert_eq!(r1.per_work, r2.per_work);
    }
}
