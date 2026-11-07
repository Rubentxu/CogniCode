//! Where the product keeps what it derives from a repository.
//!
//! ## The hole this fills
//!
//! The durable snapshot of the graph is the **product's own state**: it is
//! derived from the tree, it can be thrown away without losing anything,
//! and it says nothing about the tree. It used to live inside the tree —
//! `<repo>/.cognicode/graph.cache` — so analysing a repository wrote into
//! it. Analysing a project is a read, and a read that leaves a file
//! behind is not a read.
//!
//! ## Where it lives
//!
//! The user's cache, keyed by the workspace: the snapshot **is** the file
//! `<cache root>/cognicode/<key>`, where `<key>` says which repository the
//! entry belongs to. Two repositories never share an entry, and the same
//! repository always finds its own.
//!
//! The name of the file is a convenience, not the truth: the key
//! **inside** the snapshot still says which repository it describes, so a
//! collision would be caught and the snapshot rebuilt instead of served.

use std::path::{Path, PathBuf};

/// The root of the user's cache, where the product keeps what it derives.
///
/// `XDG_CACHE_HOME` when it is set and not empty, and `~/.cache` otherwise:
/// the convention of the platform, so the product's cache lives where the
/// user's other caches live, and a tool that cleans the cache cleans ours
/// too.
///
/// `None` when the environment says nothing about where a cache is. There
/// is no honest place to write in that case, and the product then keeps no
/// state at all instead of guessing a place — a guessed place is a place
/// nobody cleans and nobody looks at.
pub fn default_cache_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME").filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(|home| PathBuf::from(home).join(".cache"))
}

/// The directory that holds everything the product derives.
pub fn cache_dir(cache_root: &Path) -> PathBuf {
    cache_root.join("cognicode")
}

/// The path of the graph snapshot of `workspace`.
///
/// One file per repository, named by the key of the repository: the
/// snapshot of a workspace is the file `<cache root>/cognicode/<key>`.
pub fn snapshot_path(cache_root: &Path, workspace: &Path) -> PathBuf {
    cache_dir(cache_root).join(workspace_key(workspace))
}

/// The socket the daemon of `workspace` listens on.
///
/// One socket per repository, inside the cache: the daemon serves one
/// workspace, and the socket is the product's own state, so it lives
/// where the rest of that state lives — never inside the analysed tree,
/// for the same reason the snapshot does not go there.
pub fn daemon_socket_path(cache_root: &Path, workspace: &Path) -> PathBuf {
    cache_dir(cache_root)
        .join("daemon")
        .join(format!("{}.sock", workspace_key(workspace)))
}

/// The name of the workspace's entry in the cache.
///
/// The hash of the canonical path, so the entry is the same whatever route
/// reaches it, and two workspaces never share one. The hash is of the
/// *canonical* path because the same repository reached by two different
/// routes is one repository, and its snapshot is one snapshot.
///
/// It is public because the cache has more than one kind of entry —the
/// snapshot of the graph and the memo of the recipes— and both have to name
/// the workspace the same way. One owner of the name, two users of it.
pub fn workspace_key(workspace: &Path) -> String {
    use sha2::{Digest, Sha256};

    let canonical = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    let digest = Sha256::digest(canonical.to_string_lossy().as_bytes());
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    // Half of the digest: 128 bits is past any collision a user can
    // reach, and a shorter name is a name somebody can read out loud.
    hex[..32].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace that exists, so its canonical path is the path itself.
    fn workspace(root: &Path, name: &str) -> PathBuf {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).expect("the workspace is created");
        dir
    }

    #[test]
    fn two_workspaces_get_two_entries() {
        let root = tempfile::tempdir().expect("a temporary root");
        let cache = root.path().join("cache");
        let one = workspace(root.path(), "one");
        let two = workspace(root.path(), "two");

        assert_ne!(
            snapshot_path(&cache, &one),
            snapshot_path(&cache, &two),
            "two repositories never share an entry in the cache"
        );
    }

    #[test]
    fn the_same_workspace_always_finds_its_own_entry() {
        let root = tempfile::tempdir().expect("a temporary root");
        let cache = root.path().join("cache");
        let one = workspace(root.path(), "one");

        assert_eq!(
            snapshot_path(&cache, &one),
            snapshot_path(&cache, &one),
            "the same repository always finds its own snapshot"
        );
    }

    #[test]
    fn the_snapshot_is_the_file_the_key_names_in_the_cache() {
        let root = tempfile::tempdir().expect("a temporary root");
        let cache = root.path().join("cache");
        let one = workspace(root.path(), "one");

        let path = snapshot_path(&cache, &one);
        assert_eq!(
            path.parent(),
            Some(cache_dir(&cache).as_path()),
            "the snapshot of a repository is the file its key names in the \
             cache: {path:?}"
        );
        assert_eq!(
            path.file_name().and_then(|name| name.to_str()),
            Some(workspace_key(&one).as_str()),
            "the name of the file is the key of the repository: {path:?}"
        );
    }

    #[test]
    fn the_entry_lives_under_the_cache_root_and_not_under_the_workspace() {
        let root = tempfile::tempdir().expect("a temporary root");
        let cache = root.path().join("cache");
        let one = workspace(root.path(), "one");

        let path = snapshot_path(&cache, &one);
        assert!(
            path.starts_with(&cache),
            "the snapshot lives in the cache the caller named: {path:?}"
        );
        assert!(
            !path.starts_with(&one),
            "the snapshot does not live in the analysed tree: {path:?}"
        );
    }

    #[test]
    fn the_daemon_socket_is_one_per_workspace_and_lives_in_the_cache() {
        let root = tempfile::tempdir().expect("a temporary root");
        let cache = root.path().join("cache");
        let one = workspace(root.path(), "one");
        let two = workspace(root.path(), "two");

        let first = daemon_socket_path(&cache, &one);
        let second = daemon_socket_path(&cache, &two);
        assert_ne!(
            first, second,
            "two repositories never share a daemon: a daemon serves one tree"
        );
        assert_eq!(
            first,
            daemon_socket_path(&cache, &one),
            "the same repository always finds its own daemon"
        );
        assert!(
            first.starts_with(&cache),
            "the socket lives in the cache the caller named: {first:?}"
        );
        assert!(
            !first.starts_with(&one),
            "the socket does not live in the analysed tree: {first:?}"
        );
    }
}
