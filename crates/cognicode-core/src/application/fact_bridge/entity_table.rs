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
///
/// E38.1 U5 trim: the table is an INTERNAL canonical-assignment step of
/// `FactBatchBuilder::finish`; the only consumer surface is
/// [`EntityIdTable::get`], so the test-only accessors
/// (`snapshot`/`len`/`is_empty`/`iter`) were removed.
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

    /// The [`EntityId`] for a raw id string, if the string is an entity.
    pub fn get(&self, id: &str) -> Option<EntityId> {
        self.ids.get(id).copied()
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

        // Sorted strings map onto sequential 1..N ids (read via `get`, the
        // trimmed consumer surface).
        assert_eq!(a.get("src/lib.rs:util:7"), Some(EntityId::new(1)));
        assert_eq!(a.get("src/main.rs"), Some(EntityId::new(2)));
        assert_eq!(a.get("src/main.rs:main:2"), Some(EntityId::new(3)));
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
        assert_eq!(table.get("a.rs"), Some(EntityId::new(1)));
        assert_eq!(table.get("b.rs"), Some(EntityId::new(2)));
        assert_eq!(
            table.get("c.rs"),
            None,
            "only the two unique strings are entities"
        );
    }

    /// `get` resolves known strings and degrades gracefully for unknown
    /// ones (graceful read degradation contract).
    #[test]
    fn get_resolves_known_strings_and_degrades_for_unknown_ones() {
        let table = EntityIdTable::build(
            SnapshotId::new(7),
            ["z.rs", "a.rs"].into_iter().map(str::to_string),
        );
        assert_eq!(table.get("a.rs"), Some(EntityId::new(1)));
        assert_eq!(table.get("z.rs"), Some(EntityId::new(2)));
        assert_eq!(table.get("missing.rs"), None);
    }
}
