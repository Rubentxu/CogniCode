//! The three small ports a provider must satisfy. Deliberately
//! narrow and synchronous (the `RenameEvidencePort` precedent):
//! implementations perform bounded reads, the domain stays pure, and
//! callers pass the results around as plain data.

use super::error::ScmError;
use super::identity::{PathKey, SourceRef, SourceRevision};
use super::inventory::SourceInventory;

/// Which optional readings a provider actually supports. The zero
/// value promises nothing; capabilities are declared, never guessed,
/// and a consumer asking for more gets
/// [`ScmError::CapabilityUnsupported`], not an empty lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    /// The provider can pair deleted/added paths into renames.
    pub rename_evidence: bool,
    /// The provider can answer ancestry questions.
    pub ancestor_queries: bool,
    /// The provider can read a staged index (Git-like tools).
    pub staged_index: bool,
    /// The provider can report dirty working-copy paths.
    pub dirty_tree: bool,
}

impl ProviderCapabilities {
    /// A provider that supports none of the optional readings.
    pub const NONE: Self = Self {
        rename_evidence: false,
        ancestor_queries: false,
        staged_index: false,
        dirty_tree: false,
    };
}

/// Reads revisions and their tracked content by reference name.
pub trait RevisionReader: Send + Sync {
    fn capabilities(&self) -> ProviderCapabilities;

    /// Turns a reference (`"HEAD"`, a branch, a SHA, a change id)
    /// into one concrete revision. Unknown reference:
    /// `ReferenceNotFound`, always attributed to provider and
    /// reference.
    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError>;

    /// The tracked content of a revision.
    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError>;

    /// The raw bytes of one tracked path in one revision. Missing
    /// bytes are `ContentUnavailable` — never a silent empty vector.
    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError>;

    /// The bytes of many tracked paths of one revision, one answer per
    /// requested path and in the same order. A blob larger than
    /// `max_bytes` is `TooLarge` and its bytes are not returned.
    ///
    /// The default asks `content` once per path; providers whose
    /// per-call cost grows with the size of the tree override it with
    /// a batched read.
    fn contents(
        &self,
        revision: &SourceRevision,
        paths: &[PathKey],
        max_bytes: u64,
    ) -> Vec<Result<BlobRead, ScmError>> {
        paths
            .iter()
            .map(|path| {
                self.content(revision, path).map(|bytes| {
                    if bytes.len() as u64 > max_bytes {
                        BlobRead::TooLarge {
                            bytes: bytes.len() as u64,
                        }
                    } else {
                        BlobRead::Bytes(bytes)
                    }
                })
            })
            .collect()
    }
}

/// One blob answered by [`RevisionReader::contents`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobRead {
    /// The blob, within the requested bound.
    Bytes(Vec<u8>),
    /// The blob exists but is larger than the bound; only its size is
    /// reported.
    TooLarge { bytes: u64 },
}

/// Answers what changed between two revisions and whether one is the
/// ancestor of the other.
pub trait ChangeReader: Send + Sync {
    /// Requires recorded ancestry between `base` and `head`;
    /// without it, `MissingAncestor` — an unrelated pair has no
    /// honest diff.
    fn changes_between(
        &self,
        base: &SourceRevision,
        head: &SourceRevision,
    ) -> Result<ChangeSet, ScmError>;

    fn is_ancestor(
        &self,
        maybe_ancestor: &SourceRevision,
        maybe_descendant: &SourceRevision,
    ) -> Result<bool, ScmError>;
}

/// Reads the UNCOMMITTED state the tracked-revision ports never see:
/// what is staged and what is dirty. Both are optional capabilities
/// (`staged_index`, `dirty_tree`) because not every provider models
/// an index or a working copy.
pub trait WorkingCopyReader: Send + Sync {
    fn staged_changes(&self, source: &SourceRef) -> Result<ChangeSet, ScmError>;

    fn dirty_paths(&self, source: &SourceRef) -> Result<Vec<PathKey>, ScmError>;
}

use super::changeset::ChangeSet;
