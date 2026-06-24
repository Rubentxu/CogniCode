//! JSON protocol types — wasm32-agnostic, shared between Rust and JavaScript.
//!
//! These types define the stable wire format between the JS frontend and the
//! WASM shim. They are serde-serializable on both wasm32 and native targets.

use cognicode_graph_algos::{GraphBuilder, adapters::json_graph::JsonGraph};
use serde::{Deserialize, Serialize};

pub use cognicode_graph_algos::adapters::json_graph::{JsonEdge, JsonNode};

/// Graph built from JSON node/edge arrays — wraps the adapter from
/// `cognicode-graph-algos` so the wasm shim can call `build_adjacency()`.
#[derive(Debug, Clone, Default)]
pub struct Graph(pub JsonGraph);

impl Graph {
    pub fn new(nodes: Vec<JsonNode>, edges: Vec<JsonEdge>) -> Self {
        Self(JsonGraph::new(nodes, edges))
    }
}

impl GraphBuilder for Graph {
    fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>) {
        self.0.build_adjacency()
    }
}

/// PageRank options from JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankOptions {
    #[serde(default = "default_damping")]
    pub damping: f64,
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

/// PageRank output — node-id-keyed scores for JS consumption.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageRankOutput {
    pub scores: std::collections::BTreeMap<String, f64>,
}

/// GodNodes options from JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodesOptions {
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

/// A single god node in the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodeEntry {
    pub id: String,
    pub score: f64,
}

/// GodNodes output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GodNodesOutput {
    pub nodes: Vec<GodNodeEntry>,
}
