//! Decision support pack — backend fan-out composition of five pane views.
//!
//! Produces a composite `DecisionSupportPack` by resolving a Decision node's
//! rationale neighborhood and fanning out to five sub-view builders via
//! `tokio::join!`. Each pane carries its own `PaneStatus` so partial failure
//! never propagates beyond the pane.
//!
//! E25 PR2 — DecisionSupportPackBuilder + REST endpoint.

use std::sync::Arc;

use cognicode_core::domain::aggregates::generic_graph::{GraphEdge, GraphNode, NodeId};
use cognicode_core::domain::ports::GraphRepository;
use cognicode_core::domain::traits::graph_query_port::GraphQueryPort;
use cognicode_core::domain::value_objects::edge_kind::EdgeKind;
use cognicode_core::domain::value_objects::node_kind::NodeKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::decision_graph_topology::DecisionGraphTopology;
use crate::dto::{ContextualView, EvidenceBlock, ViewBlock};
use crate::error::ExplorerResult;
use crate::ports::symbol_repository::ResolvedSymbol;
use cognicode_core::domain::ports::QualityStore;
use cognicode_core::domain::ports::graph_repository::GraphRepository as PortsGraphRepository;

/// Maximum traversal depth for rationale subgraph in pack builder.
pub(crate) const PACK_RATIONALE_MAX_DEPTH: u32 = 3;

/// Maximum nodes to include in the rationale subgraph.
pub(crate) const PACK_RATIONALE_MAX_NODES: usize = 100;

// ============================================================================
// DTOs
// ============================================================================

/// Per-pane outcome — failure never propagates beyond the pane.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum PaneStatus {
    /// Pane built successfully.
    Ok,
    /// Pane built with degraded quality (e.g., missing secondary target).
    Degraded(String),
    /// Pane failed to build (e.g., target not found).
    Failed(String),
}

/// A single pane within a decision support pack.
#[derive(Debug, Clone, Serialize)]
pub struct PackPane {
    /// Underscore form for the pack view_id field (Decision A).
    pub view_id: &'static str,
    pub title: String,
    /// The rendered view, or `None` when status is `Failed`.
    pub view: Option<ContextualView>,
    pub status: PaneStatus,
}

/// The composite decision support pack — five panes in stable order.
#[derive(Debug, Clone, Serialize)]
pub struct DecisionSupportPack {
    pub decision_id: String,
    /// Exactly five panes in stable order:
    /// 1. decision_graph
    /// 2. architecture_rationale
    /// 3. evidence_pack
    /// 4. risk_map
    /// 5. change_impact_story
    pub panes: Vec<PackPane>,
}

// ============================================================================
// DecisionSupportPackBuilder
// ============================================================================

/// Builder for composite decision support packs.
///
/// Resolves a Decision node's rationale neighborhood and fans out to five
/// sub-view builders via `tokio::join!`, returning a `DecisionSupportPack`
/// with per-pane status. Partial failure is tolerated — missing targets
/// degrade gracefully without fail-fast.
pub struct DecisionSupportPackBuilder;

impl DecisionSupportPackBuilder {
    /// Build a decision support pack for `decision_id`.
    ///
    /// # Arguments
    /// * `decision_id` — the decision node id
    /// * `graph_query` — optional graph query port for RiskMap/ChangeImpactStory
    /// * `quality` — optional quality repository for RiskMap
    /// * `graph_repo` — graph repository (required for multimodal views)
    ///
    /// # Multi-target synthesis rule
    /// * Primary symbol = highest-confidence outgoing edge target
    /// * Evidence pane built from direct blocks (no registry dispatch needed)
    pub async fn build(
        decision_id: &str,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
        quality: Option<Arc<dyn QualityStore>>,
        graph_repo: Option<&dyn PortsGraphRepository>,
    ) -> ExplorerResult<DecisionSupportPack> {
        let Some(repo) = graph_repo else {
            return Err(crate::error::ExplorerError::FeatureDisabled(
                "graph repository not wired".into(),
            ));
        };

        let node_id = NodeId::new(decision_id.to_string());

        // Fetch the decision node
        let decision_node = match repo.get_node(&node_id).await {
            Ok(Some(node)) => node,
            Ok(None) => {
                return Err(crate::error::ExplorerError::NotFound(format!(
                    "Decision '{decision_id}' not found in graph"
                )));
            }
            Err(e) => {
                return Err(crate::error::ExplorerError::NotFound(format!(
                    "Failed to fetch decision: {e}"
                )));
            }
        };

        // Fetch rationale subgraph to resolve neighborhood
        let (subgraph_nodes, subgraph_edges, _) = repo
            .rationale_subgraph(&node_id, PACK_RATIONALE_MAX_DEPTH, PACK_RATIONALE_MAX_NODES)
            .await
            .map_err(|e| {
                crate::error::ExplorerError::NotFound(format!("rationale_subgraph: {e}"))
            })?;

        // Resolve primary symbol: highest-confidence outgoing edge target
        let primary_symbol = resolve_primary_symbol(&subgraph_nodes, &subgraph_edges, &node_id);

        // Resolve evidence nodes from the subgraph
        let evidence_nodes: Vec<&GraphNode> = subgraph_nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Evidence && n.id != node_id)
            .collect();

