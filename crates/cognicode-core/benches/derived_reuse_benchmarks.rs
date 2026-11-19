//! Measurement of the derived-answer reuse: what the revalidation costs
//! against what the recomputation it can avoid costs.
//!
//! Run: `cargo bench -p cognicode-core --bench derived_reuse_benchmarks`
//!
//! The measurement is a ratio, not a threshold: the question is how much
//! a revalidation costs against the recomputation it can avoid, and the
//! adversarial case — a transition that touches everything the answer
//! depends on — says what the revalidation buys when it buys nothing.
//!
//! The fixture is a synthetic tree: `files` files, each one with
//! `symbols` symbols, and every symbol calling the one of the next file.
//! The answer under measurement is the one of a single file, which is the
//! shape a real query has.

use std::collections::BTreeMap;

use criterion::{Criterion, black_box, criterion_group, criterion_main};

use cognicode_core::application::change_tracking::derived_reuse::{
    DerivedAnswerStore, FactsQuery, answer_of, answer_on, reuse_decision,
};
use cognicode_core::application::change_tracking::read_scope::{ChangedFacts, ReadBreadth};
use cognicode_core::application::change_tracking::scm_delta::RevisionFacts;
use cognicode_core::domain::evidence_kernel::continuity::view::SnapshotEntityView;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::evidence_kernel::semantic_diff::compute_fact_delta;
use cognicode_core::domain::scm::PathKey;
use cognicode_core::domain::value_objects::Provenance;

/// The producer the facts of a revision carry.
const PRODUCER: &str = "cognicode.revision-facts/v4";

/// How many symbols every file of the fixture declares.
const SYMBOLS: usize = 20;

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

/// A tree of `files` files, each one with [`SYMBOLS`] symbols; the symbol
/// `s{index}` of a file calls the symbol `s{index}` of the next file.
///
/// Every symbol sits on `line`, so a head whose line is another one has
/// moved every symbol of the tree: the adversarial transition.
fn revision(snapshot: u64, files: usize, line: u32) -> RevisionFacts {
    let mut facts: Vec<Fact> = Vec::new();
    let mut path_facts: BTreeMap<PathKey, Vec<FactId>> = BTreeMap::new();
    let mut id = 1u64;
    let mut subject = 10u64;

    for file in 0..files {
        let file_path = format!("src/file{file}.rs");
        let mut ids: Vec<FactId> = Vec::new();
        for symbol in 0..SYMBOLS {
            let name = format!("s{symbol}");
            facts.push(fact(
                id,
                subject,
                "core:defines",
                &format!("{file_path}:{name}:{line}"),
                Some("kind=Function"),
                snapshot,
            ));
            ids.push(FactId::new(id));
            id += 1;

            if file + 1 < files {
                facts.push(fact(
                    id,
                    subject,
                    "core:calls",
                    &format!("s{symbol}"),
                    None,
                    snapshot,
                ));
                ids.push(FactId::new(id));
                id += 1;
            }
            subject += 1;
        }
        path_facts.insert(path(&file_path), ids);
    }

    RevisionFacts {
        snapshot: SnapshotId::new(snapshot),
        facts,
        path_facts,
    }
}

/// The query under measurement: the symbols of one file.
fn query() -> FactsQuery {
    FactsQuery {
        path: path("src/file0.rs"),
        looks_for: Vec::new(),
        breadth: ReadBreadth::Pointwise,
    }
}

/// What changed between two revisions, in the terms the scope reads.
fn changed_between(base: &RevisionFacts, head: &RevisionFacts) -> ChangedFacts {
    let delta = compute_fact_delta(
        base.snapshot,
        base.facts.clone(),
        head.snapshot,
        head.facts.clone(),
    )
    .expect("a delta");
    ChangedFacts::from_delta(&delta, base, head)
}

/// The revalidation against the recomputation it can avoid.
fn revalidation_against_recomputation(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_reuse");

    for files in [10usize, 100] {
        let base = revision(1, files, 1);
        let head = revision(2, files, 1);
        let changed = changed_between(&base, &head);
        let stored = answer_of(&base, &query(), PRODUCER, true);
        let head_view = SnapshotEntityView::from_facts(&head.facts, head.snapshot);

        group.bench_with_input(format!("revalidate/{files}files"), &files, |b, _| {
            b.iter(|| {
                black_box(reuse_decision(
                    black_box(&stored),
                    black_box(&changed),
                    black_box(&head_view),
                    PRODUCER,
                    true,
                ))
            })
        });

        group.bench_with_input(format!("recompute/{files}files"), &files, |b, _| {
            b.iter(|| black_box(answer_of(black_box(&head), &query(), PRODUCER, true)))
        });
    }

    group.finish();
}

/// The adversarial transition: every symbol of the tree moved, so the
/// revalidation is spent and the answer has to be computed again.
fn the_adversarial_transition(c: &mut Criterion) {
    let files = 100;
    let base = revision(1, files, 1);
    // Every symbol of the tree moved: the answer does not hold any more.
    let head = revision(2, files, 2);
    let changed = changed_between(&base, &head);
    let stored = answer_of(&base, &query(), PRODUCER, true);
    let head_view = SnapshotEntityView::from_facts(&head.facts, head.snapshot);

    let decision = reuse_decision(&stored, &changed, &head_view, PRODUCER, true);
    assert!(
        !decision.is_reusable(),
        "the adversarial transition invalidates the answer: {:?}",
        decision.reasons()
    );

    let mut group = c.benchmark_group("derived_reuse/adversarial");
    group.bench_function("revalidate", |b| {
        b.iter(|| {
            black_box(reuse_decision(
                black_box(&stored),
                black_box(&changed),
                black_box(&head_view),
                PRODUCER,
                true,
            ))
        })
    });
    group.bench_function("recompute", |b| {
        b.iter(|| black_box(answer_of(black_box(&head), &query(), PRODUCER, true)))
    });
    group.finish();
}

/// What the store of answers weighs: the answers it holds, and the ones
/// its bound makes it forget.
fn the_store_of_answers(c: &mut Criterion) {
    let files = 100;
    let bases: Vec<RevisionFacts> = (1..=9u64)
        .map(|snapshot| revision(snapshot, files, 1))
        .collect();
    let changed = ChangedFacts::default();

    let mut group = c.benchmark_group("derived_reuse/store");
    group.bench_function("answer_and_store", |b| {
        b.iter(|| {
            let mut store = DerivedAnswerStore::new(8);
            for (index, head) in bases.iter().enumerate() {
                let outcome = answer_on(
                    &mut store,
                    &query(),
                    SnapshotId::new(index as u64 + 1),
                    head,
                    &changed,
                    PRODUCER,
                    true,
                );
                black_box(outcome);
            }
            black_box(store.len())
        })
    });
    group.finish();

    // The accounting the measurement reports: the store holds one answer
    // per base it saw, and its bound makes it forget the oldest.
    let mut store = DerivedAnswerStore::new(8);
    for (index, head) in bases.iter().enumerate() {
        let _ = answer_on(
            &mut store,
            &query(),
            SnapshotId::new(index as u64 + 1),
            head,
            &changed,
            PRODUCER,
            true,
        );
    }
    eprintln!(
        "derived_reuse: the store holds {} answers and has forgotten {}",
        store.len(),
        store.forgotten()
    );
}

criterion_group!(
    benches,
    revalidation_against_recomputation,
    the_adversarial_transition,
    the_store_of_answers
);
criterion_main!(benches);
