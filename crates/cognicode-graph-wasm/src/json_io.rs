//! JSON I/O helpers for the WASM shim.
//!
//! Converts between JsValue (wasm-bindgen) and Rust types using serde_json.
//! This module is only compiled for the WASM target.

use wasm_bindgen::JsValue;

/// Parse a JsValue into a typed Rust struct via serde_json.
pub fn from_value<T: serde::de::DeserializeOwned>(value: JsValue) -> Result<T, JsError> {
    let json_str = js_sys::JSON::stringify(&value)
        .map_err(|e| JsError::new(&format!("JSON stringify failed: {:?}", e)))?
        .as_string()
        .ok_or_else(|| JsError::new("JSON.stringify returned non-string"))?;
    serde_json::from_str(&json_str).map_err(|e| JsError::new(&format!("serde parse failed: {}", e)))
}

/// Convert nodes + edges JsValue arrays into a JsonGraph.
pub fn from_js(nodes_js: JsValue, edges_js: JsValue) -> Result<json_graph::JsonGraph, JsError> {
    let nodes: Vec<json_graph::JsonNode> = from_value(nodes_js)?;
    let edges: Vec<json_graph::JsonEdge> = from_value(edges_js)?;
    Ok(json_graph::JsonGraph::new(nodes, edges))
}

/// Serialize a Rust struct to JsValue via serde_json.
pub fn to_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    let json_str = serde_json::to_string(value)
        .map_err(|e| JsError::new(&format!("serde serialize failed: {}", e)))?;
    js_sys::JSON::parse(&json_str).map_err(|e| JsError::new(&format!("JSON parse failed: {:?}", e)))
}

/// Wrapper for wasm_bindgen JsError.
#[derive(Debug)]
pub struct JsError {
    message: String,
}

impl JsError {
    pub fn new(msg: &str) -> Self {
        Self {
            message: msg.to_string(),
        }
    }
}

impl std::fmt::Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JsError: {}", self.message)
    }
}

impl std::error::Error for JsError {}

impl From<wasm_bindgen::JsValue> for JsError {
    fn from(v: wasm_bindgen::JsValue) -> Self {
        Self::new(&format!("{:?}", v))
    }
}

impl From<serde_json::Error> for JsError {
    fn from(e: serde_json::Error) -> Self {
        Self::new(&format!("serde error: {}", e))
    }
}

// Re-export JsonGraph and related types from the adapters module.
// We duplicate the struct here to avoid a cyclic dependency on
// cognicode-graph-algos (which would pull in petgraph in non-WASM builds).
mod json_graph {
    use serde::{Deserialize, Serialize};

    /// JSON node DTO — matches frontend `GraphNode`.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JsonNode {
        pub id: String,
        #[serde(default)]
        pub label: Option<String>,
    }

    /// JSON edge DTO — matches frontend `GraphEdge`.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JsonEdge {
        pub source: String,
        pub target: String,
    }

    /// WASM-friendly graph: holds JSON DTOs.
    #[derive(Debug, Clone, Default)]
    pub struct JsonGraph {
        pub nodes: Vec<JsonNode>,
        pub edges: Vec<JsonEdge>,
    }

    impl JsonGraph {
        pub fn new(nodes: Vec<JsonNode>, edges: Vec<JsonEdge>) -> Self {
            Self { nodes, edges }
        }

        /// Build adjacency lists from JSON DTOs.
        pub fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>, Vec<Vec<usize>>) {
            let n = self.nodes.len();
            let id_to_idx: std::collections::HashMap<&str, usize> = self
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.id.as_str(), i))
                .collect();

            let mut in_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut out_neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
            let mut out_degree: Vec<usize> = vec![0; n];

            for edge in &self.edges {
                let s = match id_to_idx.get(edge.source.as_str()) {
                    Some(&idx) if idx < n => idx,
                    _ => continue,
                };
                let t = match id_to_idx.get(edge.target.as_str()) {
                    Some(&idx) if idx < n => idx,
                    _ => continue,
                };
                in_neighbors[t].push(s);
                out_neighbors[s].push(t);
                out_degree[s] += 1;
            }
            (in_neighbors, out_degree, out_neighbors)
        }
    }
}

// Re-export for use in lib.rs
pub use json_graph::{JsonGraph, JsonNode};
