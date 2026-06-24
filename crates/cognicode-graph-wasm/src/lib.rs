//! cognicode-graph-wasm — WASM shim for cognicode-graph-algos.
//!
//! This crate exposes graph algorithm functions to the browser via wasm-bindgen.
//! All algorithms are thin wrappers around the pure functions in `cognicode-graph-algos`.
//!
//! ## Design principles (ADR-048)
//!
//! - **Single source of truth**: Algorithm logic lives in `cognicode-graph-algos`.
//!   This crate only adds protocol translation (JsValue ↔ Rust types).
//! - **No domain coupling**: The WASM module has zero knowledge about CogniCode
//!   domain types. It operates only on JSON DTOs.
//! - **WASM-clean**: `cognicode-graph-algos` is compiled without the `petgraph-adapter`
//!   feature, so petgraph never enters the browser binary.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod json_io;
pub mod protocol;

use ::wasm_bindgen::JsValue;

use json_io::{JsonGraph, from_js, from_value, to_value};
use protocol::{
    CommunitiesOptions, CommunitiesOutput, Community, CommunityGodNode, CommunityGodNodesOptions,
    CommunityGodNodesOutput, GodNodeEntry, GodNodesOptions, GodNodesOutput, PageRankOptions,
    PageRankOutput, SurprisingConnectionsOptions, SurprisingConnectionsOutput, SurprisingEdge,
};

// =============================================================================
// Algorithm imports from cognicode-graph-algos
// =============================================================================

use cognicode_graph_algos::{
    communities as inner_communities, community_god_nodes as inner_community_god_nodes,
    page_rank as inner_page_rank, surprising_connections as inner_surprising_connections,
};

// =============================================================================
// PageRank
// =============================================================================

