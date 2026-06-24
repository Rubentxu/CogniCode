//! Wire format tests — verify that the JSON protocol structs serialize
//! and deserialize identically between Rust and the expected JS shape.
//!
//! These tests run on native only (the crate's wasm32 build is for the
//! browser). They protect the contract that `apps/explorer-ui` will
//! depend on: if any field is renamed or restructured, these tests
//! fail before the JS side breaks.

use cognicode_graph_wasm::protocol::*;

#[test]
fn page_rank_output_round_trips() {
    let output = PageRankOutput {
        scores: std::collections::BTreeMap::from([
            ("A".to_string(), 0.4),
            ("B".to_string(), 0.3),
            ("C".to_string(), 0.3),
        ]),
    };
    let json = serde_json::to_string(&output).unwrap();
    // Verify deterministic ordering — BTreeMap guarantees alphabetical.
    assert!(json.contains(r#""scores":{"A":0.4,"B":0.3,"C":0.3}"#));

    let parsed: PageRankOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.scores.get("A"), Some(&0.4));
}

#[test]
fn god_node_entry_omits_label() {
    // Spec REQ-013: label is included iff input node had a label.
    // The output entry (GodNodeEntry) only carries id+score, not label.
    let entry = GodNodeEntry {
        id: "X".to_string(),
        score: 0.95,
    };
    let json = serde_json::to_string(&entry).unwrap();
    assert_eq!(json, r#"{"id":"X","score":0.95}"#);
}

#[test]
fn page_rank_options_default_values() {
    let opts: PageRankOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.damping, 0.85);
    assert_eq!(opts.max_iterations, 100);
}

#[test]
fn god_nodes_options_default_percentile() {
    let opts: GodNodesOptions = serde_json::from_str("{}").unwrap();
    assert_eq!(opts.percentile, 0.95);
}

#[test]
fn page_rank_options_explicit_values() {
    let opts: PageRankOptions =
        serde_json::from_str(r#"{"damping":0.7,"max_iterations":50}"#).unwrap();
    assert_eq!(opts.damping, 0.7);
    assert_eq!(opts.max_iterations, 50);
}

#[test]
fn god_nodes_options_explicit_percentile() {
    let opts: GodNodesOptions = serde_json::from_str(r#"{"percentile":0.99}"#).unwrap();
    assert_eq!(opts.percentile, 0.99);
}

#[test]
fn god_nodes_output_round_trips() {
    let output = GodNodesOutput {
        nodes: vec![
            GodNodeEntry {
                id: "C".to_string(),
                score: 0.5,
            },
            GodNodeEntry {
                id: "A".to_string(),
                score: 0.9,
            },
            GodNodeEntry {
                id: "B".to_string(),
                score: 0.7,
            },
        ],
    };
    let json = serde_json::to_string(&output).unwrap();
    let parsed: GodNodesOutput = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.nodes.len(), 3);
    // Verify ordering is preserved through round-trip.
    assert_eq!(parsed.nodes[0].id, "C");
    assert_eq!(parsed.nodes[1].id, "A");
    assert_eq!(parsed.nodes[2].id, "B");
}

#[test]
fn json_node_with_label() {
    let node = JsonNode {
        id: "fn:foo".to_string(),
        label: Some("foo()".to_string()),
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains(r#""label":"foo()""#));
}

#[test]
fn json_node_without_label() {
    let node = JsonNode {
        id: "fn:foo".to_string(),
        label: None,
    };
    let json = serde_json::to_string(&node).unwrap();
    // label serializes as null when None (serde default behavior for Option<T>)
    assert_eq!(json, r#"{"id":"fn:foo","label":null}"#);
}
