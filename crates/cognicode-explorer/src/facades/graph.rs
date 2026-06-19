//! Graph facade — symbol resolution and subgraph traversal.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::dto::{GraphEdge, GraphNode, SubgraphResponse};
use crate::error::{ExplorerError, ExplorerResult};
use crate::facades::GraphService;
use crate::ports::symbol_repository::{ResolvedSymbol, SymbolRepository};

// Re-export SubgraphDirection for use within this module
use super::SubgraphDirection;

/// Implementation of GraphService.
pub struct GraphServiceImpl {
    symbol_repo: Arc<dyn SymbolRepository>,
    graph_query: Option<Arc<dyn GraphQueryPort>>,
}

impl GraphServiceImpl {
    pub fn new(
        symbol_repo: Arc<dyn SymbolRepository>,
        graph_query: Option<Arc<dyn GraphQueryPort>>,
    ) -> Self {
        Self {
            symbol_repo,
            graph_query,
        }
    }
}

fn map_repo_unavailable(e: ExplorerError) -> ExplorerError {
    match e {
        ExplorerError::GraphNotReady => {
            ExplorerError::GraphUnavailable("call graph is not loaded yet".to_string())
        }
        other => other,
    }
}

fn symbol_to_node(
    id: &str,
    s: &ResolvedSymbol,
    _style_hint: &str,
) -> GraphNode {
    let kind_label = format!("{:?}", s.kind).to_lowercase();
    GraphNode {
        id: id.to_string(),
        label: s.name.clone(),
        kind: kind_label.clone(),
        file: Some(s.file.clone()),
        line: Some(s.line),
        style_class: crate::api::style_class_for(&kind_label).to_string(),
    }
}

#[async_trait]
impl GraphService for GraphServiceImpl {
    async fn resolve_symbol(&self, id: &str) -> ExplorerResult<Option<ResolvedSymbol>> {
        let symbol_id = SymbolId::new(id);
        let resolved = self
            .symbol_repo
            .resolve(&symbol_id)
            .map_err(map_repo_unavailable)?;
        Ok(resolved)
    }

    fn graph_query(&self) -> Option<Arc<dyn GraphQueryPort>> {
        self.graph_query.clone()
    }

    async fn build_subgraph(
        &self,
        root_id: &str,
        depth: u8,
        direction: SubgraphDirection,
        max_nodes: u32,
    ) -> ExplorerResult<SubgraphResponse> {
        let root_symbol_id = SymbolId::new(root_id);

        let root_resolved = self
            .symbol_repo
            .resolve(&root_symbol_id)
            .map_err(map_repo_unavailable)?
            .ok_or_else(|| ExplorerError::SymbolNotFound(root_id.to_string()))?;

        let max_nodes_usize = max_nodes as usize;
        let mut visited_ids: Vec<String> = Vec::with_capacity(max_nodes_usize.min(1024));
        let mut visited_set: HashSet<String> = HashSet::new();
        let mut nodes: Vec<GraphNode> = Vec::with_capacity(max_nodes_usize.min(1024));
        let mut edges: Vec<GraphEdge> = Vec::new();

        let mut queue: Vec<(String, u8)> = Vec::new();
        let root_str = root_id.to_string();
        queue.push((root_str.clone(), 0));
        visited_set.insert(root_str.clone());
        visited_ids.push(root_str.clone());
        nodes.push(symbol_to_node(
            &root_resolved.id.to_string(),
            &root_resolved,
            "function",
        ));

        let mut truncated = false;

        while let Some((current_id, current_depth)) = queue.first().cloned() {
            queue.remove(0);
            if current_depth >= depth {
                continue;
            }
            if nodes.len() >= max_nodes_usize {
                truncated = true;
                break;
            }
            let current_sym = SymbolId::new(&current_id);
            let (incoming, outgoing) = match direction {
                SubgraphDirection::Incoming => (
                    self.graph_query
                        .as_ref()
                        .map(|gq| gq.callers(&current_sym))
                        .unwrap_or_default(),
                    Vec::new(),
                ),
                SubgraphDirection::Outgoing => (
                    Vec::new(),
                    self.graph_query
                        .as_ref()
                        .map(|gq| gq.callees(&current_sym))
                        .unwrap_or_default(),
                ),
                SubgraphDirection::Both => (
                    self.graph_query
                        .as_ref()
                        .map(|gq| gq.callers(&current_sym))
                        .unwrap_or_default(),
                    self.graph_query
                        .as_ref()
                        .map(|gq| gq.callees(&current_sym))
                        .unwrap_or_default(),
                ),
            };

            for neighbour in incoming
                .into_iter()
                .chain(outgoing.into_iter())
            {
                if nodes.len() >= max_nodes_usize {
                    truncated = true;
                    break;
                }
                let neighbour_id = neighbour.id.to_string();
                let is_new = visited_set.insert(neighbour_id.clone());
                if is_new {
                    visited_ids.push(neighbour_id.clone());
                    let kind_label = format!("{:?}", neighbour.kind).to_lowercase();
                    nodes.push(GraphNode {
                        id: neighbour_id.clone(),
                        label: neighbour.name.clone(),
                        kind: kind_label.clone(),
                        file: Some(neighbour.file.clone()),
                        line: Some(neighbour.line),
                        style_class: crate::api::style_class_for(&kind_label).to_string(),
                    });
                    queue.push((neighbour_id.clone(), current_depth + 1));
                }
                edges.push(GraphEdge {
                    source: current_id.clone(),
                    target: neighbour_id.clone(),
                    relation: "calls".to_string(),
                    style_class: crate::api::edge_style_class_for("calls").to_string(),
                });
            }
            if truncated {
                break;
            }
        }

        if truncated {
            let kept: HashSet<&String> = nodes.iter().map(|n| &n.id).collect();
            edges.retain(|e| kept.contains(&e.source) && kept.contains(&e.target));
        }

        Ok(SubgraphResponse {
            root: root_id.to_string(),
            nodes,
            edges,
            truncated,
            truncated_reason: if truncated {
                Some("node_cap".to_string())
            } else {
                None
            },
            corroboration_scores: HashMap::new(),
        })
    }

    async fn build_architecture(&self) -> ExplorerResult<SubgraphResponse> {
        use std::collections::HashMap;

        let modules = self.symbol_repo.module_list();

        // Each directory becomes a node-component.
        // Edges: if path is "a/b/c", parent is "a/b".
        let mut nodes: Vec<GraphNode> = Vec::with_capacity(modules.len());
        let mut edges: Vec<GraphEdge> = Vec::new();

        // Build parent map for edge creation
        let parent_of: HashMap<String, Option<String>> = modules
            .iter()
            .cloned()
            .map(|path| {
                let parent = std::path::Path::new(&path)
                    .parent()
                    .map(|p| p.to_string_lossy().to_string());
                (path, parent)
            })
            .collect();

        for path in &modules {
            let label = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());

            nodes.push(GraphNode {
                id: format!("component:{}", path),
                label,
                kind: "component".to_string(),
                file: None,
                line: None,
                style_class: "node-component".to_string(),
            });

            if let Some(ref parent) = parent_of[path] {
                if modules.contains(parent) {
                    edges.push(GraphEdge {
                        source: format!("component:{}", path),
                        target: format!("component:{}", parent),
                        relation: "part_of".to_string(),
                        style_class: "edge-part-of".to_string(),
                    });
                }
            }
        }

        Ok(SubgraphResponse {
            root: "architecture".to_string(),
            nodes,
            edges,
            truncated: false,
            truncated_reason: None,
            corroboration_scores: HashMap::new(),
        })
    }
}
