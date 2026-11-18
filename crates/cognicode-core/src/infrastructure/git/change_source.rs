//! The change a scope names, read through the Git SCM adapter.
//!
//! The SCM kernel decides which files changed and how; this adapter only
//! adds the bytes of both sides, so the pack core can derive the changed
//! lines. Every scope reads from its own place: the index, the working
//! tree, one commit, a three-dot range or a virtual merge tree. A scope never
//! reads the changes of another one, so a pack about a commit never inherits
//! what is on disk.
//!
//! Bytes that come from a revision are checked against the digest the change
//! set recorded for them; bytes that cannot be used are reported as
//! unreadable, never as empty.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::application::context_pack::change::input::{
    ChangeBytes, ChangeScope, ChangeSource, Content, FileDelta, MAX_CHANGED_FILES, MAX_FILE_BYTES,
    SourceChange,
};
use crate::domain::scm::{
    BlobRead, Change, ChangeKind, ChangeReader, ChangeSet, ContentDigest, PathKey, RevisionReader,
    ScmError, SourceRef, WorkingCopyReader,
};
use crate::infrastructure::git::GitScmAdapter;

/// Where the head side of a change set keeps its bytes.
enum HeadSide {
    Revision,
    Index,
}

/// Reads the change of a [`ChangeScope`] from one Git repository.
#[derive(Debug, Clone)]
pub struct GitChangeSource {
    scm: GitScmAdapter,
    root: PathBuf,
    source: SourceRef,
}

