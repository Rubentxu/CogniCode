//! Semantic fingerprint v1 (E38 WU-1, design D4) — ONE tagged multiset
//! computed from an entity's recovered facts by a pure function.
//!
//! The element set is `{"name:<name>"}` ∪ `{"call:<callee>"…}` ∪
//! `{"ref:<type>"…}` with occurrence counts in a sorted
//! `BTreeMap<String, usize>`. No extractor change and no new
//! `ProducerKind` — the fingerprint is derived, never produced.
//!
//! Similarity discipline (design D4): `0.0` unless the kinds are EQUAL
//! (kind is the hard pre-filter), otherwise the multiset Jaccard over the
//! tagged elements. A rename therefore costs exactly the two `name:`
//! elements (bodies dominate the score), and colliding candidates score
//! equal so the matcher's epsilon catches them. Entities with NO
//! calls/refs carry no structural evidence and score `0.0` — fail-closed,
//! they are never matched on the name element alone (the matcher's
//! fingerprint tier must not bless what T0/T1 already decide).

use std::collections::BTreeMap;

use super::view::EntityFacts;

/// The v1 semantic fingerprint of one entity occurrence (design D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticFingerprint {
    /// The entity kind — the hard pre-filter for similarity (never an
    /// element: different-kind entities can never match).
    pub kind: String,
    /// The entity name, mirrored from the input for matcher ergonomics.
    pub name: String,
    /// The tagged multiset, sorted (`name:`/`call:`/`ref:` tags → counts).
    pub elements: BTreeMap<String, usize>,
}

/// Computes the v1 semantic fingerprint of one entity (design D4, pure).
///
/// The name rides the multiset as the single `name:` element; callees and
/// type references accumulate as counted `call:`/`ref:` elements (the
/// multiset keeps multiplicity — call counts are signal).
pub fn fingerprint(e: &EntityFacts) -> SemanticFingerprint {
    let mut elements = BTreeMap::new();
    elements.insert(format!("name:{}", e.name), 1);
    for callee in &e.callees {
        *elements.entry(format!("call:{callee}")).or_insert(0) += 1;
    }
    for type_ref in &e.type_refs {
        *elements.entry(format!("ref:{type_ref}")).or_insert(0) += 1;
    }
    SemanticFingerprint {
        kind: e.kind.clone(),
        name: e.name.clone(),
        elements,
    }
}

