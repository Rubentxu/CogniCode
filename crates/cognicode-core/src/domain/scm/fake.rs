//! A conforming in-memory source provider: the SCM ports with no Git,
//! no filesystem and no process.
//!
//! It follows the port contracts exactly and models no Git behaviour
//! beyond what the contracts name: revisions are opaque names with
//! direct parents, ancestry is the transitive closure of the parents,
//! and the optional readings answer only when they are declared. A
//! passing run of the conformance suite over it says something about
//! the suite and the contract, not about a quirk of the fake. It is
//! compiled for unit tests and under the `test-support` feature.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use super::changeset::{Change, ChangeKind, ChangeSet};
use super::error::ScmError;
use super::identity::{ContentDigest, PathKey, RevisionHandle, SourceRef, SourceRevision};
use super::inventory::{InventoryEntry, SourceInventory};
use super::ports::{ChangeReader, ProviderCapabilities, RevisionReader, WorkingCopyReader};

type Files = BTreeMap<PathKey, Vec<u8>>;

struct StoredRevision {
    parents: Vec<String>,
    files: Files,
}

/// The uncommitted state: what is staged and what differs on disk. It
/// is shared with [`FakeWorkingCopy`] so a test can change it while the
/// provider is in use.
#[derive(Default)]
struct WorkingState {
    checked_out: Option<String>,
    staged: Files,
    edited: Files,
}

/// An in-memory provider whose revisions are declared by the test.
pub struct FakeSource {
    provider: String,
    capabilities: ProviderCapabilities,
    revisions: BTreeMap<String, StoredRevision>,
    working: Arc<Mutex<WorkingState>>,
}

/// Changes the uncommitted state of a [`FakeSource`] from outside the
/// provider, the way a developer edits a checkout.
#[derive(Clone)]
pub struct FakeWorkingCopy {
    working: Arc<Mutex<WorkingState>>,
}

fn key(path: &str) -> PathKey {
    PathKey::new(path).unwrap_or_else(|error| panic!("fixture path {path:?} rejected: {error}"))
}

fn handle(name: &str) -> RevisionHandle {
    RevisionHandle::new(name)
        .unwrap_or_else(|error| panic!("fixture revision {name:?} rejected: {error}"))
}

fn inventory_of(handle: &RevisionHandle, files: &Files) -> SourceInventory {
    let entries = files
        .iter()
        .map(|(path, bytes)| InventoryEntry {
            path: path.clone(),
            content: ContentDigest::from_bytes(bytes),
        })
        .collect();
    SourceInventory::new(handle, entries).expect("a map has no duplicate paths")
}

impl FakeSource {
    /// A provider named `provider` that declares `capabilities` and
    /// holds no revision yet.
    pub fn new(provider: &str, capabilities: ProviderCapabilities) -> Self {
        Self {
            provider: provider.to_string(),
            capabilities,
            revisions: BTreeMap::new(),
            working: Arc::new(Mutex::new(WorkingState::default())),
        }
    }

    /// Adds a revision named `name` (its reference and its handle) whose
    /// direct parents are `parents` and whose tracked files are `files`.
    /// Parents are declared before their children.
    pub fn with_revision(mut self, name: &str, parents: &[&str], files: &[(&str, &[u8])]) -> Self {
        let unknown = parents.iter().find(|p| !self.revisions.contains_key(**p));
        assert!(
            unknown.is_none(),
            "revision {name:?} names an undeclared parent {unknown:?}"
        );
        self.revisions.insert(
            name.to_string(),
            StoredRevision {
                parents: parents.iter().map(|p| p.to_string()).collect(),
                files: files
                    .iter()
                    .map(|(path, bytes)| (key(path), bytes.to_vec()))
                    .collect(),
            },
        );
        self
    }

    /// Makes `name` the revision the working copy sits on, clean.
    pub fn checked_out(self, name: &str) -> Self {
        assert!(
            self.revisions.contains_key(name),
            "cannot check out undeclared revision {name:?}"
        );
        *self.working.lock().expect("working copy lock") = WorkingState {
            checked_out: Some(name.to_string()),
            ..WorkingState::default()
        };
        self
    }

    /// The handle through which a test edits the uncommitted state.
    pub fn working_copy(&self) -> FakeWorkingCopy {
        FakeWorkingCopy {
            working: Arc::clone(&self.working),
        }
    }

    fn stored(&self, name: &str) -> Result<&StoredRevision, ScmError> {
        self.revisions
            .get(name)
            .ok_or_else(|| ScmError::ReferenceNotFound {
                provider: self.provider.clone(),
                reference: name.to_string(),
            })
    }

    fn revision(&self, source: &SourceRef, name: &str) -> Result<SourceRevision, ScmError> {
        let stored = self.stored(name)?;
        let handle = handle(name);
        let digest = inventory_of(&handle, &stored.files).content_digest();
        Ok(SourceRevision::new(source.clone(), handle, digest))
    }

    fn refuse_foreign(&self, source: &SourceRef, reference: &str) -> Result<(), ScmError> {
        if source.provider().as_str() == self.provider {
            return Ok(());
        }
        Err(ScmError::ReferenceNotFound {
            provider: source.provider().as_str().to_string(),
            reference: reference.to_string(),
        })
    }

    fn require(&self, declared: bool, capability: &'static str) -> Result<(), ScmError> {
        if declared {
            Ok(())
        } else {
            Err(ScmError::CapabilityUnsupported { capability })
        }
    }