impl GitChangeSource {
    /// Binds the source to the repository whose root is `root`.
    ///
    /// # Errors
    ///
    /// The root cannot be resolved to a directory.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, String> {
        let root = root
            .into()
            .canonicalize()
            .map_err(|error| format!("cannot resolve the repository root: {error}"))?;
        let source = SourceRef::new("git", &root.display().to_string())
            .map_err(|error| error.to_string())?;
        Ok(Self {
            scm: GitScmAdapter::new(root.clone()),
            root,
            source,
        })
    }

    fn staged(&self) -> Result<ChangeBytes, String> {
        let changes = self.scm.staged_changes(&self.source).map_err(unavailable)?;
        Ok(self.of_change_set(&changes, HeadSide::Index))
    }

    fn commit(&self, commit: &str) -> Result<ChangeBytes, String> {
        let head = self
            .scm
            .resolve(&self.source, commit)
            .map_err(unavailable)?;
        let parent = self
            .scm
            .resolve(&self.source, &format!("{commit}^"))
            .map_err(|_| format!("the commit `{commit}` has no first parent to compare against"))?;
        let changes = self
            .scm
            .changes_between(&parent, &head)
            .map_err(unavailable)?;
        Ok(self.of_change_set(&changes, HeadSide::Revision))
    }

    fn range(&self, base: &str, head: &str) -> Result<ChangeBytes, String> {
        let changes = self
            .scm
            .changes_since_merge_base(base, head)
            .map_err(unavailable)?;
        Ok(self.of_change_set(&changes, HeadSide::Revision))
    }

    fn merge_candidate(&self, ours: &str, theirs: &str) -> Result<ChangeBytes, String> {
        let changes = self
            .scm
            .changes_vs_merge_candidate(ours, theirs)
            .map_err(unavailable)?;
        Ok(self.of_change_set(&changes, HeadSide::Revision))
    }

    /// The change set of the kernel, with the bytes of both sides.
    fn of_change_set(&self, changes: &ChangeSet, head_side: HeadSide) -> ChangeBytes {
        let mut listed: Vec<&Change> = changes.changes().iter().collect();
        listed.sort_by(|a, b| a.path().as_str().cmp(b.path().as_str()));
        let files_not_read = listed.len().saturating_sub(MAX_CHANGED_FILES);
        listed.truncate(MAX_CHANGED_FILES);

        let readable = |change: &Change| !matches!(change.kind(), ChangeKind::Unknown);
        let before_paths: Vec<PathKey> = listed
            .iter()
            .filter(|change| readable(change) && change.before().is_some())
            .map(|change| before_path(change))
            .collect();
        let after_paths: Vec<PathKey> = listed
            .iter()
            .filter(|change| readable(change) && change.after().is_some())
            .map(|change| change.path().clone())
            .collect();
        let mut before_reads = self
            .scm
            .blobs_at(changes.base(), &before_paths, MAX_FILE_BYTES)
            .into_iter();
        let mut after_reads = match head_side {
            HeadSide::Revision => self
                .scm
                .blobs_at(changes.head(), &after_paths, MAX_FILE_BYTES),
            HeadSide::Index => self.scm.staged_blobs(&after_paths, MAX_FILE_BYTES),
        }
        .into_iter();

        let files = listed
            .into_iter()
            .map(|change| {
                let (before, after) = if readable(change) {
                    (
                        side_of(change.before(), &mut before_reads),
                        side_of(change.after(), &mut after_reads),
                    )
                } else {
                    (
                        Content::Unreadable("unclassified".to_string()),
                        Content::Unreadable("unclassified".to_string()),
                    )
                };
                SourceChange {
                    path: change.path().as_str().to_string(),
                    change: file_change(change.kind()),
                    before,
                    after,
                }
            })
            .collect();
        ChangeBytes {
            base: changes.base().as_str().to_string(),
            head: changes.head().as_str().to_string(),
            files,
            files_not_read,
        }
    }

    /// Everything the working tree holds that `HEAD` does not: staged,
    /// unstaged and untracked, with the bytes on disk as the head side.
    fn working_tree(&self) -> Result<ChangeBytes, String> {
        let staged = self.scm.staged_changes(&self.source).map_err(unavailable)?;
        let dirty = self.scm.dirty_paths(&self.source).map_err(unavailable)?;

        let staged_by_path: BTreeMap<&str, &Change> = staged
            .changes()
            .iter()
            .map(|change| (change.path().as_str(), change))
            .collect();
        let moved_from: BTreeSet<&str> = staged
            .changes()
            .iter()
            .filter_map(|change| match change.kind() {
                ChangeKind::Renamed { from } => Some(from.as_str()),
                _ => None,
            })
            .collect();
        let mut candidates: BTreeMap<String, PathKey> = BTreeMap::new();
        for key in staged
            .changes()
            .iter()
            .map(|change| change.path().clone())
            .chain(dirty)
        {
            candidates.insert(key.as_str().to_string(), key);
        }
        let files_not_read = candidates.len().saturating_sub(MAX_CHANGED_FILES);
        let candidates: Vec<PathKey> = candidates.into_values().take(MAX_CHANGED_FILES).collect();

        let before_paths: Vec<PathKey> = candidates
            .iter()
            .map(|key| {
                staged_by_path
                    .get(key.as_str())
                    .map_or_else(|| key.clone(), |change| before_path(change))
            })
            .collect();
        let before_reads = self
            .scm
            .blobs_at(staged.base(), &before_paths, MAX_FILE_BYTES);

        let mut files = Vec::new();
        for ((key, before_key), before_read) in
            candidates.iter().zip(&before_paths).zip(before_reads)
        {
            let hint = staged_by_path.get(key.as_str()).copied();
            let before = head_side_of(before_read);
            let after = self.disk(key);
            let renamed_away = hint.is_none()
                && moved_from.contains(key.as_str())
                && matches!(after, Content::Absent);
            if renamed_away {
                continue;
            }
            if let Some(file) = working_change(key, before_key, hint, before, after) {
                files.push(file);
            }
        }
        Ok(ChangeBytes {
            base: staged.base().as_str().to_string(),
            head: "working_tree".to_string(),
            files,
            files_not_read,
        })
    }

    /// The bytes of a path on disk, kept inside the repository root.
    fn disk(&self, key: &PathKey) -> Content {
        let full = self.root.join(key.as_str());
        let metadata = match std::fs::symlink_metadata(&full) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Content::Absent,
            Err(error) => return Content::Unreadable(error.to_string()),
        };
        if !metadata.is_file() {
            return Content::Unreadable("not a regular file".to_string());
        }
        if metadata.len() > MAX_FILE_BYTES {
            return Content::Unreadable(format!("larger than {MAX_FILE_BYTES} bytes"));
        }
        match full.canonicalize() {
            Ok(real) if real.starts_with(&self.root) => {}
            Ok(_) => return Content::Unreadable("outside the repository".to_string()),
            Err(error) => return Content::Unreadable(error.to_string()),
        }
        match std::fs::read(&full) {
            Ok(bytes) => Content::Bytes(bytes),
            Err(error) => Content::Unreadable(error.to_string()),
        }
    }
}

impl ChangeSource for GitChangeSource {
    fn read(&self, scope: &ChangeScope) -> Result<ChangeBytes, String> {
        match scope {
            ChangeScope::Staged => self.staged(),
            ChangeScope::WorkingTree => self.working_tree(),
            ChangeScope::Commit { commit } => self.commit(commit),
            ChangeScope::Range { base, head } => self.range(base, head),
            ChangeScope::MergeCandidate { ours, theirs } => self.merge_candidate(ours, theirs),
        }
    }
}

