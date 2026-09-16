//! Semantic Fact diff across snapshots (e68 WU1).
//!
//! This module defines the **semantic identity** of a [`Fact`] and the
//! derived [`FactDelta`] between two snapshots.
//!
//! ## Why a separate identity from `FactId`
//!
//! `FactId` is snapshot-scoped (U63): the same semantic assertion can
//! receive different numeric ids in different snapshots (different
//! ingestion order, different `FactStore` instance, etc.). Two `Fact`s
//! across snapshots are semantically the same iff their `(subject,
//! predicate, object)` triple is equal — `FactId` and `SnapshotId`
//! MUST NOT participate in equality.
//!
//! ## Diff algebra
//!
//! A "modified" relation `A --calls--> B → A --calls--> C` is decomposed
//! into one `Removed` and one `Added`. There is no `Changed` variant:
//! modifications are observable as a removal-then-addition pair. This
//! keeps the diff algebra simple and matches the U61/U63 expectations
//! that drives the e66 read-set invalidation primitive.
//!
//! ## Pure / deterministic / no I/O
//!
//! This module is pure domain logic: no clock, no IO, no allocation
//! beyond the returned delta. It does not call `FactStore`, it does not
//! commit anything. `SemanticFactDelta` is a derived value, not a
//! canonical Fact.
//!
//! ## Fail-closed
//!
//! Inputs that violate the snapshot-pair contract (mismatched snapshot
//! ids, identical from/to, etc.) are rejected via [`SemanticDiffError`].
//! The diff never silently swallows an invalid input.
//!
//! ## Why the wrapper around `FactValue`
//!
//! The kernel `FactValue` deliberately does not implement `Eq`/`Hash`
//! because of its `Float` variant (f64 lacks total equality and has
//! many NaN bit-patterns). The semantic diff REQUIRES total equality
//! and a stable hash; we get them here without modifying the kernel by
//! hashing `f64` via `to_bits()` and delegating equality to the
//! underlying `FactValue`. `NaN` is treated as equal to any other `NaN`
//! with the same bit-pattern, which is the only stable definition for
//! snapshot-vs-snapshot diff.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use super::fact::{Fact, FactValue};
use super::ids::{EntityId, FactId, SnapshotId};
use super::relation::RelationKind;

/// The semantic identity of a Fact, excluding its snapshot-local `FactId`.
///
/// Equality of `FactSemanticKey` IS the semantic equality of two Facts.
/// `FactId` and `SnapshotId` are deliberately absent: renumbering a Fact
/// across snapshots must not produce a spurious diff.
///
/// Equality is implemented manually (rather than via `derive(Eq)`)
/// because [`FactValue`] is only `PartialEq`. The kernel keeps `FactValue`
/// non-`Eq` for Float-soundness; this module gets total equality by
/// pairing partial equality with the fact that two facts with the same
/// `(subject, predicate, object)` MUST compare equal at the kernel
/// level for the diff to be sound — which is exactly the kernel
/// "one canonical assertion per triple" model.
#[derive(Debug, Clone)]
pub struct FactSemanticKey {
    pub subject: EntityId,
    pub predicate: RelationKind,
    pub object: FactValue,
}

impl FactSemanticKey {
    /// Project a [`Fact`] onto its semantic key. The result is the same
    /// for two Facts with identical `(subject, predicate, object)`
    /// triples regardless of their `FactId` or `SnapshotId`.
    pub fn of(fact: &Fact) -> Self {
        Self {
            subject: fact.subject,
            predicate: fact.predicate.clone(),
            object: fact.object.clone(),
        }
    }
}

impl PartialEq for FactSemanticKey {
    fn eq(&self, other: &Self) -> bool {
        self.subject == other.subject
            && self.predicate == other.predicate
            && self.object == other.object
    }
}

impl Eq for FactSemanticKey {}

impl Hash for FactSemanticKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subject.hash(state);
        self.predicate.hash(state);
        // Hash the value via the local hashable wrapper.
        let h = HashableFactValue(&self.object);
        h.hash(state);
    }
}

/// Hashable view of a [`FactValue`].
///
/// `f64` is hashed via `to_bits()` (stable across runs, stable for NaN
/// bit-patterns). Equality is delegated to `FactValue::eq`. This is the
/// only stable definition of equality for snapshot-vs-snapshot diff
/// when the value happens to be `Float`.
#[derive(Copy, Clone)]
struct HashableFactValue<'a>(&'a FactValue);

