//! Kernel identifiers — strongly-typed newtype ids for the evidence kernel.
//!
//! All kernel ids are `u64` newtypes in the style of `RevisionId`
//! (`domain::value_objects::revision_id`). `SnapshotId` is bijective with
//! `RevisionId` per workspace (design D4 / ADR-039): the same numeric value,
//! rendered as `snap:N` instead of `rev:N`. `SnapshotId(0)` is the invalid
//! sentinel, mirroring `RevisionId::NONE`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::domain::value_objects::RevisionId;

// ============================================================================
// EntityId
// ============================================================================

/// Identifies a code entity (symbol, module, type) the kernel records facts
/// about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityId(pub u64);

impl EntityId {
    /// Constructs an `EntityId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "entity:{}", self.0)
    }
}

// ============================================================================
// OccurrenceId
// ============================================================================

/// Identifies one concrete occurrence of an entity (e.g. a specific call
/// site or definition) within a snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OccurrenceId(pub u64);

impl OccurrenceId {
    /// Constructs an `OccurrenceId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for OccurrenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "occ:{}", self.0)
    }
}

// ============================================================================
// SnapshotId
// ============================================================================

/// Identifies a snapshot of a workspace's canonical state.
///
/// Bijective with `RevisionId` per workspace (design D4 / ADR-039):
/// `SnapshotId(n)` ↔ `RevisionId(n)`. `SnapshotId(0)` is the invalid
/// sentinel, mirroring `RevisionId::NONE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SnapshotId(pub u64);

impl SnapshotId {
    /// Reserved sentinel mirroring `RevisionId::NONE`; never valid.
    pub const NONE: SnapshotId = SnapshotId(0);

    /// Constructs a `SnapshotId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// A snapshot id is valid iff it is not the zero sentinel.
    pub const fn is_valid(self) -> bool {
        self.0 > 0
    }

    /// Maps a `RevisionId` onto its bijective `SnapshotId` (design D4).
    pub const fn from_revision(rev: RevisionId) -> Self {
        Self(rev.get())
    }

    /// Maps this `SnapshotId` back onto its bijective `RevisionId`.
    pub const fn to_revision(self) -> RevisionId {
        RevisionId::new(self.0)
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "snap:{}", self.0)
    }
}

/// Error type for [`SnapshotId::from_str`] failures.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ParseSnapshotIdError {
    #[error("invalid snapshot id format: expected 'snap:N' where N is a non-negative integer")]
    MalformedFormat,
    #[error("snapshot id must not be zero (0 is the invalid sentinel)")]
    ZeroSentinel,
}

impl FromStr for SnapshotId {
    type Err = ParseSnapshotIdError;

    /// Parse a `SnapshotId` from its `Display` form: `"snap:N"`.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let num = s
            .strip_prefix("snap:")
            .ok_or(ParseSnapshotIdError::MalformedFormat)?;
        if num.is_empty() {
            return Err(ParseSnapshotIdError::MalformedFormat);
        }
        let n: u64 = num
            .parse()
            .map_err(|_| ParseSnapshotIdError::MalformedFormat)?;
        if n == 0 {
            return Err(ParseSnapshotIdError::ZeroSentinel);
        }
        Ok(SnapshotId(n))
    }
}

// ============================================================================
// FactId
// ============================================================================

/// Identifies one canonical fact within the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FactId(pub u64);

impl FactId {
    /// Constructs a `FactId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for FactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fact:{}", self.0)
    }
}

// ============================================================================
// EvidenceId
// ============================================================================

/// Identifies one evidence record attached to a fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EvidenceId(pub u64);

impl EvidenceId {
    /// Constructs an `EvidenceId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EvidenceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evidence:{}", self.0)
    }
}

// ============================================================================
// StableEntityId (E38 design D1, additive)
// ============================================================================

/// Identifies a logical entity ACROSS snapshots (E38 M3 continuity layer).
///
/// Snapshot-scoped [`EntityId`] values are occurrence keys: they are
/// reassigned per snapshot by the canonical `EntityIdTable` (e37 design D3),
/// so any line shift or re-extraction changes them. `StableEntityId` is the
/// durable identity the continuity layer threads across snapshot pairs —
/// the code-level name for the spec's "EntityId" (e38 delta-spec
/// terminology pin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct StableEntityId(pub u64);

