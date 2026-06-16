//! Report stage — persist the GraphReport to the `graph_reports` table
//! (ADR-017, Sprint 2).
//!
//! Receives the `AnalysisSummary` from the Analyze stage and persists it
//! as a JSON blob in `graph_reports`.

use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::application::ingest::analyzer::AnalysisSummary;
use crate::infrastructure::persistence::PostgresRepository;

/// Persist the analysis summary as a GraphReport row.
/// Returns the report key (workspace_id + timestamp).
pub async fn run_report(
    repo: &PostgresRepository,
    workspace_id: &str,
    summary: &AnalysisSummary,
) -> Option<String> {
    let report_json = serde_json::to_value(summary).unwrap_or_else(|_| json!({}));

    let report_id = format!(
        "{}/{}",
        workspace_id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );

    sqlx::query(
        "INSERT INTO graph_reports \
            (id, workspace_id, report, symbol_count, edge_count, health_score) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&report_id)
    .bind(workspace_id)
    .bind(report_json)
    .bind(summary.symbol_count as i32)
    .bind(summary.edge_count as i32)
    .bind(summary.health_score)
    .execute(repo.pool())
    .await
    .map_err(|e| tracing::error!("graph_report insert failed: {e}"))
    .ok();

    tracing::info!(report_id = %report_id, "graph_report persisted");
    Some(report_id)
}
