//! A derived answer keeps the identity its dependents hold when the code
//! above it moves, and it is computed again when what it says changes.
//!
//! The scenario is the one the product cannot tell apart on its own: a
//! `core:defines` fact carries the line of the symbol it names, so editing
//! the code above a function changes the fact that names it. The answer
//! says the same thing, and the work that consumed it does not have to be
//! answered again.

use std::collections::BTreeMap;

use cognicode_core::application::change_tracking::derived_reuse::{
    DerivedAnswerStore, DerivedChange, DerivedOutcome, FactsQuery, answer_of, answer_on,
    plan_derived_work,
};
use cognicode_core::application::change_tracking::planner::{WorkDisposition, WorkId};
use cognicode_core::application::change_tracking::read_scope::ReadBreadth;
use cognicode_core::application::change_tracking::scm_delta::RevisionFacts;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::naming::NamespacedName;
use cognicode_core::domain::scm::PathKey;
use cognicode_core::domain::value_objects::Provenance;

/// The producer the facts of a revision carry.
const PRODUCER: &str = "cognicode.revision-facts/v4";

fn path(text: &str) -> PathKey {
    PathKey::new(text).expect("a path")
}

fn fact(
    id: u64,
    subject: u64,
    predicate: &str,
    object: &str,
    detail: Option<&str>,
    snapshot: u64,
) -> Fact {
    Fact::new(
        FactId::new(id),
        EntityId::new(subject),
        RelationKind::try_new(predicate).expect("a predicate"),
        FactValue::Text(object.to_string()),
        SnapshotId::new(snapshot),
        ProvenanceRecord::new(
            Provenance::Extracted,
            ProducerKind::DeterministicAnalyzer,
            detail.map(str::to_string),
        ),
    )
    .expect("a fact")
}

/// One file `src/a.rs` with `alpha`, which calls `helper`, and one file
/// `src/b.rs` with `helper`.
fn revision(snapshot: u64, alpha_line: u32, alpha_kind: &str) -> RevisionFacts {
    let alpha_detail = format!("kind={alpha_kind}");
    let facts = vec![
        fact(
            1,
            10,
            "core:defines",
            &format!("src/a.rs:alpha:{alpha_line}"),
            Some(&alpha_detail),
            snapshot,
        ),
        fact(2, 10, "core:calls", "helper", None, snapshot),
        fact(
            3,
            30,
            "core:defines",
            "src/b.rs:helper:1",
            Some("kind=Function"),
            snapshot,
        ),
    ];
    let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
    path_facts.insert(path("src/a.rs"), vec![FactId::new(1), FactId::new(2)]);
    path_facts.insert(path("src/b.rs"), vec![FactId::new(3)]);
    RevisionFacts {
        snapshot: SnapshotId::new(snapshot),
        facts,
        path_facts,
    }
}

fn query() -> FactsQuery {
    FactsQuery {
        path: path("src/a.rs"),
        looks_for: Vec::new(),
        breadth: ReadBreadth::Pointwise,
    }
}

fn changed(paths: &[&str]) -> DerivedChange {
    DerivedChange::of(&paths.iter().map(|text| path(text)).collect::<Vec<_>>())
}

fn work() -> WorkId {
    WorkId::new(NamespacedName::new("tests.pack").expect("a namespaced name"))
}

/// The answer of the query on the base revision is stored, and an
/// unrelated change reuses it: the query does not run again.
#[test]
fn an_unrelated_change_reuses_the_stored_answer() {
    let base = revision(1, 1, "Function");
    let head = revision(2, 1, "Function");
    let mut store = DerivedAnswerStore::new(4);

    let first = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &base,
        &changed(&[]),
        PRODUCER,
        true,
    );
    assert!(matches!(first, DerivedOutcome::Recomputed { .. }));

    let second = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &head,
        &changed(&["src/c.rs"]),
        PRODUCER,
        true,
    );
    assert!(
        matches!(second, DerivedOutcome::Reused { .. }),
        "nothing the answer depends on changed"
    );
}

/// The code above a symbol moves: the fact that names it changes and the
/// answer does not, so the work that consumed it keeps the identity it
/// holds and is not answered again.
#[test]
fn the_code_above_a_symbol_moving_keeps_the_identity_the_work_holds() {
    let base = revision(1, 1, "Function");
    let head = revision(2, 9, "Function");
    let mut store = DerivedAnswerStore::new(4);

    let first = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &base,
        &changed(&[]),
        PRODUCER,
        true,
    );
    let stored = match first {
        DerivedOutcome::Recomputed { answer, .. } => answer,
        other => panic!("the first run computes the answer: {other:?}"),
    };

    let second = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &head,
        &changed(&["src/a.rs"]),
        PRODUCER,
        true,
    );
    let recomputed = match &second {
        DerivedOutcome::Recomputed {
            answer,
            identity_preserved,
        } => {
            assert!(
                *identity_preserved,
                "the answer says the same thing: the dependents keep the identity they hold"
            );
            answer.clone()
        }
        other => panic!("the change forces the query to run again: {other:?}"),
    };

    assert_eq!(recomputed.identity, stored.identity);
    assert_ne!(
        recomputed.locations, stored.locations,
        "the place moved, so the answer says where the symbol is now"
    );
    assert_eq!(recomputed.locations[0].line, Some(9));

    let decision = plan_derived_work(&work(), &second);
    assert_eq!(
        decision.disposition,
        WorkDisposition::Unaffected,
        "the work holds an identity that is still the identity"
    );
}

/// What the answer says changes: the work is affected, and the answer it
/// holds is another answer.
#[test]
fn what_the_answer_says_changing_leaves_the_work_affected() {
    let base = revision(1, 1, "Function");
    let head = revision(2, 1, "Struct");
    let mut store = DerivedAnswerStore::new(4);

    let _ = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &base,
        &changed(&[]),
        PRODUCER,
        true,
    );
    let second = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &head,
        &changed(&["src/a.rs"]),
        PRODUCER,
        true,
    );

    let decision = plan_derived_work(&work(), &second);
    assert_eq!(decision.disposition, WorkDisposition::Affected);
}

/// The incremental path and the full path agree: the answer the store
/// keeps after a recomputation says what a full recomputation says.
#[test]
fn the_incremental_path_and_the_full_path_agree() {
    let base = revision(1, 1, "Function");
    let head = revision(2, 9, "Function");
    let mut store = DerivedAnswerStore::new(4);

    let _ = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &base,
        &changed(&[]),
        PRODUCER,
        true,
    );
    let incremental = answer_on(
        &mut store,
        &query(),
        SnapshotId::new(1),
        &head,
        &changed(&["src/a.rs"]),
        PRODUCER,
        true,
    );
    let incremental = match incremental {
        DerivedOutcome::Recomputed { answer, .. } => answer,
        other => panic!("the change forces the query to run again: {other:?}"),
    };

    let full = answer_of(&head, &query(), PRODUCER, true);
    assert_eq!(
        incremental.identity, full.identity,
        "the answer the store keeps says what a full recomputation says"
    );
    assert_eq!(incremental.relations, full.relations);
}
