//! ViewService and LensExecutor facade.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::domain::lens::{LensContext, LensRegistry};
use crate::dto::{ChildrenSection, ContextualGraphResponse, GraphEdge, GraphNode, LensDescriptor, LensResult, ParentSection, SameLevelSection};
use crate::domain::object_identity::ObjectIdentity;
use crate::dto::{ContextualView, ViewDescriptor, ViewSpec};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::{LensExecutor, ViewService};
use crate::ports::quality_repository::QualityRepository;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};

pub struct ViewServiceImpl {
    repo: Arc<dyn SymbolRepository>,
    reader: Arc<dyn SourceReader>,
    quality: Option<Arc<dyn QualityRepository>>,
    lens_registry: LensRegistry,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
}

impl ViewServiceImpl {
    pub fn new(
        repo: Arc<dyn SymbolRepository>,
        reader: Arc<dyn SourceReader>,
        quality: Option<Arc<dyn QualityRepository>>,
        lens_registry: LensRegistry,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
    ) -> Self {
        Self {
            repo,
            reader,
            quality,
            lens_registry,
            graph_query,
        }
    }

    fn available_views_sync(&self, object_id: &str) -> ExplorerResult<Vec<ViewDescriptor>> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        Ok(match identity {
            ObjectIdentity::Symbol { .. } => vec![
                ViewDescriptor { id: "overview".into(), title: "Overview".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "call-graph".into(), title: "Call Graph".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "source".into(), title: "Source".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "evidence".into(), title: "Evidence".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "quality".into(), title: "Quality".into(), is_builtin: true, source: None },
            ],
            ObjectIdentity::File { .. } => vec![
                ViewDescriptor { id: "overview".into(), title: "Overview".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "symbols".into(), title: "Symbols".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "quality".into(), title: "Quality".into(), is_builtin: true, source: None },
            ],
            ObjectIdentity::Scope { .. } => vec![
                ViewDescriptor { id: "overview".into(), title: "Overview".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "dependencies".into(), title: "Dependencies".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "hotspots".into(), title: "Hotspots".into(), is_builtin: true, source: None },
                ViewDescriptor { id: "quality".into(), title: "Quality".into(), is_builtin: true, source: None },
            ],
            ObjectIdentity::QualityIssue { .. } => vec![
                ViewDescriptor { id: "overview".into(), title: "Overview".into(), is_builtin: true, source: None },
            ],
            ObjectIdentity::Rule { .. } => vec![
                ViewDescriptor { id: "overview".into(), title: "Overview".into(), is_builtin: true, source: None },
            ],
        })
    }

    fn available_lenses_sync(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let object_type = match &identity {
            ObjectIdentity::Symbol { .. } => crate::dto::InspectableObjectType::Symbol,
            ObjectIdentity::File { .. } => crate::dto::InspectableObjectType::File,
            ObjectIdentity::Scope { .. } => crate::dto::InspectableObjectType::Scope,
            ObjectIdentity::QualityIssue { .. } => crate::dto::InspectableObjectType::QualityIssue,
            ObjectIdentity::Rule { .. } => crate::dto::InspectableObjectType::Rule,
        };
        Ok(self.lens_registry.applicable_to(&object_type))
    }

    fn apply_lens_sync(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        let identity = ObjectIdentity::parse_mvp_id(object_id)?;
        let lens = self.lens_registry.get(lens_id)
            .ok_or_else(|| ExplorerError::ResolutionFailed(format!("lens not found: {}", lens_id)))?;
        let ctx = LensContext::new(
            identity,
            self.repo.clone(),
            self.quality.clone(),
            self.reader.clone(),
            self.graph_query.clone(),
        );
        lens.apply(&ctx)
    }

    fn build_contextual_graph_sync(
        &self,
        focus_id: &str,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        // 1) Validate the request.
        if level != "file" {
            return Err(ExplorerError::InvalidQuery(format!(
                "level must be 'file' in Phase 1 (got: {level})"
            )));
        }
        if !(1..=2).contains(&depth) {
            return Err(ExplorerError::InvalidQuery(format!(
                "depth must be in 1..=2 (got: {depth})"
            )));
        }
        if !(50..=500).contains(&max_nodes) {
            return Err(ExplorerError::InvalidQuery(format!(
                "max_nodes must be in 50..=500 (got: {max_nodes})"
            )));
        }

        // 2) Resolve the focus symbol.
        let focus_symbol_id = SymbolId::new(focus_id);
        let focus_resolved = self
            .repo
            .resolve(&focus_symbol_id)?
            .ok_or_else(|| ExplorerError::SymbolNotFound(focus_id.to_string()))?;
        let focus_node = symbol_to_node(&focus_resolved);

        // 3) Build the parent + children section (file-level projection).
        let file_siblings = self.repo.find_symbols_by_file(&focus_resolved.file)?;
        let (parent, children, children_clipped) = if file_siblings.is_empty() {
            (None, None, false)
        } else {
            let parent_node = GraphNode {
                id: format!("file:{}", focus_resolved.file),
                label: focus_resolved.file.clone(),
                kind: "file".to_string(),
                file: Some(focus_resolved.file.clone()),
                line: None,
                style_class: "module".to_string(),
            };
            let parent_edge = GraphEdge {
                source: focus_resolved.id.to_string(),
                target: parent_node.id.clone(),
                relation: "lives_in".to_string(),
                style_class: "edge.calls".to_string(),
            };
            let parent_section = ParentSection {
                node: parent_node,
                edge: parent_edge,
            };

            let mut child_nodes: Vec<GraphNode> = Vec::new();
            let mut child_edges: Vec<GraphEdge> = Vec::new();
            for sib in file_siblings.iter().filter(|s| s.id != focus_resolved.id) {
                child_edges.push(GraphEdge {
                    source: sib.id.to_string(),
                    target: focus_resolved.id.to_string(),
                    relation: "lives_in".to_string(),
                    style_class: "edge.calls".to_string(),
                });
                child_nodes.push(symbol_to_node(sib));
            }

            let clipped = child_nodes.len() > max_nodes;
            if clipped {
                child_nodes.truncate(max_nodes);
                let kept: HashSet<String> = child_nodes.iter().map(|n| n.id.clone()).collect();
                child_edges.retain(|e| kept.contains(&e.source));
            }
            (
                Some(parent_section),
                Some(ChildrenSection {
                    nodes: child_nodes,
                    edges: child_edges,
                }),
                clipped,
            )
        };

        // 4) Build the same-level BFS up to `depth` hops, capped at
        //    the remaining budget (children take priority).
        let remaining_cap = max_nodes.saturating_sub(
            children.as_ref().map(|c| c.nodes.len()).unwrap_or(0),
        );
        let (same_nodes, same_edges) = if remaining_cap == 0 {
            (Vec::new(), Vec::new())
        } else {
            bfs_same_level(
                self.repo.as_ref(),
                self.graph_query.as_ref().map(|gq| gq.as_ref()),
                &focus_symbol_id,
                depth,
                remaining_cap,
            )
        };

        // 5) Truncation flag.
        let fan_in = self.graph_query.as_ref().map(|gq| gq.fan_in(&focus_symbol_id)).unwrap_or(0);
        let fan_out = self.graph_query.as_ref().map(|gq| gq.fan_out(&focus_symbol_id)).unwrap_or(0);
        let bfs_clipped = !same_nodes.is_empty() && same_nodes.len() >= remaining_cap
            && (fan_in + fan_out) > remaining_cap as usize;
        let truncated = children_clipped || bfs_clipped;
        let truncation_reason = if truncated {
            Some("max_nodes_exceeded".to_string())
        } else {
            None
        };

        Ok(ContextualGraphResponse {
            focus_node,
            parent,
            children,
            same_level: SameLevelSection {
                nodes: same_nodes,
                edges: same_edges,
            },
            level: "file".to_string(),
            truncated,
            truncation_reason,
        })
    }
}