impl StableEntityId {
    /// Constructs a `StableEntityId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for StableEntityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "stable:{}", self.0)
    }
}

// ============================================================================
// OccurrenceId wiring (E38 design D1, additive)
// ============================================================================

/// E38 design D1: an occurrence IS the snapshot-scoped [`EntityId`] value
/// within its snapshot, so the two convert losslessly and as `const fn`s.
impl OccurrenceId {
    /// Wires this occurrence onto the snapshot-scoped [`EntityId`] value
    /// it denotes.
    pub const fn from_entity(entity: EntityId) -> Self {
        Self(entity.0)
    }

    /// Recovers the snapshot-scoped [`EntityId`] value this occurrence
    /// denotes.
    pub const fn to_entity(self) -> EntityId {
        EntityId(self.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fmt::Debug;
    use std::str::FromStr;

    use super::*;

    // -------------------------------------------------------------------------
    // Task 3.1 RED — `snap:N` ↔ `rev:N` bijective mapping (design D4)
    // -------------------------------------------------------------------------

    /// `SnapshotId::new(7).to_string()` must produce `"snap:7"` which parses back.
    #[test]
    fn snapshot_id_display_uses_snap_prefix() {
        let id = SnapshotId::new(7);
        assert_eq!(
            id.to_string(),
            "snap:7",
            "Display must produce 'snap:N' format"
        );

        let parsed = SnapshotId::from_str("snap:7").expect("'snap:7' must parse");
        assert_eq!(parsed, id, "Parsed value must equal original");
    }

    /// `SnapshotId` must round-trip through `FromStr` for arbitrary values.
    #[test]
    fn snapshot_id_from_str_round_trip() {
        for n in [1, 42, u64::MAX] {
            let id = SnapshotId::new(n);
            let parsed = SnapshotId::from_str(&id.to_string()).expect("Display form must parse");
            assert_eq!(parsed, id);
        }
    }

    /// `FromStr` must reject malformed and zero-sentinel inputs.
    #[test]
    fn snapshot_id_from_str_rejects_malformed() {
        assert!(SnapshotId::from_str("snap:").is_err());
        assert!(SnapshotId::from_str("snap:abc").is_err());
        assert!(SnapshotId::from_str("snap:-1").is_err());
        assert!(
            SnapshotId::from_str("snap:0").is_err(),
            "0 is the invalid sentinel"
        );
        assert!(
            SnapshotId::from_str("rev:7").is_err(),
            "wrong prefix must be rejected"
        );
        assert!(SnapshotId::from_str("7").is_err());
        assert!(SnapshotId::from_str("").is_err());
    }

    /// `SnapshotId::from_revision(rev)` must map onto `rev` and back —
    /// bijective per workspace (design D4 / ADR-039).
    #[test]
    fn snapshot_id_revision_mapping_is_bijective() {
        let rev = RevisionId::new(9);
        let snap = SnapshotId::from_revision(rev);
        assert_eq!(snap, SnapshotId::new(9));
        assert_eq!(snap.to_revision(), rev);
        assert_eq!(snap.to_string(), "snap:9");
        assert_eq!(rev.to_string(), "rev:9");
    }

    /// `SnapshotId::NONE` (0) must be invalid, mirroring `RevisionId::NONE`.
    #[test]
    fn snapshot_id_zero_is_invalid_sentinel() {
        assert!(!SnapshotId::NONE.is_valid());
        assert!(!SnapshotId::new(0).is_valid());
        assert!(SnapshotId::new(1).is_valid());
        assert!(!RevisionId::NONE.is_valid());
    }

    // -------------------------------------------------------------------------
    // Task 3.1 RED — kernel id ergonomics (copyable, hashable, serializable)
    // -------------------------------------------------------------------------

    /// Every kernel id must be `Copy` so it can be passed by value.
    #[test]
    fn kernel_ids_are_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<EntityId>();
        assert_copy::<OccurrenceId>();
        assert_copy::<SnapshotId>();
        assert_copy::<FactId>();
        assert_copy::<EvidenceId>();
    }

    /// Kernel ids must be hashable so they can key maps/sets.
    #[test]
    fn kernel_ids_are_hashable() {
        let mut set: HashSet<EntityId> = HashSet::new();
        set.insert(EntityId::new(1));
        set.insert(EntityId::new(1));
        set.insert(EntityId::new(2));
        assert_eq!(set.len(), 2);
    }

    /// Every kernel id must serialize and deserialize losslessly via serde.
    #[test]
    fn kernel_ids_serde_round_trip() {
        fn assert_json_round_trip<T>(id: &T)
        where
            T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + Debug,
        {
            let json = serde_json::to_string(id).expect("serialize");
            let parsed: T = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(&parsed, id);
        }

        assert_json_round_trip(&EntityId::new(1));
        assert_json_round_trip(&OccurrenceId::new(2));
        assert_json_round_trip(&SnapshotId::new(3));
        assert_json_round_trip(&FactId::new(4));
        assert_json_round_trip(&EvidenceId::new(5));
    }

    /// Kernel ids must render distinct Display prefixes for diagnostics.
    #[test]
    fn kernel_ids_display_prefixes() {
        assert_eq!(EntityId::new(1).to_string(), "entity:1");
        assert_eq!(OccurrenceId::new(2).to_string(), "occ:2");
        assert_eq!(FactId::new(3).to_string(), "fact:3");
        assert_eq!(EvidenceId::new(4).to_string(), "evidence:4");
    }

    // -------------------------------------------------------------------------
    // E38 WU-1 (design D1) — StableEntityId newtype + OccurrenceId wiring
    // -------------------------------------------------------------------------

    /// `StableEntityId` must render the `stable:N` Display form (design D1).
    #[test]
    fn stable_entity_id_display_uses_stable_prefix() {
        assert_eq!(StableEntityId::new(7).to_string(), "stable:7");
        assert_eq!(
            StableEntityId::new(u64::MAX).to_string(),
            "stable:18446744073709551615"
        );
    }

    /// `StableEntityId` must round-trip losslessly through JSON and bincode
    /// (same derives as its siblings).
    #[test]
    fn stable_entity_id_serde_round_trip() {
        let id = StableEntityId::new(42);
        let json = serde_json::to_string(&id).expect("serialize");
        let parsed: StableEntityId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id, "json round-trip lost identity");

        let bytes = bincode::serde::encode_to_vec(id, bincode::config::standard()).expect("encode");
        let (decoded, _): (StableEntityId, usize) =
            bincode::serde::decode_from_slice(&bytes, bincode::config::standard()).expect("decode");
        assert_eq!(decoded, id, "bincode round-trip lost identity");
    }

    /// `StableEntityId` must be Copy/hashable/orderable like its siblings.
    #[test]
    fn stable_entity_id_is_copy_hashable_and_ordered() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<StableEntityId>();

        let mut set = HashSet::new();
        set.insert(StableEntityId::new(1));
        set.insert(StableEntityId::new(1));
        set.insert(StableEntityId::new(2));
        assert_eq!(set.len(), 2);

        assert!(StableEntityId::new(1) < StableEntityId::new(2));
    }

    /// An occurrence IS the snapshot-scoped `EntityId` value (design D1):
    /// `from_entity`/`to_entity` must convert losslessly, as `const fn`s
    /// (const use proves compile-time evaluability).
    #[test]
    fn occurrence_id_entity_wiring_is_const_and_lossless() {
        const WIRED: OccurrenceId = OccurrenceId::from_entity(EntityId::new(9));
        assert_eq!(WIRED, OccurrenceId::new(9));
        assert_eq!(WIRED.to_entity(), EntityId::new(9));

        let round_trip = OccurrenceId::from_entity(EntityId::new(1234)).to_entity();
        assert_eq!(round_trip, EntityId::new(1234));
    }
}
