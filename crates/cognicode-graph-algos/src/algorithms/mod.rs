//! Pure algorithm functions — same `.rs` compiles to native + wasm32.

pub mod all_simple_paths;
pub mod articulation_points;
pub mod bridges;
pub mod cfg;
pub mod cluster_components;
pub mod communities;
pub mod community_god_nodes;
pub mod condensation;
pub mod conductance;
pub mod dfg;
pub mod dominators;
pub mod dominators_cfg;
pub mod feedback_arc_set;
pub mod god_nodes;
pub mod interproc_summary;
pub mod k_core;
pub mod k_shortest_paths_helper;
pub mod modularity;
pub mod multi_source_reachability_helper;
pub mod page_rank;
pub mod personalized_pagerank;
pub mod slicing;
pub mod surprising_connections;
pub mod taint;
pub mod transitive_reduction;

pub use all_simple_paths::all_simple_paths;
pub use articulation_points::articulation_points;
pub use bridges::bridges;
pub use cfg::{BasicBlock, Cfg, CfgEdge, build_cfg, edge_count, reachable_blocks};
pub use cluster_components::cluster_components;
pub use communities::communities;
pub use community_god_nodes::community_god_nodes;
pub use condensation::condensation;
pub use conductance::conductance;
pub use dfg::{DefUseEdge, Statement, dfg_edges, reaching_uses};
pub use dominators::dominators;
pub use dominators_cfg::{DominatorInfo, dominators_cfg};
pub use feedback_arc_set::feedback_arc_set;
pub use god_nodes::god_nodes;
pub use interproc_summary::{
    FunctionLocalView, InterprocSummary, SummaryKind, compute_summaries, summary_id,
};
pub use k_core::k_core;
pub use k_shortest_paths_helper::k_shortest_paths;
pub use modularity::modularity;
pub use multi_source_reachability_helper::multi_source_reachability;
pub use page_rank::page_rank;
pub use personalized_pagerank::personalized_pagerank;
pub use slicing::{backward_slice, forward_slice};
pub use surprising_connections::surprising_connections;
pub use taint::{TaintPath, TaintResult, TaintSite as TaintSiteAlg, taint_forward};
pub use transitive_reduction::transitive_reduction;
