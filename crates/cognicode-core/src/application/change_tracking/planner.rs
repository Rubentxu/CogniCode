//! Affected-work planner (e68 WU2).
//!
//! Composes a [`FactDelta`] (semantic change between snapshots) and the
//! existing e66 execution read-sets ([`ReadSet`] per execution), and
//! produces an [`AffectedWorkPlan`]: for each piece of logical work, a
//! [`WorkDisposition`] (`Affected` / `Unaffected` / `Unknown`) plus the
//! structured [`SchedulingReason`] that originated the decision.
//!
//! ## Logical work vs execution instance
//!
//! We never return `ExecutionId(N) must rerun`: the rerun will have a
//! different `ExecutionId`. We return `WorkId("ns.name") must rerun`.
//! An [`ExecutionDependency`] ties a previous execution (for auditability
//! of the read-set) to its logical work, but the planner output speaks
//! `WorkId` only.
//!
//! ## Conservative fallback (rule of e68)
//!
//! - A fact the previous read-set read, and that has now been REMOVED
//!   → `Affected`. The earlier view is provably stale on that fact.
//! - A fact the previous read-set read, and that has now CHANGED
//!   semantically (same `(subject, predicate)` to a different
//!   `object`) → `Affected`.
//! - A fact unrelated to the previous read-set that changed →
//!   `Unaffected`.
//! - A NEW fact that did not exist in the previous snapshot →
//!   `Unknown`. The previous execution could not have read it; we
//!   cannot prove an aggregate read (`"all functions"`) is unaffected,
//!   so we conservatively mark the work as `Unknown`. This is exactly
//!   the false-negative case the user warned about.
//! - A TRUNCATED read-set (recorder hit its bound) → `Unknown` even
//!   if no relevant change is observed. Lineage is incomplete.
//!
//! Critical invariant: `Unknown` MUST NOT collapse to `Unaffected`.
//!
//! ## Pure / deterministic / no I/O
//!
//! Pure composition logic. Reads the existing [`ReadSet`] introspection
//! and WU1 [`FactDelta`] types. Writes nothing. The plan is a derived
//! operational decision, never a canonical Fact.

use crate::domain::evidence_kernel::ids::{FactId, SnapshotId};
use crate::domain::evidence_kernel::semantic_diff::{FactChange, FactDelta, FactSemanticKey};
use crate::domain::kernel_ids::ExecutionId;
use crate::domain::naming::NamespacedName;
use crate::domain::readset::ReadSet;

/// Logical identity of a piece of work.
///
/// Opaque; we do NOT yet build a job taxonomy (`enum WorkKind`).
/// e69/e70 may evolve this; e68 stays minimal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WorkId(NamespacedName);

impl WorkId {
    /// Construct a WorkId from a validated `namespace.name`.
    pub fn new(name: NamespacedName) -> Self {
        Self(name)
    }

    /// Borrow the underlying namespaced name.
    pub fn as_namespaced_name(&self) -> &NamespacedName {
        &self.0
    }
}

impl std::fmt::Display for WorkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

/// Ties a previous execution (and its read-set) to its logical work.
///
/// We keep this on the input side only — the planner output speaks
/// `WorkId` so the plan survives multiple reruns.
#[derive(Debug, Clone)]
pub struct ExecutionDependency {
    pub work: WorkId,
    pub execution: ExecutionId,
    pub read_set: ReadSet,
}

/// Structured reason a piece of work was placed in a [`WorkDisposition`].
///
/// Originated by the planner; downstream consumers (e70's
/// `why_scheduled` CLI) render this for users. The reason must be
/// precise enough to be auditable and stable enough to be diffed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulingReason {
    /// A read fact was removed between `from` and `to`. The previous
    /// execution's view of the world is provably stale with respect to
    /// that fact.
    ReadFactRemoved {
        fact: FactSemanticKey,
        previous_fact: FactId,
    },
    /// A read fact was semantically changed (the same `(subject,
    /// predicate)` now points to a different object).
    ReadFactChanged {
        before: FactSemanticKey,
        after: FactSemanticKey,
        previous_fact: FactId,
    },
    /// A fact unrelated to this work's read-set was changed. The
    /// planner can prove non-invalidation.
    UnrelatedChange { fact: FactSemanticKey },
    /// A new fact was added between snapshots. The previous
    /// execution could not have read it; we cannot prove the
    /// read-only view (e.g. `"all functions"`) is unaffected, so
    /// the work is conservatively `Unknown`.
    ConservativeAddition { added: FactSemanticKey },
    /// The read-set was truncated (recorder hit its bound). Lineage
    /// is incomplete; we cannot prove the work is unaffected.
    ReadSetTruncated,
    /// The planner has no structural information about why this work
    /// was placed in `Unknown`. Reserved for future extensions.
    UnknownDependency,
}

