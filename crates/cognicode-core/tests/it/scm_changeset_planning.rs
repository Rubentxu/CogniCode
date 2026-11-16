//! CGH-024 (CAA-06): the ChangeSet → FactDelta → plan_affected_work
//! bridge behaves as the acceptance criteria demand.
//!
//! Each test names the AC it pins:
//! #1 a changed private function preserves unrelated results;
//! #2 an exported API change and a manifest change invalidate correctly
//!    even when nothing the consumer reads appears to have changed;
//! #3 new facts, truncated read-sets and unknown changes never collapse
//!    into `Unaffected`;
//! #4 deltas and plans carry a reproducible identity.

use std::collections::BTreeSet;
use std::num::NonZeroUsize;

use cognicode_core::application::change_tracking::planner::{
    ExecutionDependency, SchedulingReason, WorkDisposition, WorkId, plan_affected_work,
};
use cognicode_core::application::change_tracking::scm_delta::{
    RevisionFacts, SCM_EVIDENCE_PREDICATE, ScmDeltaError, delta_from_change_set, plan_between,
    rename_pairs, snapshot_id_for,
};
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::evidence_kernel::semantic_diff::SemanticDiffError;
use cognicode_core::domain::kernel_ids::ExecutionId;
use cognicode_core::domain::naming::NamespacedName;
use cognicode_core::domain::readset::{InMemoryReadSetRecorder, ReadSet, ReadSetConfig};
use cognicode_core::domain::scm::{
    Change, ChangeSet, ContentDigest, InventoryEntry, PathKey, RevisionHandle, SourceInventory,
    SourceRef, SourceRevision,
};
use cognicode_core::domain::value_objects::Provenance;

fn path(s: &str) -> PathKey {
    PathKey::new(s).expect("valid path")
}

fn digest(content: &str) -> ContentDigest {
    ContentDigest::from_bytes(content.as_bytes())
}

fn handle(rev: &str) -> RevisionHandle {
    RevisionHandle::new(rev).expect("valid handle")
}

fn source() -> SourceRef {
    SourceRef::new("git", "demo").expect("valid source")
}

fn revision(rev: &str) -> SourceRevision {
    SourceRevision::new(source(), handle(rev), digest(&format!("tree of {rev}")))
}

fn work(name: &str) -> WorkId {
    WorkId::new(NamespacedName::new(name).expect("valid namespaced name"))
}

fn read_set(facts: &[u64]) -> ReadSet {
    facts.iter().map(|f| FactId(*f)).collect::<ReadSet>()
}

fn truncated_read_set(facts: &[u64], bound: usize) -> ReadSet {
    let mut recorder = InMemoryReadSetRecorder::new(ReadSetConfig {
        max_records: NonZeroUsize::new(bound),
    });
    for f in facts {
        recorder
            .record(FactId(*f))
            .expect("recording before finalize");
    }
    recorder.finalize()
}

fn dep(name: &str, execution: u64, facts: &[u64]) -> ExecutionDependency {
    ExecutionDependency {
        work: work(name),
        execution: ExecutionId(execution),
        read_set: read_set(facts),
    }
}

fn provenance() -> ProvenanceRecord {
    ProvenanceRecord::new(
        Provenance::Extracted,
        ProducerKind::DeterministicAnalyzer,
        None,
    )
}

fn fact(id: u64, subject: u64, predicate: &str, object: FactValue, snapshot: SnapshotId) -> Fact {
    Fact::new(
        FactId(id),
        EntityId(subject),
        RelationKind::try_new(predicate).expect("valid predicate"),
        object,
        snapshot,
        provenance(),
    )
    .expect("valid fact")
}

fn defines(snapshot: SnapshotId, id: u64, entity: u64, name: &str) -> Fact {
    fact(
        id,
        entity,
        "core:defines",
        FactValue::Text(name.to_string()),
        snapshot,
    )
}