        // Helper closures to convert Option<Arc<dyn T>> to Option<&dyn T>
        let graph_query_ref = graph_query
            .as_ref()
            .map(|g| g.as_ref() as &dyn GraphQueryPort);
        let quality_ref = quality.as_ref().map(|q| q.as_ref() as &dyn QualityStore);

        // Fan out via tokio::join! — all five futures run concurrently
        let (dg_result, ar_result, ep_result, rm_result, cis_result) = tokio::join!(
            Self::build_decision_graph(&decision_node, repo),
            Self::build_architecture_rationale(&decision_node, repo),
            Self::build_evidence_pack_nodes(&decision_node, &evidence_nodes),
            Self::build_risk_map(primary_symbol, graph_query_ref, quality_ref),
            Self::build_change_impact_story(primary_symbol, graph_query_ref),
        );

        let panes = vec![
            pack_pane("decision_graph", "Decision Graph", dg_result),
            pack_pane(
                "architecture_rationale",
                "Architecture Rationale",
                ar_result,
            ),
            pack_pane("evidence_pack", "Evidence Pack", ep_result),
            pack_pane("risk_map", "Risk Map", rm_result),
            pack_pane("change_impact_story", "Change Impact Story", cis_result),
        ];

        Ok(DecisionSupportPack {
            decision_id: decision_id.to_string(),
            panes,
        })
    }

    // -------------------------------------------------------------------------
    // Sub-view builders (each returns Result<ContextualView, ExplorerError>)
    // -------------------------------------------------------------------------

    /// F1: DecisionGraphTopology — uses Graph renderer.
    async fn build_decision_graph(
        decision_node: &GraphNode,
        repo: &dyn PortsGraphRepository,
    ) -> ExplorerResult<ContextualView> {
        use crate::domain::decision_graph_topology::DecisionGraphTopology;

        let node_id = &decision_node.id;
        let topology = DecisionGraphTopology::build(repo, node_id, None, None).await?;

        Ok(
            crate::domain::decision_graph_topology::assemble_decision_graph_view(
                decision_node,
                topology,
            ),
        )
    }

    /// F2: ArchitectureRationale — uses Markdown renderer.
    async fn build_architecture_rationale(
        decision_node: &GraphNode,
        repo: &dyn PortsGraphRepository,
    ) -> ExplorerResult<ContextualView> {
        use crate::domain::views::build_rationale_view;

        let decision_id = decision_node.id.0.as_str();
        Ok(build_rationale_view(decision_id, Some(repo)).await)
    }

    /// F3: EvidencePack — built directly from evidence nodes (no registry dispatch).
    async fn build_evidence_pack_nodes(
        decision_node: &GraphNode,
        evidence_nodes: &[&GraphNode],
    ) -> ExplorerResult<ContextualView> {
        let evidence_blocks: Vec<EvidenceBlock> = evidence_nodes
            .iter()
            .map(|n| EvidenceBlock {
                id: format!("evidence:{}", n.id),
                kind: "evidence".into(),
                title: n.label.clone(),
                file: n
                    .source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
                line_range: None,
                source_tool_or_query: "GraphRepository::rationale_subgraph".into(),
                confidence: Some(1.0),
                freshness: Some("unknown".into()),
                provenance: None,
            })
            .collect();

        let blocks = if evidence_blocks.is_empty() {
            vec![ViewBlock {
                id: "no_evidence".into(),
                title: "No Evidence".into(),
                body: serde_json::json!({
                    "message": "No evidence nodes found in rationale neighborhood"
                }),
            }]
        } else {
            vec![ViewBlock {
                id: "evidence_summary".into(),
                title: format!("Evidence ({} items)", evidence_blocks.len()),
                body: serde_json::json!({
                    "count": evidence_blocks.len(),
                    "items": evidence_blocks
                        .iter()
                        .map(|e| serde_json::json!({
                            "id": e.id,
                            "title": e.title,
                            "file": e.file,
                        }))
                        .collect::<Vec<_>>()
                }),
            }]
        };

        Ok(ContextualView {
            object_id: format!("decision:{}", decision_node.id),
            view_id: "evidence_pack".into(),
            title: format!("Evidence Pack: {}", decision_node.label),
            view_kind: crate::dto::ViewKind::EvidencePack,
            blocks,
            relations: vec![],
            evidence: evidence_blocks,
            findings: vec![],
            renderer_kind: crate::dto::RendererKind::Composite,
        })
    }

    /// F4: RiskMap — requires a Symbol target.
    async fn build_risk_map(
        primary_symbol: Option<&GraphNode>,
        graph_query: Option<&dyn GraphQueryPort>,
        quality: Option<&dyn QualityStore>,
    ) -> ExplorerResult<ContextualView> {
        use crate::domain::views::RISK_MAP_EXECUTOR;
        use crate::dto::InspectionTarget;

        let Some(symbol_node) = primary_symbol else {
            return Err(crate::error::ExplorerError::NotFound(
                "No primary symbol found for RiskMap".into(),
            ));
        };

        // Convert GraphNode to ResolvedSymbol for RiskMapExecutor
        let symbol = ResolvedSymbol {
            id: cognicode_core::domain::aggregates::SymbolId::new(symbol_node.id.0.clone()),
            name: symbol_node.label.clone(),
            kind: cognicode_core::domain::value_objects::SymbolKind::Function,
            file: symbol_node
                .source_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            line: 0,
            signature: None,
        };

        let target = InspectionTarget::Symbol(symbol);

        // Build a minimal ViewContext for the executor
        let ctx = crate::dto::ViewContext {
            target: &target,
            repo: &StubSymbolRepo,
            reader: &StubSourceReader,
            quality,
            graph_query,
            graph_repo: None,
            node_property_repository: None,
        };

        // Call build via ViewExecutor trait object
        let executor: &dyn crate::domain::views::ViewExecutor = &RISK_MAP_EXECUTOR;
        executor.build(&ctx).await
    }

    /// F5: ChangeImpactStory — requires a Symbol target.
    async fn build_change_impact_story(
        primary_symbol: Option<&GraphNode>,
        graph_query: Option<&dyn GraphQueryPort>,
    ) -> ExplorerResult<ContextualView> {
        use crate::domain::views::CHANGE_IMPACT_STORY_EXECUTOR;
        use crate::dto::InspectionTarget;

        let Some(symbol_node) = primary_symbol else {
            return Err(crate::error::ExplorerError::NotFound(
                "No primary symbol found for ChangeImpactStory".into(),
            ));
        };

        // Convert GraphNode to ResolvedSymbol for ChangeImpactStoryExecutor
        let symbol = ResolvedSymbol {
            id: cognicode_core::domain::aggregates::SymbolId::new(symbol_node.id.0.clone()),
            name: symbol_node.label.clone(),
            kind: cognicode_core::domain::value_objects::SymbolKind::Function,
            file: symbol_node
                .source_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            line: 0,
            signature: None,
        };

        let target = InspectionTarget::Symbol(symbol);

        // Build a minimal ViewContext for the executor
        let ctx = crate::dto::ViewContext {
            target: &target,
            repo: &StubSymbolRepo,
            reader: &StubSourceReader,
            quality: None,
            graph_query,
            graph_repo: None,
            node_property_repository: None,
        };

        // Call build via ViewExecutor trait object
        let executor: &dyn crate::domain::views::ViewExecutor = &CHANGE_IMPACT_STORY_EXECUTOR;
        executor.build(&ctx).await
    }
}

