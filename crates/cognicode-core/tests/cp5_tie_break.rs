#![cfg(feature = "evidence-kernel")]
//! E39 M4 CP-5 rider (design D8): cross-layer tie-break agreement under
//! duplicate observations.
//!
//! Same-name symbols force three independent layers to pick a canonical
//! representative:
//!
//! 1. **Batch canonical sort** ([`FactBatchBuilder::finish`], e37 D3):
//!    facts order by subject string before ids exist, so the FIRST
//!    `core:defines` fact is the lexicographically smallest FQN and it owns
//!    the smallest `EntityId`.
//! 2. **`SubjectIndex` tie-break** (`lsp_facts.rs`): a container name that
//!    maps to several same-file FQNs resolves to the lexicographically
//!    smallest one (the shared-resolver rule, E38.1 CP-3).
//! 3. **Continuity view first-defines recovery** (`SnapshotEntityView`):
//!    the first `core:defines` fact per subject in canonical order supplies
//!    the entity identity — duplicates must not change it.
//!
//! Any disagreement between the three layers fails this test: all of them
//! must name `src/a.rs:handle:1` as the canonical `handle` occurrence.

use std::path::Path;

use async_trait::async_trait;

use cognicode_core::application::fact_bridge::FactBatchBuilder;
use cognicode_core::domain::aggregates::Symbol;
use cognicode_core::domain::evidence_kernel::continuity::SnapshotEntityView;
use cognicode_core::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::traits::code_intelligence::{
    DocumentSymbol, HoverInfo, PrecisionTier, Reference, ReferenceKind,
    TieredCodeIntelligenceProvider, TieredOutcome, TypeHierarchy,
};
use cognicode_core::domain::value_objects::{Location, SymbolKind};

const SNAPSHOT: SnapshotId = SnapshotId::new(1);

/// The FQN every layer must agree on: the lexicographically smallest
/// `handle` occurrence (the duplicated name inside `src/a.rs`).
const CANONICAL: &str = "src/a.rs:handle:1";

/// An observer whose `src/a.rs` extraction context contains TWO same-name
/// `handle` symbols (0-based rows 0 and 5) and one `main`; `src/z.rs`
/// contains a third `handle`. Its reference observation carries container
/// `"handle"` sited in `src/a.rs` — the duplicate-name join.
struct DuplicateNameObserver;

#[async_trait]
impl TieredCodeIntelligenceProvider for DuplicateNameObserver {
    async fn get_symbols_tiered(&self, path: &Path) -> TieredOutcome<Vec<Symbol>> {
        let file = path.to_string_lossy();
        let symbols = if file.ends_with("a.rs") {
            let at = |line: u32| Location::new("src/a.rs", line, 0);
            vec![
                Symbol::new("handle", SymbolKind::Function, at(0)),
                Symbol::new("handle", SymbolKind::Function, at(5)),
                Symbol::new("main", SymbolKind::Function, at(9)),
            ]
        } else {
            vec![Symbol::new(
                "handle",
                SymbolKind::Function,
                Location::new("src/z.rs", 0, 0),
            )]
        };
        TieredOutcome::served(symbols, PrecisionTier::S2)
    }

    async fn find_references_tiered(
        &self,
        location: &Location,
        _include_declaration: bool,
    ) -> TieredOutcome<Vec<Reference>> {
        if location.file() == "src/a.rs" && location.line() == 0 {
            return TieredOutcome::served(
                vec![Reference {
                    location: Location::new("src/a.rs", 5, 4),
                    reference_kind: ReferenceKind::Call,
                    container: Some("handle".to_string()),
                }],
                PrecisionTier::S2,
            );
        }
        TieredOutcome::served(vec![], PrecisionTier::S2)
    }

    async fn get_hierarchy_tiered(&self, location: &Location) -> TieredOutcome<TypeHierarchy> {
        TieredOutcome::served(
            TypeHierarchy {
                symbol: Symbol::new("handle", SymbolKind::Function, location.clone()),
                parents: vec![],
                children: vec![],
            },
            PrecisionTier::S2,
        )
    }

