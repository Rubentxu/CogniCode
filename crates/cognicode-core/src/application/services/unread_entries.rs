//! What a walk over the tree could not read, as the holes a result publishes.
//!
//! Three things leave a hole in a content search or a listing: the walk fails
//! on an entry, the entry cannot be opened or measured, or the entry is a
//! symlink and the operation declines it. Each is a fact the operation keeps
//! instead of a bare `continue`; the interface turns the facts into the
//! coverage every other answer publishes.

use crate::application::dto::{UnreadEntry, UnreadKind};
use std::path::Path;

/// The walk failed on an entry: a directory it could not list.
///
/// The path is the one the walker names, wherever it sits in the error.
pub(super) fn walk_failure(error: &ignore::Error) -> UnreadEntry {
    fn path_of(error: &ignore::Error) -> Option<&Path> {
        match error {
            ignore::Error::WithPath { path, .. } => Some(path.as_path()),
            ignore::Error::Loop { child, .. } => Some(child.as_path()),
            ignore::Error::WithDepth { err, .. } | ignore::Error::WithLineNumber { err, .. } => {
                path_of(err)
            }
            _ => None,
        }
    }
    UnreadEntry {
        path: path_of(error)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "<walk>".to_string()),
        kind: UnreadKind::Walk,
        detail: error.to_string(),
    }
}

/// The walk listed an entry and the operation could not open or measure it.
pub(super) fn read_failure(path: &Path, error: &std::io::Error) -> UnreadEntry {
    UnreadEntry {
        path: path.to_string_lossy().into_owned(),
        kind: UnreadKind::Read,
        detail: error.to_string(),
    }
}

/// The hole a symlink leaves, or `None` when the entry is not one.
///
/// **A symlink is a name, not a licence to read through it.** `Path::is_file`
/// and `fs::metadata` follow links, so a walk that only checks them reads and
/// measures whatever the link points at: a file outside the workspace, or the
/// same file twice. The walker already tells what the entry itself is, without
/// following it, so the question is asked of the entry and every symlink is
/// declined, whatever its target. It is the rule the project inventory
/// applies to the same entries (`ignore_is_real_file`, `SkipReason::Symlink`),
/// and `read_coverage` publishes it with the same words.
///
/// The target goes in `detail`; a link that cannot be read still is declined,
/// it just cannot say where it pointed.
pub(super) fn declined_symlink(entry: &ignore::DirEntry) -> Option<UnreadEntry> {
    if !entry.path_is_symlink() {
        return None;
    }
    Some(UnreadEntry {
        path: entry.path().to_string_lossy().into_owned(),
        kind: UnreadKind::Symlink,
        detail: std::fs::read_link(entry.path())
            .map(|target| target.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "<unreadable>".to_string()),
    })
}