fn inventory(rev: &str, entries: &[(&str, &str)]) -> SourceInventory {
    let items = entries
        .iter()
        .map(|(p, content)| InventoryEntry {
            path: path(p),
            content: digest(content),
        })
        .collect::<Vec<_>>();
    SourceInventory::new(&handle(rev), items).expect("canonical inventory")
}

fn change_set(base_rev: &str, head_rev: &str, changes: Vec<Change>) -> ChangeSet {
    ChangeSet::explicit(handle(base_rev), handle(head_rev), changes)
}

fn facts_of(snapshot: u64, facts: Vec<Fact>) -> RevisionFacts {
    RevisionFacts {
        snapshot: SnapshotId(snapshot),
        facts,
        path_facts: Default::default(),
    }
}

fn disposition(
    plan: &cognicode_core::application::change_tracking::planner::AffectedWorkPlan,
    name: &str,
) -> WorkDisposition {
    plan.decisions
        .iter()
        .find(|d| d.work == work(name))
        .unwrap_or_else(|| panic!("no decision for {name} in {plan:?}"))
        .disposition
}

// AC #1 — changing one private function invalidates the work that read
// its facts and preserves everything unrelated.
#[test]
fn a_changed_function_only_affects_work_that_read_it() {
    let base = facts_of(
        1,
        vec![
            defines(SnapshotId(1), 100, 10, "priv fn"),
            defines(SnapshotId(1), 101, 20, "other fn"),
        ],
    );
    let head = facts_of(
        2,
        vec![
            defines(SnapshotId(2), 200, 10, "priv fn rewritten"),
            defines(SnapshotId(2), 201, 20, "other fn"),
        ],
    );
    let changes = change_set(
        "rev-a",
        "rev-b",
        vec![Change::modified(
            path("src/lib.rs"),
            digest("old bytes"),
            digest("new bytes"),
        )],
    );
    let deps = vec![dep("job.reader", 1, &[100]), dep("job.other", 2, &[101])];
    let plan = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert_eq!(disposition(&plan, "job.reader"), WorkDisposition::Affected);
    assert_eq!(
        disposition(&plan, "job.other"),
        WorkDisposition::Unaffected,
        "an unrelated read must keep its result"
    );
}

// AC #2 — an exported API change invalidates consumers through the
// planner's semantic-change rule, although the consumer's own facts
// never changed.
#[test]
fn an_api_change_invalidates_consumers_without_touching_them() {
    let base = facts_of(
        1,
        vec![
            fact(
                100,
                10,
                "core:exports",
                FactValue::Ref(EntityId(200)),
                SnapshotId(1),
            ),
            defines(SnapshotId(1), 101, 30, "consumer"),
        ],
    );
    let head = facts_of(
        2,
        vec![
            fact(
                200,
                10,
                "core:exports",
                FactValue::Ref(EntityId(300)),
                SnapshotId(2),
            ),
            defines(SnapshotId(2), 201, 30, "consumer"),
        ],
    );
    let changes = change_set(
        "rev-a",
        "rev-b",
        vec![Change::modified(
            path("src/api.rs"),
            digest("old api"),
            digest("new api"),
        )],
    );
    let deps = vec![dep("job.consumer", 1, &[100, 101])];
    let plan = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert_eq!(
        disposition(&plan, "job.consumer"),
        WorkDisposition::Affected
    );
    let reasons = &plan.decisions[0].reasons;
    assert!(
        reasons
            .iter()
            .any(|r| matches!(r, SchedulingReason::ReadFactChanged { .. })),
        "the reason must name the changed read: {reasons:?}"
    );
}

