//! Snapshot entity views (E38 WU-1, design D3) — per-snapshot entity
//! identity recovered from committed facts ALONE.
//!
//! Facts record only `EntityId` subjects, so a [`SnapshotEntityView`] is
//! rebuilt on demand from a `FactStore::facts_in_snapshot` slice (design
//! key insight): each `core:defines` fact carries the entity's OWN fully
//! qualified name as the fact object (`tree_sitter_facts.rs:41-51` grammar)
//! and its `SymbolKind` as the `kind=<K>` provenance detail. Callees and
//! type references come from the same subject's `core:calls` /
//! `core:references` objects. The bridge needs NO edit — the view consumes
//! plain facts.
//!
//! Determinism: `from_facts` sorts the input itself (commit order in,
//! canonical order out), so identical fact sets yield identical views
//! regardless of commit or walk order (spec "Fingerprint is
//! fact-deterministic" / e38 determinism requirement). Entities are
//! `core:defines` subjects ONLY — files are evidence input, not tracked
//! entities.

use std::collections::BTreeMap;

use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::{EntityId, SnapshotId};

/// The definition predicate of the canonical `core:*` vocabulary
/// (`bootstrap::CORE_RELATIONS`): object = the subject's own FQN.
const DEFINES: &str = "core:defines";
/// The call predicate: object = the callee name.
const CALLS: &str = "core:calls";
/// The reference predicate: object = the referenced name/type.
const REFERENCES: &str = "core:references";

/// One entity's per-snapshot facts recovered from the canonical grammar
/// (design D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityFacts {
    /// The snapshot-scoped occurrence key (the `core:defines` subject).
    pub entity: EntityId,
    /// Fully qualified identity string, recovered from the `core:defines`
    /// fact OBJECT (`"{file}:{name}:{line}"` per the e37 D3 grammar).
    pub fqn: String,
    /// Symbol kind, recovered from the `core:defines` fact's
    /// `provenance.detail` `kind=<K>` (serde name of `SymbolKind`).
    /// Empty string when the fact carries no parsable kind detail.
    pub kind: String,
    /// The symbol name — the second-to-last `:`-separated segment of the
    /// FQN (extractor grammar `{file}:{name}:{line}`). Falls back to the
    /// whole FQN for shorter identity strings.
    pub name: String,
    /// Callee names from the subject's `core:calls` objects, sorted WITH
    /// duplicates kept (a multiset: call multiplicity is signal).
    pub callees: Vec<String>,
    /// Referenced names/types from the subject's `core:references`
    /// objects, sorted WITH duplicates kept (a multiset).
    pub type_refs: Vec<String>,
}

/// Per-snapshot entity view (design D3): every `core:defines` subject of
/// one snapshot with its recovered identity. Deterministic — keyed by the
/// snapshot-scoped [`EntityId`] in a `BTreeMap` and built from canonically
/// sorted facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotEntityView {
    /// The snapshot the view was built for (the caller's query pin).
    pub snapshot: SnapshotId,
    /// One entry per `core:defines` subject, ordered by `EntityId`.
    pub entities: BTreeMap<EntityId, EntityFacts>,
}

/// Canonical total order over facts: `(subject, predicate, object, detail,
/// fact id)`. The fact id is unique per batch (e37 D3 assigns `FactId`
/// `1..M` in canonical order), so the key is a total order even for
/// duplicate observations.
fn fact_order_key(f: &Fact) -> (u64, String, (u8, String), String, u64) {
    (
        f.subject.get(),
        f.predicate.as_str().to_string(),
        fact_value_key(&f.object),
        f.provenance.detail.clone().unwrap_or_default(),
        f.id.get(),
    )
}

/// Deterministic sort key over [`FactValue`] (`Float` has no total order,
/// so its bits are keyed).
fn fact_value_key(v: &FactValue) -> (u8, String) {
    match v {
        FactValue::Text(s) => (0, s.clone()),
        FactValue::Int(i) => (1, i.to_string()),
        FactValue::Float(f) => (2, f.to_bits().to_string()),
        FactValue::Bool(b) => (3, b.to_string()),
        FactValue::Ref(id) => (4, id.get().to_string()),
    }
}

/// The object's text when the fact grammar stores a name there
/// (`FactValue::Text`); every other object kind carries no recoverable
/// name and is skipped.
fn object_text(object: &FactValue) -> Option<&str> {
    match object {
        FactValue::Text(s) => Some(s),
        _ => None,
    }
}