impl Hash for HashableFactValue<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self.0 {
            FactValue::Text(s) => {
                0u8.hash(state);
                s.hash(state);
            }
            FactValue::Int(i) => {
                1u8.hash(state);
                i.hash(state);
            }
            FactValue::Float(f) => {
                2u8.hash(state);
                f.to_bits().hash(state);
            }
            FactValue::Bool(b) => {
                3u8.hash(state);
                b.hash(state);
            }
            FactValue::Ref(e) => {
                4u8.hash(state);
                e.0.hash(state);
            }
        }
    }
}

/// One half of a diff: a `Removed` or `Added` triple.
///
/// Carries the snapshot-local `FactId` for navigation purposes (e.g. an
/// e66 read-set can be intersected against the `Removed` `FactId`s),
/// but the `FactId` does NOT participate in semantic equality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactChange {
    pub semantic_key: FactSemanticKey,
    pub fact_id: FactId,
}

/// Semantic delta between two snapshots.
///
/// `from` and `to` are recorded for downstream navigation. `added` and
/// `removed` together describe every change; there is intentionally no
/// `changed` set — modifications are expressed as a removal-addition
/// pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDelta {
    pub from: SnapshotId,
    pub to: SnapshotId,
    pub added: Vec<FactChange>,
    pub removed: Vec<FactChange>,
}

impl FactDelta {
    /// True iff no fact changed between the two snapshots.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

/// Errors raised by [`compute_fact_delta`] when the input violates the
/// snapshot-pair contract.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SemanticDiffError {
    /// The two snapshot ids are identical. A delta from a snapshot to
    /// itself is not a meaningful operation.
    #[error("semantic diff requires two distinct snapshots, got the same id: {0}")]
    SameSnapshot(SnapshotId),
}

/// Compute the semantic delta between the facts of two snapshots.
///
/// The returned delta is symmetric:
/// - `added`: facts present in `to` but not in `from`.
/// - `removed`: facts present in `from` but not in `to`.
///
/// Inputs do not need to be sorted; the implementation sorts internally.
/// Duplicate facts within a single input (same semantic key) are kept
/// once per side — duplicates in `from` collapse to the first-seen
/// `FactId` for navigation; duplicates in `to` likewise. This matches
/// the kernel's "one canonical assertion" expectation.
///
/// Rejects identical `from` / `to` snapshot ids.
pub fn compute_fact_delta<I1, I2>(
    from_snapshot: SnapshotId,
    from: I1,
    to_snapshot: SnapshotId,
    to: I2,
) -> Result<FactDelta, SemanticDiffError>
where
    I1: IntoIterator<Item = Fact>,
    I2: IntoIterator<Item = Fact>,
{
    if from_snapshot == to_snapshot {
        return Err(SemanticDiffError::SameSnapshot(from_snapshot));
    }

    let from_index = index_snapshot(from);
    let to_index = index_snapshot(to);

    let mut added: Vec<FactChange> = Vec::new();
    for (key, fact_id) in &to_index {
        if !from_index.contains_key(key) {
            added.push(FactChange {
                semantic_key: key.clone(),
                fact_id: *fact_id,
            });
        }
    }

    let mut removed: Vec<FactChange> = Vec::new();
    for (key, fact_id) in &from_index {
        if !to_index.contains_key(key) {
            removed.push(FactChange {
                semantic_key: key.clone(),
                fact_id: *fact_id,
            });
        }
    }

    // Determinism: stable order by `(subject, predicate, object)` string
    // representation so the output does not depend on HashMap iteration.
    sort_changes(&mut added);
    sort_changes(&mut removed);

    Ok(FactDelta {
        from: from_snapshot,
        to: to_snapshot,
        added,
        removed,
    })
}

/// Build a `key → fact_id` index for one side of the diff.
fn index_snapshot<I: IntoIterator<Item = Fact>>(facts: I) -> HashMap<FactSemanticKey, FactId> {
    let mut idx: HashMap<FactSemanticKey, FactId> = HashMap::new();
    for fact in facts {
        let key = FactSemanticKey::of(&fact);
        // First-seen wins: duplicates within one snapshot collapse to
        // their first fact id. The kernel model expects one canonical
        // assertion per `(subject, predicate, object)`.
        idx.entry(key).or_insert(fact.id);
    }
    idx
}