// AC #2 — a manifest change with no fact evidence at all still
// invalidates: the triple diff is blind to it, so the bridge must
// state it and the planner must escalate.
#[test]
fn a_manifest_change_escalates_even_when_no_fact_changed() {
    let shared = vec![defines(SnapshotId(0), 100, 10, "fn")];
    let base = RevisionFacts {
        snapshot: SnapshotId(1),
        facts: shared.clone(),
        path_facts: Default::default(),
    };
    let head = RevisionFacts {
        snapshot: SnapshotId(2),
        facts: shared,
        path_facts: Default::default(),
    };
    let manifests: BTreeSet<PathKey> = [path("Cargo.toml")].into_iter().collect();
    let changes = change_set(
        "rev-a",
        "rev-b",
        vec![Change::modified(
            path("Cargo.toml"),
            digest("dep = 1"),
            digest("dep = 2"),
        )],
    );
    let deps = vec![dep("job.build", 1, &[100])];

    let plan = plan_between(&base, &head, &changes, &manifests, &deps).expect("plan");
    assert_eq!(disposition(&plan, "job.build"), WorkDisposition::Unknown);

    // Same inputs through the raw planner (no SCM evidence) prove the
    // escalation came from the bridge, not from a stricter planner.
    let raw = delta_from_change_set(&base, &head, &changes).expect("delta");
    assert!(raw.delta.added.is_empty() && raw.delta.removed.is_empty());
    let raw_plan = plan_affected_work(&raw.delta, &deps);
    assert!(raw_plan.is_fully_unaffected());
}

// AC #3 — an `Unknown` provider change speaks as loudly as a manifest:
// work that the triple diff would skip is escalated, never silenced.
#[test]
fn an_unknown_change_never_collapses_into_unaffected() {
    let shared = vec![defines(SnapshotId(0), 100, 10, "fn")];
    let base = RevisionFacts {
        snapshot: SnapshotId(1),
        facts: shared.clone(),
        path_facts: Default::default(),
    };
    let head = RevisionFacts {
        snapshot: SnapshotId(2),
        facts: shared,
        path_facts: Default::default(),
    };
    let mut changes = change_set(
        "rev-a",
        "rev-b",
        vec![Change::added(path("x.rs"), digest("x"))],
    );
    changes.mark_unknown(path("weird.rs"));
    let deps = vec![dep("job.build", 1, &[100])];
    let plan = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert_eq!(disposition(&plan, "job.build"), WorkDisposition::Unknown);
    assert!(plan
        .decisions
        .iter()
        .flat_map(|d| &d.reasons)
        .any(|r| matches!(r, SchedulingReason::ConservativeAddition { added } if added.predicate.as_str() == SCM_EVIDENCE_PREDICATE)));

    let scm = delta_from_change_set(&base, &head, &changes).expect("delta");
    assert_eq!(scm.unclassified, vec![path("weird.rs")]);
}

// AC #3 — new facts and truncated lineage keep the planner's own
// conservative rules through the bridge: never `Unaffected`.
#[test]
fn new_facts_and_truncated_lineage_are_never_unaffected() {
    let base = facts_of(1, vec![defines(SnapshotId(1), 100, 10, "fn")]);
    let head = facts_of(
        2,
        vec![
            defines(SnapshotId(2), 200, 10, "fn"),
            defines(SnapshotId(2), 201, 40, "brand new fn"),
        ],
    );
    let changes = change_set("rev-a", "rev-b", vec![]);
    let deps = vec![
        dep("job.grew", 1, &[100]),
        ExecutionDependency {
            work: work("job.truncated"),
            execution: ExecutionId(2),
            read_set: truncated_read_set(&[100, 999], 1),
        },
    ];
    let plan = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert_eq!(disposition(&plan, "job.grew"), WorkDisposition::Unknown);
    assert_eq!(
        disposition(&plan, "job.truncated"),
        WorkDisposition::Unknown
    );
    assert!(
        plan.decisions
            .iter()
            .flat_map(|d| &d.reasons)
            .any(|r| matches!(r, SchedulingReason::ReadSetTruncated))
    );
}