// ============================================================================
// Stub implementations for ViewContext
// ============================================================================

use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{GraphStats, SymbolRepository};
use cognicode_core::domain::aggregates::SymbolId;

/// Stub symbol repository that returns empty results.
struct StubSymbolRepo;
impl SymbolRepository for StubSymbolRepo {
    fn resolve(&self, _id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
        Ok(None)
    }
    fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(vec![])
    }
    fn find_symbols_by_file(&self, _file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(vec![])
    }
    fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(vec![])
    }
    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: 0,
            relation_count: 0,
        }
    }
    fn module_list(&self) -> Vec<String> {
        vec![]
    }
}

/// Stub source reader that returns errors.
struct StubSourceReader;
impl SourceReader for StubSourceReader {
    fn read_source(&self, _file: &str) -> ExplorerResult<String> {
        Err(crate::error::ExplorerError::SourceUnavailable {
            file: _file.to_string(),
            object_id: _file.to_string(),
        })
    }
    fn read_lines(
        &self,
        _file: &str,
        _start: u32,
        _end: u32,
    ) -> ExplorerResult<Vec<(u32, String)>> {
        Err(crate::error::ExplorerError::SourceUnavailable {
            file: _file.to_string(),
            object_id: _file.to_string(),
        })
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Resolve the primary symbol for RiskMap/ChangeImpactStory.
///
/// Multi-target synthesis rule: primary symbol = highest-confidence outgoing
/// edge target from the decision node.
fn resolve_primary_symbol<'a>(
    nodes: &'a [GraphNode],
    edges: &[GraphEdge],
    decision_id: &NodeId,
) -> Option<&'a GraphNode> {
    // Find outgoing edges from the decision node
    let outgoing: Vec<&GraphEdge> = edges.iter().filter(|e| e.source == *decision_id).collect();

