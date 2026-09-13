//! Snapshot-scoped entity identity (E37 design D3) — `EntityIdTable`.

use std::collections::{BTreeMap, BTreeSet};

use crate::domain::evidence_kernel::ids::{EntityId, SnapshotId};

/// Maps raw entity id strings onto sequential [`EntityId`]s (design D3).
///
/// Snapshot-scoped: one table per `(workspace, snapshot)` key, so id
/// collisions across snapshots are impossible by construction. Assignment
/// is DETERMINISTIC: id strings are sorted lexicographically and mapped
/// onto `EntityId(1..N)`. No hashing is involved — default hashers are
/// process-random and would break byte-identical fact identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityIdTable {
    snapshot: SnapshotId,
    ids: BTreeMap<String, EntityId>,
}

impl EntityIdTable {
    /// Builds the table from raw id strings in ANY order: duplicates
    /// collapse and the sorted unique strings map onto `EntityId(1..N)`.
    pub fn build(snapshot: SnapshotId, id_strings: impl IntoIterator<Item = String>) -> Self {
        let unique: BTreeSet<String> = id_strings.into_iter().collect();
        let ids = unique
            .into_iter()
            .enumerate()
            .map(|(i, id)| (id, EntityId::new(i as u64 + 1)))
            .collect();
        Self { snapshot, ids }
    }

    /// The snapshot this table is scoped to.
    pub const fn snapshot(&self) -> SnapshotId {
        self.snapshot
    }

    /// The [`EntityId`] for a raw id string, if the string is an entity.
    pub fn get(&self, id: &str) -> Option<EntityId> {
        self.ids.get(id).copied()
    }

    /// Number of entities in the table.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// True when the table holds no entities.
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    /// Iterates `(id_string, EntityId)` pairs in sorted string order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, EntityId)> {
        self.ids.iter().map(|(id, e)| (id.as_str(), *e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sequential `EntityId(1..N)` assignment follows the sorted string
    /// order, whatever the input order — two tables built from permuted
    /// inputs are equal (design D3 determinism).
    #[test]
    fn assigns_sequential_ids_in_sorted_order_regardless_of_input_order() {
        let a = EntityIdTable::build(
            SnapshotId::new(1),
            ["src/main.rs", "src/main.rs:main:2", "src/lib.rs:util:7"]
                .into_iter()
                .map(str::to_string),
        );
        let b = EntityIdTable::build(
            SnapshotId::new(1),
            ["src/lib.rs:util:7", "src/main.rs", "src/main.rs:main:2"]
                .into_iter()
                .map(str::to_string),
        );
        assert_eq!(a, b, "input order must not change the table");

        let pairs: Vec<(&str, u64)> = a.iter().map(|(s, e)| (s, e.get())).collect();
        assert_eq!(
            pairs,
            vec![
                ("src/lib.rs:util:7", 1),
                ("src/main.rs", 2),
                ("src/main.rs:main:2", 3),
            ],
            "sorted strings map onto sequential 1..N ids"
        );
    }

    /// Duplicate id strings collapse into one entity.
    #[test]
    fn deduplicates_id_strings() {
        let table = EntityIdTable::build(
            SnapshotId::new(1),
            ["a.rs", "a.rs", "b.rs", "a.rs"]
                .into_iter()
                .map(str::to_string),
        );
        assert_eq!(table.len(), 2);
        assert_eq!(table.get("a.rs"), Some(EntityId::new(1)));
        assert_eq!(table.get("b.rs"), Some(EntityId::new(2)));
    }

    /// No id strings means an empty table.
    #[test]
    fn empty_input_yields_empty_table() {
        let table = EntityIdTable::build(SnapshotId::new(1), std::iter::empty());
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
    }

    /// `get` resolves known strings and degrades gracefully for unknown
    /// ones; the table reports its snapshot scope.
    #[test]
    fn get_resolves_known_strings_and_reports_scope() {
        let table = EntityIdTable::build(
            SnapshotId::new(7),
            ["z.rs", "a.rs"].into_iter().map(str::to_string),
        );
        assert_eq!(table.snapshot(), SnapshotId::new(7));
        assert_eq!(table.get("a.rs"), Some(EntityId::new(1)));
        assert_eq!(table.get("z.rs"), Some(EntityId::new(2)));
        assert_eq!(table.get("missing.rs"), None);
    }
}
