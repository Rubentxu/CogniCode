//! Loading the SCIP index a workspace declares.
//!
//! A workspace that has a SCIP index declares it in two files:
//!
//! - `<workspace>/.cognicode/scip/index.scip` — the encoded index;
//! - `<workspace>/.cognicode/scip/index.scip.json` — the identity envelope
//!   ([`ScipIndexBinding`]) the producer vouches for.
//!
//! The loader reads both and checks the index against the state of the
//! workspace it is about to be used for: the root, the current revision and
//! the digest of the payload. An index produced for another workspace,
//! another revision or another payload is refused, and the caller keeps the
//! lower tiers.
//!
//! Nothing is executed. The loader reads files — the index, the envelope
//! and the workspace's git metadata — and never runs an indexer, a compiler
//! or a version-control command.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::infrastructure::scip::{
    DEFAULT_MAX_INDEX_BYTES, ScipIndex, ScipIndexBinding, ScipIndexError,
};

/// The path of the index payload, relative to the workspace root.
pub const SCIP_INDEX_RELATIVE_PATH: &str = ".cognicode/scip/index.scip";

/// The path of the identity envelope, relative to the workspace root.
pub const SCIP_BINDING_RELATIVE_PATH: &str = ".cognicode/scip/index.scip.json";

/// A SCIP index that is bound to the workspace it was loaded from.
#[derive(Debug)]
pub struct WorkspaceScipIndex {
    /// The bound index.
    pub index: ScipIndex,
    /// The envelope the index was bound with.
    pub binding: ScipIndexBinding,
}

/// Why a declared SCIP index cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum ScipLoadError {
    #[error("the SCIP index could not be read: {0}")]
    Index(String),
    #[error("the SCIP identity envelope could not be read: {0}")]
    Binding(String),
    #[error("the SCIP index is not bound to this workspace: {0}")]
    NotBound(String),
}

impl From<ScipIndexError> for ScipLoadError {
    fn from(error: ScipIndexError) -> Self {
        ScipLoadError::NotBound(error.to_string())
    }
}

/// Loads the SCIP index the workspace declares, when it declares one.
///
/// `Ok(None)` means the workspace declares no index, which is the ordinary
/// case. An error means the workspace declares one that cannot be used, and
/// the caller decides what to do about it: the pipeline without the S3 tier
/// still answers, so the safe answer is to report the error and continue.
pub fn load_workspace_scip_index(
    workspace_root: &Path,
) -> Result<Option<WorkspaceScipIndex>, ScipLoadError> {
    let binding_path = workspace_root.join(SCIP_BINDING_RELATIVE_PATH);
    if !binding_path.exists() {
        return Ok(None);
    }
    let binding_bytes =
        std::fs::read(&binding_path).map_err(|error| ScipLoadError::Binding(error.to_string()))?;
    let declared: ScipIndexBinding = serde_json::from_slice(&binding_bytes)
        .map_err(|error| ScipLoadError::Binding(error.to_string()))?;

    let index_path = workspace_root.join(SCIP_INDEX_RELATIVE_PATH);
    let payload =
        std::fs::read(&index_path).map_err(|error| ScipLoadError::Index(error.to_string()))?;

    let revision = git_head_revision(workspace_root).ok_or_else(|| {
        ScipLoadError::NotBound(
            "the current revision of the workspace could not be established".to_string(),
        )
    })?;

    let expected = ScipIndexBinding {
        repository_id: declared.repository_id.clone(),
        revision,
        build_configuration: declared.build_configuration.clone(),
        indexer: declared.indexer.clone(),
        indexer_version: declared.indexer_version.clone(),
        project_root: format!("file://{}", workspace_root.to_string_lossy()),
        scope: declared.scope.clone(),
        payload_sha256: hex::encode(Sha256::digest(&payload)),
    };

    let index = ScipIndex::parse_bound(&payload, DEFAULT_MAX_INDEX_BYTES, &declared, &expected)?;

    Ok(Some(WorkspaceScipIndex {
        index,
        binding: declared,
    }))
}

/// The commit the workspace is at, read from its git metadata.
///
/// The revision is read from the files git keeps, not from a git command:
/// the loader must not run anything. A workspace that is not a git
/// repository, or whose metadata cannot be read, has no revision to state.
pub fn git_head_revision(workspace_root: &Path) -> Option<String> {
    let git_dir = resolve_git_dir(workspace_root)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref:") {
        resolve_reference(&git_dir, reference.trim())
    } else if is_commit_id(head) {
        Some(head.to_string())
    } else {
        None
    }
}