    /// Whether `ancestor` is `descendant` or reachable from it through
    /// parents: ancestry is transitive.
    fn reaches(&self, ancestor: &str, descendant: &str) -> bool {
        let mut seen = BTreeSet::new();
        let mut pending = vec![descendant.to_string()];
        while let Some(name) = pending.pop() {
            if name == ancestor {
                return true;
            }
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(stored) = self.revisions.get(&name) {
                pending.extend(stored.parents.iter().cloned());
            }
        }
        false
    }

    /// The change set from `base` to `head`, with exact-content moves
    /// paired into renames only when the provider declares it can.
    fn derive(&self, base: &SourceInventory, head: &SourceInventory) -> ChangeSet {
        let mut changes = ChangeSet::between_raw(base, head);
        if self.capabilities.rename_evidence {
            let pairs = exact_moves(&changes);
            changes.apply_rename_pairing(&pairs);
        }
        changes
    }
}

/// `(old, new)` for every deleted path whose exact content reappears as
/// exactly one added path.
fn exact_moves(changes: &ChangeSet) -> Vec<(PathKey, PathKey)> {
    let added: Vec<&Change> = changes
        .changes()
        .iter()
        .filter(|c| matches!(c.kind(), ChangeKind::Added))
        .collect();
    changes
        .changes()
        .iter()
        .filter(|c| matches!(c.kind(), ChangeKind::Deleted))
        .filter_map(|deleted| {
            let mut same = added.iter().filter(|a| a.after() == deleted.before());
            match (same.next(), same.next()) {
                (Some(moved), None) => Some((deleted.path().clone(), moved.path().clone())),
                _ => None,
            }
        })
        .collect()
}

impl RevisionReader for FakeSource {
    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities
    }

    fn resolve(&self, source: &SourceRef, reference: &str) -> Result<SourceRevision, ScmError> {
        self.refuse_foreign(source, reference)?;
        self.revision(source, reference)
    }

    fn inventory(&self, revision: &SourceRevision) -> Result<SourceInventory, ScmError> {
        let stored = self.stored(revision.handle().as_str())?;
        Ok(inventory_of(revision.handle(), &stored.files))
    }

    fn content(&self, revision: &SourceRevision, path: &PathKey) -> Result<Vec<u8>, ScmError> {
        self.stored(revision.handle().as_str())?
            .files
            .get(path)
            .cloned()
            .ok_or_else(|| ScmError::ContentUnavailable {
                path: path.as_str().to_string(),
            })
    }
}

impl ChangeReader for FakeSource {
    fn changes_between(
        &self,
        base: &SourceRevision,
        head: &SourceRevision,
    ) -> Result<ChangeSet, ScmError> {
        self.require(self.capabilities.ancestor_queries, "ancestor_queries")?;
        let (base_name, head_name) = (base.handle().as_str(), head.handle().as_str());
        let (base_stored, head_stored) = (self.stored(base_name)?, self.stored(head_name)?);
        if !self.reaches(base_name, head_name) {
            return Err(ScmError::MissingAncestor {
                base: base_name.to_string(),
                head: head_name.to_string(),
            });
        }
        let derived = self.derive(
            &inventory_of(base.handle(), &base_stored.files),
            &inventory_of(head.handle(), &head_stored.files),
        );
        Ok(derived)
    }

    fn is_ancestor(
        &self,
        maybe_ancestor: &SourceRevision,
        maybe_descendant: &SourceRevision,
    ) -> Result<bool, ScmError> {
        self.require(self.capabilities.ancestor_queries, "ancestor_queries")?;
        let (ancestor, descendant) = (
            maybe_ancestor.handle().as_str(),
            maybe_descendant.handle().as_str(),
        );
        self.stored(ancestor)?;
        self.stored(descendant)?;
        Ok(self.reaches(ancestor, descendant))
    }
}

impl WorkingCopyReader for FakeSource {
    fn staged_changes(&self, source: &SourceRef) -> Result<ChangeSet, ScmError> {
        self.require(self.capabilities.staged_index, "staged_index")?;
        self.refuse_foreign(source, "index")?;
        let working = self.working.lock().expect("working copy lock");
        let (base_name, base_files) = match &working.checked_out {
            Some(name) => (name.clone(), self.stored(name)?.files.clone()),
            None => ("empty".to_string(), Files::new()),
        };
        let mut index = base_files.clone();
        index.extend(working.staged.clone());
        let (base_handle, index_handle) = (handle(&base_name), handle("index"));
        Ok(self.derive(
            &inventory_of(&base_handle, &base_files),
            &inventory_of(&index_handle, &index),
        ))
    }

    fn dirty_paths(&self, source: &SourceRef) -> Result<Vec<PathKey>, ScmError> {
        self.require(self.capabilities.dirty_tree, "dirty_tree")?;
        self.refuse_foreign(source, "status")?;
        let working = self.working.lock().expect("working copy lock");
        Ok(working.edited.keys().cloned().collect())
    }
}

impl FakeWorkingCopy {
    /// Writes `bytes` to `path` and stages it: the index and the disk
    /// agree afterwards.
    pub fn stage(&self, path: &str, bytes: &[u8]) {
        let mut working = self.working.lock().expect("working copy lock");
        let path = key(path);
        working.edited.remove(&path);
        working.staged.insert(path, bytes.to_vec());
    }

    /// Writes `bytes` to `path` on disk without staging it.
    pub fn edit(&self, path: &str, bytes: &[u8]) {
        let mut working = self.working.lock().expect("working copy lock");
        working.edited.insert(key(path), bytes.to_vec());
    }
}
