//! Graph module - Petgraph-based graph implementations
//!
//! This module provides various graph construction strategies:
//! - `PetGraphStore`: Full petgraph-based graph store
//! - `CallGraphProjection`: Read-side petgraph projection for graph algorithms
//! - `GraphCache`: Thread-safe cache for call graphs
//! - `LightweightIndex`: Fast symbol index without edges
//! - `SymbolIndex`: Symbol index with cache management
//! - `OnDemandGraphBuilder`: Lazy graph construction
//! - `PerFileGraphCache`: Per-file graph caching
//! - `GraphStrategy`: Unified interface for different strategies
//! - `FileManifest` / `IncrementalScanner` (gated behind `persistence`):
//!   mtime + blake3 content-hash tracking for incremental graph rescans.

mod call_graph_projection;
mod graph_cache;
mod lightweight_index;
mod on_demand_graph;
mod per_file_graph;
mod pet_graph_store;
mod strategy;
mod symbol_index;

#[cfg(feature = "persistence")]
mod file_manifest;
#[cfg(feature = "persistence")]
mod incremental_scanner;

pub use call_graph_projection::{
    CallGraphProjection, ExplanationHop, ExplanationView, ProjectionError, SubgraphDirection,
    SubgraphEdge, SubgraphView,
};
pub use graph_cache::GraphCache;
pub use lightweight_index::{LightweightIndex, SymbolLocation};
pub use on_demand_graph::{
    CallHierarchyResult, HierarchyEntry, OnDemandGraphBuilder, TraversalDirection,
};
pub use per_file_graph::PerFileGraphCache;
pub use pet_graph_store::PetGraphStore;
pub use strategy::{
    FullGraphStrategy, GraphStrategy, GraphStrategyFactory, LightweightStrategy, OnDemandStrategy,
    PerFileStrategy,
};
pub use symbol_index::{CacheConfig, SymbolIndex};

#[cfg(feature = "persistence")]
pub use file_manifest::{FileManifest, FileRecord, ScanDelta};
#[cfg(feature = "persistence")]
pub use incremental_scanner::{IncrementalScanResult, IncrementalScanner};