/// The directory holding the git metadata of the workspace.
///
/// A worktree or a submodule keeps a `.git` file that names the directory
/// where the metadata really lives.
fn resolve_git_dir(workspace_root: &Path) -> Option<PathBuf> {
    let candidate = workspace_root.join(".git");
    if candidate.is_dir() {
        return Some(candidate);
    }
    let pointer = std::fs::read_to_string(&candidate).ok()?;
    let path = pointer.trim().strip_prefix("gitdir:")?.trim();
    let path = Path::new(path);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    })
}

/// The commit a reference points at, loose or packed.
fn resolve_reference(git_dir: &Path, reference: &str) -> Option<String> {
    if let Ok(loose) = std::fs::read_to_string(git_dir.join(reference)) {
        let loose = loose.trim();
        if is_commit_id(loose) {
            return Some(loose.to_string());
        }
    }
    let packed = std::fs::read_to_string(git_dir.join("packed-refs")).ok()?;
    for line in packed.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.starts_with('^') {
            continue;
        }
        let Some((commit, name)) = line.split_once(' ') else {
            continue;
        };
        if name.trim() == reference && is_commit_id(commit) {
            return Some(commit.trim().to_string());
        }
    }
    None
}

/// True for a full commit id.
fn is_commit_id(candidate: &str) -> bool {
    candidate.len() == 40 && candidate.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn write_bytes(path: &Path, contents: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// A workspace with a git metadata directory and a loose reference.
    fn workspace_with_head() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        write(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
        write(
            &tmp.path().join(".git/refs/heads/main"),
            "0123456789abcdef0123456789abcdef01234567\n",
        );
        tmp
    }

    #[test]
    fn reads_the_commit_a_loose_reference_points_at() {
        let tmp = workspace_with_head();
        assert_eq!(
            git_head_revision(tmp.path()),
            Some("0123456789abcdef0123456789abcdef01234567".to_string())
        );
    }

    #[test]
    fn reads_the_commit_a_packed_reference_points_at() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(&tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n");
        write(
            &tmp.path().join(".git/packed-refs"),
            "# pack-refs with: peeled fully-peeled sorted\n\
             fedcba9876543210fedcba9876543210fedcba98 refs/heads/main\n",
        );
        assert_eq!(
            git_head_revision(tmp.path()),
            Some("fedcba9876543210fedcba9876543210fedcba98".to_string())
        );
    }

    #[test]
    fn reads_a_detached_head() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(
            &tmp.path().join(".git/HEAD"),
            "0123456789abcdef0123456789abcdef01234567\n",
        );
        assert_eq!(
            git_head_revision(tmp.path()),
            Some("0123456789abcdef0123456789abcdef01234567".to_string())
        );
    }

    #[test]
    fn follows_a_git_file_that_names_another_directory() {
        let tmp = tempfile::TempDir::new().unwrap();
        let real_git = tmp.path().join("real-git");
        write(&real_git.join("HEAD"), "ref: refs/heads/main\n");
        write(
            &real_git.join("refs/heads/main"),
            "0123456789abcdef0123456789abcdef01234567\n",
        );
        let worktree = tmp.path().join("worktree");
        std::fs::create_dir_all(&worktree).unwrap();
        write(&worktree.join(".git"), "gitdir: ../real-git\n");
        assert_eq!(
            git_head_revision(&worktree),
            Some("0123456789abcdef0123456789abcdef01234567".to_string())
        );
    }

    #[test]
    fn a_workspace_without_git_has_no_revision() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(git_head_revision(tmp.path()), None);
    }

    #[test]
    fn a_workspace_that_declares_no_index_loads_none() {
        let tmp = workspace_with_head();
        assert!(load_workspace_scip_index(tmp.path()).unwrap().is_none());
    }

    #[test]
    fn an_index_declared_for_another_revision_is_refused() {
        let tmp = workspace_with_head();
        let payload = b"not an index";
        let binding = ScipIndexBinding {
            repository_id: "fixture-repo".to_string(),
            revision: "fedcba9876543210fedcba9876543210fedcba98".to_string(),
            build_configuration: "cargo-default".to_string(),
            indexer: "rust-analyzer".to_string(),
            indexer_version: "1.96.0".to_string(),
            project_root: format!("file://{}", tmp.path().to_string_lossy()),
            scope: vec!["src/lib.rs".to_string()],
            payload_sha256: hex::encode(Sha256::digest(payload)),
        };
        write_bytes(
            &tmp.path().join(SCIP_BINDING_RELATIVE_PATH),
            &serde_json::to_vec_pretty(&binding).unwrap(),
        );
        write_bytes(&tmp.path().join(SCIP_INDEX_RELATIVE_PATH), payload);

        let error = load_workspace_scip_index(tmp.path()).unwrap_err();
        assert!(
            matches!(error, ScipLoadError::NotBound(_)),
            "a stale index is refused, got {error}"
        );
    }
}
