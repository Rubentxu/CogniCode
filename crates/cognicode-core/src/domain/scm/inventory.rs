//! The tracked content of one revision: path → content digest.
//!
//! Not to be confused with the working-tree `SourceInventory` of
//! `infrastructure::graph::source_files`, which inventories what is
//! ON DISK right now. This type inventories what a PROVIDER says is
//! tracked in ONE revision — the versioned side of the same idea,
//! and the diff substrate for [`super::ChangeSet`].

use super::error::ScmError;
use super::identity::{ContentDigest, PathKey, RevisionHandle};
use sha2::{Digest as _, Sha256};

/// One tracked file of a revision: where and what (never how).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryEntry {
    pub path: PathKey,
    pub content: ContentDigest,
}

/// The full tracked content of one revision, canonically ordered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInventory {
    revision_handle: RevisionHandle,
    entries: Vec<InventoryEntry>,
    content_digest: ContentDigest,
}

impl SourceInventory {
    /// Sorts entries by path and folds them into one canonical
    /// content digest. A duplicated path is a provider bug and is
    /// refused, never silently merged.
    pub fn new(
        revision_handle: &RevisionHandle,
        mut entries: Vec<InventoryEntry>,
    ) -> Result<Self, ScmError> {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        if let Some(dup) = entries.windows(2).find(|w| w[0].path == w[1].path) {
            return Err(ScmError::DuplicatePath {
                path: dup[0].path.as_str().to_string(),
            });
        }
        // Digest over (path, content-hex) pairs with separators that
        // make re-grouping impossible: length-prefixed fields.
        let mut hasher = Sha256::new();
        for entry in &entries {
            let path = entry.path.as_str();
            let hex = entry.content.as_hex();
            hasher.update((path.len() as u64).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update((hex.len() as u64).to_le_bytes());
            hasher.update(hex.as_bytes());
        }
        let content_digest = ContentDigest::from_hex(&hex::encode(hasher.finalize()))
            .expect("a self-generated sha256 hex is well-formed by construction");
        Ok(Self {
            revision_handle: revision_handle.clone(),
            entries,
            content_digest,
        })
    }

    pub fn revision_handle(&self) -> &RevisionHandle {
        &self.revision_handle
    }

    pub fn entries(&self) -> &[InventoryEntry] {
        &self.entries
    }

    pub fn find(&self, path: &PathKey) -> Option<&InventoryEntry> {
        self.entries.iter().find(|e| &e.path == path)
    }

    /// The reuse key: two inventories with equal digests describe the
    /// same tracked content regardless of their revision handles.
    pub fn content_digest(&self) -> ContentDigest {
        self.content_digest.clone()
    }
}
