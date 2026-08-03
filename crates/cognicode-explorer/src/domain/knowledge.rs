//! Knowledge-layer read facades — pure projections from `GraphNode` to
//! `InspectableObjectSummary` for Doc, Decision, and Evidence families.
//!
//! These are NOT new ports. They are thin shaper functions that consume
//! `&dyn GraphRepository` and produce family-shaped summaries. No storage,
//! no cache, no second query surface.
//!
//! Design contract (D2): mirrors `build_rationale_view` in `domain/views.rs`.
//! Both are pure shapers over graph_repo. Deterministic: same `GraphNode`
//! → same `InspectableObjectSummary`.

use crate::dto::{InspectableObjectSummary, InspectableObjectType, Property};
use cognicode_core::domain::ports::GraphRepository;
use cognicode_core::domain::aggregates::generic_graph::GraphNode;

/// Generic projector for Doc/Decision/Evidence nodes.
///
/// - `graph`: GraphRepository to fetch the node
/// - `id`: Node ID to look up
/// - `kind_match`: returns `Some(label_prefix)` if node kind matches, `None` otherwise
/// - `mvp_prefix`: Prefix for the returned id (e.g., `"doc"`, `"decision"`)
/// - `obj_type`: The InspectableObjectType variant
/// - `subtitle_default`: Default subtitle when source_path is absent
/// - `extract_props`: extracts additional properties from the node into the vector
async fn project_generic<F>(
    graph: &dyn GraphRepository,
    id: &str,
    kind_match: impl FnOnce(&GraphNode) -> Option<&'static str>,
    mvp_prefix: &str,
    obj_type: InspectableObjectType,
    subtitle_default: &'static str,
    mut extract_props: F,
) -> Option<InspectableObjectSummary>
where
    F: FnMut(&GraphNode, &mut Vec<Property>),
{
    let node = graph.get_node(&id.into()).await.ok().flatten()?;
    let label_prefix = kind_match(&node)?;

    let label = if node.label.is_empty() {
        format!("{label_prefix} {}", id)
    } else {
        node.label.clone()
    };

    let subtitle = node
        .source_path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| subtitle_default.to_string());

    let mut properties = Vec::new();
    extract_props(&node, &mut properties);

    Some(InspectableObjectSummary {
        id: format!("{mvp_prefix}:{id}"),
        object_type: obj_type,
        label,
        subtitle,
        properties,
        available_views: Vec::new(),
    })
}

