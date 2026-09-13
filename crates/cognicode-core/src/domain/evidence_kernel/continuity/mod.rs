//! Continuity layer (E38 M3, design D1/D3/D4) — durable stable identity
//! across snapshots, derived in-memory from committed facts alone.
//!
//! Snapshot-scoped `EntityId` values are occurrence keys (e37 D3); this
//! module recovers per-snapshot entity views and semantic fingerprints
//! from `facts_in_snapshot` slices so a later matcher (WU-3, design D3/D6)
//! can thread [`StableEntityId`]s across snapshot pairs. Rename evidence
//! enters as plain DATA resolved by the caller through the kernel
//! `RenameEvidencePort` (design D2) — the matcher never performs I/O and
//! produces no facts, so no `ProducerKind`/bincode surface is touched.
//!
//! ## Module map (WU-1 + WU-3)
//!
//! - [`view`] — [`EntityFacts`] + [`SnapshotEntityView::from_facts`]:
//!   identity/kind/name/relations recovered from the fact grammar.
//! - [`fingerprint`] — semantic fingerprint v1 (tagged multiset) and its
//!   kind-gated multiset Jaccard similarity.
//! - [`matcher`] — the tiered T0–T3 deterministic matcher with pinned
//!   thresholds, fail-closed ambiguity, and `StableEntityId` threading
//!   (design D3/D6, WU-3).

pub mod fingerprint;
pub mod matcher;
pub mod view;

pub use fingerprint::{SemanticFingerprint, fingerprint, similarity};
pub use matcher::{
    ContinuityOutcome, ContinuityResult, ContinuityStatus, MatchTier, MatcherThresholds,
    PINNED_AMBIGUITY_EPSILON, PINNED_JACCARD_MATCH_THRESHOLD, PINNED_RENAME_SIMILARITY_FLOOR,
    match_snapshots,
};
pub use view::{EntityFacts, SnapshotEntityView};
