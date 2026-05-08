//! Agent Tasks persistence layer
//!
//! Provides SQLite persistence for agent task queue.
//! Used by the dashboard for task scheduling and status tracking.

use rusqlite::{Connection, params};
use anyhow::Result;
use chrono::Utc;

/// Store for agent tasks
pub struct AgentTasksStore;

impl AgentTasksStore {
    /// Create a new task and return its ID.
    #[allow(dead_code)]
    pub fn create_task(
        conn: &Connection,
        task_type: &str,
        priority: i32,
        payload_json: &str,
        created_by: Option<&str>,
    ) -> Result<i64> {
        let created_at = Utc::now().to_rfc3339();
        let created_by = created_by.unwrap_or("dashboard");
        
        conn.execute(
            "INSERT INTO agent_tasks (task_type, priority, payload_json, status, created_by, created_at) VALUES (?1, ?2, ?3, 'pending', ?4, ?5)",
            params![
                task_type,
                priority,
                payload_json,
                created_by,
                created_at,
            ],
        )?;
        
        Ok(conn.last_insert_rowid())
    }
    
    /// Poll for pending tasks, atomically claiming them.
    /// Returns up to `limit` tasks with status changed to 'in_progress'.
    #[allow(dead_code)]
    pub fn poll_pending(conn: &Connection, limit: usize) -> Result<Vec<AgentTask>> {
        let now = Utc::now().to_rfc3339();
        
        // Get the IDs of tasks to claim (ordered by priority desc, created_at asc)
        let mut select_stmt = conn.prepare(
            "SELECT id FROM agent_tasks WHERE status = 'pending' ORDER BY priority DESC, created_at ASC LIMIT ?1"
        )?;
        
        let ids: Vec<i64> = select_stmt
            .query_map(params![limit as i64], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        
        if ids.is_empty() {
            return Ok(vec![]);
        }
        
        // Build dynamic UPDATE with IN clause
        let in_clause: String = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let update_sql = format!(
            "UPDATE agent_tasks SET status = 'in_progress', assigned_at = ?1 WHERE id IN ({})",
            in_clause
        );
        
        // Build params: now first, then all ids
        let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        for id in &ids {
            all_params.push(Box::new(*id));
        }
        
        let params_refs: Vec<&dyn rusqlite::ToSql> = all_params.iter().map(|b| b.as_ref()).collect();
        conn.execute(&update_sql, params_refs.as_slice())?;
        
        // Fetch the claimed tasks - preserve order using CASE in ORDER BY
        let case_order: String = ids.iter()
            .enumerate()
            .map(|(i, id)| format!("WHEN {} THEN {}", id, i))
            .collect::<Vec<_>>()
            .join(" ");
        let fetch_sql = format!(
            "SELECT id, task_type, priority, payload_json, status, created_by, created_at, assigned_at, completed_at, result_json, error_message FROM agent_tasks WHERE id IN ({}) ORDER BY CASE id {} END",
            in_clause, case_order
        );
        
        let mut fetch_params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
        for id in &ids {
            fetch_params.push(Box::new(*id));
        }
        let fetch_params_refs: Vec<&dyn rusqlite::ToSql> = fetch_params.iter().map(|b| b.as_ref()).collect();
        
        let mut fetch_stmt = conn.prepare(&fetch_sql)?;
        
        let rows = fetch_stmt.query_map(fetch_params_refs.as_slice(), |row| {
            Ok(AgentTask {
                id: row.get(0)?,
                task_type: row.get(1)?,
                priority: row.get(2)?,
                payload_json: row.get(3)?,
                status: row.get(4)?,
                created_by: row.get(5)?,
                created_at: row.get(6)?,
                assigned_at: row.get(7)?,
                completed_at: row.get(8)?,
                result_json: row.get(9)?,
                error_message: row.get(10)?,
            })
        })?;
        
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
    
    /// Mark a task as completed with result or error.
    #[allow(dead_code)]
    pub fn complete_task(
        conn: &Connection,
        id: i64,
        status: &str,
        result_json: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let completed_at = Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agent_tasks SET status = ?1, completed_at = ?2, result_json = ?3, error_message = ?4 WHERE id = ?5",
            params![
                status,
                completed_at,
                result_json,
                error_message,
                id,
            ],
        )?;
        
        Ok(())
    }
    
    /// Mark stale tasks (assigned but not completed within `hours`) as failed.
    /// Returns the number of tasks marked as stale.
    #[allow(dead_code)]
    pub fn mark_stale_timeout(conn: &Connection, hours: i32) -> Result<usize> {
        let cutoff = Utc::now()
            .checked_sub_signed(chrono::Duration::hours(hours as i64))
            .unwrap()
            .to_rfc3339();
        
        let updated = conn.execute(
            "UPDATE agent_tasks SET status = 'failed', error_message = 'Task timed out' WHERE status = 'in_progress' AND assigned_at < ?1",
            params![cutoff],
        )?;
        
        Ok(updated)
    }
    
    /// Get a task by ID.
    #[allow(dead_code)]
    pub fn get_task(conn: &Connection, id: i64) -> Result<Option<AgentTask>> {
        let mut stmt = conn.prepare(
            "SELECT id, task_type, priority, payload_json, status, created_by, created_at, assigned_at, completed_at, result_json, error_message FROM agent_tasks WHERE id = ?1"
        )?;
        
        let result = stmt.query_row(params![id], |row| {
            Ok(AgentTask {
                id: row.get(0)?,
                task_type: row.get(1)?,
                priority: row.get(2)?,
                payload_json: row.get(3)?,
                status: row.get(4)?,
                created_by: row.get(5)?,
                created_at: row.get(6)?,
                assigned_at: row.get(7)?,
                completed_at: row.get(8)?,
                result_json: row.get(9)?,
                error_message: row.get(10)?,
            })
        });
        
        match result {
            Ok(task) => Ok(Some(task)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

/// An agent task record
#[derive(Debug, Clone)]
pub struct AgentTask {
    pub id: i64,
    pub task_type: String,
    pub priority: i32,
    pub payload_json: String,
    pub status: String,
    pub created_by: String,
    pub created_at: String,
    pub assigned_at: Option<String>,
    pub completed_at: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
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
            "CREATE TABLE IF NOT EXISTS agent_tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_type TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 5,
                payload_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_by TEXT DEFAULT 'dashboard',
                created_at TEXT NOT NULL,
                assigned_at TEXT,
                completed_at TEXT,
                result_json TEXT,
                error_message TEXT
            );
            CREATE INDEX idx_agent_tasks_status ON agent_tasks(status);
            CREATE INDEX idx_agent_tasks_priority ON agent_tasks(priority);"
        ).unwrap();
        
        (temp_dir, conn)
    }
    
    #[test]
    fn test_create_and_get_task() {
        let (_dir, conn) = create_test_db();
        
        let id = AgentTasksStore::create_task(
            &conn,
            "analyze_file",
            5,
            r#"{"file": "test.rs"}"#,
            Some("tester"),
        ).unwrap();
        
        assert!(id > 0);
        
        let task = AgentTasksStore::get_task(&conn, id).unwrap();
        assert!(task.is_some());
        
        let task = task.unwrap();
        assert_eq!(task.task_type, "analyze_file");
        assert_eq!(task.priority, 5);
        assert_eq!(task.status, "pending");
        assert_eq!(task.created_by, "tester");
    }
    
    #[test]
    fn test_poll_pending_claims_tasks() {
        let (_dir, conn) = create_test_db();
        
        // Create 3 tasks
        AgentTasksStore::create_task(&conn, "task1", 5, "{}", None).unwrap();
        AgentTasksStore::create_task(&conn, "task2", 10, "{}", None).unwrap();
        AgentTasksStore::create_task(&conn, "task3", 5, "{}", None).unwrap();
        
        // Poll should return highest priority first
        let tasks = AgentTasksStore::poll_pending(&conn, 2).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].task_type, "task2"); // Higher priority (10)
        assert_eq!(tasks[1].task_type, "task1"); // Then older task1 (5) before task3
        
        // Verify tasks are now in_progress
        assert_eq!(tasks[0].status, "in_progress");
        assert_eq!(tasks[1].status, "in_progress");
        
        // Poll again should get the remaining task
        let tasks = AgentTasksStore::poll_pending(&conn, 10).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_type, "task3");
    }
    
    #[test]
    fn test_complete_task_success() {
        let (_dir, conn) = create_test_db();
        
        let id = AgentTasksStore::create_task(&conn, "test", 5, "{}", None).unwrap();
        AgentTasksStore::poll_pending(&conn, 1).unwrap();
        
        AgentTasksStore::complete_task(
            &conn,
            id,
            "completed",
            Some(r#"{"result": "ok"}"#),
            None,
        ).unwrap();
        
        let task = AgentTasksStore::get_task(&conn, id).unwrap().unwrap();
        assert_eq!(task.status, "completed");
        assert_eq!(task.result_json, Some(r#"{"result": "ok"}"#.to_string()));
        assert!(task.completed_at.is_some());
    }
    
    #[test]
    fn test_complete_task_failure() {
        let (_dir, conn) = create_test_db();
        
        let id = AgentTasksStore::create_task(&conn, "test", 5, "{}", None).unwrap();
        AgentTasksStore::poll_pending(&conn, 1).unwrap();
        
        AgentTasksStore::complete_task(
            &conn,
            id,
            "failed",
            None,
            Some("Something went wrong"),
        ).unwrap();
        
        let task = AgentTasksStore::get_task(&conn, id).unwrap().unwrap();
        assert_eq!(task.status, "failed");
        assert_eq!(task.error_message, Some("Something went wrong".to_string()));
    }
    
    #[test]
    fn test_mark_stale_timeout() {
        let (_dir, conn) = create_test_db();
        
        // Create and claim a task
        let id = AgentTasksStore::create_task(&conn, "slow_task", 5, "{}", None).unwrap();
        AgentTasksStore::poll_pending(&conn, 1).unwrap();
        
        // Manually set assigned_at to the past to simulate stale task
        conn.execute(
            "UPDATE agent_tasks SET assigned_at = '2020-01-01T00:00:00Z' WHERE id = ?1",
            params![id],
        ).unwrap();
        
        let marked = AgentTasksStore::mark_stale_timeout(&conn, 1).unwrap();
        assert_eq!(marked, 1);
        
        let task = AgentTasksStore::get_task(&conn, id).unwrap().unwrap();
        assert_eq!(task.status, "failed");
        assert_eq!(task.error_message, Some("Task timed out".to_string()));
    }
}
