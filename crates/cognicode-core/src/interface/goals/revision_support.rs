//! What the goals that look at one revision share: resolving a reference
//! over the Git adapter and assembling that revision's facts.

use std::path::Path;

use crate::application::change_tracking::revision_facts::{
    FactObjectStore, FactsLimits, ProducedRevision, produce_revision_facts,
};
use crate::application::revision_snapshots::RevisionSnapshotStore;
use crate::domain::scm::{RevisionReader, SourceRef, SourceRevision};
use crate::infrastructure::git::GitScmAdapter;

use super::compare::{RevisionFactsCoverage, side_coverage};

/// A revision resolved from a caller's reference, with its facts.
pub(super) struct ProducedRef {
    pub revision: SourceRevision,
    pub produced: ProducedRevision,
    /// Whether the revision snapshot was written to the cache.
    pub persisted: bool,
}

impl ProducedRef {
    pub(super) fn coverage(&self) -> RevisionFactsCoverage {
        side_coverage(&self.produced)
    }
}

/// The snapshot store of `workspace`, when the environment names a cache.
pub(super) fn snapshot_store(
    workspace: &Path,
    cache_root: Option<&Path>,
) -> Option<RevisionSnapshotStore> {
    cache_root.map(|root| RevisionSnapshotStore::for_workspace(root, workspace))
}

/// Resolves `reference` and assembles its facts.
///
/// With `persist`, per-file objects are reused from and written to the
/// cache and the snapshot is saved. Without it nothing is read from or
/// written to the cache: the facts are a full rebuild from the bytes.
pub(super) fn produce_for_ref(
    workspace: &Path,
    cache_root: Option<&Path>,
    reference: &str,
    persist: bool,
) -> Result<ProducedRef, String> {
    let root = workspace
        .canonicalize()
        .map_err(|error| format!("cannot resolve repository root: {error}"))?;
    let source =
        SourceRef::new("git", &root.to_string_lossy()).map_err(|error| error.to_string())?;
    let scm = GitScmAdapter::new(root.clone());
    let revision = scm
        .resolve(&source, reference)
        .map_err(|error| format!("cannot resolve revision `{reference}`: {error}"))?;
    let store = if persist {
        snapshot_store(&root, cache_root)
    } else {
        None
    };
    let objects = store.as_ref().map(FactObjectStore::beside);
    let produced =
        produce_revision_facts(&scm, &revision, objects.as_ref(), &FactsLimits::default())
            .map_err(|error| format!("cannot assemble the facts of `{reference}`: {error}"))?;
    let persisted = store
        .as_ref()
        .is_some_and(|store| store.save(&produced.snapshot).is_ok());
    Ok(ProducedRef {
        revision,
        produced,
        persisted,
    })
}