/// How a piece of work should be treated by a scheduler.
///
/// `Unknown` is NOT the same as `Unaffected`: an `Unknown` work item
/// requires a conservative fallback (e.g. rerun with the new
/// snapshot), whereas `Unaffected` may be safely skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkDisposition {
    Affected,
    Unaffected,
    Unknown,
}

impl WorkDisposition {
    /// True iff this disposition would block the gate for skipping.
    pub fn must_rerun(self) -> bool {
        matches!(self, WorkDisposition::Affected | WorkDisposition::Unknown)
    }
}

/// Per-work decision in the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkDecision {
    pub work: WorkId,
    pub disposition: WorkDisposition,
    pub reasons: Vec<SchedulingReason>,
}

/// The full plan: one [`WorkDecision`] per distinct `WorkId` covered
/// by the input dependencies. If a `WorkId` appears in multiple
/// dependencies (e.g. multiple historical executions), the planner
/// merges their reasons using the conservative rule (`Unknown` wins
/// over `Unaffected`, `Affected` wins over `Unknown`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AffectedWorkPlan {
    pub from_snapshot: Option<SnapshotId>,
    pub to_snapshot: Option<SnapshotId>,
    pub decisions: Vec<WorkDecision>,
}

impl AffectedWorkPlan {
    /// True iff every work item is `Unaffected`. The plan may still be
    /// empty (no dependencies → no decisions).
    pub fn is_fully_unaffected(&self) -> bool {
        self.decisions
            .iter()
            .all(|d| d.disposition == WorkDisposition::Unaffected)
    }
}

/// Errors raised by [`plan_affected_work`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlannerError {
    /// The delta does not match the snapshot pair implied by the plan
    /// inputs (currently unused — kept for future cross-validation).
    #[error("fact delta snapshot pair does not match the plan inputs")]
    SnapshotMismatch,
}

/// Compute the affected-work plan for a snapshot transition.
///
/// `delta` describes the semantic change; `dependencies` carry the
/// historical read-sets. The function is pure: no I/O, no clock, no
/// randomness. The output ordering of `decisions` is sorted by
/// `WorkId` for determinism.
pub fn plan_affected_work(
    delta: &FactDelta,
    dependencies: &[ExecutionDependency],
) -> AffectedWorkPlan {
    // Group dependencies by WorkId so the planner reasons over the
    // union of read-sets per logical work (conservative: if any
    // historical execution saw a fact, we treat the work as having
    // seen it).
    let mut by_work: std::collections::BTreeMap<WorkId, Vec<&ExecutionDependency>> =
        std::collections::BTreeMap::new();
    for dep in dependencies {
        by_work.entry(dep.work.clone()).or_default().push(dep);
    }

    let mut decisions: Vec<WorkDecision> = Vec::new();
    for (work, deps) in by_work {
        let decision = decide_for_work(&work, delta, &deps);
        decisions.push(decision);
    }

    AffectedWorkPlan {
        from_snapshot: Some(delta.from),
        to_snapshot: Some(delta.to),
        decisions,
    }
}