/// Multiset Jaccard similarity over the tagged elements (design D4).
///
/// Returns `0.0` when the kinds differ (hard pre-filter) or when either
/// side carries no `call:`/`ref:` element (no structural evidence —
/// fail-closed); otherwise `|a ∩ b| / |a ∪ b|` with per-element counts
/// (`min`/`max`). Identical structural bodies with the same name score
/// exactly `1.0`.
pub fn similarity(a: &SemanticFingerprint, b: &SemanticFingerprint) -> f64 {
    if a.kind != b.kind {
        return 0.0;
    }
    let has_structure = |f: &SemanticFingerprint| {
        f.elements
            .keys()
            .any(|element| element.starts_with("call:") || element.starts_with("ref:"))
    };
    if !has_structure(a) || !has_structure(b) {
        return 0.0;
    }

    let mut intersection = 0usize;
    let mut union = 0usize;
    for (element, count) in &a.elements {
        let other = b.elements.get(element).copied().unwrap_or(0);
        intersection += (*count).min(other);
        union += (*count).max(other);
    }
    for (element, count) in &b.elements {
        if !a.elements.contains_key(element) {
            union += *count;
        }
    }
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::evidence_kernel::continuity::view::SnapshotEntityView;
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::value_objects::Provenance;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);

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

    fn defines(id: u64, subject: u64, fqn: &str, kind: &str) -> Fact {
        fact(
            id,
            subject,
            "core:defines",
            FactValue::Text(fqn.to_string()),
            Some(format!("kind={kind}")),
        )
    }

    fn text_fact(id: u64, subject: u64, predicate: &str, object: &str) -> Fact {
        fact(
            id,
            subject,
            predicate,
            FactValue::Text(object.to_string()),
            None,
        )
    }

    fn entity_with(kind: &str, name: &str, callees: &[&str], refs: &[&str]) -> EntityFacts {
        EntityFacts {
            entity: EntityId::new(1),
            fqn: format!("src/lib.rs:{name}:1"),
            kind: kind.to_string(),
            name: name.to_string(),
            callees: callees.iter().map(|c| c.to_string()).collect(),
            type_refs: refs.iter().map(|r| r.to_string()).collect(),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn fingerprint_composes_the_tagged_multiset_with_counts() {
        let e = entity_with(
            "Function",
            "greet",
            &["alpha", "alpha", "zeta"],
            &["HashMap"],
        );
        let fp = fingerprint(&e);

        assert_eq!(fp.kind, "Function");
        assert_eq!(fp.name, "greet");
        let mut expected = BTreeMap::new();
        expected.insert("name:greet".to_string(), 1);
        expected.insert("call:alpha".to_string(), 2);
        expected.insert("call:zeta".to_string(), 1);
        expected.insert("ref:HashMap".to_string(), 1);
        assert_eq!(fp.elements, expected);
    }

    #[test]
    fn similarity_is_zero_unless_kinds_are_equal() {
        let a = fingerprint(&entity_with("Function", "greet", &["fmt"], &[]));
        let b = fingerprint(&entity_with("Struct", "greet", &["fmt"], &[]));
        assert_eq!(
            similarity(&a, &b),
            0.0,
            "kind is the hard pre-filter (design D4)"
        );
    }

    #[test]
    fn similarity_of_identical_bodies_is_exactly_one() {
        let a = fingerprint(&entity_with(
            "Function",
            "greet",
            &["fmt", "print"],
            &["Map"],
        ));
        let b = fingerprint(&entity_with(
            "Function",
            "greet",
            &["fmt", "print"],
            &["Map"],
        ));
        assert!(close(similarity(&a, &b), 1.0));
    }

    #[test]
    fn similarity_of_a_pure_rename_costs_exactly_the_two_name_elements() {
        // Same kind and body, different name: the symmetric difference is
        // the two `name:` elements — here ∩ = 2 (the body), ∪ = 4 → 0.5.
        let before = fingerprint(&entity_with("Function", "foo", &["fmt", "fmt"], &[]));
        let after = fingerprint(&entity_with("Function", "bar", &["fmt", "fmt"], &[]));
        assert!(close(similarity(&before, &after), 2.0 / 4.0));

        // The cost is body-dominated: a larger unchanged body pushes the
        // same two-element rename cost toward 1.0 (here ∩ = 20, ∪ = 22).
        let big_body: [&str; 20] = [
            "fn0", "fn0", "fn1", "fn1", "fn2", "fn2", "fn3", "fn3", "fn4", "fn4", "fn5", "fn5",
            "fn6", "fn6", "fn7", "fn7", "fn8", "fn8", "fn9", "fn9",
        ];
        let big_before = fingerprint(&entity_with("Function", "foo", &big_body, &[]));
        let big_after = fingerprint(&entity_with("Function", "bar", &big_body, &[]));
        assert!(close(similarity(&big_before, &big_after), 20.0 / 22.0));
    }

    #[test]
    fn similarity_of_disjoint_elements_is_zero() {
        let a = fingerprint(&entity_with("Function", "foo", &["a"], &["T1"]));
        let b = fingerprint(&entity_with("Function", "bar", &["b"], &["T2"]));
        assert!(close(similarity(&a, &b), 0.0));
    }

    #[test]
    fn bodyless_entities_score_zero_fail_closed() {
        // Same name and kind but NO calls/refs: no structural evidence, so
        // the fingerprint must not bless a match on the name element alone.
        let a = fingerprint(&entity_with("Function", "greet", &[], &[]));
        let b = fingerprint(&entity_with("Function", "greet", &[], &[]));
        assert_eq!(similarity(&a, &b), 0.0);

        // One-sided structural evidence also degrades to zero (fail-closed
        // for the bodyless side).
        let with_body = fingerprint(&entity_with("Function", "greet", &["fmt"], &[]));
        assert_eq!(similarity(&a, &with_body), 0.0);
        assert_eq!(similarity(&with_body, &a), 0.0);
    }

    /// Spec scenario "Fingerprint is fact-deterministic": the same committed
    /// fact set fingerprinted twice (through the view recovery) produces
    /// identical fingerprints regardless of fact order.
    #[test]
    fn fingerprint_is_fact_deterministic() {
        let facts = vec![
            defines(1, 10, "src/lib.rs:greet:3", "Function"),
            text_fact(2, 10, "core:calls", "format"),
            text_fact(3, 10, "core:references", "HashMap"),
            text_fact(4, 10, "core:calls", "write"),
        ];

        let run_a = fingerprint(
            &SnapshotEntityView::from_facts(&facts, SNAPSHOT).entities[&EntityId::new(10)],
        );

        let mut shuffled = facts.clone();
        shuffled.reverse();
        shuffled.swap(0, 2);
        let run_b = fingerprint(
            &SnapshotEntityView::from_facts(&shuffled, SNAPSHOT).entities[&EntityId::new(10)],
        );

        assert_eq!(
            run_a, run_b,
            "identical fact sets must fingerprint identically"
        );
    }
}