/// PageRank — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `options_js`: `{ damping?: number, max_iterations?: number }`
///
/// # Returns
///
/// `{ scores: { [nodeId: string]: number } }`
#[wasm_bindgen_macro::wasm_bindgen]
pub fn pagerank(
    nodes_js: JsValue,
    edges_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsValue> {
    let graph: JsonGraph = from_js(nodes_js, edges_js).map_err(|e| JsValue::from(e.to_string()))?;
    let options: PageRankOptions =
        from_value(options_js).map_err(|e| JsValue::from(e.to_string()))?;

    let (in_neighbors, out_degree, _) = graph.build_adjacency();
    let n = graph.nodes.len();

    let raw_scores = inner_page_rank(
        &in_neighbors,
        &out_degree,
        n,
        options.damping,
        options.max_iterations,
    );

    let scores: std::collections::HashMap<String, f64> = raw_scores
        .into_iter()
        .filter_map(|(idx, score)| graph.nodes.get(idx).map(|n| (n.id.clone(), score)))
        .collect();

    let output = PageRankOutput { scores };
    to_value(&output).map_err(|e| JsValue::from(e.to_string()))
}

// =============================================================================
// God Nodes
// =============================================================================

/// God nodes — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `options_js`: `{ percentile?: number }` — defaults to 0.95
///
/// # Returns
///
/// `{ nodes: Array<{ id: string, score: number }> }`
#[wasm_bindgen_macro::wasm_bindgen]
pub fn god_nodes(
    nodes_js: JsValue,
    edges_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsValue> {
    let graph: JsonGraph = from_js(nodes_js, edges_js).map_err(|e| JsValue::from(e.to_string()))?;
    let options: GodNodesOptions =
        from_value(options_js).map_err(|e| JsValue::from(e.to_string()))?;

    let (in_neighbors, out_degree, _) = graph.build_adjacency();
    let n = graph.nodes.len();

    let raw_scores = inner_page_rank(&in_neighbors, &out_degree, n, 0.85, 100);
    let percentile = options.percentile.clamp(0.0, 1.0);

    // Compute god nodes: top percentile by PageRank score.
    let mut scored: Vec<(usize, f64)> = raw_scores.into_iter().collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let threshold_idx = ((scored.len() as f64) * percentile) as usize;
    let threshold_idx = threshold_idx.min(scored.len().saturating_sub(1));
    let threshold = scored.get(threshold_idx).map(|(_, s)| *s).unwrap_or(0.0);

    let nodes: Vec<GodNodeEntry> = scored
        .into_iter()
        .filter(|(_, score)| *score >= threshold)
        .filter_map(|(idx, score)| {
            let id = graph.nodes.get(idx)?.id.clone();
            Some(GodNodeEntry { id, score })
        })
        .collect();

    let output = GodNodesOutput { nodes };
    to_value(&output).map_err(|e| JsValue::from(e.to_string()))
}

// =============================================================================
// Communities
// =============================================================================

/// Communities (Label Propagation) — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `options_js`: `{ max_iterations?: number }` — defaults to 100
///
/// # Returns
///
/// `{ communities: Array<{ node_ids: string[] }> }`
#[wasm_bindgen_macro::wasm_bindgen]
pub fn communities(
    nodes_js: JsValue,
    edges_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsValue> {
    let graph: JsonGraph = from_js(nodes_js, edges_js).map_err(|e| JsValue::from(e.to_string()))?;
    let options: CommunitiesOptions =
        from_value(options_js).map_err(|e| JsValue::from(e.to_string()))?;

    let (in_neighbors, _, out_neighbors) = graph.build_adjacency();
    let n = graph.nodes.len();

    let raw = inner_communities(&in_neighbors, &out_neighbors, n, options.max_iterations);

    // Translate usize indices to node IDs.
    let communities: Vec<Community> = raw
        .into_iter()
        .map(|community| {
            let node_ids: Vec<String> = community
                .into_iter()
                .filter_map(|idx| graph.nodes.get(idx).map(|n| n.id.clone()))
                .collect();
            Community { node_ids }
        })
        .collect();

    let output = CommunitiesOutput { communities };
    to_value(&output).map_err(|e| JsValue::from(e.to_string()))
}

// =============================================================================
// Community God Nodes
// =============================================================================

/// Community god nodes — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `communities_js`: `Array<Array<string>>` — array of communities (each a list of node IDs)
/// - `options_js`: `{ percentile?: number }` — defaults to 0.95
///
/// # Returns
///
/// `{ nodes: Array<{ community_index: number, id: string, score: number }> }`
#[wasm_bindgen_macro::wasm_bindgen]
pub fn community_god_nodes(
    nodes_js: JsValue,
    edges_js: JsValue,
    communities_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsValue> {
    let graph: JsonGraph = from_js(nodes_js, edges_js).map_err(|e| JsValue::from(e.to_string()))?;
    let options: CommunityGodNodesOptions =
        from_value(options_js).map_err(|e| JsValue::from(e.to_string()))?;
    let community_node_ids: Vec<Vec<String>> =
        from_value(communities_js).map_err(|e| JsValue::from(e.to_string()))?;

    // Convert node-id-based communities to usize-based.
    let id_to_idx: std::collections::HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let usize_communities: Vec<Vec<usize>> = community_node_ids
        .iter()
        .map(|comm| {
            comm.iter()
                .filter_map(|id| id_to_idx.get(id.as_str()).copied())
                .collect()
        })
        .collect();

    // Compute PageRank scores.
    let (in_neighbors, out_degree, _) = graph.build_adjacency();
    let n = graph.nodes.len();
    let raw_scores = inner_page_rank(&in_neighbors, &out_degree, n, 0.85, 100);

    let raw_god_nodes =
        inner_community_god_nodes(&usize_communities, &raw_scores, options.percentile);

    // Translate to (community_index, node_id, score).
    let nodes_out: Vec<CommunityGodNode> = raw_god_nodes
        .into_iter()
        .filter_map(|(idx, score)| {
            // Find which community this node belongs to.
            let mut community_index = usize::MAX;
            for (ci, community) in usize_communities.iter().enumerate() {
                if community.contains(&idx) {
                    community_index = ci;
                    break;
                }
            }
            if community_index == usize::MAX {
                return None;
            }
            let id = graph.nodes.get(idx)?.id.clone();
            Some(CommunityGodNode {
                community_index,
                id,
                score,
            })
        })
        .collect();

    let output = CommunityGodNodesOutput { nodes: nodes_out };
    to_value(&output).map_err(|e| JsValue::from(e.to_string()))
}

// =============================================================================
// Surprising Connections
// =============================================================================

/// Surprising connections — wasm-bindgen export.
///
/// # Arguments
///
/// - `nodes_js`: `Array<{ id: string, label?: string }>`
/// - `edges_js`: `Array<{ source: string, target: string }>`
/// - `communities_js`: `Array<Array<string>>` — array of communities
/// - `options_js`: `{ limit?: number }` — defaults to 10
///
/// # Returns
///
/// `{ edges: Array<{ source_id: string, target_id: string, score: number }> }`
#[wasm_bindgen_macro::wasm_bindgen]
pub fn surprising_connections(
    nodes_js: JsValue,
    edges_js: JsValue,
    communities_js: JsValue,
    options_js: JsValue,
) -> Result<JsValue, JsValue> {
    let graph: JsonGraph = from_js(nodes_js, edges_js).map_err(|e| JsValue::from(e.to_string()))?;
    let options: SurprisingConnectionsOptions =
        from_value(options_js).map_err(|e| JsValue::from(e.to_string()))?;
    let community_node_ids: Vec<Vec<String>> =
        from_value(communities_js).map_err(|e| JsValue::from(e.to_string()))?;

    // Convert node-id-based communities to usize-based.
    let id_to_idx: std::collections::HashMap<&str, usize> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let usize_communities: Vec<Vec<usize>> = community_node_ids
        .iter()
        .map(|comm| {
            comm.iter()
                .filter_map(|id| id_to_idx.get(id.as_str()).copied())
                .collect()
        })
        .collect();

    // Build community_of: usize for each node.
    let n = graph.nodes.len();
    let mut community_of: Vec<usize> = vec![0; n];
    for (comm_idx, community) in usize_communities.iter().enumerate() {
        for &node_idx in community {
            if node_idx < n {
                community_of[node_idx] = comm_idx;
            }
        }
    }

    // Build adjacency lists for PageRank and surprising connections.
    let (in_neighbors, out_degree, out_neighbors) = graph.build_adjacency();

    // PageRank scores.
    let raw_scores = inner_page_rank(&in_neighbors, &out_degree, n, 0.85, 100);

    let raw_edges =
        inner_surprising_connections(&out_neighbors, &community_of, &raw_scores, options.limit);

    // Translate to (source_id, target_id, score).
    let edges_out: Vec<SurprisingEdge> = raw_edges
        .into_iter()
        .filter_map(|(s, t, score)| {
            let source_id = graph.nodes.get(s)?.id.clone();
            let target_id = graph.nodes.get(t)?.id.clone();
            Some(SurprisingEdge {
                source_id,
                target_id,
                score,
            })
        })
        .collect();

    let output = SurprisingConnectionsOutput { edges: edges_out };
    to_value(&output).map_err(|e| JsValue::from(e.to_string()))
}