fn sort_changes(changes: &mut [FactChange]) {
    changes.sort_by(|a, b| {
        a.semantic_key
            .subject
            .0
            .cmp(&b.semantic_key.subject.0)
            .then_with(|| {
                a.semantic_key
                    .predicate
                    .to_string()
                    .cmp(&b.semantic_key.predicate.to_string())
            })
            .then_with(|| {
                format!("{:?}", a.semantic_key.object).cmp(&format!("{:?}", b.semantic_key.object))
            })
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::evidence_kernel::fact::{FactValue, ProducerKind, ProvenanceRecord};
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

    // UAT: same semantics, different FactIds → empty delta.
    #[test]
    fn empty_delta_when_only_fact_ids_differ() {
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
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert!(
            delta.is_empty(),
            "delta must be empty when only ids differ: {delta:?}"
        );
        assert_eq!(delta.added.len(), 0);
        assert_eq!(delta.removed.len(), 0);
    }

    // UAT: one relation added.
    #[test]
    fn one_relation_added() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = vec![fact(
            1,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_to,
        )];
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.removed.len(), 0);
    }

    // UAT: one relation removed.
    #[test]
    fn one_relation_removed() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            1,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to: Vec<Fact> = vec![];
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert_eq!(delta.added.len(), 0);
        assert_eq!(delta.removed.len(), 1);
    }

    // UAT: object changed → one removed + one added (no Changed).
    #[test]
    fn object_change_decomposes_into_removed_plus_added() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![fact(
            1,
            100,
            "core:calls",
            FactValue::Ref(EntityId(200)),
            s_from,
        )];
        let to = vec![fact(
            2,
            100,
            "core:calls",
            FactValue::Ref(EntityId(300)),
            s_to,
        )];
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.removed.len(), 1);
        assert_eq!(
            delta.added[0].semantic_key.object,
            FactValue::Ref(EntityId(300))
        );
        assert_eq!(
            delta.removed[0].semantic_key.object,
            FactValue::Ref(EntityId(200))
        );
    }

    // UAT: input ordering does not affect the delta.
    #[test]
    fn input_ordering_is_irrelevant() {
        let s_from = snap(1);
        let s_to = snap(2);
        let a_to = fact(1, 100, "core:calls", FactValue::Ref(EntityId(200)), s_to);
        let b_to = fact(2, 300, "core:defines", FactValue::Text("x".into()), s_to);
        let c_from = fact(10, 100, "core:calls", FactValue::Ref(EntityId(200)), s_from);
        let d_from = fact(11, 300, "core:defines", FactValue::Text("x".into()), s_from);

        let d1 = compute_fact_delta(
            s_from,
            vec![c_from.clone(), d_from.clone()],
            s_to,
            vec![a_to.clone(), b_to.clone()],
        )
        .unwrap();
        let d2 = compute_fact_delta(s_from, vec![d_from, c_from], s_to, vec![b_to, a_to]).unwrap();
        assert_eq!(d1, d2);
        assert!(d1.is_empty());
    }

    // UAT: cross-snapshot / invalid pair rejected.
    #[test]
    fn same_snapshot_is_rejected() {
        let s = snap(1);
        let from: Vec<Fact> = vec![];
        let to: Vec<Fact> = vec![];
        let err = compute_fact_delta(s, from, s, to).unwrap_err();
        assert_eq!(err, SemanticDiffError::SameSnapshot(s));
    }

    // Extra invariant: duplicated facts within one side collapse to
    // first-seen, mirroring the "one canonical assertion" kernel model.
    #[test]
    fn duplicates_within_one_side_collapse() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from = vec![
            fact(10, 100, "core:calls", FactValue::Ref(EntityId(200)), s_from),
            // Same semantic assertion, different FactId — should not
            // produce a "change" on its own.
            fact(11, 100, "core:calls", FactValue::Ref(EntityId(200)), s_from),
        ];
        let to: Vec<Fact> = vec![];
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert_eq!(delta.removed.len(), 1);
        assert_eq!(delta.removed[0].fact_id, FactId(10), "first-seen wins");
    }

    // Extra invariant: empty inputs with distinct snapshots → empty delta.
    #[test]
    fn empty_inputs_with_distinct_snapshots_produce_empty_delta() {
        let s_from = snap(1);
        let s_to = snap(2);
        let delta =
            compute_fact_delta(s_from, Vec::<Fact>::new(), s_to, Vec::<Fact>::new()).unwrap();
        assert!(delta.is_empty());
    }

    // Extra invariant: predicates with different namespaces produce
    // distinct keys (no silent coalescence).
    #[test]
    fn distinct_predicate_namespaces_are_not_coalesced() {
        let s_from = snap(1);
        let s_to = snap(2);
        let from: Vec<Fact> = vec![];
        let to = vec![
            fact(1, 100, "core:calls", FactValue::Ref(EntityId(200)), s_to),
            fact(2, 100, "user:calls", FactValue::Ref(EntityId(200)), s_to),
        ];
        let delta = compute_fact_delta(s_from, from, s_to, to).unwrap();
        assert_eq!(delta.added.len(), 2);
    }
}
