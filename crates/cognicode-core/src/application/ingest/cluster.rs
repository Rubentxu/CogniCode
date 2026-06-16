//! Cluster stage — Label Propagation community detection + persistence
//! (ADR-017, Sprint 2).
//!
//! Uses the existing `CommunityDetector` service to detect communities
//! in the in-memory CallGraph, then persists community assignments to
//! `graph_nodes.properties.community`.

use sqlx::Acquire;

use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::persistence::PostgresRepository;

/// Run community detection on the current in-memory graph, and persist
/// the community assignment for each symbol node.
///
/// Returns the number of communities detected.
pub async fn run_cluster(
    repo: &PostgresRepository,
    cache: &GraphCache,
    workspace_id: &str,
) -> usize {
    let graph = cache.get();
    if graph.symbol_count() == 0 {
        return 0;
    }

    // Detect communities using Label Propagation
    let result = crate::application::services::community_detector::CommunityDetector::detect(
        &graph,
        100, // max_iterations
    );

    let communities = result.communities.len();
    let mut conn = match repo.pool().acquire().await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("cluster: pool acquire failed: {e}");
            return communities;
        }
    };
    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("cluster: begin tx failed: {e}");
            return communities;
        }
    };

    // Persist community labels to graph_nodes.properties
    for community in &result.communities {
        for node in &community.nodes {
            let _ = sqlx::query(
                "UPDATE graph_nodes \
                 SET properties = jsonb_set(properties, '{community}', $2::jsonb) \
                 WHERE workspace_id = $1 AND id = $3",
            )
            .bind(workspace_id)
            .bind(serde_json::json!(community.id.to_string()))
            .bind(node.to_string())
            .execute(&mut *tx)
            .await;
        }
    }

    let _ = tx.commit().await;
    tracing::info!(communities = communities, symbols = graph.symbol_count(), "cluster completed");
    communities
}