fn decide_for_work(
    work: &WorkId,
    delta: &FactDelta,
    deps: &[&ExecutionDependency],
) -> WorkDecision {
    // Pre-check: if ANY dependency's read-set was truncated, the work
    // is conservatively `Unknown` regardless of the delta. Lineage
    // is incomplete.
    let truncated = deps.iter().any(|d| d.read_set.is_truncated());
    if truncated {
        return WorkDecision {
            work: work.clone(),
            disposition: WorkDisposition::Unknown,
            reasons: vec![SchedulingReason::ReadSetTruncated],
        };
    }

    let mut reasons: Vec<SchedulingReason> = Vec::new();
    let mut disposition = WorkDisposition::Unaffected;

    // 1) Removed facts: if any historical execution read a fact that
    //    is now removed, the work is `Affected`.
    for removed in &delta.removed {
        if deps.iter().any(|d| d.read_set.contains(&removed.fact_id)) {
            reasons.push(SchedulingReason::ReadFactRemoved {
                fact: removed.semantic_key.clone(),
                previous_fact: removed.fact_id,
            });
            disposition = WorkDisposition::Affected;
        }
    }

    // 2) Semantic changes: detect (subject, predicate) pairs that
    //    changed object. If any historical read-set contained the
    //    fact id on the "removed" side, the work is `Affected`.
    for change in pair_semantic_changes(delta) {
        // The "before" side carries the previous FactId.
        if deps
            .iter()
            .any(|d| d.read_set.contains(&change.previous_fact))
        {
            reasons.push(SchedulingReason::ReadFactChanged {
                before: change.before,
                after: change.after,
                previous_fact: change.previous_fact,
            });
            disposition = WorkDisposition::Affected;
        }
    }

    // 3) Unrelated changes: any removed/changed fact that NO
    //    dependency read strengthens the `Unaffected` evidence (we
    //    record it as an explicit reason so the consumer can audit).
    for removed in &delta.removed {
        if deps.iter().all(|d| !d.read_set.contains(&removed.fact_id)) {
            reasons.push(SchedulingReason::UnrelatedChange {
                fact: removed.semantic_key.clone(),
            });
            // disposition stays whatever it was (Unaffected or Affected).
        }
    }

    // 4) Additions are ALWAYS `Unknown` for that work: the previous
    //    execution could not have read what did not exist. The
    //    disposition can only climb from `Unaffected` up to `Unknown`
    //    here — never down to `Unaffected`.
    if !delta.added.is_empty() {
        for added in &delta.added {
            reasons.push(SchedulingReason::ConservativeAddition {
                added: added.semantic_key.clone(),
            });
        }
        // Conservative promotion: Unknown beats Unaffected; Affected
        // beats Unknown.
        disposition = match disposition {
            WorkDisposition::Unaffected => WorkDisposition::Unknown,
            WorkDisposition::Unknown | WorkDisposition::Affected => disposition,
        };
    }

    WorkDecision {
        work: work.clone(),
        disposition,
        reasons,
    }
}

/// Pair up removed/added facts with matching `(subject, predicate)` to
/// express a "semantic change" as a single transition.
///
/// Modification = `removed(A, p, X) + added(A, p, Y)`. We pair by
/// `(subject, predicate)` only — the object change is what makes it
/// a "change" rather than two independent transitions.
struct SemanticChange {
    before: FactSemanticKey,
    after: FactSemanticKey,
    previous_fact: FactId,
}

