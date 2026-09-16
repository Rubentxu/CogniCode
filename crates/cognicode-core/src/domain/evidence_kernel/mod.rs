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
//! - [`symbol_fqn`] — typed `"{file}:{name}:{line}"` identity (E38.1 CP-1).
//! - [`symbol_kind_detail`] — the `kind=<SerdeName>` codec (E38.1 CP-2).
//!
//! Gating exception (E38.1): [`symbol_fqn`] is compiled UNCONDITIONALLY —
//! the identity grammar is shared by the always-compiled legacy path
//! (`domain::aggregates::Symbol`) and the fact path, and the canonical
//! grammar must be centralized, not duplicated per feature gate.

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
pub mod semantic_diff;
#[cfg(feature = "evidence-kernel")]
pub mod snapshot;
pub mod symbol_fqn;
#[cfg(feature = "evidence-kernel")]
pub mod symbol_kind_detail;

// Facade re-exports (E38.1 U5 trim): only the two names actually consumed
// through this module path remain. Every other kernel type is reached via
// its canonical submodule path (`evidence_kernel::fact::Fact`,
// `evidence_kernel::ports::FactStore`, …) — a grep-verified zero-consumer
// re-export block (bootstrap/continuity/evidence/fact/ids/ports/relation/
// snapshot) was removed.
pub use symbol_fqn::SymbolFqn;
#[cfg(feature = "evidence-kernel")]
pub use symbol_kind_detail::SymbolKindDetail;
