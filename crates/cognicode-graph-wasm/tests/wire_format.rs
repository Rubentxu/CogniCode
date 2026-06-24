//! Wire-format tests for the WASM protocol types.
//!
//! These tests verify that the Rust protocol types round-trip correctly
//! through serde_json, which is the serialization format used for the
//! JsValue boundary.

use cognicode_graph_wasm::protocol::{
    CommunitiesOptions, CommunitiesOutput, Community, CommunityGodNode, CommunityGodNodesOptions,
    CommunityGodNodesOutput, GodNodeEntry, GodNodesOptions, GodNodesOutput, PageRankOptions,
    PageRankOutput, SurprisingConnectionsOptions, SurprisingConnectionsOutput, SurprisingEdge,
};

#[test]
fn pagerank_options_defaults() {
    let opts: PageRankOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.damping, 0.85);
    assert_eq!(opts.max_iterations, 100);
}

#[test]
fn pagerank_options_custom() {
    let opts: PageRankOptions =
        serde_json::from_str(r#"{ "damping": 0.7, "max_iterations": 50 }"#).unwrap();
    assert_eq!(opts.damping, 0.7);
    assert_eq!(opts.max_iterations, 50);
}

#[test]
fn pagerank_output_round_trip() {
    use std::collections::HashMap;
    let mut scores = HashMap::new();
    scores.insert("A".to_string(), 0.5);
    scores.insert("B".to_string(), 0.3);
    let output = PageRankOutput { scores };
    let json = serde_json::to_string(&output).unwrap();
    let parsed: PageRankOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.scores.get("A"), Some(&0.5));
    assert_eq!(parsed.scores.get("B"), Some(&0.3));
}

#[test]
fn god_nodes_options_default_percentile() {
    let opts: GodNodesOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.percentile, 0.95);
}

#[test]
fn god_nodes_options_clamped() {
    let opts: GodNodesOptions = serde_json::from_str(r#"{ "percentile": 0.99 }"#).unwrap();
    assert_eq!(opts.percentile, 0.99);
}

#[test]
fn god_nodes_output_round_trip() {
    let output = GodNodesOutput {
        nodes: vec![
            GodNodeEntry {
                id: "A".to_string(),
                score: 1.0,
            },
            GodNodeEntry {
                id: "B".to_string(),
                score: 0.5,
            },
        ],
    };
    let json = serde_json::to_string(&output).unwrap();
    assert_eq!(
        json,
        r#"{"nodes":[{"id":"A","score":1.0},{"id":"B","score":0.5}]}"#
    );
    let parsed: GodNodesOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.nodes.len(), 2);
    assert_eq!(parsed.nodes[0].id, "A");
    assert_eq!(parsed.nodes[1].score, 0.5);
}

#[test]
fn communities_options_default_max_iter() {
    let opts: CommunitiesOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.max_iterations, 100);
}

#[test]
fn community_round_trip() {
    let community = Community {
        node_ids: vec!["A".to_string(), "B".to_string()],
    };
    let json_str = serde_json::to_string(&community).unwrap();
    assert_eq!(json_str, r#"{"node_ids":["A","B"]}"#);
    let parsed: Community = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.node_ids, vec!["A", "B"]);
}

#[test]
fn communities_output_round_trip() {
    let output = CommunitiesOutput {
        communities: vec![
            Community {
                node_ids: vec!["A".to_string(), "B".to_string()],
            },
            Community {
                node_ids: vec!["C".to_string()],
            },
        ],
    };
    let json = serde_json::to_string(&output).unwrap();
    let parsed: CommunitiesOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.communities.len(), 2);
    assert_eq!(parsed.communities[0].node_ids, vec!["A", "B"]);
    assert_eq!(parsed.communities[1].node_ids, vec!["C"]);
}

#[test]
fn community_god_nodes_options_default_percentile() {
    let opts: CommunityGodNodesOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.percentile, 0.95);
}

#[test]
fn community_god_node_round_trip() {
    let node = CommunityGodNode {
        community_index: 0,
        id: "A".to_string(),
        score: 0.75,
    };
    let json_str = serde_json::to_string(&node).unwrap();
    assert_eq!(json_str, r#"{"community_index":0,"id":"A","score":0.75}"#);
    let parsed: CommunityGodNode = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.community_index, 0);
    assert_eq!(parsed.id, "A");
    assert!((parsed.score - 0.75).abs() < 1e-9);
}

#[test]
fn community_god_nodes_output_round_trip() {
    let output = CommunityGodNodesOutput {
        nodes: vec![
            CommunityGodNode {
                community_index: 0,
                id: "A".to_string(),
                score: 1.0,
            },
            CommunityGodNode {
                community_index: 1,
                id: "C".to_string(),
                score: 0.8,
            },
        ],
    };
    let json = serde_json::to_string(&output).unwrap();
    let parsed: CommunityGodNodesOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.nodes.len(), 2);
    assert_eq!(parsed.nodes[0].community_index, 0);
    assert_eq!(parsed.nodes[1].community_index, 1);
}

#[test]
fn surprising_connections_options_default_limit() {
    let opts: SurprisingConnectionsOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.limit, 10);
}

#[test]
fn surprising_edge_round_trip() {
    let edge = SurprisingEdge {
        source_id: "X".to_string(),
        target_id: "Y".to_string(),
        score: 0.75,
    };
    let json_str = serde_json::to_string(&edge).unwrap();
    assert_eq!(
        json_str,
        r#"{"source_id":"X","target_id":"Y","score":0.75}"#
    );
    let parsed: SurprisingEdge = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.source_id, "X");
    assert_eq!(parsed.target_id, "Y");
    assert!((parsed.score - 0.75).abs() < 1e-9);
}

#[test]
fn surprising_connections_output_round_trip() {
    let output = SurprisingConnectionsOutput {
        edges: vec![
            SurprisingEdge {
                source_id: "A".to_string(),
                target_id: "B".to_string(),
                score: 0.5,
            },
            SurprisingEdge {
                source_id: "C".to_string(),
                target_id: "D".to_string(),
                score: 0.3,
            },
        ],
    };
    let json = serde_json::to_string(&output).unwrap();
    let parsed: SurprisingConnectionsOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.edges.len(), 2);
    assert_eq!(parsed.edges[0].source_id, "A");
    assert_eq!(parsed.edges[1].target_id, "D");
}
