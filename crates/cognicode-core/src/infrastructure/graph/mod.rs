//! Graph module - Petgraph-based graph implementations
//!
//! This module provides various graph construction strategies:
//! - `PetGraphStore`: Full petgraph-based graph store
//! - `GraphCache`: Thread-safe cache for call graphs
//! - `LightweightIndex`: Fast symbol index without edges
//! - `SymbolIndex`: Symbol index with cache management
//! - `OnDemandGraphBuilder`: Lazy graph construction
//! - `PerFileGraphCache`: Per-file graph caching
//! - `GraphStrategy`: Unified interface for different strategies

mod graph_cache;
mod lightweight_index;
mod on_demand_graph;
mod per_file_graph;
mod pet_graph_store;
mod strategy;
mod symbol_index;

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