// CAA-02 contract — an evidenced rename needs no reparse: the bridge
// pairs it, the folded change set speaks `Renamed`, and no work is
// invalidated.
#[test]
fn an_evidenced_rename_pairs_and_invalidates_nothing() {
    let base_inv = inventory(
        "rev-a",
        &[("src/api.rs", "same bytes"), ("src/keep.rs", "k")],
    );
    let head_inv = inventory(
        "rev-b",
        &[("src/keep.rs", "k"), ("src/lib/api.rs", "same bytes")],
    );
    let pairs = rename_pairs(&base_inv, &head_inv);
    assert_eq!(pairs, vec![(path("src/api.rs"), path("src/lib/api.rs"))]);

    let mut changes =
        ChangeSet::between(&revision("rev-a"), &revision("rev-b"), &base_inv, &head_inv);
    let folded = changes.apply_rename_pairing(&pairs);
    assert_eq!(folded, 1);
    assert_eq!(changes.changes().len(), 1);
    assert!(!changes.changes()[0].requires_reparse());

    let shared = vec![defines(SnapshotId(0), 100, 10, "fn")];
    let base = RevisionFacts {
        snapshot: SnapshotId(1),
        facts: shared.clone(),
        path_facts: Default::default(),
    };
    let head = RevisionFacts {
        snapshot: SnapshotId(2),
        facts: shared,
        path_facts: Default::default(),
    };
    let deps = vec![dep("job.build", 1, &[100])];
    let plan = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert!(
        plan.is_fully_unaffected(),
        "a rename must not invent work: {plan:?}"
    );
}

// The pairing refuses ambiguity instead of guessing: two deletions
// sharing one digest pair with nothing.
#[test]
fn ambiguous_rename_evidence_is_skipped() {
    let base_inv = inventory(
        "rev-a",
        &[
            ("src/a.rs", "dup"),
            ("src/b.rs", "dup"),
            ("src/c.rs", "solo"),
        ],
    );
    let head_inv = inventory("rev-b", &[("src/new.rs", "dup"), ("src/c.rs", "solo")]);
    assert!(
        rename_pairs(&base_inv, &head_inv).is_empty(),
        "one digest deleted twice and added once is not evidence of a rename"
    );
}

// AC #4 — snapshot identity is a pure function of the revision, and
// the plan it anchors is reproducible entry for entry.
#[test]
fn identity_and_plan_are_reproducible() {
    let a = revision("rev-a");
    let b = revision("rev-b");
    assert_eq!(snapshot_id_for(&a), snapshot_id_for(&revision("rev-a")));
    assert_ne!(snapshot_id_for(&a), snapshot_id_for(&b));
    assert_ne!(snapshot_id_for(&a).0, 0, "zero is reserved by the kernel");

    let base = facts_of(
        snapshot_id_for(&a).0,
        vec![defines(snapshot_id_for(&a), 100, 10, "fn")],
    );
    let head = facts_of(
        snapshot_id_for(&b).0,
        vec![defines(snapshot_id_for(&b), 200, 10, "fn rewritten")],
    );
    let changes = change_set(
        "rev-a",
        "rev-b",
        vec![Change::modified(
            path("src/lib.rs"),
            digest("x"),
            digest("y"),
        )],
    );
    let deps = vec![dep("job.build", 1, &[100])];
    let first = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    let second = plan_between(&base, &head, &changes, &BTreeSet::new(), &deps).expect("plan");
    assert_eq!(first, second);
    assert_eq!(first.from_snapshot, Some(snapshot_id_for(&a)));
    assert_eq!(first.to_snapshot, Some(snapshot_id_for(&b)));
}

// The kernel's snapshot-pair contract survives the bridge untouched.
#[test]
fn one_snapshot_twice_is_not_a_question() {
    let facts = facts_of(7, vec![defines(SnapshotId(7), 100, 10, "fn")]);
    let changes = change_set("rev-a", "rev-a", vec![]);
    let err = delta_from_change_set(&facts, &facts, &changes).expect_err("same snapshot");
    assert_eq!(
        err,
        ScmDeltaError::Semantic(SemanticDiffError::SameSnapshot(SnapshotId(7)))
    );
}
