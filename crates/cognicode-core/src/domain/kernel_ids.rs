//! Kernel identifiers — strongly-typed newtype ids for the evidence kernel.
//!
//! **Ungated domain vocabulary (cycle e56).** These ids are fundamental
//! domain identifiers, not infrastructure, so they live outside the
//! `evidence-kernel` feature gate. The gated `evidence_kernel::ids` module
//! is now a thin re-export shim, so every existing
//! `evidence_kernel::ids::*` path keeps resolving and there is a single
//! source of truth.
//!
//! All kernel ids are `u64` newtypes in the style of `RevisionId`
//! (`domain::value_objects::revision_id`). `SnapshotId` is bijective with
//! `RevisionId` per workspace (design D4 / ADR-039): the same numeric value,
//! rendered as `snap:N` instead of `rev:N`. `SnapshotId(0)` is the invalid
//! sentinel, mirroring `RevisionId::NONE`. E38.1 U5 trim: the unused
//! `FromStr`/`ParseSnapshotIdError`/`to_revision`/`is_valid` surface was
//! removed — the bijective mapping enters through `SnapshotId::from_revision`
//! and the `snap:N` form stays `Display`-only.
//!
//! [`ExecutionId`] is the M6 addition: it links a produced finding to a
//! concrete detector execution record.

use std::fmt;

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
///
/// E38.1 U5 trim: the unused `new` constructor was removed — the only
/// production entry point is [`OccurrenceId::from_entity`] (the E38 design
/// D1 wiring), and the raw `u64` stays reachable via the public field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OccurrenceId(pub u64);

impl OccurrenceId {
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

    /// Maps a `RevisionId` onto its bijective `SnapshotId` (design D4).
    pub const fn from_revision(rev: RevisionId) -> Self {
        Self(rev.get())
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "snap:{}", self.0)
    }
}

// ============================================================================
// FactId
// ============================================================================

/// How a piece of kernel evidence relates to the fact it grades.
///
/// **Ungated domain vocabulary (cycle e62.4).** Lifted out of the gated
/// `evidence_kernel::evidence` for the same reason the ids were lifted in e56:
/// the findings domain must be able to reason about whether evidence supports
/// or refutes a fact **without** compiling in the whole kernel. The gated
/// module re-exports it, so there is a single source of truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EvidenceGrade {
    /// The evidence supports the fact.
    Supports,
    /// The evidence refutes the fact.
    Refutes,
    /// Independent evidence that agrees with the fact (corroboration).
    Corroborates,
}

impl EvidenceGrade {
    /// Whether this grade may back a finding.
    ///
    /// `Refutes` never may: evidence that contradicts the fact cannot license
    /// a gate.
    pub fn supports_a_claim(self) -> bool {
        match self {
            Self::Supports | Self::Corroborates => true,
            Self::Refutes => false,
        }
    }
}

impl std::fmt::Display for EvidenceGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            EvidenceGrade::Supports => "Supports",
            EvidenceGrade::Refutes => "Refutes",
            EvidenceGrade::Corroborates => "Corroborates",
        };
        f.write_str(s)
    }
}

/// Identifies one record in the Intelligence Event Log.
///
/// **Ungated domain vocabulary (cycle e63).** The log is append-only and its
/// ids are assigned by the store, never by the producer — the same lesson the
/// evidence kernel learned with `EvidenceId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EventId(pub u64);

impl EventId {
    /// Construct an event id.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The raw value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "evt:{}", self.0)
    }
}

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
// ExecutionId (M6, additive)
// ============================================================================

/// Identifies one detector execution record (M6 findings).
///
/// Links a produced finding to the concrete run that produced it, so
/// historical replay and audit can distinguish two runs of the same
/// detector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExecutionId(pub u64);

impl ExecutionId {
    /// Constructs an `ExecutionId` from a raw u64.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// Returns the raw u64 value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ExecutionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exec:{}", self.0)
    }
}

// ============================================================================
// OccurrenceId wiring (E38 design D1, additive)
// ============================================================================

