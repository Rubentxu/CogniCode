//! Refresh stage — reload the in-memory `CallGraph` from PG and set it
//! in the `GraphCache` (ArcSwap), so the Explorer serves fresh data.
//!
//! For Sprint 1, this is a full reload (ADR-017). Sprint 4 will replace
//! this with incremental updates via `GraphDiffCalculator` (ADR-022).

use crate::domain::traits::repository::RepositoryError;
use crate::infrastructure::graph::graph_cache::GraphCache;
use crate::infrastructure::persistence::PostgresRepository;

/// Refresh the `GraphCache` from PG. Loads all symbols and edges via the
/// `symbols` and `call_edges` VIEWs, constructs a `CallGraph`, and sets
/// it in the ArcSwap cache.
pub async fn refresh_from_pg(
    repo: &PostgresRepository,
    cache: &GraphCache,
) -> Result<RefreshStats, RepositoryError> {
    let graph = repo.load_call_graph().await?;

    if let Some(graph) = graph {
        let stats = RefreshStats {
            symbols: graph.symbol_count(),
            edges: graph.edge_count(),
        };
        cache.set(graph);
        Ok(stats)
    } else {
        // Empty database — clear the cache
        cache.clear();
        Ok(RefreshStats::default())
    }
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
