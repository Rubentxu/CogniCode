//! The read projection of a published snapshot, built once and lent to every
//! query of that snapshot.
//!
//! A projection is a pure function of one immutable
//! [`CallGraph`], and building it costs
//! far more than answering a query from it. A handler that built a fresh one
//! per call paid that build, and the array adjacency the projection carries,
//! on every call. [`ProjectionCache`] keeps the projection of the snapshots
//! being served, so the projection and its adjacency are built once per
//! snapshot and shared by every reader.
//!
//! The cache is a [`SnapshotCache`]: its keying, invalidation and
//! concurrency rules are those of
//! [`snapshot_cache`](crate::infrastructure::graph::snapshot_cache). A change
//! to the graph publishes a new snapshot, which gets a new projection; the
//! projection a reader already holds is not touched.
//!
//! # Memory
//!
//! A projection retains about 700 bytes per symbol (measured at 100 000
//! symbols: 67 MiB, of which the adjacency is 7 MiB), far more than the
//! ranking statistics, so the budget is smaller than the default of the
//! generic cache: two snapshots within [`PROJECTION_CACHE_MAX_SYMBOLS`]
//! symbols. A larger snapshot is projected per call, as before.

use std::sync::Arc;

use crate::domain::aggregates::CallGraph;
use crate::domain::ports::call_graph_projection::CallGraphProjectionPort;
use crate::infrastructure::graph::analytics::ranking_stats::RankingStatsCache;
use crate::infrastructure::graph::project_call_graph;
use crate::infrastructure::graph::snapshot_cache::{
    SnapshotCache, SnapshotCacheCounters, SnapshotCacheLimits,
};

/// Most symbols the projection cache retains, summed over its snapshots.
pub const PROJECTION_CACHE_MAX_SYMBOLS: usize = 250_000;

/// Counters of a [`ProjectionCache`].
pub type ProjectionCacheCounters = SnapshotCacheCounters;

/// What a serving context derives once per published snapshot: the ranking
/// statistics and the read projection, each keyed and bounded on its own.
#[derive(Default)]
pub struct SnapshotCaches {
    /// Ranking statistics of the snapshots being served.
    pub ranking_stats: Arc<RankingStatsCache>,
    /// Read projections of the snapshots being served.
    pub projections: ProjectionCache,
}

/// Shares one projection per published snapshot between queries.
pub struct ProjectionCache {
    cache: SnapshotCache<Arc<dyn CallGraphProjectionPort>>,
}

impl Default for ProjectionCache {
    fn default() -> Self {
        Self::new(SnapshotCacheLimits {
            max_entries: 2,
            max_symbols: PROJECTION_CACHE_MAX_SYMBOLS,
        })
    }
}

impl ProjectionCache {
    /// An empty cache that keeps at most what `limits` allow.
    pub fn new(limits: SnapshotCacheLimits) -> Self {
        Self {
            cache: SnapshotCache::new(limits),
        }
    }

    /// The projection of `graph`, built by the first caller and shared by
    /// the rest for as long as the entry lives.
    pub fn projection_for(&self, graph: &Arc<CallGraph>) -> Arc<dyn CallGraphProjectionPort> {
        self.cache.get_or_build(graph, || project_call_graph(graph))
    }

    /// What the cache has done and holds right now.
    pub fn counters(&self) -> ProjectionCacheCounters {
        self.cache.counters()
    }
}