/// E38 design D1: an occurrence IS the snapshot-scoped [`EntityId`] value
/// within its snapshot, so the two convert losslessly as a `const fn`.
/// E38.1 U5 trim: the unused `to_entity` inverse was removed — no consumer
/// ever round-tripped an occurrence back onto its entity.
impl OccurrenceId {
    /// Wires this occurrence onto the snapshot-scoped [`EntityId`] value
    /// it denotes.
    pub const fn from_entity(entity: EntityId) -> Self {
        Self(entity.0)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fmt::Debug;

    use super::*;

    // -------------------------------------------------------------------------
    // Task 3.1 RED — `snap:N` Display form + `rev:N` bijective mapping (D4)
    // -------------------------------------------------------------------------

    /// `SnapshotId::new(7).to_string()` must produce `"snap:7"` (the
    /// `snap:N` form is Display-only since the E38.1 U5 trim).
    #[test]
    fn snapshot_id_display_uses_snap_prefix() {
        let id = SnapshotId::new(7);
        assert_eq!(
            id.to_string(),
            "snap:7",
            "Display must produce 'snap:N' format"
        );
    }

    /// `SnapshotId::from_revision(rev)` must map onto the revision's value
    /// (design D4 / ADR-039). E38.1 U5 trim: the unused `to_revision`
    /// inverse was removed, so the mapping is asserted value-wise.
    #[test]
    fn snapshot_id_from_revision_maps_revision_value() {
        let rev = RevisionId::new(9);
        let snap = SnapshotId::from_revision(rev);
        assert_eq!(snap, SnapshotId::new(9));
        assert_eq!(snap.get(), rev.get());
        assert_eq!(snap.to_string(), "snap:9");
        assert_eq!(rev.to_string(), "rev:9");
    }

    /// `SnapshotId::NONE` is the zero sentinel, mirroring
    /// `RevisionId::NONE` (E38.1 U5 trim: `is_valid` was removed — the
    /// sentinel contract is asserted by value).
    #[test]
    fn snapshot_id_none_is_the_zero_sentinel() {
        assert_eq!(SnapshotId::NONE, SnapshotId::new(0));
        assert_eq!(SnapshotId::NONE.to_string(), "snap:0");
        assert_eq!(RevisionId::NONE, RevisionId::new(0));
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
        assert_json_round_trip(&OccurrenceId(2));
        assert_json_round_trip(&SnapshotId::new(3));
        assert_json_round_trip(&FactId::new(4));
        assert_json_round_trip(&EvidenceId::new(5));
    }

    /// Kernel ids must render distinct Display prefixes for diagnostics.
    #[test]
    fn kernel_ids_display_prefixes() {
        assert_eq!(EntityId::new(1).to_string(), "entity:1");
        assert_eq!(OccurrenceId(2).to_string(), "occ:2");
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
    /// `from_entity` must convert losslessly as a `const fn` (const use
    /// proves compile-time evaluability). E38.1 U5 trim: the unused
    /// `to_entity` inverse was removed; the raw value stays reachable via
    /// the public field.
    #[test]
    fn occurrence_id_entity_wiring_is_const_and_lossless() {
        const WIRED: OccurrenceId = OccurrenceId::from_entity(EntityId::new(9));
        assert_eq!(WIRED, OccurrenceId(9));
        assert_eq!(WIRED.get(), 9);

        let wired = OccurrenceId::from_entity(EntityId::new(1234));
        assert_eq!(wired.get(), 1234);
    }

    // -------------------------------------------------------------------------
    // e56 — ExecutionId (M6 finding → execution link)
    // -------------------------------------------------------------------------

    /// `ExecutionId` renders the `exec:N` Display form and round-trips.
    #[test]
    fn execution_id_display_and_round_trip() {
        let id = ExecutionId::new(11);
        assert_eq!(id.to_string(), "exec:11");
        assert_eq!(id.get(), 11);

        let json = serde_json::to_string(&id).expect("serialize");
        let parsed: ExecutionId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, id);
    }
}
