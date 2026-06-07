//! Domain port for reading source code from disk.
//!
//! The trait is intentionally narrow: view builders need either a file's full
//! contents or a 1-based line slice. Anything richer (range queries, mtimes,
//! incremental updates) is out of scope for Phase 1A.

use crate::error::ExplorerResult;

/// Read-only port for source files.
pub trait SourceReader: Send + Sync {
    /// Return the file's entire contents as a `String`.
    fn read_source(&self, file: &str) -> ExplorerResult<String>;

    /// Return the inclusive 1-based line range `[start, end]` numbered.
    ///
    /// `start` and `end` are 1-based and inclusive. Out-of-range requests
    /// are clamped to the available lines (no padding lines are synthesised
    /// — what you ask for is what you get, or fewer lines if the file is
    /// shorter).
    fn read_lines(&self, file: &str, start: u32, end: u32) -> ExplorerResult<Vec<(u32, String)>>;
}
