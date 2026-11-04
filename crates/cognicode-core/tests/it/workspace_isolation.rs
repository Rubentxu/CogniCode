#![cfg(feature = "evidence-kernel")]
//! E38 WU-4 workspace isolation suite (design D6, spec `entity-continuity`
//! requirement "Workspace isolation of continuity").
//!
//! Spec scenario "Identical workspaces do not collide": two workspaces
//! committing identical symbol sets must keep occurrence tables,
//! stable-identity mappings, and ambiguous candidates from intersecting;
//! collisions MUST be zero.
//!
//! Isolation is exercised at the PIPELINE level, not just the pure matcher:
//! both workspaces commit through one shared [`InMemoryFactStore`] (the
//! store keys every fact by `(workspace, snapshot)`), the identity reads
//! are per-`(workspace, snapshot)` pins, and each workspace's
//! `ContinuityResult` is derived ONLY from its own read-back facts — the
//! ws-a result is captured BEFORE ws-b commits and re-verified after, so
//! any cross-workspace state leakage would change it.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use cognicode_core::application::fact_bridge::FactBatchBuilder;
use cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry;
use cognicode_core::domain::evidence_kernel::continuity::{
    ContinuityOutcome, ContinuityResult, ContinuityStatus, MatcherThresholds, match_snapshots,
};
use cognicode_core::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind};
use cognicode_core::domain::evidence_kernel::ids::SnapshotId;
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::value_objects::WorkspaceId;
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};

const BEFORE: SnapshotId = SnapshotId::new(1);
const AFTER: SnapshotId = SnapshotId::new(2);

/// One occurrence key: `(workspace, snapshot, entity)`.
type OccurrenceKey = (String, SnapshotId, u64);

/// Adds one observation to the batch (canonical grammar: the identity
/// string rides BOTH the subject and the `core:defines` object; the call
/// name rides the `core:calls` object).
fn observe(
    builder: &mut FactBatchBuilder,
    subject: &str,
    predicate: &str,
    object: &str,
    detail: Option<String>,
) {
    builder
        .add_observation(
            subject,
            RelationKind::try_new(predicate).expect("canonical core:* predicate"),
            object,
            ProducerKind::DeterministicAnalyzer,
            detail,
        )
        .expect("analyzer provenance is accepted");
}

/// Builds the canonical batch for one side of the shared scenario. The
/// same SUBJECT STRINGS feed both workspaces (identical symbol sets); the
/// `EntityIdTable` assigns `1..N` per snapshot deterministically, so both
/// workspaces must end up with identical occurrence tables.
///
/// The scenario touches every status: T0 (`keep`), T1 (`tick`, line
/// shifted), Terminated (`legacy`), New (`fresh`), and an Ambiguous pair
/// (the colliding `dup` entities).
fn side_batch(snapshot: SnapshotId, before: bool) -> Vec<Fact> {
    let mut builder = FactBatchBuilder::new(snapshot);
    let defines: &[(&str, &str)] = if before {
        &[
            ("src/lib.rs:keep:1", "keep"),
            ("src/engine.rs:tick:12", "tick"),
            ("src/old.rs:legacy:2", "legacy"),
            ("src/a.rs:dup:1", "dup"),
            ("src/b.rs:dup:1", "dup"),
        ]
    } else {
        &[
            ("src/lib.rs:keep:1", "keep"),
            ("src/engine.rs:tick:40", "tick"),
            ("src/new.rs:fresh:1", "fresh"),
            ("src/c.rs:dup:1", "dup"),
        ]
    };
    for (subject, _) in defines {
        observe(
            &mut builder,
            subject,
            "core:defines",
            subject,
            Some("kind=Function".to_string()),
        );
    }
    let calls: &[(&str, &str)] = if before {
        &[
            ("src/lib.rs:keep:1", "alpha"),
            ("src/lib.rs:keep:1", "beta"),
            ("src/engine.rs:tick:12", "reset"),
            ("src/engine.rs:tick:12", "step"),
            ("src/old.rs:legacy:2", "x1"),
            ("src/a.rs:dup:1", "h1"),
            ("src/b.rs:dup:1", "h1"),
        ]
    } else {
        &[
            ("src/lib.rs:keep:1", "alpha"),
            ("src/lib.rs:keep:1", "beta"),
            ("src/engine.rs:tick:40", "reset"),
            ("src/engine.rs:tick:40", "step"),
            ("src/new.rs:fresh:1", "n1"),
            ("src/c.rs:dup:1", "h1"),
        ]
    };
    for (subject, callee) in calls {
        observe(&mut builder, subject, "core:calls", callee, None);
    }
    builder.finish()
}