fn unavailable(error: ScmError) -> String {
    format!("the change is not available: {error}")
}

/// The path the base side of a change is read from: the origin of a move.
fn before_path(change: &Change) -> PathKey {
    match change.kind() {
        ChangeKind::Renamed { from } | ChangeKind::Copied { from } => from.clone(),
        _ => change.path().clone(),
    }
}

fn file_change(kind: &ChangeKind) -> FileDelta {
    match kind {
        ChangeKind::Added => FileDelta::Added,
        ChangeKind::Deleted => FileDelta::Deleted,
        ChangeKind::Modified => FileDelta::Modified,
        ChangeKind::Renamed { from } => FileDelta::Renamed {
            from: from.as_str().to_string(),
        },
        ChangeKind::Copied { from } => FileDelta::Copied {
            from: from.as_str().to_string(),
        },
        ChangeKind::Unknown => FileDelta::Unknown(
            "the change could not be classified, a conflict among the possible causes".to_string(),
        ),
    }
}

/// One side of a change set: the bytes the batch answered, checked against
/// the digest the kernel recorded, or why they cannot be used.
fn side_of(
    digest: Option<&ContentDigest>,
    reads: &mut impl Iterator<Item = Result<BlobRead, ScmError>>,
) -> Content {
    let Some(digest) = digest else {
        return Content::Absent;
    };
    match reads.next() {
        Some(Ok(BlobRead::Bytes(bytes))) if ContentDigest::from_bytes(&bytes) == *digest => {
            Content::Bytes(bytes)
        }
        Some(Ok(BlobRead::Bytes(_))) => {
            Content::Unreadable("the bytes differ from the digest the change recorded".to_string())
        }
        Some(Ok(BlobRead::TooLarge { bytes })) => {
            Content::Unreadable(format!("{bytes} bytes, larger than {MAX_FILE_BYTES} bytes"))
        }
        Some(Err(error)) => Content::Unreadable(error.to_string()),
        None => Content::Unreadable("the read returned no answer".to_string()),
    }
}

/// The `HEAD` side of a working-tree path: a path `HEAD` does not hold is
/// absent there.
fn head_side_of(read: Result<BlobRead, ScmError>) -> Content {
    match read {
        Ok(BlobRead::Bytes(bytes)) => Content::Bytes(bytes),
        Ok(BlobRead::TooLarge { bytes }) => {
            Content::Unreadable(format!("{bytes} bytes, larger than {MAX_FILE_BYTES} bytes"))
        }
        Err(ScmError::ContentUnavailable { .. }) => Content::Absent,
        Err(error) => Content::Unreadable(error.to_string()),
    }
}

/// What a path of the working tree is, against `HEAD`; `None` when it holds
/// what `HEAD` holds.
fn working_change(
    key: &PathKey,
    before_key: &PathKey,
    staged: Option<&Change>,
    before: Content,
    after: Content,
) -> Option<SourceChange> {
    let path = key.as_str().to_string();
    let file = |change: FileDelta| SourceChange {
        path: path.clone(),
        change,
        before: before.clone(),
        after: after.clone(),
    };
    if matches!(staged.map(Change::kind), Some(ChangeKind::Unknown)) {
        return Some(file(FileDelta::Unknown(
            "the index holds a conflict at this path".to_string(),
        )));
    }
    if let (Content::Unreadable(why), _) | (_, Content::Unreadable(why)) = (&before, &after) {
        return Some(file(FileDelta::Unknown(why.clone())));
    }
    let origin = || before_key.as_str().to_string();
    let moved = staged.and_then(|change| match change.kind() {
        ChangeKind::Renamed { .. } => Some(FileDelta::Renamed { from: origin() }),
        ChangeKind::Copied { .. } => Some(FileDelta::Copied { from: origin() }),
        _ => None,
    });
    match (&before, &after) {
        (Content::Absent, Content::Absent) => None,
        (Content::Bytes(_), Content::Absent) => Some(file(FileDelta::Deleted)),
        (Content::Absent, Content::Bytes(_)) => Some(file(moved.unwrap_or(FileDelta::Added))),
        (Content::Bytes(was), Content::Bytes(now)) if was == now => moved.map(file),
        _ => Some(file(moved.unwrap_or(FileDelta::Modified))),
    }
}
