//! Agent Interactions persistence layer
//!
//! Provides SQLite persistence for MCP tool usage telemetry.
//! Used by suggest_context for interaction history augmentation (Phase 3A)
//! and future Phase 3B dashboard analytics.

use rusqlite::{Connection, params};
use anyhow::Result;
use chrono::Utc;

use crate::tool_names::ALL_TOOL_NAMES;

/// Maximum length for result_summary field (truncate if longer)
const MAX_RESULT_SUMMARY_LEN: usize = 256;

/// Store for agent interaction telemetry
pub struct AgentInteractionStore;

impl AgentInteractionStore {
    /// Record a tool usage event (best-effort: errors logged, not propagated)
    pub fn record(
        conn: &Connection,
        tool_name: &str,
        result_summary: &str,
        duration_ms: f64,
        contract_id: Option<&str>,
    ) -> Result<()> {
        // Truncate result_summary to MAX_RESULT_SUMMARY_LEN
        let summary = if result_summary.len() > MAX_RESULT_SUMMARY_LEN {
            &result_summary[..MAX_RESULT_SUMMARY_LEN]
        } else {
            result_summary
        };

        conn.execute(
            "INSERT INTO agent_interactions (timestamp, tool_name, contract_id, result_summary, duration_ms) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                Utc::now().to_rfc3339(),
                tool_name,
                contract_id,
                summary,
                duration_ms,
            ],
        )?;
        Ok(())
    }

    /// Record a tool usage event with graceful degradation on schema absence
    /// Returns Ok(true) if recorded, Ok(false) if table doesn't exist
    /// Note: This is a simplified version - caller should handle errors appropriately
    #[allow(dead_code)]
    pub fn record_graceful(
        conn: &Connection,
        tool_name: &str,
        result_summary: &str,
        duration_ms: f64,
        contract_id: Option<&str>,
    ) -> bool {
        match Self::record(conn, tool_name, result_summary, duration_ms, contract_id) {
            Ok(()) => true,
            Err(_) => {
                // On any error (including "no such table"), return false for graceful degradation
                // Caller in cognicode-core will handle logging if needed
                false
            }
        }
    }

    /// Query recent interactions for a tool name
    #[allow(dead_code)]
    pub fn query_recent(
        conn: &Connection,
        tool_name: &str,
        limit: usize,
    ) -> Result<Vec<AgentInteraction>> {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, tool_name, contract_id, result_summary, duration_ms FROM agent_interactions WHERE tool_name = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![tool_name, limit as i64], |row| {
            Ok(AgentInteraction {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                tool_name: row.get(2)?,
                contract_id: row.get(3)?,
                result_summary: row.get(4)?,
                duration_ms: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Query recent interactions across all tools
    #[allow(dead_code)]
    pub fn query_all_recent(
        conn: &Connection,
        limit: usize,
    ) -> Result<Vec<AgentInteraction>> {
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, tool_name, contract_id, result_summary, duration_ms FROM agent_interactions ORDER BY timestamp DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(AgentInteraction {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                tool_name: row.get(2)?,
                contract_id: row.get(3)?,
                result_summary: row.get(4)?,
                duration_ms: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Check if the agent_interactions table exists
    pub fn table_exists(conn: &Connection) -> bool {
        conn.query_row(
            "SELECT name FROM sqlite_master WHERE type='table' AND name='agent_interactions'",
            [],
            |_| Ok(()),
        ).is_ok()
    }

    /// Aggregate tool usage statistics grouped by tool_name.
    /// If `since` is provided (ISO 8601 timestamp), only records at or after that time are included.
    /// Returns an empty vector if the table doesn't exist or has no matching records.
    pub fn aggregate_stats(conn: &Connection, since: Option<&str>) -> Result<Vec<ToolStats>> {
        if !Self::table_exists(conn) {
            return Ok(vec![]);
        }

        // Collect all matching rows
        let collected: Vec<(String, f64, String)> = if let Some(since_val) = since {
            let mut stmt = conn.prepare(
                "SELECT tool_name, duration_ms, result_summary FROM agent_interactions WHERE timestamp >= ?1"
            )?;
            stmt.query_map(params![since_val], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?.collect::<Result<Vec<_>, _>>()?
        } else {
            let mut stmt = conn.prepare(
                "SELECT tool_name, duration_ms, result_summary FROM agent_interactions"
            )?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?.collect::<Result<Vec<_>, _>>()?
        };

        // Group by tool_name using known tool names
        let mut stats_map: std::collections::HashMap<String, ToolStats> = std::collections::HashMap::new();

        for tool_name in ALL_TOOL_NAMES {
            stats_map.insert(tool_name.to_string(), ToolStats {
                tool_name: tool_name.to_string(),
                count: 0,
                avg_duration_ms: 0.0,
                success_count: 0,
                error_count: 0,
                other_count: 0,
            });
        }

        for (tool_name, _duration_ms, result_summary) in &collected {
            let stats = stats_map.entry(tool_name.clone()).or_insert_with(|| ToolStats {
                tool_name: tool_name.clone(),
                count: 0,
                avg_duration_ms: 0.0,
                success_count: 0,
                error_count: 0,
                other_count: 0,
            });

            stats.count += 1;
            let status = classify_result_status(result_summary);
            match status {
                ResultStatus::Success => stats.success_count += 1,
                ResultStatus::Error => stats.error_count += 1,
                ResultStatus::Other => stats.other_count += 1,
            }
        }

        // Compute averages and filter out tools with no interactions
        let mut result: Vec<ToolStats> = stats_map
            .into_values()
            .filter(|s| s.count > 0)
            .map(|mut s| {
                let total: f64 = collected
                    .iter()
                    .filter(|(name, _, _)| name == &s.tool_name)
                    .map(|(_, dur, _)| dur)
                    .sum();
                s.avg_duration_ms = total / s.count as f64;
                s
            })
            .collect();

        result.sort_by(|a, b| a.tool_name.cmp(&b.tool_name));
        Ok(result)
    }
}

/// Result status classification based on result_summary substring scan
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultStatus {
    Success,
    Error,
    Other,
}

/// Classify a result_summary string into success/error/other.
/// - Contains "Error" → Error
/// - Contains "Found" → Success
/// - Everything else → Other
pub fn classify_result_status(result_summary: &str) -> ResultStatus {
    if result_summary.contains("Error") {
        ResultStatus::Error
    } else if result_summary.contains("Found") {
        ResultStatus::Success
    } else {
        ResultStatus::Other
    }
}

/// Aggregated tool usage statistics
#[derive(Debug, Clone)]
pub struct ToolStats {
    pub tool_name: String,
    pub count: usize,
    pub avg_duration_ms: f64,
    pub success_count: usize,
    pub error_count: usize,
    pub other_count: usize,
}

/// An agent interaction record
#[derive(Debug, Clone)]
pub struct AgentInteraction {
    pub id: i64,
    pub timestamp: String,
    pub tool_name: String,
    pub contract_id: Option<String>,
    pub result_summary: String,
    pub duration_ms: f64,
}