/// Project a `NodeKind::Doc` node into an `InspectableObjectSummary`.
///
/// Returns `None` if the node does not exist or is not a Doc node.
/// The returned summary carries the doc's `section` metadata field (if present)
/// as a `Property`.
pub async fn project_doc(
    graph: &dyn GraphRepository,
    id: &str,
) -> Option<InspectableObjectSummary> {
    let kind_match = |n: &GraphNode| {
        matches!(
            n.kind,
            cognicode_core::domain::value_objects::node_kind::NodeKind::Doc
        )
        .then_some("Document")
    };
    let extract_props = |n: &GraphNode, props: &mut Vec<Property>| {
        if let Some(section) = n.properties.get("section").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "section".into(),
                value: serde_json::Value::String(section.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
    };
    project_generic(
        graph,
        id,
        kind_match,
        "doc",
        InspectableObjectType::Doc,
        "Graph node",
        extract_props,
    )
    .await
}

/// Project a `NodeKind::Decision` node into an `InspectableObjectSummary`.
///
/// Returns `None` if the node does not exist or is not a Decision node.
/// The returned summary carries the ADR's `status` and `date` metadata fields
/// as `Property` entries.
pub async fn project_decision(
    graph: &dyn GraphRepository,
    id: &str,
) -> Option<InspectableObjectSummary> {
    let kind_match = |n: &GraphNode| {
        matches!(
            n.kind,
            cognicode_core::domain::value_objects::node_kind::NodeKind::Decision
        )
        .then_some("Decision")
    };
    let extract_props = |n: &GraphNode, props: &mut Vec<Property>| {
        if let Some(status) = n.properties.get("status").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "status".into(),
                value: serde_json::Value::String(status.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
        if let Some(date) = n.properties.get("date").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "date".into(),
                value: serde_json::Value::String(date.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
        if let Some(adr) = n.properties.get("adr_number").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "adr_number".into(),
                value: serde_json::Value::String(adr.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
    };
    project_generic(
        graph,
        id,
        kind_match,
        "decision",
        InspectableObjectType::DecisionArtifact,
        "Decision artifact",
        extract_props,
    )
    .await
}

/// Project a `NodeKind::Evidence` node into an `InspectableObjectSummary`.
///
/// Returns `None` if the node does not exist or is not an Evidence node.
/// The returned summary carries provenance fields (`source_tool`, `confidence`,
/// `freshness`) as `Property` entries.
pub async fn project_evidence(
    graph: &dyn GraphRepository,
    id: &str,
) -> Option<InspectableObjectSummary> {
    let kind_match = |n: &GraphNode| {
        matches!(
            n.kind,
            cognicode_core::domain::value_objects::node_kind::NodeKind::Evidence
        )
        .then_some("Evidence")
    };
    let extract_props = |n: &GraphNode, props: &mut Vec<Property>| {
        if let Some(tool) = n.properties.get("source_tool").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "source_tool".into(),
                value: serde_json::Value::String(tool.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
        if let Some(c) = n.properties.get("confidence") {
            // Accept both JSON number (0.85) and JSON string ("0.85")
            let parsed = c
                .as_f64()
                .or_else(|| c.as_str().and_then(|s| s.parse::<f64>().ok()));
            if let Some(parsed) = parsed {
                props.push(Property {
                    key: "confidence".into(),
                    value: serde_json::json!(parsed),
                    value_type: "number".into(),
                    source: "graph_nodes.metadata".into(),
                });
            }
        }
        if let Some(fresh) = n.properties.get("freshness").and_then(|v| v.as_str()) {
            props.push(Property {
                key: "freshness".into(),
                value: serde_json::Value::String(fresh.to_string()),
                value_type: "string".into(),
                source: "graph_nodes.metadata".into(),
            });
        }
    };
    project_generic(
        graph,
        id,
        kind_match,
        "evidence",
        InspectableObjectType::Evidence,
        "Evidence node",
        extract_props,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryGraphRepository;
    use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
    use cognicode_core::domain::value_objects::node_kind::NodeKind;
    use serde_json::Map;
    use std::collections::HashMap;

    fn make_doc_node(id: &str, label: &str, section: Option<&str>) -> GraphNode {
        let mut props = Map::new();
        if let Some(s) = section {
            props.insert(
                "section".to_string(),
                serde_json::Value::String(s.to_string()),
            );
        }
        GraphNode {
            id: NodeId(id.to_string()),
            kind: NodeKind::Doc,
            label: label.to_string(),
            source_path: Some(std::path::PathBuf::from("/docs/guide.md")),
            properties: serde_json::Value::Object(props),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_decision_node(
        id: &str,
        label: &str,
        status: Option<&str>,
        date: Option<&str>,
    ) -> GraphNode {
        let mut props = Map::new();
        if let Some(s) = status {
            props.insert(
                "status".to_string(),
                serde_json::Value::String(s.to_string()),
            );
        }
        if let Some(d) = date {
            props.insert("date".to_string(), serde_json::Value::String(d.to_string()));
        }
        props.insert(
            "adr_number".to_string(),
            serde_json::Value::String(id.to_string()),
        );
        GraphNode {
            id: NodeId(id.to_string()),
            kind: NodeKind::Decision,
            label: label.to_string(),
            source_path: Some(std::path::PathBuf::from("/docs/adr")),
            properties: serde_json::Value::Object(props),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_evidence_node(
        id: &str,
        label: &str,
        source_tool: Option<&str>,
        confidence: Option<&str>,
    ) -> GraphNode {
        let mut props = Map::new();
        if let Some(t) = source_tool {
            props.insert(
                "source_tool".to_string(),
                serde_json::Value::String(t.to_string()),
            );
        }
        if let Some(c) = confidence {
            props.insert(
                "confidence".to_string(),
                serde_json::Value::String(c.to_string()),
            );
        }
        GraphNode {
            id: NodeId(id.to_string()),
            kind: NodeKind::Evidence,
            label: label.to_string(),
            source_path: Some(std::path::PathBuf::from("/evidence/run1.json")),
            properties: serde_json::Value::Object(props),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    // -------------------------------------------------------------------------
    // Doc facade tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn project_doc_returns_summary_when_node_exists() {
        let node = make_doc_node("doc-001", "Getting Started Guide", Some("Introduction"));
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_doc(&repo, "doc-001").await;
        assert!(result.is_some());
        let summary = result.unwrap();
        assert_eq!(summary.id, "doc:doc-001");
        assert_eq!(summary.object_type, InspectableObjectType::Doc);
        assert_eq!(summary.label, "Getting Started Guide");
        assert!(summary.properties.iter().any(|p| p.key == "section"));
    }

    #[tokio::test]
    async fn project_doc_returns_none_for_unknown_id() {
        let repo = InMemoryGraphRepository::empty();
        let result = project_doc(&repo, "doc-999").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_doc_returns_none_for_non_doc_node() {
        let node = make_decision_node("adr-001", "Use PostgreSQL", Some("Accepted"), None);
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_doc(&repo, "adr-001").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_doc_is_deterministic() {
        let node = make_doc_node("doc-001", "Design Doc", Some("Architecture"));
        let repo = InMemoryGraphRepository::new(vec![node.clone()], Vec::new());

        let first = project_doc(&repo, "doc-001").await;
        let second = project_doc(&repo, "doc-001").await;
        assert!(first.is_some());
        assert!(second.is_some());
        // Same id and label on both invocations — deterministic output.
        assert_eq!(first.as_ref().unwrap().id, second.as_ref().unwrap().id);
        assert_eq!(
            first.as_ref().unwrap().label,
            second.as_ref().unwrap().label
        );
    }

    // -------------------------------------------------------------------------
    // Decision facade tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn project_decision_returns_summary_with_status_and_date() {
        let node = make_decision_node(
            "adr-009",
            "Use GraphRepository as knowledge port",
            Some("Accepted"),
            Some("2026-01-15"),
        );
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_decision(&repo, "adr-009").await;
        assert!(result.is_some());
        let summary = result.unwrap();
        assert_eq!(summary.id, "decision:adr-009");
        assert_eq!(summary.object_type, InspectableObjectType::DecisionArtifact);
        assert_eq!(summary.label, "Use GraphRepository as knowledge port");
        assert!(summary.properties.iter().any(|p| p.key == "status"));
        assert!(summary.properties.iter().any(|p| p.key == "date"));
    }

    #[tokio::test]
    async fn project_decision_returns_none_for_unknown_id() {
        let repo = InMemoryGraphRepository::empty();
        let result = project_decision(&repo, "adr-999").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_decision_returns_none_for_non_decision_node() {
        let node = make_doc_node("doc-001", "Guide", Some("Intro"));
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_decision(&repo, "doc-001").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_decision_is_deterministic() {
        let node = make_decision_node("adr-001", "Use Rust", Some("Proposed"), None);
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let first = project_decision(&repo, "adr-001").await;
        let second = project_decision(&repo, "adr-001").await;
        assert!(first.is_some());
        assert!(second.is_some());
        assert_eq!(first.as_ref().unwrap().id, second.as_ref().unwrap().id);
        assert_eq!(
            first.as_ref().unwrap().label,
            second.as_ref().unwrap().label
        );
    }

    // -------------------------------------------------------------------------
    // Evidence facade tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn project_evidence_returns_summary_with_provenance() {
        let node = make_evidence_node(
            "ev-001",
            "Benchmark: 2x throughput",
            Some("perf-bench"),
            Some("0.85"),
        );
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_evidence(&repo, "ev-001").await;
        assert!(result.is_some());
        let summary = result.unwrap();
        assert_eq!(summary.id, "evidence:ev-001");
        assert_eq!(summary.object_type, InspectableObjectType::Evidence);
        assert_eq!(summary.label, "Benchmark: 2x throughput");
        assert!(summary.properties.iter().any(|p| p.key == "source_tool"));
        assert!(summary.properties.iter().any(|p| p.key == "confidence"));
    }

    #[tokio::test]
    async fn project_evidence_returns_none_for_unknown_id() {
        let repo = InMemoryGraphRepository::empty();
        let result = project_evidence(&repo, "ev-999").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_evidence_returns_none_for_non_evidence_node() {
        let node = make_doc_node("doc-001", "Guide", Some("Intro"));
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let result = project_evidence(&repo, "doc-001").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn project_evidence_is_deterministic() {
        let node = make_evidence_node("ev-001", "Finding", Some("fuzzer"), Some("0.9"));
        let repo = InMemoryGraphRepository::new(vec![node], Vec::new());

        let first = project_evidence(&repo, "ev-001").await;
        let second = project_evidence(&repo, "ev-001").await;
        assert!(first.is_some());
        assert!(second.is_some());
        assert_eq!(first.as_ref().unwrap().id, second.as_ref().unwrap().id);
        assert_eq!(
            first.as_ref().unwrap().label,
            second.as_ref().unwrap().label
        );
    }
}
