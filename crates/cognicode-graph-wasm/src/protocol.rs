//! JSON protocol types for the WASM shim.
//!
//! These types are the serialization boundary between the frontend JS and the
//! WASM module. They mirror the Rust algorithm output structs and are designed
//! to round-trip through serde_json without data loss.
//!
//! IMPORTANT: These must stay in sync with the TypeScript types in
//! `apps/explorer-ui/src/api/types.ts`.

use serde::{Deserialize, Serialize};

// =============================================================================
// PageRank
// =============================================================================

/// Options for `pagerank` (WASM and backend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankOptions {
    /// Damping factor for PageRank. Defaults to 0.85.
    #[serde(default = "default_damping")]
    pub damping: f64,
    /// Maximum number of iterations. Defaults to 100.
    #[serde(default = "default_max_iter")]
    pub max_iterations: usize,
}

fn default_damping() -> f64 {
    0.85
}

fn default_max_iter() -> usize {
    100
}

impl Default for PageRankOptions {
    fn default() -> Self {
        Self {
            damping: default_damping(),
            max_iterations: default_max_iter(),
        }
    }
}

/// PageRank output — `scores[node_id] = score`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankOutput {
    /// Map of node ID to PageRank score.
    pub scores: std::collections::HashMap<String, f64>,
}

// =============================================================================
// God Nodes
// =============================================================================

/// Options for `god_nodes` (WASM and backend).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodesOptions {
    /// Percentile threshold for god nodes (0.0-1.0). Defaults to 0.95.
    #[serde(default = "default_percentile")]
    pub percentile: f64,
}

fn default_percentile() -> f64 {
    0.95
}

impl Default for GodNodesOptions {
    fn default() -> Self {
        Self {
            percentile: default_percentile(),
        }
    }
}

/// A god node entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodeEntry {
    /// Node identifier.
    pub id: String,
    /// PageRank score.
    pub score: f64,
}

/// Output for `god_nodes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodesOutput {
    /// List of god nodes sorted by score descending.
    pub nodes: Vec<GodNodeEntry>,
}

// =============================================================================
// Communities
// =============================================================================

/// Options for `communities` (Label Propagation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitiesOptions {
    /// Maximum number of iterations. Defaults to 100.
    #[serde(default = "default_community_max_iter")]
    pub max_iterations: usize,
}

fn default_community_max_iter() -> usize {
    100
}

impl Default for CommunitiesOptions {
    fn default() -> Self {
        Self {
            max_iterations: default_community_max_iter(),
        }
    }
}

/// A single community — list of node IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Community {
    /// List of node IDs in this community.
    pub node_ids: Vec<String>,
}

/// Output for `communities`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunitiesOutput {
    /// List of communities.
    pub communities: Vec<Community>,
}

// =============================================================================
// Community God Nodes
// =============================================================================

/// Options for `community_god_nodes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityGodNodesOptions {
    /// Percentile threshold for god nodes within each community (0.0-1.0). Defaults to 0.95.
    #[serde(default = "default_percentile")]
    pub percentile: f64,
}

impl Default for CommunityGodNodesOptions {
    fn default() -> Self {
        Self {
            percentile: default_percentile(),
        }
    }
}

/// A god node within a specific community.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityGodNode {
    /// Index of the community this node belongs to.
    pub community_index: usize,
    /// Node identifier.
    pub id: String,
    /// PageRank score.
    pub score: f64,
}

/// Output for `community_god_nodes`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityGodNodesOutput {
    /// List of god nodes within communities.
    pub nodes: Vec<CommunityGodNode>,
}

// =============================================================================
// Surprising Connections
// =============================================================================

/// Options for `surprising_connections`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurprisingConnectionsOptions {
    /// Maximum number of surprising connections to return. Defaults to 10.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    10
}

impl Default for SurprisingConnectionsOptions {
    fn default() -> Self {
        Self {
            limit: default_limit(),
        }
    }
}

/// A surprising cross-community edge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurprisingEdge {
    /// Source node ID.
    pub source_id: String,
    /// Target node ID.
    pub target_id: String,
    /// Edge score (product of PageRank scores).
    pub score: f64,
}

/// Output for `surprising_connections`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurprisingConnectionsOutput {
    /// List of surprising cross-community edges.
    pub edges: Vec<SurprisingEdge>,
}

// =============================================================================
// Condensation (SCC)
// =============================================================================

/// Output for `condensation` — strongly connected components.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CondensationOutput {
    /// List of SCCs. Each SCC is a list of node IDs.
    pub components: Vec<Vec<String>>,
}

// =============================================================================
// Transitive Reduction
// =============================================================================

/// A directed edge in a transitive reduction output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitiveReductionEdge {
    /// Source node ID.
    pub source_id: String,
    /// Target node ID.
    pub target_id: String,
}

/// Output for `transitive_reduction`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitiveReductionOutput {
    /// List of edges that survive the transitive reduction.
    pub edges: Vec<TransitiveReductionEdge>,
}

// =============================================================================
// Feedback Arc Set
// =============================================================================

/// Output for `feedback_arc_set`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackArcSetOutput {
    /// List of edges whose removal breaks all cycles.
    pub edges: Vec<TransitiveReductionEdge>,
}

// =============================================================================
// All Simple Paths
// =============================================================================

/// Options for `all_simple_paths`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllSimplePathsOptions {
    /// Maximum number of intermediate nodes (hops). Defaults to 10.
    #[serde(default = "default_max_hops")]
    pub max_hops: usize,
}

fn default_max_hops() -> usize {
    10
}

impl Default for AllSimplePathsOptions {
    fn default() -> Self {
        Self {
            max_hops: default_max_hops(),
        }
    }
}

/// Output for `all_simple_paths`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllSimplePathsOutput {
    /// List of paths. Each path is a sequence of node IDs.
    pub paths: Vec<Vec<String>>,
}

// =============================================================================
// Cluster Components
// =============================================================================

/// Output for `cluster_components`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterComponentsOutput {
    /// List of clusters. Each cluster is a list of node IDs.
    pub clusters: Vec<Vec<String>>,
}
