//! File tracking (BLAKE3 hashes, change detection)

use rusqlite::{Connection, params};
use crate::types::FileState;

pub struct FileStore {
    db: Connection,
}

impl FileStore {
    pub fn new(db: Connection) -> Self {
        Self { db }
    }

    pub fn hash_file(path: &std::path::Path) -> Option<String> {
        let content = std::fs::read(path).ok()?;
        let hash = blake3::hash(&content);
        Some(hash.to_hex().to_string())
    }

    pub fn is_changed(&self, path: &str) -> bool {
        let stored_hash: Option<String> = self.db.query_row(
            "SELECT hash FROM file_states WHERE path = ?1", params![path], |row| row.get(0)
        ).ok();
        match stored_hash {
            Some(h) => Self::hash_file(std::path::Path::new(path)).map(|current| current != h).unwrap_or(true),
            None => true, // New file
        }
    }

    pub fn update(&self, path: &str, issues_count: usize) {
        if let Some(hash) = Self::hash_file(std::path::Path::new(path)) {
            self.db.execute(
                "INSERT OR REPLACE INTO file_states (path, hash, issues_count, last_analyzed) VALUES (?1, ?2, ?3, ?4)",
                params![path, hash, issues_count as i64, chrono::Utc::now().to_rfc3339()],
            ).ok();
        }
    }

    pub fn get_state(&self, path: &str) -> Option<FileState> {
        self.db.query_row(
            "SELECT hash, issues_count, last_analyzed FROM file_states WHERE path = ?1",
            params![path], |row| Ok(FileState {
                hash: row.get(0)?, issues_count: row.get::<_, i64>(1)? as usize, last_analyzed: row.get(2)?
            })
        ).ok()
    }
}