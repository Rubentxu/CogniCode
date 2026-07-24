//! Decision graph topology builder.
//!
//! Produces a typed graph representation for the DecisionGraph view, preserving
//! edge kind, provenance, and confidence from the rationale subgraph.
//!
//! This is the differentiated topology builder for DecisionGraph (Decision A),
//! distinct from the generic rationale subgraph used by ArchitectureRationale.

use crate::dto::{
    ContextualView, EvidenceBlock, FindingSeverity, LineRange, RelationDirection, TypedRelation,
    ViewBlock,
};
use crate::error::ExplorerResult;
use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::ports::graph_repository::GraphRepository;
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use serde_json::json;

/// Maximum traversal depth for decision graph BFS.
pub const DECISION_GRAPH_MAX_DEPTH: u32 = 3;

/// Maximum nodes to include in the decision graph.
pub const DECISION_GRAPH_MAX_NODES: usize = 100;

/// A typed edge in the decision graph topology.
#[derive(Debug, Clone)]
pub struct DecisionGraphEdge {
    pub source: NodeId,
    pub target: NodeId,
    pub edge_kind: EdgeKind,
    pub provenance: cognicode_core::domain::value_objects::Provenance,
    pub confidence: f64,
}

/// Decision graph topology result.
#[derive(Debug)]
pub struct DecisionGraphTopology {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<DecisionGraphEdge>,
    pub truncated: bool,
}

impl DecisionGraphTopology {
    /// Build the decision graph topology by traversing the rationale subgraph.
    ///
    /// Uses `rationale_subgraph` (BFS over Justifies, Cites, Resolves,
    /// CorroboratedBy) starting from the focus node, bounded by `max_depth`
    /// and `max_nodes`. Cycle-safe.
    pub async fn build(
        repo: &dyn GraphRepository,
        focus_id: &NodeId,
        max_depth: Option<u32>,
        max_nodes: Option<usize>,
    ) -> ExplorerResult<Self> {
        let depth = max_depth.unwrap_or(DECISION_GRAPH_MAX_DEPTH);
        let node_cap = max_nodes.unwrap_or(DECISION_GRAPH_MAX_NODES);

        let (nodes, raw_edges, truncated) = repo
            .rationale_subgraph(focus_id, depth, node_cap)
            .await
            .map_err(|e| crate::error::ExplorerError::NotFound(format!("rationale_subgraph: {e}")))?;

        let edges = raw_edges
            .into_iter()
            .map(|e| DecisionGraphEdge {
                source: e.source,
                target: e.target,
                edge_kind: e.kind,
                provenance: e.provenance,
                confidence: e.confidence,
            })
            .collect();

        Ok(Self {
            nodes,
            edges,
            truncated,
        })
    }
}

/// Build the typed relations for the decision graph.
///
/// Preserves edge kind, provenance, and confidence as per the topology contract.
pub fn build_decision_graph_relations(
    topology: &DecisionGraphTopology,
    focus_id: &NodeId,
    evidence_id: &str,
) -> Vec<TypedRelation> {
    topology
        .edges
        .iter()
        .map(|e| {
            let (target_id, direction) = if e.source == *focus_id {
                (e.target.to_string(), RelationDirection::Outgoing)
            } else {
                (e.source.to_string(), RelationDirection::Incoming)
            };
            TypedRelation {
                relation_type: format!("{:?}", e.edge_kind),
                direction,
                target_object_id: target_id,
                target_label: "related node".to_string(),
                evidence_ids: vec![evidence_id.to_string()],
                provenance: Some(e.provenance.to_string()),
                confidence: Some(e.confidence),
            }
        })
        .collect()
}

/// Build the graph block for the decision graph view.
///
/// Shapes the topology data for the `GraphViewRenderer` frontend component.
pub fn build_decision_graph_block(topology: &DecisionGraphTopology, focus_id: &NodeId) -> ViewBlock {
    let nodes_json: Vec<serde_json::Value> = topology
        .nodes
        .iter()
        .map(|n| {
            json!({
                "id": n.id.to_string(),
                "label": n.label,
                "kind": format!("{:?}", n.kind),
                "is_focus": n.id == *focus_id,
            })
        })
        .collect();

    let edges_json: Vec<serde_json::Value> = topology
        .edges
        .iter()
        .map(|e| {
            json!({
                "source": e.source.to_string(),
                "target": e.target.to_string(),
                "kind": format!("{:?}", e.edge_kind),
                "confidence": e.confidence,
            })
        })
        .collect();

    ViewBlock {
        id: "decision_graph_topology".into(),
        title: format!(
            "Decision neighbourhood ({}{})",
            topology.nodes.len(),
            if topology.truncated { "+" } else { "" }
        ),
        body: json!({
            "focus_id": focus_id.to_string(),
            "total_nodes": topology.nodes.len(),
            "truncated": topology.truncated,
            "nodes": nodes_json,
            "edges": edges_json,
        }),
    }
}

/// Build the decision graph evidence block.
pub fn build_decision_graph_evidence(
    focus_node: &GraphNode,
    evidence_id: &str,
) -> Vec<EvidenceBlock> {
    vec![EvidenceBlock {
        id: evidence_id.to_string(),
        kind: "decision_graph".into(),
        title: format!("Decision Graph: {}", focus_node.label),
        file: focus_node.source_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        line_range: None,
        source_tool_or_query: "GraphRepository::rationale_subgraph".into(),
        confidence: Some(1.0),
        freshness: Some("unknown".into()),
        provenance: None,
    }]
}

