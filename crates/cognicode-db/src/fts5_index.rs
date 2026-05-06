//! FTS5 Symbol Index
//!
//! Provides full-text search for code symbols.

use rusqlite::{Connection, params};
use anyhow::Result;

/// FTS5 index for symbol search
pub struct Fts5Index;

/// Search result from FTS5 index
#[derive(Debug, Clone)]
pub struct SymbolSearchResult {
    pub symbol_name: String,
    pub symbol_kind: String,
    pub file_path: String,
    pub docstring: String,
    pub body_tokens: String,
}

impl Fts5Index {
    /// Insert symbol into FTS5 virtual table
    pub fn insert_symbol(
        conn: &Connection,
        name: &str,
        kind: &str,
        file: &str,
        docstring: &str,
        tokens: &str,
    ) -> Result<()> {
        conn.execute(
            "INSERT INTO symbol_index (symbol_name, symbol_kind, file_path, docstring, body_tokens) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, kind, file, docstring, tokens],
        )?;
        Ok(())
    }

    /// Search symbols using FTS5 MATCH
    pub fn search(conn: &Connection, query: &str, limit: usize) -> Result<Vec<SymbolSearchResult>> {
        let mut stmt = conn.prepare(
            "SELECT symbol_name, symbol_kind, file_path, docstring, body_tokens FROM symbol_index WHERE symbol_index MATCH ?1 LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![query, limit as i64], |row| {
            Ok(SymbolSearchResult {
                symbol_name: row.get(0)?,
                symbol_kind: row.get(1)?,
                file_path: row.get(2)?,
                docstring: row.get(3)?,
                body_tokens: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Delete a symbol by name from FTS5 index
    #[allow(dead_code)]
    pub fn delete_symbol(conn: &Connection, name: &str) -> Result<()> {
        conn.execute(
            "DELETE FROM symbol_index WHERE symbol_name = ?1",
            params![name],
        )?;
        Ok(())
    }
}
