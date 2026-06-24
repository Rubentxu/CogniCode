//! JsValue ⇄ Rust translation (wasm32 only).
//!
//! Uses `serde-wasm-bindgen` for direct JsValue↔Rust conversion.

use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

use super::protocol::{GodNodesOptions, Graph, JsonEdge, JsonNode, PageRankOptions};

/// Deserialize a `JsValue` into a typed Rust value.
pub fn from_value<T: for<'de> serde::Deserialize<'de>>(js: JsValue) -> Result<T, JsError> {
    serde_wasm_bindgen::from_value(js).map_err(|e| JsError::new(&e.to_string()))
}

/// Deserialize nodes + edges JsValues into a `Graph`.
pub fn graph_from_js(nodes_js: JsValue, edges_js: JsValue) -> Result<Graph, JsError> {
    let nodes: Vec<JsonNode> = from_value(nodes_js)?;
    let edges: Vec<JsonEdge> = from_value(edges_js)?;
    Ok(Graph::new(nodes, edges))
}

/// Serialize a Rust value into `JsValue`.
pub fn to_value<T: serde::Serialize>(value: &T) -> Result<JsValue, JsError> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsError::new(&e.to_string()))
}

/// Parse `PageRankOptions` from a `JsValue`.
pub fn page_rank_options_from_js(js: JsValue) -> Result<PageRankOptions, JsError> {
    from_value(js)
}

/// Parse `GodNodesOptions` from a `JsValue`.
pub fn god_nodes_options_from_js(js: JsValue) -> Result<GodNodesOptions, JsError> {
    from_value(js)
}
