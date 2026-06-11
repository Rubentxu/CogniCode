//! Analysis state — formerly a thin wrapper over `cognicode-db`'s
//! SQLite-backed `QualityStore` + `FileStore`.
//!
//! ## Status
//!
//! The SQLite persistence layer was removed in the Graph Intelligence
//! v2 cleanup. The `AnalysisState` API surface is kept as a no-op
//! stub so the `QualityAnalysisHandler` dispatch path keeps linking
//! and existing tool wiring does not break. The data types
//! (`BaselineDiff`, `FileState`, `QualityBaseline`, `QualitySnapshot`)
//! are re-defined locally — they were originally sourced from
//! `cognicode_db::types`.
//!
//! The handler therefore loses:
//! - Cross-session incremental change detection (always treats every
//!   file as "changed" when `changed_only` is requested, which makes
//!   the flag effectively a no-op).
//! - Persistent baseline / snapshot history (always reports
//!   `baseline_diff = None` and `latest_run_id = 0`).
//!
//! When a PostgreSQL-backed `AnalysisState` lands, the stub will be
//! replaced and incremental analysis will return.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Snapshot of a stored quality baseline (was persisted in SQLite).
#[derive(Debug, Clone)]
pub struct QualityBaseline {
    pub timestamp: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
    pub blockers: usize,
    pub criticals: usize,
}

/// Snapshot of a single quality run (was persisted in SQLite).
#[derive(Debug, Clone)]
pub struct QualitySnapshot {
    pub timestamp: String,
    pub total_issues: usize,
    pub debt_minutes: u64,
    pub rating: String,
    pub files_changed: usize,
    pub new_issues: usize,
    pub fixed_issues: usize,
}

/// Diff between a current run and the stored baseline.
#[derive(Debug, Clone)]
pub struct BaselineDiff {
    pub baseline_timestamp: String,
    pub issues_delta: i64,
    pub debt_delta: i64,
    pub rating_before: String,
    pub rating_after: String,
    pub blockers_before: usize,
    pub blockers_after: usize,
}

/// Per-file state (was persisted in SQLite).
#[derive(Debug, Clone)]
pub struct FileState {
    pub hash: String,
    pub issues_count: usize,
    pub last_analyzed: String,
}

/// No-op analysis state.
///
/// The original implementation wrapped `cognicode_db::QualityStore`
/// and `cognicode_db::FileStore` for cross-session persistence. With
/// SQLite gone, every method is either a no-op (write side) or
/// returns the empty default (read side). Callers that depended on
/// incremental change detection will see "every file is new" until a
/// PostgreSQL-backed reimplementation lands.
pub struct AnalysisState {
    project_root: PathBuf,
}

impl AnalysisState {
    /// Load the analysis state. After the SQLite removal this is a
    /// no-op constructor — no on-disk state is consulted.
    pub fn load(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
        }
    }

    /// Set the baseline. No-op.
    pub fn set_baseline(
        &self,
        _total_issues: usize,
        _debt: u64,
        _rating: &str,
        _blockers: usize,
        _criticals: usize,
    ) {
    }

    /// Get the stored baseline. Always `None` after the SQLite removal.
    pub fn get_baseline(&self) -> Option<QualityBaseline> {
        None
    }

    /// Record a snapshot. No-op.
    #[allow(clippy::too_many_arguments)]
    pub fn add_snapshot(
        &self,
        _total_issues: usize,
        _debt: u64,
        _rating: &str,
        _files_changed: usize,
        _new: usize,
        _fixed: usize,
    ) {
    }

    /// Diff the supplied metrics against the baseline. Always `None`
    /// because no baseline is ever stored.
    pub fn diff_vs_baseline(
        &self,
        _total_issues: usize,
        _debt: u64,
        _rating: &str,
        _blockers: usize,
    ) -> Option<BaselineDiff> {
        None
    }

    /// Return changed files. Without persistent state, the in-memory
    /// view is always empty, so every input file is reported as
    /// changed. This matches the post-cleanup semantics: callers
    /// that pass `changed_only = true` will simply analyse every
    /// file.
    pub fn find_changed_files(&self, all_files: &[PathBuf]) -> Vec<PathBuf> {
        all_files.to_vec()
    }

    /// Update the per-file issue count. No-op.
    pub fn update_file_state(&self, _path: &Path, _issues_count: usize) {}

    /// BLAKE3 hash of a file. Pure function (no persistence) — kept
    /// for callers that need a stable identity hash for a path.
    pub fn hash_file(path: &Path) -> Option<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        path.hash(&mut hasher);
        Some(format!("{:016x}", hasher.finish()))
    }

    /// Stub — the call-graph import extraction used to live in
    /// `cognicode_db::files::FileStore::extract_imports`. The
    /// language-specific regexes have been removed; this now returns
    /// an empty vec. Callers can re-introduce a real implementation
    /// when a non-SQLite store is available.
    pub fn extract_imports(_source: &str, _file_path: &str) -> Vec<String> {
        Vec::new()
    }

    /// Stub for the SQLite import-tracking path. No-op.
    pub fn update_file_imports(&self, _source_file: &str, _imports: &[String]) {}

    /// Find changed files including their dependents.
    ///
    /// Without persistent dependents tracking this is equivalent to
    /// `find_changed_files`.
    pub fn find_changed_with_dependents(&self, all_files: &[PathBuf]) -> Vec<PathBuf> {
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut out = Vec::with_capacity(all_files.len());
        for f in self.find_changed_files(all_files) {
            if seen.insert(f.clone()) {
                out.push(f);
            }
        }
        out
    }

    /// Latest run id. Always `None` post-cleanup.
    pub fn latest_run_id(&self) -> Option<i64> {
        None
    }

    /// Insert issues for a run. No-op.
    pub fn insert_issues(
        &self,
        _run_id: i64,
        _issues: &[cognicode_axiom::rules::types::Issue],
    ) {
    }

    /// Stub — returns an empty list (was backed by SQLite `issues`).
    pub fn get_open_issues(&self) -> Vec<cognicode_axiom::rules::types::Issue> {
        Vec::new()
    }

    /// Stub — returns an empty list (was backed by SQLite `files`).
    pub fn get_new_code_files(&self) -> Vec<String> {
        Vec::new()
    }

    /// Return the persisted run history. Always empty.
    pub fn get_history(&self) -> Vec<QualitySnapshot> {
        Vec::new()
    }

    /// Return the per-file state. Always `None` (no persistence).
    pub fn get_file_state(&self, _path: &str) -> Option<FileState> {
        None
    }
}
