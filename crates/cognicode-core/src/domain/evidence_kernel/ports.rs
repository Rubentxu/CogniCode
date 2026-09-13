//! Kernel store ports (design D2 / D5 / D6).
//!
//! These ports are kernel-namespaced ON PURPOSE: the legacy
//! `domain::ports::EvidenceStore` (investigation evidence discovery) stays
//! untouched, and there are NO cross re-exports between the two (A4,
//! extend-never-mutate).
//!
//! Read pinning (design D5): every store read takes `(&WorkspaceId,
//! &SnapshotId)` and returns only that snapshot's data — mixing snapshots is
//! a contract violation (umbrella scenario "Historical read remains stable").
//!
//! `SchemaRegistry` is intentionally SYNC: it governs an in-process
//! vocabulary and performs no I/O (`QualityStore` precedent, design D6).
//! `FactStore` implementations MUST reject unregistered predicates at
//! commit.
//!
//! Domain purity: these traits perform no I/O themselves; adapters live in
//! `infrastructure::evidence_kernel`.

use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use std::path::Path;

use crate::domain::value_objects::{RevisionId, WorkspaceId};

use super::evidence::Evidence;
use super::fact::Fact;
use super::ids::{EntityId, EvidenceId, FactId, SnapshotId};
use super::relation::{RelationKind, RelationSpec};
use super::snapshot::SnapshotDescriptor;

/// Errors returned by the kernel store ports.
#[derive(Debug, thiserror::Error)]
pub enum KernelError {
    /// The requested snapshot does not exist for the workspace.
    #[error("snapshot {0} not found in workspace {1}")]
    SnapshotNotFound(SnapshotId, WorkspaceId),

    /// A fact references a predicate that is not registered in the schema.
    #[error("relation predicate is not registered: {0}")]
    UnregisteredPredicate(RelationKind),

    /// A fact (or batch) carried `ProducerKind::LlmAgent` provenance.
    #[error("LLM-agent output cannot be committed as an extracted Fact")]
    LlmProvenance,

    /// A fact's own snapshot pin disagrees with the commit target.
    #[error("fact {0} is pinned to snapshot {1}, but the commit targets snapshot {2}")]
    SnapshotMismatch(FactId, SnapshotId, SnapshotId),

    /// A commit batch re-uses fact-id space already assigned in the target
    /// snapshot (e38.2 CP-4): fact-id spaces start at 1 per batch, so an
    /// already-assigned fact id ≤ the batch's maximum id means the same id
    /// would be assigned twice inside one snapshot. Caller violation
    /// (`SnapshotMismatch` precedent): the caller computed the id space
    /// wrongly; the store rejects the batch atomically, carrying the
    /// smallest already-assigned id the batch would collide with.
    #[error("fact id {0} collides with ids already assigned in snapshot {1}")]
    FactIdSpaceCollision(FactId, SnapshotId),

    /// The revision cannot be mapped onto a snapshot (`RevisionId::NONE`).
    #[error("revision {0} is invalid (0 is the NONE sentinel)")]
    InvalidRevision(RevisionId),

    /// Backend failure (storage, serialization).
    ///
    /// RESERVED (e36 D4/D6 surface, first consumer pending): no production
    /// adapter constructs this variant yet (the in-memory adapter is
    /// infallible) — it is kept as the declared error surface so a future
    /// I/O-backed store does not reshape the port.
    #[error("kernel store error: {0}")]
    Store(String),
}

/// Write/read port for canonical facts, pinned per snapshot (design D5).
///
/// `commit` validates the batch atomically: every fact must carry non-LLM
/// provenance, agree with the target snapshot, and use a predicate
/// registered in the `SchemaRegistry` (design D6).
#[async_trait]
pub trait FactStore: Send + Sync {
    /// Commits a batch of facts into `snap` of `ws`, returning their ids in
    /// batch order. Rejects LLM provenance, unregistered predicates, facts
    /// whose `snapshot` field disagrees with `snap`, and batches whose
    /// fact-id space overlaps ids already assigned in the target snapshot
    /// (fact-id spaces start at 1 per batch, so any existing fact id ≤ the
    /// batch's maximum id is a collision — e38.2 CP-4) — a failed batch
    /// leaves no partial state.
    async fn commit(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
        batch: Vec<Fact>,
    ) -> Result<Vec<FactId>, KernelError>;

    /// All facts of `subject` recorded in `snap` of `ws`. Unknown subjects
    /// yield an empty vector (graceful read degradation).
    ///
    /// RESERVED (e36 D4/D6 surface, first consumer pending): no production
    /// consumer exercises this read yet — it is kept as the declared port
    /// surface (proven by the in-memory adapter tests) and MUST NOT be
    /// removed or repurposed until its first consumer lands.
    async fn facts_of(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
        subject: &EntityId,
    ) -> Result<Vec<Fact>, KernelError>;