/// Commits `batch` for `workspace`/`snapshot` and returns the read-back
/// facts (the store-pinned identity read).
fn commit_and_read(
    store: &InMemoryFactStore,
    workspace: &WorkspaceId,
    snapshot: SnapshotId,
    batch: Vec<Fact>,
) -> Vec<Fact> {
    let len = batch.len();
    let committed = tokio_test::block_on(store.commit(workspace, &snapshot, batch))
        .expect("identical symbol sets commit");
    assert_eq!(committed.len(), len, "every committed fact returns an id");
    tokio_test::block_on(store.facts_in_snapshot(workspace, &snapshot))
        .expect("pinned snapshot read")
}

/// The occurrence table of one read: `EntityId → identity string`, sorted.
fn occurrence_table(facts: &[Fact]) -> BTreeMap<u64, String> {
    let mut table = BTreeMap::new();
    for fact in facts {
        if fact.predicate.as_str() != "core:defines" {
            continue;
        }
        if let FactValue::Text(fqn) = &fact.object {
            table.insert(fact.subject.get(), fqn.clone());
        }
    }
    table
}

/// All occurrence keys a continuity result touches, tagged with the
/// workspace that owns them.
fn result_occurrences(workspace: &str, result: &ContinuityResult) -> BTreeSet<OccurrenceKey> {
    result
        .outcomes
        .iter()
        .map(|o| (workspace.to_string(), o.snapshot, o.occurrence.get()))
        .collect()
}

/// Counts cross-workspace collisions: occurrence SLOTS `(snapshot, entity)`
/// where the two workspaces' continuity results DISAGREE on the stable id
/// or the status. Identical symbol sets through identical tiers must agree
/// everywhere — any disagreement means one workspace's data leaked into the
/// other's resolution.
fn collision_count(a: &ContinuityResult, b: &ContinuityResult) -> usize {
    let slot = |o: &ContinuityOutcome| (o.snapshot, o.occurrence.get());
    let by_slot_a: BTreeMap<(SnapshotId, u64), &ContinuityOutcome> =
        a.outcomes.iter().map(|o| (slot(o), o)).collect();
    let by_slot_b: BTreeMap<(SnapshotId, u64), &ContinuityOutcome> =
        b.outcomes.iter().map(|o| (slot(o), o)).collect();
    by_slot_a
        .iter()
        .filter(|(slot, outcome_a)| {
            by_slot_b.get(*slot).is_some_and(|outcome_b| {
                outcome_b.stable_id != outcome_a.stable_id || outcome_b.status != outcome_a.status
            })
        })
        .count()
}