/// The symbol kind riding a `core:defines` fact's provenance detail as
/// `kind=<K>` (design D3). Anything unparsable degrades to the empty kind.
fn kind_from_detail(detail: Option<&str>) -> String {
    detail
        .and_then(|d| d.strip_prefix("kind="))
        .unwrap_or_default()
        .to_string()
}

/// Derives the symbol name from a recovered FQN (v1 heuristic, design D3):
/// the extractor grammar is `"{file}:{name}:{line}"` (e37 D3 /
/// `extractor.rs:346`), so the name is the second-to-last `:`-separated
/// segment. Identity strings with fewer than three segments (foreign fact
/// producers) fall back to the whole string.
fn symbol_name_from_fqn(fqn: &str) -> String {
    let segments: Vec<&str> = fqn.split(':').collect();
    if segments.len() >= 3 {
        segments[segments.len() - 2].to_string()
    } else {
        fqn.to_string()
    }
}

impl SnapshotEntityView {
    /// Recovers the entity view of `snapshot` from a `facts_in_snapshot`
    /// slice (design D3).
    ///
    /// The slice is sorted internally — commit order in, canonical order
    /// out — so identical fact sets always produce an identical view. The
    /// first `core:defines` fact per subject (in canonical order) supplies
    /// the FQN and kind; `core:calls`/`core:references` facts accumulate
    /// the callee/type-ref multisets. Subjects without a `core:defines`
    /// fact are NOT entities. Facts whose snapshot pin disagrees with
    /// `snapshot` are trusted as-is: the caller owns snapshot pinning via
    /// the `FactStore` read contract.
    pub fn from_facts(facts: &[Fact], snapshot: SnapshotId) -> Self {
        let mut sorted: Vec<&Fact> = facts.iter().collect();
        sorted.sort_by_key(|f| fact_order_key(f));

        // First pass: definitions fix the entity set (defines subjects only).
        let mut entities: BTreeMap<EntityId, EntityFacts> = BTreeMap::new();
        for fact in &sorted {
            if fact.predicate.as_str() != DEFINES {
                continue;
            }
            let Some(fqn) = object_text(&fact.object) else {
                continue;
            };
            entities.entry(fact.subject).or_insert_with(|| EntityFacts {
                entity: fact.subject,
                fqn: fqn.to_string(),
                kind: kind_from_detail(fact.provenance.detail.as_deref()),
                name: symbol_name_from_fqn(fqn),
                callees: Vec::new(),
                type_refs: Vec::new(),
            });
        }

        // Second pass: relational facts enrich the existing entities.
        for fact in &sorted {
            let predicate = fact.predicate.as_str();
            if predicate != CALLS && predicate != REFERENCES {
                continue;
            }
            let Some(name) = object_text(&fact.object) else {
                continue;
            };
            let Some(entity) = entities.get_mut(&fact.subject) else {
                continue;
            };
            if predicate == CALLS {
                entity.callees.push(name.to_string());
            } else {
                entity.type_refs.push(name.to_string());
            }
        }

        // Multisets need a canonical order for deterministic fingerprints.
        for entity in entities.values_mut() {
            entity.callees.sort();
            entity.type_refs.sort();
        }

        Self { snapshot, entities }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::evidence_kernel::fact::{ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::FactId;
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::value_objects::Provenance;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);

    /// Builds one fact with the canonical analyzer provenance.
    fn fact(
        id: u64,
        subject: u64,
        predicate: &str,
        object: FactValue,
        detail: Option<String>,
    ) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(subject),
            RelationKind::try_new(predicate).expect("valid predicate"),
            object,
            SNAPSHOT,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                detail,
            ),
        )
        .expect("analyzer provenance is accepted")
    }

    /// A defines fact per the `tree_sitter_facts.rs` grammar: the FQN rides
    /// the object, the serde kind name rides `provenance.detail`.
    fn defines(id: u64, subject: u64, fqn: &str, kind: &str) -> Fact {
        fact(
            id,
            subject,
            "core:defines",
            FactValue::Text(fqn.to_string()),
            Some(format!("kind={kind}")),
        )
    }

    #[test]
    fn view_recovers_identity_kind_and_relations_from_defines_facts() {
        let facts = vec![
            defines(1, 10, "src/lib.rs:greet:3", "Function"),
            fact(
                2,
                10,
                "core:calls",
                FactValue::Text("format".to_string()),
                None,
            ),
            fact(
                3,
                10,
                "core:references",
                FactValue::Text("HashMap".to_string()),
                None,
            ),
            defines(4, 20, "src/other.rs:Widget:7", "Struct"),
            // File containment/imports do NOT create entities.
            fact(
                5,
                30,
                "core:contains",
                FactValue::Text("src/lib.rs:greet:3".to_string()),
                None,
            ),
            fact(
                6,
                30,
                "core:imports",
                FactValue::Text("std::collections".to_string()),
                None,
            ),
        ];

        let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);

        assert_eq!(view.snapshot, SNAPSHOT);
        assert_eq!(
            view.entities.len(),
            2,
            "only core:defines subjects are entities"
        );

        let greet = view.entities.get(&EntityId::new(10)).expect("greet");
        assert_eq!(greet.fqn, "src/lib.rs:greet:3");
        assert_eq!(greet.kind, "Function");
        assert_eq!(greet.name, "greet");
        assert_eq!(greet.callees, vec!["format".to_string()]);
        assert_eq!(greet.type_refs, vec!["HashMap".to_string()]);

        let widget = view.entities.get(&EntityId::new(20)).expect("widget");
        assert_eq!(widget.fqn, "src/other.rs:Widget:7");
        assert_eq!(widget.kind, "Struct");
        assert_eq!(widget.name, "Widget");
        assert!(widget.callees.is_empty());
        assert!(widget.type_refs.is_empty());
    }

    #[test]
    fn view_recoveries_are_sorted_multisets_and_entity_order_is_canonical() {
        let facts = vec![
            defines(1, 20, "src/b.rs:second:1", "Function"),
            defines(2, 10, "src/a.rs:first:1", "Function"),
            fact(
                3,
                10,
                "core:calls",
                FactValue::Text("zeta".to_string()),
                None,
            ),
            fact(
                4,
                10,
                "core:calls",
                FactValue::Text("alpha".to_string()),
                None,
            ),
            fact(
                5,
                10,
                "core:calls",
                FactValue::Text("alpha".to_string()),
                None,
            ),
        ];

        let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);

        let ids: Vec<u64> = view.entities.keys().map(|e| e.get()).collect();
        assert_eq!(ids, vec![10, 20], "BTreeMap orders entities by EntityId");

        let first = &view.entities[&EntityId::new(10)];
        assert_eq!(
            first.callees,
            vec!["alpha".to_string(), "alpha".to_string(), "zeta".to_string()],
            "callees are a sorted multiset (duplicates kept)"
        );
    }

    #[test]
    fn view_is_identical_under_shuffled_fact_input() {
        let facts = vec![
            defines(1, 10, "src/lib.rs:greet:3", "Function"),
            fact(
                2,
                10,
                "core:calls",
                FactValue::Text("format".to_string()),
                None,
            ),
            fact(
                3,
                10,
                "core:references",
                FactValue::Text("HashMap".to_string()),
                None,
            ),
            defines(4, 20, "src/other.rs:Widget:7", "Struct"),
            fact(
                5,
                20,
                "core:calls",
                FactValue::Text("draw".to_string()),
                None,
            ),
            fact(
                6,
                10,
                "core:calls",
                FactValue::Text("write".to_string()),
                None,
            ),
        ];

        let canonical = SnapshotEntityView::from_facts(&facts, SNAPSHOT);

        // Deterministic permutations (xorshift-seeded Fisher-Yates; no rand
        // dependency) of the same committed fact set.
        for seed in [1u64, 7, 42, 12345] {
            let mut shuffled = facts.clone();
            let mut state = seed;
            for i in (1..shuffled.len()).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let j = (state % (i as u64 + 1)) as usize;
                shuffled.swap(i, j);
            }
            let permuted = SnapshotEntityView::from_facts(&shuffled, SNAPSHOT);
            assert_eq!(
                permuted, canonical,
                "shuffled fact input (seed {seed}) must produce the identical view"
            );
        }
    }

    #[test]
    fn view_handles_missing_kind_detail_and_short_fqns() {
        let facts = vec![
            // No kind detail and a two-segment identity string.
            fact(
                1,
                40,
                "core:defines",
                FactValue::Text("plain-name".to_string()),
                None,
            ),
        ];
        let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);
        let entity = view.entities.get(&EntityId::new(40)).expect("entity");
        assert_eq!(entity.kind, "", "unparsable kind degrades to empty");
        assert_eq!(entity.name, "plain-name", "short FQN falls back whole");
    }
}