    /// All facts recorded in `snap` of `ws`, in commit order (design D6,
    /// E37 additive read). Consumers that need a canonical order sort the
    /// result themselves. Unknown snapshots yield an empty vector (graceful
    /// read degradation, `facts_of` precedent).
    async fn facts_in_snapshot(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> Result<Vec<Fact>, KernelError>;
}

/// Kernel evidence port.
///
/// Kernel-namespaced (design D2): distinct from the legacy
/// `domain::ports::EvidenceStore`; no cross re-exports exist.
#[async_trait]
pub trait EvidenceStore: Send + Sync {
    /// Records one piece of evidence for a workspace, returning its id.
    async fn add(&self, ws: &WorkspaceId, e: Evidence) -> Result<EvidenceId, KernelError>;

    /// All evidence for `fact`, read pinned to `snap` of `ws`. Evidence
    /// attaches to a fact, so the snapshot pin is transitive through the
    /// fact's own pin; unknown facts yield an empty vector.
    async fn for_fact(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
        fact: FactId,
    ) -> Result<Vec<Evidence>, KernelError>;
}

/// Port mapping workspace revisions onto snapshot descriptors (design D4).
///
/// Facade over the existing revision model (ADR-039): `SnapshotId` is
/// bijective with `RevisionId` per workspace; no parallel snapshot timeline
/// is introduced.
#[async_trait]
pub trait SnapshotStore: Send + Sync {
    /// Returns (creating on first use) the descriptor for `rev` in `ws`.
    /// A published snapshot is immutable: later calls for the same
    /// `(ws, rev)` return the first descriptor. Rejects
    /// `RevisionId::NONE`.
    // Design D4 mandates the `from_revision` port name (ADR-039 facade);
    // it is a store operation taking `&self`, not a `from_*` constructor.
    #[allow(clippy::wrong_self_convention)]
    async fn from_revision(
        &self,
        ws: &WorkspaceId,
        rev: RevisionId,
    ) -> Result<SnapshotDescriptor, KernelError>;

    /// Looks up a previously published descriptor.
    async fn descriptor(
        &self,
        ws: &WorkspaceId,
        id: &SnapshotId,
    ) -> Result<SnapshotDescriptor, KernelError>;
}

/// Errors returned by [`SchemaRegistry`] operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// The predicate is already registered (the vocabulary is append-only).
    #[error("relation already registered: {0}")]
    AlreadyRegistered(RelationKind),
}

/// Governance port for the `RelationKind` vocabulary (design D6).
///
/// Intentionally SYNC: an in-process vocabulary with no I/O
/// (`QualityStore` precedent). `FactStore` implementations MUST reject
/// unregistered predicates at commit.
pub trait SchemaRegistry: Send + Sync {
    /// Registers a predicate. Re-registering the same predicate fails —
    /// the vocabulary is append-only.
    fn register(&self, k: RelationKind, s: RelationSpec) -> Result<(), SchemaError>;

    /// Looks up the spec for a predicate.
    fn lookup(&self, k: &RelationKind) -> Option<RelationSpec>;

    /// Lists the full vocabulary as `(kind, spec)` pairs, deterministically
    /// ordered by kind.
    fn list(&self) -> Vec<(RelationKind, RelationSpec)>;
}

/// One file rename/move observed by version control between two revisions
/// (E38 design D5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileRename {
    /// Repo-relative path of the file BEFORE the rename/move.
    pub old_path: String,
    /// Repo-relative path of the file AFTER the rename/move.
    pub new_path: String,
    /// Rename similarity reported by the tool, normalized to `[0.0, 1.0]`
    /// (git's `R<nnn>` score divided by 100).
    pub similarity: f64,
}

/// Sync port for version-control rename/move evidence (E38 design D5).
///
/// Fail-closed contract (spec "Version control unavailable degrades
/// safely"): on tool absence, repository absence, unknown or unborn
/// revisions, subprocess failure, non-UTF-8 output, or unparseable rows, an
/// implementation MUST return an EMPTY vector — no evidence, never invented
/// evidence — so continuity matching simply falls through the rename tier.
///
/// Intentionally SYNC with no `async` (kernel `SchemaRegistry` precedent):
/// the adapter performs one bounded subprocess; the continuity matcher
/// consumes the result as plain DATA (design D2 — the CALLER resolves this
/// port and passes `&[FileRename]` into the matcher, keeping the matcher
/// pure and deterministic). Domain purity: this trait performs no I/O;
/// the production adapter lives in `infrastructure::git`.
pub trait RenameEvidencePort: Send + Sync {
    /// All renames/moves between `before_rev` and `after_rev` in the
    /// repository rooted at `repo_root`. Both revisions are opaque caller
    /// strings; implementations pass them to the version-control tool as
    /// separate fixed arguments (never interpolated into a shell).
    fn renames_between(
        &self,
        repo_root: &Path,
        before_rev: &str,
        after_rev: &str,
    ) -> Vec<FileRename>;
}
