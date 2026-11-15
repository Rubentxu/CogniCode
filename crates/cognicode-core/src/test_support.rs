//! Helpers that make a test independent of the wall clock.
//!
//! ## Why
//!
//! A test that changes a file and then waits for the clock to move is
//! waiting for a filesystem tick that may never come: the write and the
//! read can land in the same tick, the tick can be coarse, and the wait
//! is paid in every run of the suite. Thirteen tests did that, for four
//! and a half seconds per run.
//!
//! What those tests actually needed was a modification time that
//! differs from the one the manifest recorded. The modification time is
//! a value, not a moment: it can be set to a known instant, and then the
//! comparison the product makes —recorded time against the time on
//! disk— sees the difference without anybody waiting for it.

use std::path::Path;
use std::time::{Duration, SystemTime};

/// Move a file's modification time one second past the one it has now.
///
/// The staleness checks compare the time a manifest recorded with the
/// time on disk, and a second of difference is a difference they can
/// see. The new time is derived from the file's own, so it moves with
/// the clock instead of jumping to a fixed instant: a test that runs
/// tomorrow sees the same thing as a test that runs today.
///
/// The file must exist and be writable by its owner: the helper opens
/// it to set the time.
pub fn bump_mtime(path: &Path) {
    let current = std::fs::metadata(path)
        .expect("the file the test changed is readable")
        .modified()
        .expect("the file has a modification time");
    set_mtime(path, current + Duration::from_secs(1));
}

/// Set a file's modification time to a known instant.
///
/// The same thing the rest of the tree does with the standard library:
/// open the file to write and hand the time over.
pub fn set_mtime(path: &Path, time: SystemTime) {
    std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("the file can be opened to set its time")
        .set_modified(time)
        .expect("the modification time is settable");
}
