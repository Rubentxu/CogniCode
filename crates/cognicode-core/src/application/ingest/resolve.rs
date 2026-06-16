//! Resolve stage — cross-file call resolution (ADR-017, Sprint 2).
//!
//! After all files are extracted and their nodes are in `graph_nodes`,
//! unresolved edges (where the callee is only known by name) are
//! matched against the global symbol index.
//!
//! Resolution strategy:
//! 1. For each unresolved edge, query `graph_nodes` for nodes whose
//!    `label` matches the callee name (case-insensitive).
//! 2. If exactly one match is found, insert a `Calls` edge with
//!    `Provenance::Inferred` (0.7).
//! 3. If zero or multiple matches, the edge stays ambiguous (not
//!    inserted — the caller remains unresolved).
//!
//! The stage runs as a single SQL batch after PgUpsert completes.

use sqlx::Acquire;

use crate::application::ingest::types::{ExtractionEdge, TargetRef};
use crate::infrastructure::persistence::PostgresRepository;

/// Resolve cross-file calls from the queue of unresolved edges.
/// Runs a single SQL pass: for each unresolved callee name, find
/// matching `graph_nodes` and insert `Inferred` edges.
///
/// Returns the number of edges successfully resolved.
pub async fn resolve_cross_file_calls(
    repo: &PostgresRepository,
    workspace_id: &str,
    unresolved: &[ExtractionEdge],
) -> usize {
    if unresolved.is_empty() {
        return 0;
    }

    let mut resolved = 0;
    let mut conn = match repo.pool().acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("resolve: pool acquire failed: {e}");
            return 0;
        }
    };

    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("resolve: begin tx failed: {e}");
            return 0;
        }
    };

    for edge in unresolved {
        let callee_name = match &edge.target_ref {
            TargetRef::Unresolved(name) => name.to_lowercase(),
            TargetRef::Resolved(_) => continue, // skip already-resolved
        };

        if callee_name.is_empty() {
            continue;
        }

        // Find matching node by label (case-insensitive).
        // Only match `symbol.*` kind nodes — document/config nodes
        // are not valid call targets.
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM graph_nodes \
             WHERE workspace_id = $1 \
               AND LOWER(label) = $2 \
               AND kind LIKE 'symbol.%' \
             LIMIT 2", // 2 = we only care if exactly 1 match exists
        )
        .bind(workspace_id)
        .bind(&callee_name)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        if rows.len() != 1 {
            // Zero or multiple matches — ambiguous, skip
            continue;
        }

        let target_id = &rows[0];

        // Insert inferred edge
        let metadata = serde_json::json!({
            "source_file": edge.source,
            "line": edge.line,
            "resolved_by": "resolve_stage"
        });

        let result = sqlx::query(
            "INSERT INTO graph_edges \
                (source_id, target_id, kind, provenance, confidence, metadata, workspace_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (source_id, target_id, kind) DO NOTHING",
        )
        .bind(&edge.source)
        .bind(target_id)
        .bind(&edge.kind)
        .bind("Inferred") // Provenance::Inferred
        .bind(0.7_f64)
        .bind(metadata)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await;

        match result {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    resolved += 1;
                }
            }
            Err(e) => {
                tracing::warn!(
                    callee = %callee_name,
                    source = %edge.source,
                    "resolve: insert failed: {e}"
                );
            }
        }
    }

    if let Err(e) = tx.commit().await {
        tracing::error!("resolve: commit failed: {e}");
        return resolved;
    }

    tracing::info!(
        resolved = resolved,
        total = unresolved.len(),
        "resolve_cross_file_calls completed"
    );

    resolved
}

/// Resolve import edges — match imported module names against file-level
/// graph nodes.
///
/// Import edges link file A to file B's file-level node. The file-level
/// node has `source_path` equal to the imported module path.
pub async fn resolve_imports(
    repo: &PostgresRepository,
    workspace_id: &str,
    unresolved: &[ExtractionEdge],
) -> usize {
    let mut resolved = 0;
    let mut conn = match repo.pool().acquire().await {
        Ok(c) => c,
        Err(_) => return 0,
    };
    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(_) => return 0,
    };

    for edge in unresolved {
        if !edge.kind.contains("imports") {
            continue;
        }
        let module_name = match &edge.target_ref {
            TargetRef::Unresolved(name) => name,
            TargetRef::Resolved(_) => continue,
        };

        // Try to find a file-level node whose source_path contains
        // the module name (e.g. "models::user" matches "src/models/user.rs")
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM graph_nodes \
             WHERE workspace_id = $1 \
               AND kind = 'symbol.file' \
               AND source_path LIKE $2 \
             LIMIT 2",
        )
        .bind(workspace_id)
        .bind(format!("%{}%", module_name))
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        if rows.len() == 1 {
            let target = &rows[0];
            let _ = sqlx::query(
                "INSERT INTO graph_edges \
                    (source_id, target_id, kind, provenance, confidence, metadata, workspace_id) \
                 VALUES ($1, $2, $3, 'Inferred', 0.8, '{}', $4) \
                 ON CONFLICT DO NOTHING",
            )
            .bind(&edge.source)
            .bind(target)
            .bind(&edge.kind)
            .bind(workspace_id)
            .execute(&mut *tx)
            .await;
            resolved += 1;
        }
    }

    let _ = tx.commit().await;
    resolved
}
