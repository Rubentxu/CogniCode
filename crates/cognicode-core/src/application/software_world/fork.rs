//! Fork operation (e71 WU2 — M9).
//!
//! Given a parent [`SoftwareWorld`] and a [`SourceMutation`] describing how
//! the candidate source is to differ from the parent, produce a child
//! [`SoftwareWorld`] that:
//!
//! - Carries its own id (caller-supplied; this module never mints one).
//! - Inherits the parent's `base_snapshot` — the canonical measurement
//!   target does not change across a fork. Only the candidate under
//!   analysis changes.
//! - Records the parent as `parent_world`.
//! - Carries the new `WorldSourceState::Forked { path, content_hash }`
//!   from the mutation.
//!
//! The parent is **never mutated**: `fork` takes `&SoftwareWorld`. Rust's
//! borrow checker already guarantees this; the test
//! `parent_is_unchanged_after_fork` asserts it structurally.
//!
//! ## Pure / deterministic / no I/O
//!
//! - No clock access.
//! - No UUID minting at the call site (the caller supplies the child id).
//! - No filesystem access (no materialization, no hashing).
//! - No graph-store interaction.
//!
//! The whole operation is a pure data transformation: it builds one
//! struct from two inputs.
//!
//! ## Why there is no fallible variant
//!
//! The operation is total by construction: every `SourceMutation` is a
//! valid candidate for a fork. A "fork without source change" (same
//! path + same hash as the parent) is a legitimate edge case: it
//! produces a child that is structurally a fork but is observationally
//! identical to the parent. The diff against the parent's snapshot is
//! empty — that is the UAT decisive case the envelope asks WU3 to
//! prove. We do not reject that case here; we let it flow through.
//!
//! If a future requirement forbids identity-forks, the rejection
//! belongs in a higher-level policy layer, not in this pure operation.
//!
//! ## Why the new world inherits `base_snapshot`
//!
//! The whole point of a fork is "candidate vs base". The `base_snapshot`
//! answers "against which canonical revision are we measuring this
//! candidate?". A fork does not change the base — it changes only what
//! is being measured. Changing the base would mean "different
//! experiment", not "fork of the same experiment".
//!
//! ## Why the parent is borrowed (and returned), not consumed
//!
//! The caller often needs both the parent (to keep computing with it)
//! and the new child. Returning `&SoftwareWorld` for the parent lets us
//! hand back the unchanged handle without forcing the caller to clone.

use std::path::PathBuf;

use crate::application::software_world::world::{ContentHash, SoftwareWorld, SoftwareWorldId};

/// Description of how a candidate source differs from a parent's source.
///
/// The mutation is **descriptive**: it names a new logical source
/// reference and a new content digest. It does not apply the change.
/// Applying the change is the job of the analysis layer (e67 ingest
/// over the new source), which is a separate concern.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceMutation {
    /// New logical source reference (workspace-relative path or commit
    /// URL — caller decides the convention).
    pub new_path: PathBuf,
    /// New content digest. The caller is responsible for computing or
    /// sourcing this digest; this module never hashes content.
    pub new_content_hash: ContentHash,
}

impl SourceMutation {
    /// Build a mutation that is observationally identical to the parent
    /// (same path, same digest). The diff between the parent snapshot
    /// and the candidate snapshot of this fork is empty — that is the
    /// UAT decisive case for the envelope's "fork without changes"
    /// requirement.
    pub fn identity(path: PathBuf, content_hash: ContentHash) -> Self {
        Self {
            new_path: path,
            new_content_hash: content_hash,
        }
    }

    /// Build a mutation from a logical change: a new path (or branch)
    /// and its digest.
    pub fn new_source(new_path: PathBuf, new_content_hash: ContentHash) -> Self {
        Self {
            new_path,
            new_content_hash,
        }
    }
}

/// Outcome of [`fork`]: the new world plus a borrow of the unchanged
/// parent.
///
/// The borrow on the parent is the structural proof that the parent was
/// not mutated: if Rust lets us hand it back, then nothing inside
/// `fork` could have written to it.
#[derive(Debug)]
pub struct ForkOutcome<'p> {
    /// The new (child) world produced by the fork.
    pub new_world: SoftwareWorld,
    /// Borrow of the parent world, unchanged.
    pub parent: &'p SoftwareWorld,
}

/// Fork a parent world into a new child world carrying the given
/// mutation.
///
/// - `parent`: the world being forked from. Borrowed, not consumed;
///   guaranteed unchanged after the call.
/// - `child_id`: id of the new world. Caller-supplied (this module
///   never mints ids).
/// - `mutation`: how the candidate source differs from the parent's
///   source.
///
/// The new world inherits the parent's `base_snapshot` — see the
/// module-level docs for the rationale.
pub fn fork<'p>(
    parent: &'p SoftwareWorld,
    child_id: SoftwareWorldId,
    mutation: SourceMutation,
) -> ForkOutcome<'p> {
    let new_world = SoftwareWorld::new_forked(
        child_id,
        parent.base_snapshot,
        parent.id.clone(),
        mutation.new_path,
        mutation.new_content_hash,
    );
    ForkOutcome { new_world, parent }
}
