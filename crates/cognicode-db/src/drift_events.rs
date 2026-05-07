//! Drift Events persistence layer
//!
//! Provides SQLite persistence for intent-drift detection events.
//! Used by detect_drift MCP tool and /api/drift dashboard endpoint.

use rusqlite::{Connection, params};
use anyhow::Result;
use chrono::Utc;

/// Default limit for query methods
const DEFAULT_LIMIT: usize = 20;

/// Store for drift detection events
pub struct DriftEventStore;

impl DriftEventStore {
    /// Insert a drift event, auto-generating timestamp and returning new id
    pub fn insert(conn: &Connection, event: &DriftEvent) -> Result<i64> {
        let timestamp = if event.timestamp.is_empty() {
            Utc::now().to_rfc3339()
        } else {
            event.timestamp.clone()
        };

        let severity = if event.severity.is_empty() {
            "warning"
        } else {
            &event.severity
        };

        conn.execute(
            "INSERT INTO drift_events (timestamp, file_path, function_name, drift_score, intent, severity) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                timestamp,
                event.file_path,
                event.function_name,
                event.drift_score,
                event.intent,
                severity,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// Query recent events ordered by timestamp DESC
    pub fn query_recent(conn: &Connection, limit: usize) -> Result<Vec<DriftEvent>> {
        let actual_limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity FROM drift_events ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![actual_limit as i64], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Query events by exact file_path match
    pub fn query_by_file(conn: &Connection, file_path: &str, limit: usize) -> Result<Vec<DriftEvent>> {
        let actual_limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity FROM drift_events WHERE file_path = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![file_path, actual_limit as i64], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Query events by exact function_name match
    pub fn query_by_function(conn: &Connection, function_name: &str, limit: usize) -> Result<Vec<DriftEvent>> {
        let actual_limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity FROM drift_events WHERE function_name = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![function_name, actual_limit as i64], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Query events by exact severity match
    pub fn query_by_severity(conn: &Connection, severity: &str, limit: usize) -> Result<Vec<DriftEvent>> {
        let actual_limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity FROM drift_events WHERE severity = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![severity, actual_limit as i64], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Query events where drift_score > threshold, ordered by score DESC
    pub fn get_high_drift(conn: &Connection, threshold: f64) -> Result<Vec<DriftEvent>> {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity FROM drift_events WHERE drift_score > ?1 ORDER BY drift_score DESC"
        )?;
        let rows = stmt.query_map(params![threshold], |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

/// Filter parameters for drift event queries — all fields optional, AND-combined
#[derive(Debug, Default)]
pub struct DriftFilter {
    /// LIKE %file% on file_path
    pub file: Option<String>,
    /// Exact match on function_name
    pub function_name: Option<String>,
    /// Exact match on severity
    pub severity: Option<String>,
    /// drift_score > min_score
    pub min_score: Option<f64>,
    /// Pagination offset
    pub offset: usize,
    /// Pagination limit
    pub limit: usize,
}

impl DriftEventStore {
    /// Query with optional filters, returns (events, total_matching_count)
    pub fn query_filtered(conn: &Connection, filter: &DriftFilter) -> Result<(Vec<DriftEvent>, usize)> {
        let actual_limit = if filter.limit == 0 { DEFAULT_LIMIT } else { filter.limit };

        // Build dynamic WHERE clause
        let mut conditions = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref file) = filter.file {
            conditions.push("file_path LIKE ?");
            params.push(Box::new(format!("%{}%", file)));
        }
        if let Some(ref fn_name) = filter.function_name {
            conditions.push("function_name = ?");
            params.push(Box::new(fn_name.clone()));
        }
        if let Some(ref severity) = filter.severity {
            conditions.push("severity = ?");
            params.push(Box::new(severity.clone()));
        }
        if let Some(min) = filter.min_score {
            conditions.push("drift_score > ?");
            params.push(Box::new(min));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Count total matching rows
        let count_sql = format!("SELECT COUNT(*) FROM drift_events {}", where_clause);
        let total_count: i64 = {
            let mut stmt = conn.prepare(&count_sql)?;
            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
            stmt.query_row(param_refs.as_slice(), |row| row.get(0))?
        };

        // Fetch paginated rows
        let select_sql = format!(
            "SELECT id, timestamp, file_path, function_name, drift_score, intent, severity \
             FROM drift_events {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_clause
        );
        let mut stmt = conn.prepare(&select_sql)?;
        let mut param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        param_refs.push(&actual_limit as &dyn rusqlite::ToSql);
        param_refs.push(&filter.offset as &dyn rusqlite::ToSql);

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            Ok(DriftEvent {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                file_path: row.get(2)?,
                function_name: row.get(3)?,
                drift_score: row.get(4)?,
                intent: row.get(5)?,
                severity: row.get(6)?,
            })
        })?;
        let events: Vec<DriftEvent> = rows.collect::<Result<Vec<_>, _>>()?;

        Ok((events, total_count as usize))
    }
}

/// A drift detection event record
#[derive(Debug, Clone)]
pub struct DriftEvent {
    pub id: i64,
    pub timestamp: String,
    pub file_path: String,
    pub function_name: String,
    pub drift_score: f64,
    pub intent: Option<String>,
    pub severity: String,
}
