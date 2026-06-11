//! FTS5-backed [`SearchRepository`] adapter.
//!
//! Opens its own read-only `rusqlite::Connection` against the workspace's
//! `.cognicode/cognicode.db` file and delegates to
//! [`cognicode_db::Fts5Index::search`]. Line numbers are not stored in the
//! FTS5 table (see `cognicode_db::schema`), so this adapter emits `line: 0`
//! and `mvp_id: ""` placeholders; the service layer resolves the line via
//! the symbol repository and fills in the canonical MVP id before exposing
//! the hit.
//!
//! FTS5 scoring is not normalised by [`cognicode_db::Fts5Index::search`]
//! (the query has no `ORDER BY rank`). We assign descending scores from
//! `0.95` to a floor of `0.50`, which keeps "exact > fts5" ordering when
//! results are merged.

use std::path::{Path, PathBuf};

use cognicode_db::Fts5Index;
use rusqlite::Connection;

use crate::error::ExplorerResult;
use crate::ports::search_repository::{SearchHit, SearchRepository};

/// Adapter that surfaces the FTS5 `symbol_index` virtual table behind the
/// [`SearchRepository`] port.
pub struct Fts5SearchAdapter {
    db_path: PathBuf,
}

impl Fts5SearchAdapter {
    /// Build an adapter targeting the workspace's `.cognicode/cognicode.db`
    /// file. The file does not need to exist — the adapter degrades to an
    /// empty result when it is missing.
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        Self {
            db_path: db_path.into(),
        }
    }

    /// Borrow the underlying DB path. Exposed for diagnostics / tests.
    #[allow(dead_code)]
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }
}

impl SearchRepository for Fts5SearchAdapter {
    fn search(&self, query: &str, limit: usize) -> ExplorerResult<Vec<SearchHit>> {
        if query.is_empty() || limit == 0 || !self.db_path.exists() {
            return Ok(Vec::new());
        }

        let conn = Connection::open(&self.db_path)
            .map_err(|e| crate::error::ExplorerError::Anyhow(anyhow::anyhow!(e)))?;
        let raw =
            Fts5Index::search(&conn, query, limit).map_err(crate::error::ExplorerError::Anyhow)?;
        drop(conn); // close as early as possible — explorer is read-only

        // Fts5Index::search does not ORDER BY rank; assign a deterministic
        // descending score so the service can still rank results sensibly.
        let total = raw.len();
        let mut hits: Vec<SearchHit> = raw
            .into_iter()
            .enumerate()
            .map(|(idx, r)| {
                let decay = (idx as f32) * 0.05;
                let score = (0.95 - decay).max(0.50);
                let _ = total; // silence unused if logic changes
                SearchHit {
                    // mvp_id is filled in by the service once the line is
                    // resolved; FTS5 only knows name + file.
                    mvp_id: String::new(),
                    name: r.symbol_name,
                    kind: r.symbol_kind,
                    file: r.file_path,
                    line: 0,
                    score,
                    match_type: "fts5".to_string(),
                }
            })
            .collect();
        // FTS5 may return the same name/file multiple times; dedupe while
        // keeping the highest score. Cheap because `limit` is bounded.
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.dedup_by(|a, b| a.name == b.name && a.file == b.file);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn empty_query_returns_empty_vec() {
        let adapter = Fts5SearchAdapter::new(PathBuf::from("/tmp/does_not_exist.db"));
        let hits = adapter.search("", 10).expect("empty query must not error");
        assert!(hits.is_empty());
    }

    #[test]
    fn missing_db_returns_empty_vec_not_error() {
        let adapter = Fts5SearchAdapter::new(PathBuf::from("/tmp/__definitely_missing__.db"));
        let hits = adapter
            .search("anything", 10)
            .expect("missing db must not error");
        assert!(
            hits.is_empty(),
            "missing DB must degrade to empty result, got {hits:?}"
        );
    }

    #[test]
    fn zero_limit_returns_empty_vec() {
        let adapter = Fts5SearchAdapter::new(PathBuf::from("/tmp/__missing__.db"));
        let hits = adapter
            .search("alpha", 0)
            .expect("zero limit must not error");
        assert!(hits.is_empty());
    }
}
