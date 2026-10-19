//! The one law every project walk must obey.
//!
//! **A walk rooted at `R` yields files under `R`, and only files
//! under `R`.** A symlink is a name, not a licence to read through
//! it.
//!
//! This needs saying because the obvious way to write it fails twice,
//! and each failure hides the other:
//!
//! - `WalkDir::follow_links(true)` descends through symlinked
//!   directories. A project containing `linked -> ~/.cargo/registry`
//!   indexes a registry.
//! - `Path::is_file()` is `fs::metadata(path).is_file()`, and
//!   `fs::metadata` *follows* symlinks. So switching to
//!   `follow_links(false)` alone does not close the hole: a symlinked
//!   **file** is still yielded as an entry and still reports
//!   `is_file() == true`.
//!
//! Only the pair closes it. Because every caller needs the pair, and
//! a caller who remembers one half has reintroduced the bug they were
//! fixing, the pair lives here and nowhere else. Use
//! [`contained_walk`] plus [`is_real_file`].

use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// A project walk that does not leave the root it was given.
///
/// Returns the builder rather than an iterator so callers can still
/// compose `filter_entry` before descending.
pub fn contained_walk(root: impl AsRef<Path>) -> WalkDir {
    WalkDir::new(root).follow_links(false)
}

/// Whether `entry` is a real file — as opposed to a symlink to one.
///
/// lstat-based, unlike [`Path::is_file`]. Paired with
/// [`contained_walk`], a symlink fails the walk and fails this check,
/// so neither a symlinked file nor anything behind a symlinked
/// directory reaches the caller.
pub fn is_real_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_symlinked_file_is_not_a_real_file() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside.rs");
        std::fs::write(&outside, "pub fn f() {}\n").unwrap();

        let link = tmp.path().join("link.rs");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let entries: Vec<DirEntry> = contained_walk(tmp.path())
            .into_iter()
            .filter_map(Result::ok)
            .collect();

        let link_entry = entries
            .iter()
            .find(|e| e.path() == link)
            .expect("the symlink is still yielded as an entry — that is why the pair is needed");

        assert!(
            !is_real_file(link_entry),
            "a symlink to a real file must not pass the real-file check"
        );
    }

    #[test]
    fn a_real_file_is_a_real_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("real.rs");
        std::fs::write(&file, "pub fn f() {}\n").unwrap();

        let entries: Vec<DirEntry> = contained_walk(tmp.path())
            .into_iter()
            .filter_map(Result::ok)
            .collect();

        let file_entry = entries.iter().find(|e| e.path() == file).unwrap();
        assert!(is_real_file(file_entry));
    }

    #[test]
    fn contained_walk_does_not_descend_into_a_symlinked_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("hidden.rs"), "pub fn f() {}\n").unwrap();

        let root = tmp.path().join("root");
        std::fs::create_dir_all(&root).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

        let leaked: Vec<String> = contained_walk(&root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(is_real_file)
            .map(|e| e.path().to_string_lossy().to_string())
            .collect();

        assert!(
            leaked.is_empty(),
            "the walk descended through the symlink: {leaked:?}"
        );
    }
}
