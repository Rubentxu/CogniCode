//! cognicode-graph-wasm — JavaScript bindings for cognicode-graph-algos.
//!
//! This crate is a **thin translation layer**. It converts JavaScript
//! `JsValue` arguments to Rust slices, calls the pure functions in
//! `cognicode_graph_algos`, and converts the results back to `JsValue`.
//! No algorithm logic lives here.
//!
//! # WASM target
//!
//! Build with `wasm-pack build --target web --release`. The output is
//! dropped into `apps/explorer-ui/src/wasm/` and lazy-loaded by the
//! frontend hook (PR #4).
//!
//! # Native target
//!
//! Native build is used for tests only — the `#[wasm_bindgen]` exports
//! are no-ops on native, and `serde_json` round-trips provide test
//! coverage of the translation logic.

#![cfg_attr(
    target_arch = "wasm32",
    doc = "Built for wasm32-unknown-unknown via wasm-pack."
)]

pub mod protocol;

// wasm_bind.rs is only compiled for wasm32.
#[cfg(target_arch = "wasm32")]
mod wasm_bind;

#[cfg(target_arch = "wasm32")]
use cognicode_graph_algos::{
    GraphBuilder, god_nodes as inner_god_nodes, page_rank as inner_page_rank,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

// Re-export protocol types at crate root for ergonomic access by tests.
pub use protocol::*;

// ============================================================================
// WASM EXPORTS — only compiled for wasm32 target
// ============================================================================

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn _start() {
    // wasm-bindgen requires this for panic hook setup.
    // No-op for now; could install console_error_panic_hook in future.
}

/// PageRank — wasm-bindgen export.
///
/// # Arguments (all `JsValue` from JavaScript)
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `options_js`: `{ damping?: number, max_iterations?: number }`
///   - `damping` defaults to 0.85
///   - `max_iterations` defaults to 100
///
/// # Returns
///
/// `{ scores: { [nodeId: string]: number } }` — node-id-keyed `BTreeMap`
/// shape for deterministic ordering.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn pagerank(
    nodes_js: JsValue,
    edges_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsError> {
    let graph = wasm_bind::graph_from_js(nodes_js, edges_js)?;
    let options = wasm_bind::page_rank_options_from_js(options_js)?;
    let (in_neighbors, out_degree) = graph.build_adjacency();
    let raw = inner_page_rank(
        &in_neighbors,
        &out_degree,
        graph.0.nodes.len(),
        options.damping,
        options.max_iterations,
    );
    // Translate usize-indexed scores back to node-id-keyed scores.
    let mut scores = std::collections::BTreeMap::new();
    for (i, node) in graph.0.nodes.iter().enumerate() {
        if let Some(&s) = raw.get(&i) {
            scores.insert(node.id.clone(), s);
        }
    }
    let output = protocol::PageRankOutput { scores };
    wasm_bind::to_value(&output).map_err(|_e| JsError::new("serialization error"))
}

/// god_nodes — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `options_js`: `{ percentile?: number }` — defaults to 0.95,
///   clamped to `[0.0, 1.0]`
///
/// # Returns
///
/// `{ nodes: Array<{ id: string, score: number }> }` — sorted desc by
/// score, ties broken by `id` ascending (per spec REQ-052).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn god_nodes(
    nodes_js: JsValue,
    edges_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsError> {
    let graph = wasm_bind::graph_from_js(nodes_js, edges_js)?;
    let options = wasm_bind::god_nodes_options_from_js(options_js)?;
    let (in_neighbors, out_degree) = graph.build_adjacency();
    let raw_scores = inner_page_rank(
        &in_neighbors,
        &out_degree,
        graph.0.nodes.len(),
        0.85, // default damping for god_nodes (matches existing impl)
        100,  // default max_iter
    );
    let god_indices = inner_god_nodes(&raw_scores, options.percentile);
    // Translate usize-indexed god_indices back to node records.
    let mut nodes_out: Vec<protocol::GodNodeEntry> = Vec::with_capacity(god_indices.len());
    for (idx, score) in god_indices {
        if let Some(node) = graph.0.nodes.get(idx) {
            nodes_out.push(protocol::GodNodeEntry {
                id: node.id.clone(),
                score,
            });
        }
    }
    let output = protocol::GodNodesOutput { nodes: nodes_out };
    wasm_bind::to_value(&output).map_err(|_e| JsError::new("serialization error"))
}
