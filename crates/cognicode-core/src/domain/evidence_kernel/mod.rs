//! LSI evidence kernel (E36 M1) — the canonical Fact/Evidence/Provenance/
//! Snapshot model for Living Software Intelligence.
//!
//! Everything in this module is feature-gated behind the `evidence-kernel`
//! Cargo feature (off by default) following the `multimodal` precedent, so
//! the default build's public surface is unchanged.
//!
//! ## Module map
//!
//! - [`ids`] — kernel identifiers (`EntityId`, `OccurrenceId`, `SnapshotId`,
//!   `FactId`, `EvidenceId`, `StableEntityId`).
//! - [`relation`] — namespaced `RelationKind` (`"ns:name"`) + `RelationSpec`.
//! - [`fact`] — `FactValue`, `ProvenanceRecord`, `Fact`.
//! - [`evidence`] — `Evidence`, `EvidenceGrade`.
//! - [`snapshot`] — `SnapshotDescriptor` facade over the revision model.
//! - [`ports`] — kernel store ports (`FactStore`, `EvidenceStore`,
//!   `SnapshotStore`, `SchemaRegistry`) plus the sync `RenameEvidencePort`
//!   (E38 design D5).
//! - [`bootstrap`] — canonical `core:*` vocabulary + idempotent
//!   `bootstrap_registry` (E37 design D2).
//! - [`continuity`] — stable-identity continuity layer (E38 design D1/D3/D4):
//!   snapshot entity views + semantic fingerprint v1 + the tiered T0–T3
//!   matcher with pinned thresholds (WU-3).

#[cfg(feature = "evidence-kernel")]
pub mod bootstrap;
#[cfg(feature = "evidence-kernel")]
pub mod continuity;
#[cfg(feature = "evidence-kernel")]
pub mod evidence;
#[cfg(feature = "evidence-kernel")]
pub mod fact;
#[cfg(feature = "evidence-kernel")]
pub mod ids;
#[cfg(feature = "evidence-kernel")]
pub mod ports;
#[cfg(feature = "evidence-kernel")]
pub mod relation;
#[cfg(feature = "evidence-kernel")]
pub mod snapshot;

#[cfg(feature = "evidence-kernel")]
pub use bootstrap::{CORE_RELATIONS, bootstrap_registry};
#[cfg(feature = "evidence-kernel")]
pub use continuity::{
    ContinuityOutcome, ContinuityResult, ContinuityStatus, EntityFacts, MatchTier,
    MatcherThresholds, SemanticFingerprint, SnapshotEntityView, fingerprint, match_snapshots,
    similarity,
};
#[cfg(feature = "evidence-kernel")]
pub use evidence::{Evidence, EvidenceGrade};
#[cfg(feature = "evidence-kernel")]
pub use fact::{Fact, FactError, FactValue, ProducerKind, ProvenanceRecord};
#[cfg(feature = "evidence-kernel")]
pub use ids::{
    EntityId, EvidenceId, FactId, OccurrenceId, ParseSnapshotIdError, SnapshotId, StableEntityId,
};
#[cfg(feature = "evidence-kernel")]
pub use ports::{
    EvidenceStore, FactStore, FileRename, KernelError, RenameEvidencePort, SchemaError,
    SchemaRegistry, SnapshotStore,
};
#[cfg(feature = "evidence-kernel")]
pub use relation::{RelationKind, RelationKindError, RelationSpec};
#[cfg(feature = "evidence-kernel")]
pub use snapshot::SnapshotDescriptor;
