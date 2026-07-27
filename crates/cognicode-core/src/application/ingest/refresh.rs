//! Refresh stage — reload the in-memory `CallGraph` from PG and set it
//! in the `GraphCache` (ArcSwap), so the Explorer serves fresh data.
//!
//! Uses [`SnapshotProvider`] to fetch the current head snapshot from PostgreSQL.

use crate::domain::traits::repository::RepositoryError;
use crate::domain::value_objects::WorkspaceId;
use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::graph::snapshot_provider::SnapshotProvider;

/// Refresh the `GraphCache` from PostgreSQL via [`SnapshotProvider`].
///
/// Calls `current_head(ws)` to discover the live revision, then `snapshot(ws, head)`
/// to fetch the graph, and stores it in the local `GraphCache` ring buffer.
pub async fn refresh_from_pg(
    provider: &dyn SnapshotProvider,
    cache: &GraphCache,
    workspace: &WorkspaceId,
) -> Result<RefreshStats, RepositoryError> {
    // Discover current head revision
    let head = provider.current_head(workspace).map_err(|e| {
        RepositoryError::Store(format!("refresh_from_pg: current_head failed: {}", e))
    })?;

    if !head.is_valid() {
        // No revision yet — clear the cache
        cache.clear();
        return Ok(RefreshStats::default());
    }

    // Fetch the snapshot for the current head
    let graph = provider.snapshot(workspace, head).map_err(|e| {
        RepositoryError::Store(format!("refresh_from_pg: snapshot failed: {}", e))
    })?;

    let stats = RefreshStats {
        symbols: graph.symbol_count(),
        edges: graph.edge_count(),
    };

    // Store in local ring buffer (sets the head checkpoint)
    cache.set((*graph).clone());

    Ok(stats)
}

/// Statistics from a refresh operation.
#[derive(Debug, Default, Clone, Copy)]
pub struct RefreshStats {
    pub symbols: usize,
    pub edges: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::graph::graph_cache::GraphCache;

    #[test]
    fn test_refresh_stats_default() {
        let stats = RefreshStats::default();
        assert_eq!(stats.symbols, 0);
        assert_eq!(stats.edges, 0);
    }

    #[test]
    fn test_graph_cache_starts_empty() {
        let cache = GraphCache::new();
        assert_eq!(cache.get().symbol_count(), 0);
        assert_eq!(cache.get().edge_count(), 0);
    }
}