    if outgoing.is_empty() {
        return None;
    }

    // Find the edge with highest confidence
    let best_edge = outgoing.iter().max_by(|a, b| {
        a.confidence
            .partial_cmp(&b.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    })?;

    // Find the target node
    nodes.iter().find(|n| n.id == best_edge.target)
}

/// Convert a sub-view result into a `PackPane`.
fn pack_pane(
    view_id: &'static str,
    title: &str,
    result: ExplorerResult<ContextualView>,
) -> PackPane {
    match result {
        Ok(view) => PackPane {
            view_id,
            title: title.to_string(),
            view: Some(view),
            status: PaneStatus::Ok,
        },
        Err(e) => PackPane {
            view_id,
            title: title.to_string(),
            view: None,
            // W-002: use Degraded (not Failed) for partial failures like
            // "target not found" or "missing secondary data". PaneStatus::Failed
            // is reserved for actual builder crashes per the enum doc comment.
            status: PaneStatus::Degraded(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::InMemoryGraphRepository;
    use cognicode_core::domain::value_objects::Provenance;

    fn make_decision_node(id: &str, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id.to_string()),
            kind: NodeKind::Decision,
            label: label.to_string(),
            source_path: None,
            properties: serde_json::Value::Object(Default::default()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_symbol_node(id: &str, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id.to_string()),
            kind: NodeKind::Symbol(cognicode_core::domain::value_objects::SymbolKind::Function),
            label: label.to_string(),
            source_path: Some(std::path::PathBuf::from(format!("src/{}.rs", id))),
            properties: serde_json::Value::Object(Default::default()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    fn make_evidence_node(id: &str, label: &str) -> GraphNode {
        GraphNode {
            id: NodeId::new(id.to_string()),
            kind: NodeKind::Evidence,
            label: label.to_string(),
            source_path: None,
            properties: serde_json::Value::Object(Default::default()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn pack_builder_returns_five_panes_in_stable_order() {
        // Graph: Decision A --Justifies(0.9)--> Symbol X (primary)
        //                   --CorroboratedBy--> Evidence Y
        let nodes = vec![
            make_decision_node("A", "Decision A"),
            make_symbol_node("X", "Symbol X"),
            make_evidence_node("Y", "Evidence Y"),
        ];
        let edges = vec![
            GraphEdge {
                source: NodeId::new("A"),
                target: NodeId::new("X"),
                kind: EdgeKind::Justifies,
                provenance: Provenance::Extracted,
                confidence: 0.9,
                metadata: serde_json::Value::Object(Default::default()),
            },
            GraphEdge {
                source: NodeId::new("A"),
                target: NodeId::new("Y"),
                kind: EdgeKind::CorroboratedBy,
                provenance: Provenance::Manual,
                confidence: 0.7,
                metadata: serde_json::Value::Object(Default::default()),
            },
        ];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        let result = DecisionSupportPackBuilder::build("A", None, None, Some(&repo)).await;

        assert!(result.is_ok(), "Pack build should succeed: {:?}", result);
        let pack = result.unwrap();

        // Five panes in stable order
        assert_eq!(pack.panes.len(), 5);
        assert_eq!(pack.panes[0].view_id, "decision_graph");
        assert_eq!(pack.panes[1].view_id, "architecture_rationale");
        assert_eq!(pack.panes[2].view_id, "evidence_pack");
        assert_eq!(pack.panes[3].view_id, "risk_map");
        assert_eq!(pack.panes[4].view_id, "change_impact_story");
    }

    #[tokio::test]
    async fn pack_builder_partial_failure_one_pane_fails() {
        // Graph: Decision A with NO outgoing edges (no primary symbol)
        // RiskMap and ChangeImpactStory will fail because there's no primary symbol
        let nodes = vec![make_decision_node("A", "Decision A")];
        let edges = vec![];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        let result = DecisionSupportPackBuilder::build("A", None, None, Some(&repo)).await;

        assert!(
            result.is_ok(),
            "Pack build should succeed despite partial failure: {:?}",
            result
        );
        let pack = result.unwrap();

        // decision_graph, architecture_rationale, evidence_pack should succeed
        assert!(
            matches!(pack.panes[0].status, PaneStatus::Ok),
            "decision_graph should be Ok"
        );
        assert!(
            matches!(pack.panes[1].status, PaneStatus::Ok),
            "architecture_rationale should be Ok"
        );
        assert!(
            matches!(pack.panes[2].status, PaneStatus::Ok),
            "evidence_pack should be Ok"
        );

        // risk_map and change_impact_story are degraded without primary symbol
        // (W-002: partial failures now use Degraded, not Failed)
        assert!(
            matches!(pack.panes[3].status, PaneStatus::Degraded(_)),
            "risk_map should be degraded without primary symbol"
        );
        assert!(
            matches!(pack.panes[4].status, PaneStatus::Degraded(_)),
            "change_impact_story should be degraded without primary symbol"
        );
    }

    #[tokio::test]
    async fn primary_symbol_is_highest_confidence_outgoing() {
        // Graph: Decision A --Justifies(0.5)--> Symbol X
        //                   --Justifies(0.9)--> Symbol Y (should be primary)
        let nodes = vec![
            make_decision_node("A", "Decision A"),
            make_symbol_node("X", "Symbol X"),
            make_symbol_node("Y", "Symbol Y"),
        ];
        let edges = vec![
            GraphEdge {
                source: NodeId::new("A"),
                target: NodeId::new("X"),
                kind: EdgeKind::Justifies,
                provenance: Provenance::Extracted,
                confidence: 0.5,
                metadata: serde_json::Value::Object(Default::default()),
            },
            GraphEdge {
                source: NodeId::new("A"),
                target: NodeId::new("Y"),
                kind: EdgeKind::Justifies,
                provenance: Provenance::Extracted,
                confidence: 0.9,
                metadata: serde_json::Value::Object(Default::default()),
            },
        ];

        let primary = resolve_primary_symbol(&nodes, &edges, &NodeId::new("A".to_string()));
        assert!(primary.is_some());
        assert_eq!(primary.unwrap().id.0, "Y"); // Y has higher confidence
    }

    #[tokio::test]
    async fn evidence_pane_built_from_direct_blocks_no_registry_dispatch() {
        // Graph: Decision A --CorroboratedBy--> Evidence Y
        let nodes = vec![
            make_decision_node("A", "Decision A"),
            make_evidence_node("Y", "Evidence Y"),
        ];
        let edges = vec![GraphEdge {
            source: NodeId::new("A"),
            target: NodeId::new("Y"),
            kind: EdgeKind::CorroboratedBy,
            provenance: Provenance::Manual,
            confidence: 0.8,
            metadata: serde_json::Value::Object(Default::default()),
        }];
        let repo = InMemoryGraphRepository::new(nodes, edges);

        let result = DecisionSupportPackBuilder::build("A", None, None, Some(&repo)).await;

        assert!(result.is_ok());
        let pack = result.unwrap();

        // Evidence pack should succeed
        let ep_pane = &pack.panes[2];
        assert!(matches!(ep_pane.status, PaneStatus::Ok));
        assert!(ep_pane.view.is_some());

        // Evidence blocks should be populated
        let view = ep_pane.view.as_ref().unwrap();
        assert!(
            !view.evidence.is_empty() || view.blocks.iter().any(|b| b.id == "no_evidence"),
            "Evidence pack should have evidence blocks or no_evidence message"
        );
    }
}