#[async_trait]
impl ViewService for ViewServiceImpl {
    async fn available_views(&self, object_id: &str) -> ExplorerResult<Vec<ViewDescriptor>> {
        let object_id = object_id.to_string();
        let result = self.available_views_sync(&object_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn contextual_view(
        &self,
        object_id: &str,
        view_id: &str,
    ) -> ExplorerResult<ContextualView> {
        Err(ExplorerError::FeatureDisabled(
            "contextual_view not implemented".into(),
        ))
    }

    async fn build_contextual_graph(
        &self,
        focus_id: &str,
        level: &str,
        depth: u8,
        max_nodes: usize,
    ) -> ExplorerResult<ContextualGraphResponse> {
        let focus_id = focus_id.to_string();
        let level = level.to_string();
        let result = self.build_contextual_graph_sync(&focus_id, &level, depth, max_nodes);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn available_lenses(&self, object_id: &str) -> ExplorerResult<Vec<LensDescriptor>> {
        let object_id = object_id.to_string();
        let result = self.available_lenses_sync(&object_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        let object_id = object_id.to_string();
        let lens_id = lens_id.to_string();
        let result = self.apply_lens_sync(&object_id, &lens_id);
        tokio::task::spawn_blocking(move || result)
            .await
            .map_err(|e| ExplorerError::Anyhow(anyhow::anyhow!("join error: {}", e)))?
    }

    async fn execute_view_spec(
        &self,
        _spec: &ViewSpec,
        _object_id: &str,
    ) -> ExplorerResult<ContextualView> {
        Err(ExplorerError::FeatureDisabled(
            "execute_view_spec not implemented".into(),
        ))
    }
}

// ============================================================================
// Helper functions (mirrored from service.rs)
// ============================================================================

fn symbol_to_node(s: &ResolvedSymbol) -> GraphNode {
    let kind_label = format!("{:?}", s.kind).to_lowercase();
    GraphNode {
        id: s.id.to_string(),
        label: s.name.clone(),
        kind: kind_label.clone(),
        file: Some(s.file.clone()),
        line: Some(s.line),
        style_class: style_class_for_kind(&kind_label).to_string(),
    }
}

fn style_class_for_kind(kind: &str) -> &'static str {
    match kind {
        "function" | "method" | "fn" => "function",
        "module" | "crate" | "trait" => "module",
        "external" => "external",
        "file" => "module",
        _ => "function",
    }
}

/// BFS of same-level neighbours (callers + callees) of `start` up to
/// `depth` hops, capped at `cap` collected nodes.
fn bfs_same_level(
    repo: &dyn SymbolRepository,
    graph_query: Option<&dyn GraphQueryPort>,
    start: &SymbolId,
    depth: u8,
    cap: usize,
) -> (Vec<GraphNode>, Vec<GraphEdge>) {
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(start.to_string());
    let mut frontier: Vec<SymbolId> = vec![start.clone()];
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    for _ in 0..depth {
        if nodes.len() >= cap {
            break;
        }
        let mut next: Vec<SymbolId> = Vec::new();
        for n in &frontier {
            let callers = graph_query.map(|gq| gq.callers(n)).unwrap_or_default();
            let callees = graph_query.map(|gq| gq.callees(n)).unwrap_or_default();
            for rel in callers.into_iter().chain(callees) {
                let nid = rel.id.to_string();
                if !visited.insert(nid.clone()) {
                    continue;
                }
                if nodes.len() >= cap {
                    break;
                }
                edges.push(GraphEdge {
                    source: n.to_string(),
                    target: nid.clone(),
                    relation: "calls".to_string(),
                    style_class: "edge.calls".to_string(),
                });
                if let Ok(Some(resolved)) = repo.resolve(&rel.id) {
                    nodes.push(symbol_to_node(&resolved));
                }
                next.push(rel.id);
            }
            if nodes.len() >= cap {
                break;
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    (nodes, edges)
}

#[async_trait]
impl LensExecutor for ViewServiceImpl {
    async fn apply_lens(&self, object_id: &str, lens_id: &str) -> ExplorerResult<LensResult> {
        ViewService::apply_lens(self, object_id, lens_id).await
    }
}