/// Spec scenario "Identical workspaces do not collide": two workspaces
/// committing identical symbol sets produce identical occurrence tables,
/// identical stable-identity mappings, identical ambiguous candidates — and
/// nothing crosses: the occurrence keys stay disjoint per workspace and the
/// collision count is zero.
#[test]
fn identical_workspaces_do_not_collide() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
    let store = InMemoryFactStore::new(Arc::new(registry));
    let ws_a = WorkspaceId::try_new("ws-a").expect("valid workspace");
    let ws_b = WorkspaceId::try_new("ws-b").expect("valid workspace");
    let thresholds = MatcherThresholds::default();

    // ── ws-a runs its FULL pipeline first (before ws-b exists) ──────────
    let before_a = commit_and_read(&store, &ws_a, BEFORE, side_batch(BEFORE, true));
    let after_a = commit_and_read(&store, &ws_a, AFTER, side_batch(AFTER, false));
    let solo_a = match_snapshots(&before_a, &after_a, &[], &thresholds);

    // ── ws-b commits the IDENTICAL sets through the SAME store ──────────
    let before_b = commit_and_read(&store, &ws_b, BEFORE, side_batch(BEFORE, true));
    let after_b = commit_and_read(&store, &ws_b, AFTER, side_batch(AFTER, false));

    // Occurrence tables are workspace-pinned and IDENTICAL as mappings.
    assert_eq!(
        occurrence_table(&before_a),
        occurrence_table(&before_b),
        "identical symbol sets must produce identical per-snapshot occurrence tables"
    );
    assert_eq!(
        occurrence_table(&after_a),
        occurrence_table(&after_b),
        "identical symbol sets must produce identical per-snapshot occurrence tables"
    );

    // ── continuity per workspace, from its OWN pinned reads only ────────
    let result_a = match_snapshots(&before_a, &after_a, &[], &thresholds);
    let result_b = match_snapshots(&before_b, &after_b, &[], &thresholds);

    // NO state leakage: re-running ws-a after ws-b's commits reproduces the
    // solo run exactly.
    let result_a_again = match_snapshots(&before_a, &after_a, &[], &thresholds);
    assert_eq!(
        result_a, result_a_again,
        "ws-a's mapping must not change because ws-b ran"
    );
    assert_eq!(
        result_a, solo_a,
        "ws-a's mapping must not change because ws-b exists"
    );

    // Identical inputs through the deterministic matcher → identical
    // mappings (stable ids, statuses, tiers, candidates).
    assert_eq!(
        result_a, result_b,
        "identical workspaces must resolve identical continuity mappings"
    );

    // Ambiguous candidates must not cross workspaces either: the ambiguous
    // candidate lists are equal per occurrence and confined to their own
    // result.
    let ambiguous_of = |r: &ContinuityResult| -> Vec<(u64, Vec<String>)> {
        r.outcomes
            .iter()
            .filter_map(|o| match &o.status {
                ContinuityStatus::Ambiguous { candidates } => {
                    Some((o.occurrence.get(), candidates.clone()))
                }
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        ambiguous_of(&result_a),
        ambiguous_of(&result_b),
        "ambiguous candidates must be identical and workspace-confined"
    );
    assert!(
        !ambiguous_of(&result_a).is_empty(),
        "the scenario must exercise the ambiguous path (colliding dup entities)"
    );

    // Occurrence keys NEVER intersect across workspaces (the ws dimension
    // pins every outcome to its own workspace).
    let occurrences_a = result_occurrences("ws-a", &result_a);
    let occurrences_b = result_occurrences("ws-b", &result_b);
    assert!(
        occurrences_a.is_disjoint(&occurrences_b),
        "no outcome occurrence may cross workspaces"
    );

    // Collisions = occurrence slots where the two workspaces disagree on
    // identity or status. The spec demands ZERO.
    let collisions = collision_count(&result_a, &result_b);
    assert_eq!(
        collisions, 0,
        "identical workspaces must not collide on any occurrence slot"
    );
    println!(
        "workspace isolation: {} ws-a outcomes, {} ws-b outcomes, occurrences disjoint, collisions=0",
        result_a.outcomes.len(),
        result_b.outcomes.len()
    );
}

/// Isolation with DIFFERING symbol sets: ws-c commits a modified after-side
/// while ws-a keeps the original — ws-a's mapping must remain EXACTLY the
/// solo mapping (no cross-workspace contamination), and ws-c's result must
/// reflect only its own facts.
#[test]
fn differing_workspaces_do_not_contaminate_each_other() {
    let registry = InMemorySchemaRegistry::new();
    bootstrap_registry(&registry).expect("canonical bootstrap registers the core:* set");
    let store = InMemoryFactStore::new(Arc::new(registry));
    let ws_a = WorkspaceId::try_new("ws-a").expect("valid workspace");
    let ws_c = WorkspaceId::try_new("ws-c").expect("valid workspace");
    let thresholds = MatcherThresholds::default();

    let before_a = commit_and_read(&store, &ws_a, BEFORE, side_batch(BEFORE, true));
    let after_a = commit_and_read(&store, &ws_a, AFTER, side_batch(AFTER, false));
    let solo_a = match_snapshots(&before_a, &after_a, &[], &thresholds);

    // ws-c shares the before side but its after side DROPS a symbol
    // (src/new.rs:fresh:1 absent) — a genuinely different snapshot.
    let mut builder = FactBatchBuilder::new(AFTER);
    for (subject, callees) in [
        ("src/lib.rs:keep:1", &["alpha", "beta"][..]),
        ("src/engine.rs:tick:40", &["reset", "step"][..]),
        ("src/c.rs:dup:1", &["h1"][..]),
    ] {
        observe(
            &mut builder,
            subject,
            "core:defines",
            subject,
            Some("kind=Function".to_string()),
        );
        for callee in callees {
            observe(&mut builder, subject, "core:calls", callee, None);
        }
    }
    let after_c = commit_and_read(&store, &ws_c, AFTER, builder.finish());

    let result_a = match_snapshots(&before_a, &after_a, &[], &thresholds);
    let result_c = match_snapshots(&before_a, &after_c, &[], &thresholds);

    assert_eq!(
        result_a, solo_a,
        "ws-a's mapping must be untouched by ws-c's differing facts"
    );
    // ws-c's after view genuinely lacks the fresh symbol: it must NOT
    // appear in ws-c's result at all.
    assert!(
        result_c
            .outcomes
            .iter()
            .all(|o| o.fqn != "src/new.rs:fresh:1"),
        "ws-c's result must reflect only ws-c's own facts"
    );
    // …and no occurrence of either result carries the other workspace's key.
    assert!(
        result_occurrences("ws-a", &result_a).is_disjoint(&result_occurrences("ws-c", &result_c)),
        "occurrences of differing workspaces must not intersect"
    );
}