fn pair_semantic_changes(delta: &FactDelta) -> Vec<SemanticChange> {
    use std::collections::HashMap;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct EntityIdInner(u64);

    let mut removed_by_sp: HashMap<(EntityIdInner, String), Vec<&FactChange>> = HashMap::new();
    for r in &delta.removed {
        let key = (
            EntityIdInner(r.semantic_key.subject.0),
            r.semantic_key.predicate.to_string(),
        );
        removed_by_sp.entry(key).or_default().push(r);
    }

    let mut changes: Vec<SemanticChange> = Vec::new();
    for added in &delta.added {
        let key = (
            EntityIdInner(added.semantic_key.subject.0),
            added.semantic_key.predicate.to_string(),
        );
        if let Some(removeds) = removed_by_sp.get(&key)
            && let Some(prev) = removeds.first()
        {
            changes.push(SemanticChange {
                before: prev.semantic_key.clone(),
                after: added.semantic_key.clone(),
                previous_fact: prev.fact_id,
            });
        }
    }

    // Determinism: stable order by (subject, predicate, before object).
    changes.sort_by(|a, b| {
        a.before
            .subject
            .0
            .cmp(&b.before.subject.0)
            .then_with(|| {
                a.before
                    .predicate
                    .to_string()
                    .cmp(&b.before.predicate.to_string())
            })
            .then_with(|| format!("{:?}", a.before.object).cmp(&format!("{:?}", a.before.object)))
    });
    changes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::kernel_ids::ExecutionId;
    use crate::domain::naming::NamespacedName;
    use crate::domain::readset::{InMemoryReadSetRecorder, ReadSetConfig};
    use crate::domain::value_objects::Provenance;
    use std::num::NonZeroUsize;

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
        WorkId::new(NamespacedName::new(s).expect("valid ns.name"))
    }

    fn rec_with(facts: &[u64]) -> ReadSet {
        let mut r = InMemoryReadSetRecorder::new(ReadSetConfig { max_records: None });
        for f in facts {
            r.record(FactId(*f)).expect("record");
        }
        r.finalize()
    }

    // ---- UAT: read fact removed → Affected ----
    #[test]
    fn removed_read_fact_yields_affected() {
        // from: {fact 10} (read). to: {}.
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = compute_diff(&from, s_from, &to, s_to);

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Affected);
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::ReadFactRemoved { .. }))
        );
    }

    // ---- UAT: read fact changed (same subject+predicate, different object) → Affected ----
    #[test]
    fn read_fact_semantic_change_yields_affected() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![fact(
            20,
            100,
            "core:calls",
            FactValue::Ref(EntityId(300)),
            s_to,
        )];
        let delta = compute_diff(&from, s_from, &to, s_to);

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Affected);
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::ReadFactChanged { .. }))
        );
    }

    // ---- UAT: unrelated change → Unaffected ----
    #[test]
    fn unrelated_change_yields_unaffected() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = compute_diff(&from, s_from, &to, s_to);

        // Work reads a DIFFERENT fact id that was not changed.
        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[999]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Unaffected);
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::UnrelatedChange { .. }))
        );
    }

    // ---- UAT: addition (with no removals/changes) → Unknown ----
    #[test]
    fn addition_without_removals_yields_unknown() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = vec![fact(
            50,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = compute_diff(&from, s_from, &to, s_to);

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[1, 2, 3]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(
            plan.decisions[0].disposition,
            WorkDisposition::Unknown,
            "additions must collapse to Unknown, never Unaffected"
        );
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::ConservativeAddition { .. }))
        );
    }

    // ---- UAT: truncated read-set → Unknown ----
    #[test]
    fn truncated_read_set_yields_unknown() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to: Vec<Fact> = vec![];
        let delta = compute_diff(&from, s_from, &to, s_to);

        // A truncated recorder (capacity 2, recorded 3 facts).
        let mut r = InMemoryReadSetRecorder::new(ReadSetConfig {
            max_records: NonZeroUsize::new(2),
        });
        r.record(FactId(1)).unwrap();
        r.record(FactId(2)).unwrap();
        r.record(FactId(3)).unwrap();
        let rs = r.finalize();
        assert!(rs.is_truncated());

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rs,
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Unknown);
        assert_eq!(
            plan.decisions[0].reasons,
            vec![SchedulingReason::ReadSetTruncated]
        );
    }

    // ---- Critical invariant: addition + unrelated change → still Unknown ----
    #[test]
    fn addition_plus_unrelated_change_does_not_collapse_to_unaffected() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = vec![
            // Added — drives Unknown.
            fact(50, 100, "core:calls", FactValue::Ref(EntityId(200)), s_to),
        ];
        let delta = compute_diff(&from, s_from, &to, s_to);

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[1, 2, 3]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Unknown);
    }

    // ---- Determinism: unordered input → identical plan ----
    #[test]
    fn planner_is_deterministic_under_input_permutation() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![
            fact(10, 100, "core:calls", FactValue::Ref(EntityId(200)), s_from),
            fact(11, 300, "core:defines", FactValue::Text("x".into()), s_from),
        ];
        let to = vec![fact(
            20,
            100,
            "core:calls",
            FactValue::Ref(EntityId(250)),
            s_to,
        )];
        let d1 = compute_diff(&from, s_from, &to, s_to);
        let d2 = compute_diff(
            &[from[1].clone(), from[0].clone()],
            s_from,
            &[to[0].clone()],
            s_to,
        );

        let dep_a = ExecutionDependency {
            work: make_work("ci.a"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10]),
        };
        let dep_b = ExecutionDependency {
            work: make_work("ci.b"),
            execution: ExecutionId(2),
            read_set: rec_with(&[11]),
        };

        let p1 = plan_affected_work(&d1, &[dep_a.clone(), dep_b.clone()]);
        let p2 = plan_affected_work(&d2, &[dep_a, dep_b]);
        assert_eq!(p1, p2);
    }

    // ---- Same world, different FactIds → empty delta → all Unaffected ----
    #[test]
    fn empty_delta_yields_all_unaffected() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![fact(
            99,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = compute_diff(&from, s_from, &to, s_to);
        assert!(delta.is_empty());

        let dep = ExecutionDependency {
            work: make_work("ci.test-suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10, 99]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Unaffected);
    }

    fn compute_diff(from: &[Fact], s_from: SnapshotId, to: &[Fact], s_to: SnapshotId) -> FactDelta {
        crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s_from,
            from.iter().cloned(),
            s_to,
            to.iter().cloned(),
        )
        .expect("valid diff inputs")
    }

    // =========================================================================
    // WU3 — adversarial matrix and invariants.
    // =========================================================================

    // The decisive case the user called out: an `added` triple inside a
    // read-only view (e.g. an aggregate query) MUST collapse to Unknown,
    // not Unaffected.
    #[test]
    fn additions_with_aggregate_query_must_collapse_to_unknown() {
        // from: {fact 1}. to: {fact 1, fact 50 (added)}.
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            1,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![
            fact(1, 100, "core:calls", FactValue::Ref(EntityId(200)), s_to),
            fact(50, 100, "core:calls", FactValue::Ref(EntityId(300)), s_to),
        ];
        let delta = compute_diff(&from, s_from, &to, s_to);

        // Aggregate read-set: the previous execution recorded every
        // fact it could see, not just specific ids. Its "view" of the
        // world would change if the aggregate answer changed.
        let dep = ExecutionDependency {
            work: make_work("ci.aggregate-functions"),
            execution: ExecutionId(1),
            read_set: rec_with(&[1, 2, 3, 4]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(
            plan.decisions[0].disposition,
            WorkDisposition::Unknown,
            "an added fact MUST collapse to Unknown even when the \
             read-set did not contain it: the aggregate view of the \
             world may have changed."
        );
    }

    // Multiple historical executions for the same WorkId are merged
    // conservatively: any read fact that any execution saw drives
    // the decision.
    #[test]
    fn multiple_executions_merge_conservatively() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = compute_diff(&from, s_from, &to, s_to);

        // Two executions for the same work; only the second one read
        // the removed fact.
        let dep_a = ExecutionDependency {
            work: make_work("ci.suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[999]),
        };
        let dep_b = ExecutionDependency {
            work: make_work("ci.suite"),
            execution: ExecutionId(2),
            read_set: rec_with(&[10]),
        };
        let plan = plan_affected_work(&delta, &[dep_a, dep_b]);
        assert_eq!(plan.decisions.len(), 1);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Affected);
    }

    // Unknown beats Unaffected in the merge, but Affected beats
    // Unknown.
    #[test]
    fn disposition_merge_order_is_unknown_beats_unaffected_affected_beats_unknown() {
        // Build a delta that produces: removed-read (Affected) +
        // added (Unknown merge trigger).
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![fact(
            50,
            500,
            "core:defines",
            FactValue::Text("y".into()),
            s_to,
        )];
        let delta = compute_diff(&from, s_from, &to, s_to);

        let dep = ExecutionDependency {
            work: make_work("ci.suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        // removed fact 10 was read → Affected. Added fact 50 would
        // promote Unknown, but Affected wins.
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Affected);
        // The reasons list must contain BOTH kinds so the auditor can
        // see the merge took place.
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::ReadFactRemoved { .. }))
        );
        assert!(
            plan.decisions[0]
                .reasons
                .iter()
                .any(|r| matches!(r, SchedulingReason::ConservativeAddition { .. }))
        );
    }

    // Cross-workspace / invalid snapshot pair is rejected by the diff,
    // never silently swallowed by the planner.
    #[test]
    fn planner_rejects_invalid_snapshot_pair_via_diff_error() {
        let s = snap(1);
        let from: Vec<Fact> = vec![];
        let to: Vec<Fact> = vec![];
        let err = crate::domain::evidence_kernel::semantic_diff::compute_fact_delta(
            s,
            from.iter().cloned(),
            s,
            to.iter().cloned(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            crate::domain::evidence_kernel::semantic_diff::SemanticDiffError::SameSnapshot(s)
        );
    }

    // Same world, different FactIds across snapshots → empty delta →
    // every work Unaffected. (This is the e68 identity invariant:
    // renumbering Facts MUST NOT trigger scheduling.)
    #[test]
    fn renumbered_facts_yield_no_planning_disruption() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![fact(
            7_777,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = compute_diff(&from, s_from, &to, s_to);
        assert!(delta.is_empty());

        let dep = ExecutionDependency {
            work: make_work("ci.suite"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10, 7_777]),
        };
        let plan = plan_affected_work(&delta, &[dep]);
        assert_eq!(plan.decisions[0].disposition, WorkDisposition::Unaffected);
        assert!(plan.is_fully_unaffected());
    }

    // Determinism of `pair_semantic_changes` exposed at the planner
    // level: the same delta + the same deps, regardless of the order
    // they appear in the dependency list, MUST produce the same plan.
    #[test]
    fn planner_is_deterministic_under_dependency_permutation() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![
            fact(10, 100, "core:calls", FactValue::Ref(EntityId(200)), s_from),
            fact(11, 300, "core:defines", FactValue::Text("x".into()), s_from),
        ];
        let to = vec![fact(
            20,
            100,
            "core:calls",
            FactValue::Ref(EntityId(250)),
            s_to,
        )];
        let d1 = compute_diff(&from, s_from, &to, s_to);

        let dep_a = ExecutionDependency {
            work: make_work("ci.a"),
            execution: ExecutionId(1),
            read_set: rec_with(&[10]),
        };
        let dep_b = ExecutionDependency {
            work: make_work("ci.b"),
            execution: ExecutionId(2),
            read_set: rec_with(&[11]),
        };

        let p1 = plan_affected_work(&d1, &[dep_a.clone(), dep_b.clone()]);
        let p2 = plan_affected_work(&d1, &[dep_b, dep_a]);
        assert_eq!(p1, p2);
    }

    // Empty dependencies → empty plan (no work to plan against).
    #[test]
    fn empty_dependencies_yield_empty_plan() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            10,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = compute_diff(&from, s_from, &to, s_to);
        let plan = plan_affected_work(&delta, &[]);
        assert_eq!(plan.decisions.len(), 0);
    }

    // Invariant: WorkDisposition::Unaffected is never produced when
    // additions exist for the same work, regardless of the read-set
    // shape. This is the "planner must never turn uncertainty into
    // Unaffected" rule.
    #[test]
    fn unaffected_must_never_coexist_with_additions() {
        // For every combination of read-set shape, if additions exist
        // the disposition MUST be at least Unknown.
        for read_facts in &[&[][..], &[10_u64][..], &[10_u64, 20][..]] {
            let s_from = snap(1);
            let s_to = snap(2);
            let from: Vec<Fact> = vec![];
            let to = vec![fact(
                50,
                100,
                "core:calls",
                FactValue::Ref(EntityId(200)),
                s_to,
            )];
            let delta = compute_diff(&from, s_from, &to, s_to);

            let dep = ExecutionDependency {
                work: make_work("ci.x"),
                execution: ExecutionId(1),
                read_set: rec_with(read_facts),
            };
            let plan = plan_affected_work(&delta, &[dep]);
            assert_ne!(
                plan.decisions[0].disposition,
                WorkDisposition::Unaffected,
                "additions must never collapse to Unaffected (read-set \
                 was {read_facts:?})"
            );
        }
    }
}