/// Assemble a full DecisionGraph ContextualView from a resolved focus node and topology.
pub fn assemble_decision_graph_view(
    focus_node: &GraphNode,
    topology: DecisionGraphTopology,
) -> ContextualView {
    let evidence_id = "evidence:decision_graph".to_string();
    let relations = build_decision_graph_relations(&topology, &focus_node.id, &evidence_id);
    let graph_block = build_decision_graph_block(&topology, &focus_node.id);
    let evidence = build_decision_graph_evidence(focus_node, &evidence_id);

    let blocks = vec![
        ViewBlock {
            id: "decision_identity".into(),
            title: "Decision".into(),
            body: json!({
                "id": focus_node.id.to_string(),
                "label": focus_node.label,
                "kind": format!("{:?}", focus_node.kind),
                "properties": focus_node.properties,
            }),
        },
        graph_block,
        ViewBlock {
            id: "edges_summary".into(),
            title: format!("Connections ({})", topology.edges.len()),
            body: json!({
                "count": topology.edges.len(),
                "edge_kinds": topology
                    .edges
                    .iter()
                    .map(|e| format!("{:?}", e.edge_kind))
                    .collect::<Vec<_>>(),
            }),
        },
    ];

    ContextualView {
        object_id: format!("decision:{}", focus_node.id),
        view_id: "decision_graph".into(),
        title: format!("Decision Graph: {}", focus_node.label),
        view_kind: crate::dto::ViewKind::DecisionGraph,
        blocks,
        relations,
        evidence,
        findings: Vec::new(),
        renderer_kind: crate::dto::RendererKind::Graph,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_decision_node(id: &str, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id.to_string()),
            kind: NodeKind::Decision,
            label: label.to_string(),
            source_path: Some(std::path::PathBuf::from(format!("docs/adr/{}.md", id))),
            properties: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn make_doc_node(id: &str, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id.to_string()),
            kind: NodeKind::Doc,
            label: label.to_string(),
            source_path: None,
            properties: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn decision_graph_topology_edge_preserves_kind_and_confidence() {
        // Verify DecisionGraphEdge preserves edge kind, provenance, and confidence
        let edge = DecisionGraphEdge {
            source: NodeId::new("ADR-001".to_string()),
            target: NodeId::new("doc-1".to_string()),
            edge_kind: EdgeKind::Justifies,
            provenance: cognicode_core::domain::value_objects::Provenance::Extracted,
            confidence: 0.95,
        };

        assert!(matches!(edge.edge_kind, EdgeKind::Justifies));
        assert!(matches!(
            edge.provenance,
            cognicode_core::domain::value_objects::Provenance::Extracted
        ));
        assert!((edge.confidence - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn decision_graph_relations_preserve_edge_metadata() {
        let topology = DecisionGraphTopology {
            nodes: vec![
                make_decision_node("ADR-001", "Use PostgreSQL"),
                make_doc_node("doc-1", "Database Comparison"),
            ],
            edges: vec![DecisionGraphEdge {
                source: NodeId::new("ADR-001".to_string()),
                target: NodeId::new("doc-1".to_string()),
                edge_kind: EdgeKind::Justifies,
                provenance: cognicode_core::domain::value_objects::Provenance::Extracted,
                confidence: 0.88,
            }],
            truncated: false,
        };

        let focus_id = NodeId::new("ADR-001".to_string());
        let evidence_id = "evidence:test";
        let relations = build_decision_graph_relations(&topology, &focus_id, evidence_id);

        assert_eq!(relations.len(), 1);
        let rel = &relations[0];
        assert_eq!(rel.relation_type, "Justifies");
        assert_eq!(rel.direction, RelationDirection::Outgoing);
        assert_eq!(rel.provenance, Some("Extracted".to_string()));
        assert!((rel.confidence.unwrap() - 0.88).abs() < f64::EPSILON);
    }

    #[test]
    fn decision_graph_block_contains_topology_data() {
        let topology = DecisionGraphTopology {
            nodes: vec![
                make_decision_node("ADR-001", "Use PostgreSQL"),
                make_doc_node("doc-1", "Database Comparison"),
            ],
            edges: vec![DecisionGraphEdge {
                source: NodeId::new("ADR-001".to_string()),
                target: NodeId::new("doc-1".to_string()),
                edge_kind: EdgeKind::Justifies,
                provenance: cognicode_core::domain::value_objects::Provenance::Inferred,
                confidence: 0.75,
            }],
            truncated: false,
        };

        let focus_id = NodeId::new("ADR-001".to_string());
        let block = build_decision_graph_block(&topology, &focus_id);

        assert_eq!(block.id, "decision_graph_topology");
        let body = &block.body;
        assert_eq!(body.get("total_nodes").and_then(|v| v.as_i64()), Some(2));
        assert_eq!(body.get("truncated").and_then(|v| v.as_bool()), Some(false));
        assert_eq!(
            body.get("focus_id").and_then(|v| v.as_str()),
            Some("ADR-001")
        );
    }
}