    async fn get_definition_tiered(&self, _location: &Location) -> TieredOutcome<Option<Location>> {
        TieredOutcome::unresolved(vec![])
    }

    async fn get_document_symbols_tiered(
        &self,
        _path: &Path,
    ) -> TieredOutcome<Vec<DocumentSymbol>> {
        TieredOutcome::unresolved(vec![])
    }

    async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
        TieredOutcome::unresolved(vec![])
    }
}

/// One `core:defines` observation: the FQN rides BOTH subject and object
/// (the canonical grammar), with the declaring producer.
fn defines(subject: &str, producer: ProducerKind) -> (String, String, ProducerKind) {
    (subject.to_string(), subject.to_string(), producer)
}

/// GIVEN same-name `handle` symbols across files AND a duplicated defines
/// observation, WHEN one batch commits and the continuity view is recovered,
/// THEN the batch's first `core:defines` fact, the bridge's container
/// tie-break, and the view's first-defines recovery ALL agree on
/// [`CANONICAL`].
#[tokio::test]
async fn batch_sort_subject_index_and_view_agree_on_the_smallest_fqn() {
    let files = vec![
        std::path::PathBuf::from("src/a.rs"),
        std::path::PathBuf::from("src/z.rs"),
    ];

    let mut builder = FactBatchBuilder::new(SNAPSHOT);
    // Extraction-side defines: two same-name `handle` symbols in a.rs, one
    // in z.rs. The duplicate (same subject, second producer) proves the
    // view's first-defines recovery is stable under duplicate observations.
    for (subject, object, producer) in [
        defines("src/a.rs:handle:1", ProducerKind::DeterministicAnalyzer),
        defines("src/a.rs:handle:6", ProducerKind::DeterministicAnalyzer),
        defines("src/z.rs:handle:1", ProducerKind::DeterministicAnalyzer),
        // Duplicate observation of the canonical subject.
        defines("src/a.rs:handle:1", ProducerKind::RuntimeObserver),
    ] {
        builder
            .add_observation(
                subject,
                RelationKind::try_new("core:defines").expect("canonical predicate"),
                object,
                producer,
                Some("kind=Function".to_string()),
            )
            .expect("deterministic producers are accepted");
    }

    // The provider joins a container reference named `handle` sited in
    // src/a.rs — the SubjectIndex tie-break must pick the smallest FQN.
    builder.add_provider(&DuplicateNameObserver, &files).await;
    let facts = builder.finish();

    // ── Layer 1: batch canonical sort ───────────────────────────────────
    let defines_objects: Vec<&str> = facts
        .iter()
        .filter(|fact| fact.predicate.as_str() == "core:defines")
        .filter_map(|fact| match &fact.object {
            FactValue::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    let smallest = defines_objects
        .iter()
        .min()
        .expect("the batch defines at least one entity");
    let first = defines_objects
        .first()
        .expect("the batch defines at least one entity");
    assert_eq!(
        *first, *smallest,
        "the FIRST core:defines fact in canonical order must be the \
         lexicographically smallest FQN"
    );
    assert_eq!(*first, CANONICAL);

    let canonical_subject = facts
        .iter()
        .find(|fact| fact.predicate.as_str() == "core:defines")
        .map(|fact| fact.subject)
        .expect("first defines fact exists");
    assert_eq!(
        canonical_subject,
        EntityId::new(1),
        "subject strings sort before ids, so the smallest FQN owns EntityId(1)"
    );

    // ── Layer 2: SubjectIndex tie-break through the bridge ──────────────
    let call = facts
        .iter()
        .find(|fact| {
            fact.predicate.as_str() == "core:calls"
                && matches!(&fact.object, FactValue::Text(text) if text == "handle")
        })
        .expect("the container reference produced a core:calls fact");
    assert_eq!(
        call.subject, canonical_subject,
        "the bridge must resolve the duplicated container name to the same \
         entity the canonical batch order calls first"
    );

    // ── Layer 3: continuity view first-defines recovery ─────────────────
    let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);
    let first_entity = view
        .entities
        .values()
        .next()
        .expect("the view recovers at least one entity");
    assert_eq!(
        first_entity.fqn, CANONICAL,
        "the first entity by EntityId must carry the canonical FQN"
    );
    assert_eq!(first_entity.name, "handle");

    // Every same-name occurrence recovers its OWN FQN from its defines
    // fact — the duplicate observation must not smear identities.
    let handle_fqns: Vec<&str> = view
        .entities
        .values()
        .filter(|entity| entity.name == "handle")
        .map(|entity| entity.fqn.as_str())
        .collect();
    assert_eq!(
        handle_fqns,
        vec![
            "src/a.rs:handle:1",
            "src/a.rs:handle:6",
            "src/z.rs:handle:1"
        ],
        "the view recovers one FQN per defines subject in canonical order"
    );

    // And the canonical entity carries the joined callee (the reference
    // observation landed on it), so the layers agree on identity AND
    // relations.
    assert_eq!(
        first_entity.callees,
        vec!["handle".to_string()],
        "the joined call fact must enrich the canonical entity"
    );
}

/// The duplicate observation does not change the recovered view: rebuilding
/// with an extra identical defines observation yields the same identity set
/// (the first-defines rule is idempotent under duplicates).
#[tokio::test]
async fn duplicate_defines_observations_do_not_change_the_view() {
    let build = |duplicate: bool| {
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        for subject in ["src/a.rs:handle:1", "src/a.rs:handle:6"] {
            builder
                .add_observation(
                    subject,
                    RelationKind::try_new("core:defines").expect("canonical predicate"),
                    subject,
                    ProducerKind::DeterministicAnalyzer,
                    Some("kind=Function".to_string()),
                )
                .expect("deterministic producers are accepted");
        }
        if duplicate {
            builder
                .add_observation(
                    "src/a.rs:handle:1",
                    RelationKind::try_new("core:defines").expect("canonical predicate"),
                    "src/a.rs:handle:1",
                    ProducerKind::RuntimeObserver,
                    Some("kind=Function".to_string()),
                )
                .expect("runtime observers are accepted");
        }
        SnapshotEntityView::from_facts(&builder.finish(), SNAPSHOT)
    };

    let without = build(false);
    let with = build(true);
    assert_eq!(
        without, with,
        "a duplicate defines observation must not change the recovered view"
    );
    assert_eq!(
        without
            .entities
            .values()
            .next()
            .expect("entity present")
            .fqn,
        CANONICAL
    );
}

/// Guards the CP-5 invariant against a drift in any one layer: the shared
/// three-way agreement is asserted on real facts, and every same-name
/// symbol keeps a distinct EntityId (no silent collision).
#[tokio::test]
async fn same_name_entities_keep_distinct_ids_and_agree() {
    let mut builder = FactBatchBuilder::new(SNAPSHOT);
    for subject in [
        "src/a.rs:handle:1",
        "src/a.rs:handle:6",
        "src/z.rs:handle:1",
    ] {
        builder
            .add_observation(
                subject,
                RelationKind::try_new("core:defines").expect("canonical predicate"),
                subject,
                ProducerKind::DeterministicAnalyzer,
                Some("kind=Function".to_string()),
            )
            .expect("deterministic producers are accepted");
    }
    let facts: Vec<Fact> = builder.finish();
    let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);

    assert_eq!(
        view.entities.len(),
        3,
        "same-name symbols across files stay distinct entities"
    );
    let ids: Vec<EntityId> = view.entities.keys().copied().collect();
    assert_eq!(
        ids,
        vec![EntityId::new(1), EntityId::new(2), EntityId::new(3)]
    );
    assert_eq!(view.entities[&EntityId::new(1)].fqn, CANONICAL);
}
