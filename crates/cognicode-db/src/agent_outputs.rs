//! Agent Outputs persistence layer
//!
//! Provides SQLite persistence for agent tool outputs.
//! Used by the dashboard to display agent activity and results.

use rusqlite::{Connection, params};
use anyhow::Result;
use chrono::Utc;

/// Store for agent tool outputs
pub struct AgentOutputsStore;

impl AgentOutputsStore {
    /// Insert a new agent output record.
    pub fn insert(
        conn: &Connection,
        tool_name: &str,
        session_id: Option<&str>,
        output_json: &str,
        summary_text: Option<&str>,
        expires_at: Option<&str>,
    ) -> Result<i64> {
        let created_at = Utc::now().to_rfc3339();
        
        conn.execute(
            "INSERT INTO agent_outputs (tool_name, session_id, output_json, summary_text, created_at, expires_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                tool_name,
                session_id,
                output_json,
                summary_text,
                created_at,
                expires_at,
            ],
        )?;
        
        Ok(conn.last_insert_rowid())
    }
    
    /// Get the latest output for a given tool name.
    #[allow(dead_code)]
    pub fn get_latest(conn: &Connection, tool_name: &str) -> Result<Option<AgentOutput>> {
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, session_id, output_json, summary_text, created_at, expires_at FROM agent_outputs WHERE tool_name = ?1 ORDER BY created_at DESC LIMIT 1"
        )?;
        
        let result = stmt.query_row(params![tool_name], |row| {
            Ok(AgentOutput {
                id: row.get(0)?,
                tool_name: row.get(1)?,
                session_id: row.get(2)?,
                output_json: row.get(3)?,
                summary_text: row.get(4)?,
                created_at: row.get(5)?,
                expires_at: row.get(6)?,
            })
        });
        
        match result {
            Ok(output) => Ok(Some(output)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    
    /// Get all recent outputs, ordered by created_at descending.
    #[allow(dead_code)]
    pub fn get_all_recent(conn: &Connection, limit: usize) -> Result<Vec<AgentOutput>> {
        let mut stmt = conn.prepare(
            "SELECT id, tool_name, session_id, output_json, summary_text, created_at, expires_at FROM agent_outputs ORDER BY created_at DESC LIMIT ?1"
        )?;
        
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(AgentOutput {
                id: row.get(0)?,
                tool_name: row.get(1)?,
                session_id: row.get(2)?,
                output_json: row.get(3)?,
                summary_text: row.get(4)?,
                created_at: row.get(5)?,
                expires_at: row.get(6)?,
            })
        })?;
        
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    
    /// Delete all expired output records.
    /// Returns the number of deleted rows.
    #[allow(dead_code)]
    pub fn delete_expired(conn: &Connection) -> Result<usize> {
        let deleted = conn.execute(
            "DELETE FROM agent_outputs WHERE expires_at IS NOT NULL AND expires_at < ?1",
            params![Utc::now().to_rfc3339()],
        )?;
        
        Ok(deleted)
    }
}

/// An agent output record
#[derive(Debug, Clone)]
pub struct AgentOutput {
    pub id: i64,
    pub tool_name: String,
    pub session_id: Option<String>,
    pub output_json: String,
    pub summary_text: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn create_test_db() -> (TempDir, Connection) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let conn = Connection::open(&db_path).unwrap();
        
        // Initialize schema
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS agent_outputs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tool_name TEXT NOT NULL,
                session_id TEXT,
                output_json TEXT NOT NULL,
                summary_text TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT
            );
            CREATE INDEX idx_agent_outputs_tool ON agent_outputs(tool_name);
            CREATE INDEX idx_agent_outputs_created ON agent_outputs(created_at);"
        ).unwrap();
        
        (temp_dir, conn)
    }
    
    #[test]
    fn test_insert_and_get_latest() {
        let (_dir, conn) = create_test_db();
        
        let id = AgentOutputsStore::insert(
            &conn,
            "test_tool",
            Some("session_123"),
            r#"{"result": "ok"}"#,
            Some("Test output"),
            None,
        ).unwrap();
        
        assert!(id > 0);
        
        let latest = AgentOutputsStore::get_latest(&conn, "test_tool").unwrap();
        assert!(latest.is_some());
        
        let output = latest.unwrap();
        assert_eq!(output.tool_name, "test_tool");
        assert_eq!(output.session_id, Some("session_123".to_string()));
        assert_eq!(output.output_json, r#"{"result": "ok"}"#);
    }
    
    #[test]
    fn test_get_all_recent() {
        let (_dir, conn) = create_test_db();
        
        AgentOutputsStore::insert(&conn, "tool_a", None, "{}", None, None).unwrap();
        AgentOutputsStore::insert(&conn, "tool_b", None, "{}", None, None).unwrap();
        AgentOutputsStore::insert(&conn, "tool_a", None, "{}", None, None).unwrap();
        
        let recent = AgentOutputsStore::get_all_recent(&conn, 10).unwrap();
        assert_eq!(recent.len(), 3);
        
        // Most recent first
        assert_eq!(recent[0].tool_name, "tool_a");
        assert_eq!(recent[1].tool_name, "tool_b");
        assert_eq!(recent[2].tool_name, "tool_a");
    }
    
    #[test]
    fn test_delete_expired() {
        let (_dir, conn) = create_test_db();
        
        // Insert with past expiry
        AgentOutputsStore::insert(
            &conn,
            "expired_tool",
            None,
            "{}",
            None,
            Some("2020-01-01T00:00:00Z"),
        ).unwrap();
        
        // Insert with future expiry
        AgentOutputsStore::insert(
            &conn,
            "valid_tool",
            None,
            "{}",
            None,
            Some("2099-01-01T00:00:00Z"),
        ).unwrap();
        
        let deleted = AgentOutputsStore::delete_expired(&conn).unwrap();
        assert_eq!(deleted, 1);
        
        let remaining = AgentOutputsStore::get_all_recent(&conn, 10).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].tool_name, "valid_tool");
    }
}
