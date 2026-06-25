//! Integration tests for the landing-page truncation contract introduced
//! in cycle `e8b-landing-payload-truncation` (v0.24.2).
//!
//! TDD contract: every test here is RED before the matching GREEN
//! implementation lands. The tests document:
//! - The pure `apply_landing_cap` helper's boundary behaviour.
//! - The `LandingPayload` DTO serde round-trip with the new
//!   `truncated` and `truncated_reason` fields.
//!
//! Mirrors the spec at
//! `openspec/changes/e8b-landing-payload-truncation/specs/graphlanding-affordances/spec.md`.

use cognicode_explorer::api::apply_landing_cap;
use cognicode_explorer::dto::{
    GodNodeEntry, GraphNode, InspectableObjectSummary, LandingPayload, WorkspaceSummary,
};

// ============================================================================
// apply_landing_cap — pure helper, single source of truth for truncation
// ============================================================================

#[test]
fn apply_landing_cap_zero_returns_no_truncation() {
    let (truncated, reason) = apply_landing_cap(0);
    assert!(!truncated);
    assert!(reason.is_none());
}

#[test]
fn apply_landing_cap_under_cap_returns_no_truncation() {
    // LANDING_NODE_CAP = 50; total = 49 is under cap.
    let (truncated, reason) = apply_landing_cap(49);
    assert!(!truncated);
    assert!(reason.is_none());
}

#[test]
fn apply_landing_cap_at_cap_returns_no_truncation() {
    // Boundary: at cap is NOT over cap.
    let (truncated, reason) = apply_landing_cap(50);
    assert!(!truncated);
    assert!(reason.is_none());
}

#[test]
fn apply_landing_cap_over_cap_returns_node_cap_truncation() {
    // First value strictly greater than the cap.
    let (truncated, reason) = apply_landing_cap(51);
    assert!(truncated);
    assert_eq!(reason.as_deref(), Some("node_cap"));
}

#[test]
fn apply_landing_cap_large_value_still_node_cap() {
    // Sanity: the reason is always "node_cap" — not a different reason for
    // "way over cap" vs "slightly over cap".
    let (truncated, reason) = apply_landing_cap(1000);
    assert!(truncated);
    assert_eq!(reason.as_deref(), Some("node_cap"));
}

// ============================================================================
// LandingPayload DTO serde — round-trip with new truncation fields
// ============================================================================

fn empty_landing_payload() -> LandingPayload {
    LandingPayload {
        workspace: WorkspaceSummary {
            id: "ws-1".into(),
            root_path: "/tmp/ws-1".into(),
            graph_status: cognicode_explorer::dto::GraphStatus::Missing,
            indexed_at: None,
            symbol_count: 0,
            relation_count: 0,
        },
        nodes: Vec::<GraphNode>::new(),
        edges: Vec::new(),
        entry_points: Vec::<InspectableObjectSummary>::new(),
        hot_paths: Vec::new(),
        god_nodes: Vec::<GodNodeEntry>::new(),
        suggested_questions: Vec::new(),
        graph_status: cognicode_explorer::dto::GraphStatus::Missing,
        truncated: false,
        truncated_reason: None,
    }
}

#[test]
fn landing_payload_serializes_with_truncated_false() {
    let payload = empty_landing_payload();
    let json = serde_json::to_value(&payload).expect("serialize LandingPayload");
    assert_eq!(json["truncated"], serde_json::Value::Bool(false));
    assert_eq!(
        json["truncated_reason"],
        serde_json::Value::Null,
        "truncated_reason must serialise as JSON null when None"
    );
}

#[test]
fn landing_payload_serializes_with_truncated_true_and_reason() {
    let mut payload = empty_landing_payload();
    payload.truncated = true;
    payload.truncated_reason = Some("node_cap".to_string());
    let json = serde_json::to_value(&payload).expect("serialize LandingPayload");
    assert_eq!(json["truncated"], serde_json::Value::Bool(true));
    assert_eq!(json["truncated_reason"], serde_json::Value::String("node_cap".into()));
}

#[test]
fn landing_payload_deserializes_with_truncated_true_and_reason() {
    let json = serde_json::json!({
        "workspace": {
            "id": "ws-1",
            "root_path": "/tmp/ws-1",
            "graph_status": "ready",
            "indexed_at": null,
            "symbol_count": 0,
            "relation_count": 0,
        },
        "nodes": [],
        "edges": [],
        "entry_points": [],
        "hot_paths": [],
        "god_nodes": [],
        "suggested_questions": [],
        "graph_status": "ready",
        "truncated": true,
        "truncated_reason": "node_cap",
    });
    let payload: LandingPayload =
        serde_json::from_value(json).expect("deserialize LandingPayload with truncation");
    assert!(payload.truncated);
    assert_eq!(payload.truncated_reason.as_deref(), Some("node_cap"));
}

#[test]
fn landing_payload_deserializes_with_truncated_false_and_null_reason() {
    let json = serde_json::json!({
        "workspace": {
            "id": "ws-1",
            "root_path": "/tmp/ws-1",
            "graph_status": "missing",
            "indexed_at": null,
            "symbol_count": 0,
            "relation_count": 0,
        },
        "nodes": [],
        "edges": [],
        "entry_points": [],
        "hot_paths": [],
        "god_nodes": [],
        "suggested_questions": [],
        "graph_status": "missing",
        "truncated": false,
        "truncated_reason": null,
    });
    let payload: LandingPayload =
        serde_json::from_value(json).expect("deserialize LandingPayload empty stub");
    assert!(!payload.truncated);
    assert!(payload.truncated_reason.is_none());
}
