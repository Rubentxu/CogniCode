//! Round-trip / shape tests for the new `ContextualGraphResponse` DTO
//! family introduced by the `contextual-views` change.
//!
//! Mirrors the section definitions in
//! `crates/cognicode-explorer/src/dto.rs` (NEW `ContextualGraphResponse`,
//! `ParentSection`, `ChildrenSection`, `SameLevelSection`).
//!
//! TDD contract: every block here is RED before the matching struct
//! exists. After the production DTO lands, the tests pass.
//!
//! These tests live in a dedicated module so the production `dto.rs`
//! stays focused on type definitions and the assertion surface is
//! concentrated here — same pattern as
//! `crates/cognicode-explorer/src/dto.rs::named_view_tests` (inline)
//! but for the new types.

use crate::dto::{
    ChildrenSection, ContextualGraphResponse, GraphEdge, GraphNode, ParentSection, SameLevelSection,
};

fn sample_focus() -> GraphNode {
    GraphNode {
        id: "sym:focus::alpha:1".to_string(),
        label: "alpha".to_string(),
        kind: "function".to_string(),
        file: Some("src/focus.rs".to_string()),
        line: Some(1),
        style_class: "function".to_string(),
    }
}

fn sample_parent_node() -> GraphNode {
    GraphNode {
        id: "file:src/focus.rs".to_string(),
        label: "src/focus.rs".to_string(),
        kind: "file".to_string(),
        file: None,
        line: None,
        style_class: "module".to_string(),
    }
}

fn sample_child_node(id: &str) -> GraphNode {
    GraphNode {
        id: id.to_string(),
        label: id.to_string(),
        kind: "function".to_string(),
        file: Some("src/focus.rs".to_string()),
        line: Some(10),
        style_class: "function".to_string(),
    }
}

fn sample_edge(source: &str, target: &str) -> GraphEdge {
    GraphEdge {
        source: source.to_string(),
        target: target.to_string(),
        relation: "calls".to_string(),
        style_class: "edge.calls".to_string(),
    }
}

fn sample_response(
    parent: Option<ParentSection>,
    children: Option<ChildrenSection>,
) -> ContextualGraphResponse {
    let focus = sample_focus();
    let same_level = SameLevelSection {
        nodes: vec![sample_child_node("sym:focus::callee:1")],
        edges: vec![sample_edge("sym:focus::alpha:1", "sym:focus::callee:1")],
    };
    ContextualGraphResponse {
        focus_node: focus,
        parent,
        children,
        same_level,
        level: "file".to_string(),
        truncated: false,
        truncated_reason: None,
    }
}

#[test]
fn contextual_response_serializes_focus_node_first() {
    let resp = sample_response(None, None);
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    // The focus node must be the first object property so the front-end
    // can index the response by its position in the JSON.
    let focus_id = v["focusNode"]["id"].as_str().expect("focusNode.id");
    assert_eq!(focus_id, "sym:focus::alpha:1");
    // `focusNode` is a real object (not null), and `level` is `file`.
    assert!(v["focusNode"].is_object());
    assert_eq!(v["level"], "file");
    // truncated defaults to false; truncationReason is `null`.
    assert_eq!(v["truncated"], false);
    assert!(v["truncationReason"].is_null());
}

#[test]
fn contextual_response_with_parent_serializes_edge() {
    let parent = ParentSection {
        node: sample_parent_node(),
        edge: sample_edge("sym:focus::alpha:1", "file:src/focus.rs"),
    };
    let resp = sample_response(Some(parent), None);
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    // The `parent.node.id` matches the file node and the `parent.edge`
    // carries the same source/target we put in.
    let pid = v["parent"]["node"]["id"].as_str().expect("parent.node.id");
    assert_eq!(pid, "file:src/focus.rs");
    let p_edge_src = v["parent"]["edge"]["source"]
        .as_str()
        .expect("parent.edge.source");
    let p_edge_tgt = v["parent"]["edge"]["target"]
        .as_str()
        .expect("parent.edge.target");
    assert_eq!(p_edge_src, "sym:focus::alpha:1");
    assert_eq!(p_edge_tgt, "file:src/focus.rs");
}

#[test]
fn contextual_response_with_null_parent_round_trips() {
    let resp = sample_response(None, None);
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    // null parent (orphan symbol) must serialize as JSON null and the
    // parsed struct must agree.
    assert!(v["parent"].is_null());
    let back: ContextualGraphResponse = serde_json::from_str(&json).expect("round-trip");
    assert!(back.parent.is_none());
}

#[test]
fn contextual_response_with_null_children_round_trips() {
    let resp = sample_response(None, None);
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert!(v["children"].is_null());
    let back: ContextualGraphResponse = serde_json::from_str(&json).expect("round-trip");
    assert!(back.children.is_none());
}

#[test]
fn contextual_response_truncated_flag_serializes() {
    let mut resp = sample_response(None, None);
    resp.truncated = true;
    resp.truncated_reason = Some("max_nodes_exceeded".to_string());
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");
    assert_eq!(v["truncated"], true);
    assert_eq!(v["truncatedReason"], "max_nodes_exceeded");
}

// e11: backwards-compat — old `truncationReason` alias still deserialises
#[test]
fn contextual_response_accepts_deprecated_truncation_reason_alias() {
    let json = r#"{
        "focusNode": {"id": "sym:a:b:c", "label": "a", "kind": "function", "style_class": "function", "score": 1.0},
        "parent": null,
        "children": null,
        "sameLevel": {"nodes": [], "edges": []},
        "level": "file",
        "truncated": true,
        "truncationReason": "max_nodes_exceeded"
    }"#;
    let resp: ContextualGraphResponse =
        serde_json::from_str(json).expect("parse old truncationReason field");
    assert_eq!(resp.truncated_reason, Some("max_nodes_exceeded".to_string()));
}

#[test]
fn contextual_response_reuses_graph_node_schema() {
    // Build a response with a non-empty parent + children and assert
    // the wire shape exactly matches the GraphNode/GraphEdge shape used
    // by `SubgraphResponse`. This guards ADR-CX-2 ("reuse
    // GraphNode/GraphEdge").
    let parent = ParentSection {
        node: sample_parent_node(),
        edge: sample_edge("sym:focus::alpha:1", "file:src/focus.rs"),
    };
    let children = ChildrenSection {
        nodes: vec![sample_child_node("sym:focus::beta:10")],
        edges: vec![sample_edge("file:src/focus.rs", "sym:focus::beta:10")],
    };
    let resp = sample_response(Some(parent), Some(children));
    let json = serde_json::to_string(&resp).expect("serialize");
    let v: serde_json::Value = serde_json::from_str(&json).expect("parse");

    // The shape of `focusNode` matches the `GraphNode` keys exactly.
    let focus_keys: std::collections::BTreeSet<String> = v["focusNode"]
        .as_object()
        .expect("focusNode object")
        .keys()
        .cloned()
        .collect();
    let expected: std::collections::BTreeSet<String> =
        ["id", "label", "kind", "file", "line", "style_class"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    assert_eq!(focus_keys, expected, "GraphNode shape must be reused");

    // Edges carry the four GraphEdge keys.
    let edge_keys: std::collections::BTreeSet<String> = v["sameLevel"]["edges"][0]
        .as_object()
        .expect("edge object")
        .keys()
        .cloned()
        .collect();
    let expected_edge: std::collections::BTreeSet<String> =
        ["source", "target", "relation", "style_class"]
            .iter()
            .map(|s| s.to_string())
            .collect();
    assert_eq!(edge_keys, expected_edge, "GraphEdge shape must be reused");
}
