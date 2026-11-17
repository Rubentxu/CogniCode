//! SCM-neutral source identity (CAA-02 / CGH-020).
//!
//! The analysis pipeline will consume sources from any version-control
//! provider, not only Git. This module owns the NEUTRAL vocabulary:
//! what a source is, what one of its revisions is, what the tracked
//! content of a revision is, and what changed between two revisions.
//!
//! Deliberate boundaries:
//! * [`RevisionHandle`](crate::domain::scm::RevisionHandle) is an OPAQUE provider string. It is NOT the
//!   canonical numeric `RevisionId` of `domain::value_objects` and it
//!   is NOT a `SnapshotId`: those stay the internal identity of the
//!   analysis kernel, and adapters map opaque handles onto them
//!   (CAA-03/CAA-04), never the other way round.
//! * [`ContentDigest`](crate::domain::scm::ContentDigest) — a SHA-256 over canonical bytes — is the
//!   content identity that lets parsed artifacts be REUSED across
//!   different revision handles with identical trees. Semantic
//!   dependencies decide everything beyond that; this module never
//!   claims them.
//! * Nothing here performs I/O. The ports in [`ports`] are trait
//!   boundaries satisfied by fakes in tests and by adapters in
//!   `infrastructure` (the Git one arrives in CAA-03).
//!
//! Fail-closed rule, shared by every type: construction validates
//! (non-empty identities, well-formed digests, safe relative paths)
//! and ports answer with named [`ScmError`](crate::domain::scm::ScmError) variants instead
//! of invented data.

mod changeset;
#[cfg(any(test, feature = "test-support"))]
pub mod conformance;
mod error;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;
mod identity;
mod inventory;
pub mod ports;

pub use changeset::{Change, ChangeKind, ChangeSet};
pub use error::ScmError;
pub use identity::{ContentDigest, PathKey, ProviderId, RevisionHandle, SourceRef, SourceRevision};
pub use inventory::{InventoryEntry, SourceInventory};
pub use ports::{BlobRead, ChangeReader, ProviderCapabilities, RevisionReader, WorkingCopyReader};

#[cfg(test)]
mod conformance_tests;
