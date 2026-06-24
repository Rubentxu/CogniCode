//! Graph facade — symbol resolution and subgraph traversal.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;
use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::traits::GraphQueryPort;

use crate::dto::{
    DriftFinding, DriftKind, DriftReport, ExpectedArchitecture, ExpectedContainer, GraphEdge,
    GraphNode, SubgraphResponse,
};
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

fn symbol_to_node(id: &str, s: &ResolvedSymbol, _style_hint: &str) -> GraphNode {
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

            for neighbour in incoming.into_iter().chain(outgoing.into_iter()) {
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

    async fn build_architecture(&self, root_path: &str) -> ExplorerResult<SubgraphResponse> {
        #[cfg(feature = "multimodal")]
        {
            self.build_architecture_impl(root_path).await
        }
        #[cfg(not(feature = "multimodal"))]
        {
            Err(ExplorerError::FeatureDisabled(
                "build_architecture requires multimodal feature".into(),
            ))
        }
    }

    async fn compare_architecture(&self, root_path: &str) -> ExplorerResult<DriftReport> {
        #[cfg(feature = "multimodal")]
        {
            self.compare_architecture_impl(root_path).await
        }
        #[cfg(not(feature = "multimodal"))]
        {
            Err(ExplorerError::FeatureDisabled(
                "compare_architecture requires multimodal feature".into(),
            ))
        }
    }
}

#[cfg(feature = "multimodal")]
impl GraphServiceImpl {
    async fn build_architecture_impl(&self, root_path: &str) -> ExplorerResult<SubgraphResponse> {
        use cognicode_core::domain::value_objects::{edge_kind::EdgeKind, node_kind::NodeKind};
        use std::collections::HashMap;
        use std::path::Path;

        let root = Path::new(root_path);
        let workspace_name = root
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "system".to_string());
        let system_id = format!("system:{}", workspace_name.to_lowercase());
        let system_label = workspace_name;

        // =========================================================================
        // C1 — System node
        // =========================================================================
        let mut nodes = vec![GraphNode {
            id: system_id.clone(),
            label: system_label,
            kind: NodeKind::System.as_str().to_string(),
            file: None,
            line: None,
            style_class: "node-system".to_string(),
        }];
        let mut edges = Vec::new();

        // =========================================================================
        // C2 — Containers
        // =========================================================================
        let workspace_toml = root.join("Cargo.toml");

        // Track container IDs for later edge creation
        let mut container_ids: HashSet<String> = HashSet::new();

        // Parse workspace Cargo.toml to find members
        if let Ok(toml_content) = std::fs::read_to_string(&workspace_toml) {
            if let Ok(toml_value) = toml_content.parse::<toml::Value>() {
                // Get workspace members (array of paths)
                if let Some(members) = toml_value
                    .get("workspace")
                    .and_then(|w| w.get("members"))
                    .and_then(|m| m.as_array())
                {
                    for member_value in members {
                        if let Some(member_path) = member_value.as_str() {
                            let member_dir = root.join(member_path);
                            let member_toml_path = member_dir.join("Cargo.toml");

                            if member_toml_path.exists() {
                                if let Ok(member_toml_content) =
                                    std::fs::read_to_string(&member_toml_path)
                                {
                                    if let Ok(member_toml) =
                                        member_toml_content.parse::<toml::Value>()
                                    {
                                        // Determine sub_kind: library if [lib] present, binary if [[bin]] present
                                        let has_lib = member_toml.get("lib").is_some();
                                        let has_bin = member_toml
                                            .get("bin")
                                            .map(|b| {
                                                b.as_array()
                                                    .map(|arr| !arr.is_empty())
                                                    .unwrap_or(false)
                                            })
                                            .unwrap_or(false);

                                        let _sub_kind = if has_lib {
                                            "library"
                                        } else if has_bin {
                                            "binary"
                                        } else {
                                            "library"
                                        };
                                        let container_id = format!("container:{}", member_path);
                                        container_ids.insert(container_id.clone());

                                        nodes.push(GraphNode {
                                            id: container_id.clone(),
                                            label: Path::new(member_path)
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| member_path.to_string()),
                                            kind: NodeKind::Container.as_str().to_string(),
                                            file: Some(member_toml_path.display().to_string()),
                                            line: None,
                                            style_class: "node-container".to_string(),
                                        });

                                        // Edge from container to system
                                        edges.push(GraphEdge {
                                            source: container_id,
                                            target: system_id.to_string(),
                                            relation: EdgeKind::PartOf.as_str().to_string(),
                                            style_class: "edge-part-of".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Parse apps/* directories for node-app containers
        let apps_dir = root.join("apps");
        if apps_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&apps_dir) {
                for entry in entries.flatten() {
                    let app_path = entry.path();
                    if app_path.is_dir() {
                        let package_json = app_path.join("package.json");
                        if package_json.exists() {
                            let app_name = app_path
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();

                            let container_id = format!("container:apps/{}", app_name);
                            container_ids.insert(container_id.clone());

                            nodes.push(GraphNode {
                                id: container_id.clone(),
                                label: app_name,
                                kind: NodeKind::Container.as_str().to_string(),
                                file: Some(package_json.display().to_string()),
                                line: None,
                                style_class: "node-container".to_string(),
                            });

                            // Edge from container to system
                            edges.push(GraphEdge {
                                source: container_id,
                                target: system_id.to_string(),
                                relation: EdgeKind::PartOf.as_str().to_string(),
                                style_class: "edge-part-of".to_string(),
                            });
                        }
                    }
                }
            }
        }

        // =========================================================================
        // C3 — Components (directories from module_list)
        // =========================================================================
        let modules = self.symbol_repo.module_list();

        // Build a map of module -> container
        // A module belongs to a container if it's inside that container's path
        let _module_to_container: HashMap<String, String> = HashMap::new();
        let mut component_ids: HashSet<String> = HashSet::new();

        for module_path in &modules {
            // Find which container this module belongs to
            let module_container = container_ids
                .iter()
                .find(|container_id| {
                    let container_path = container_id.strip_prefix("container:").unwrap_or("");
                    module_path.starts_with(container_path)
                        || module_path.starts_with(&format!("{}/", container_path))
                })
                .cloned();

            let component_id = format!("component:{}", module_path);
            component_ids.insert(component_id.clone());

            let label = Path::new(module_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| module_path.clone());

            nodes.push(GraphNode {
                id: component_id.clone(),
                label,
                kind: NodeKind::Component.as_str().to_string(),
                file: None,
                line: None,
                style_class: "node-component".to_string(),
            });

            // Edge from component to its container
            if let Some(container_id) = module_container {
                edges.push(GraphEdge {
                    source: component_id,
                    target: container_id,
                    relation: EdgeKind::PartOf.as_str().to_string(),
                    style_class: "edge-part-of".to_string(),
                });
            }
        }

        // =========================================================================
        // C4 — Code (symbols, capped at 200)
        // =========================================================================
        const C4_CODE_CAP: usize = 200;
        let all_symbols = self.symbol_repo.all_symbols()?;
        let mut code_count = 0;

        for symbol in all_symbols {
            if code_count >= C4_CODE_CAP {
                break;
            }

            // Check if the symbol's file's parent directory matches a C3 component
            let file_path = Path::new(&symbol.file);
            let parent_dir = file_path
                .parent()
                .map(|p| {
                    // Get the relative path from the module root
                    p.to_string_lossy().to_string()
                })
                .unwrap_or_default();

            // Check if parent matches a module (C3 component)
            if component_ids.contains(&format!("component:{}", parent_dir)) {
                let code_id = format!("code:{}", symbol.id.as_str());
                // "code" is a C4-specific mapping (SymbolKind → "code" when in a C4 component).
                // It's not part of the global NodeKind enum. See
                // build_architecture_emits_code_kind_for_c4_code_nodes test.
                nodes.push(GraphNode {
                    id: code_id.clone(),
                    label: symbol.name.clone(),
                    kind: "code".to_string(),
                    file: Some(symbol.file.clone()),
                    line: Some(symbol.line),
                    style_class: "node-code".to_string(),
                });

                edges.push(GraphEdge {
                    source: code_id,
                    target: format!("component:{}", parent_dir),
                    relation: EdgeKind::PartOf.as_str().to_string(),
                    style_class: "edge-part-of".to_string(),
                });

                code_count += 1;
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

    async fn compare_architecture_impl(&self, root_path: &str) -> ExplorerResult<DriftReport> {
        use std::collections::HashMap;
        use std::path::Path;

        // Parse expected architecture file if it exists
        let expected_file = Path::new(root_path).join(".cognicode/expected-architecture.yaml");
        let expected_arch: ExpectedArchitecture = if expected_file.exists() {
            let content = std::fs::read_to_string(&expected_file).map_err(|e| {
                ExplorerError::InvalidInput(format!(
                    "Failed to read expected-architecture.yaml: {}",
                    e
                ))
            })?;
            serde_yaml::from_str(&content).map_err(|e| {
                ExplorerError::InvalidInput(format!(
                    "Failed to parse expected-architecture.yaml: {}",
                    e
                ))
            })?
        } else {
            // Graceful degradation: no expected architecture file
            return Ok(DriftReport::default());
        };

        // Build the inferred architecture to get container names and sub_kinds
        let inferred = self.build_architecture_impl(root_path).await?;

        // Collect inferred containers: { name -> sub_kind }
        let mut inferred_containers: HashMap<String, String> = HashMap::new();
        for node in &inferred.nodes {
            if node.kind == "container" {
                // Extract container name from id (format: "container:<name>")
                let name = node.id.strip_prefix("container:").unwrap_or(&node.id);
                // Infer sub_kind from the file path in the node's file field
                let sub_kind = infer_sub_kind(node.file.as_deref());
                inferred_containers.insert(name.to_string(), sub_kind);
            }
        }

        // Compare expected vs inferred
        let expected_names: HashSet<String> = expected_arch
            .containers
            .iter()
            .map(|c| c.name.clone())
            .collect();
        let inferred_names: HashSet<String> = inferred_containers.keys().cloned().collect();

        let mut findings = Vec::new();

        // Missing containers: in expected but not in inferred
        for container in &expected_arch.containers {
            if !inferred_names.contains(&container.name) {
                findings.push(DriftFinding {
                    kind: DriftKind::MissingContainer,
                    expected: container.name.clone(),
                    actual: "—".to_string(),
                    severity: "warning".to_string(),
                    detail: format!(
                        "Expected container '{}' (sub_kind: {}) is not present in the inferred architecture",
                        container.name, container.sub_kind
                    ),
                });
            }
        }

        // Extra containers: in inferred but not in expected
        for name in &inferred_names {
            if !expected_names.contains(name) {
                let actual_sub_kind = inferred_containers.get(name).cloned().unwrap_or_default();
                findings.push(DriftFinding {
                    kind: DriftKind::ExtraContainer,
                    expected: "—".to_string(),
                    actual: name.clone(),
                    severity: "warning".to_string(),
                    detail: format!(
                        "Container '{}' (sub_kind: {}) is present but not in expected architecture",
                        name, actual_sub_kind
                    ),
                });
            }
        }

        // Wrong sub_kind: name matches but sub_kind differs
        for expected in &expected_arch.containers {
            if let Some(actual_sub_kind) = inferred_containers.get(&expected.name) {
                if actual_sub_kind != &expected.sub_kind {
                    findings.push(DriftFinding {
                        kind: DriftKind::WrongSubKind,
                        expected: format!("{} ({})", expected.name, expected.sub_kind),
                        actual: format!("{} ({})", expected.name, actual_sub_kind),
                        severity: "info".to_string(),
                        detail: format!(
                            "Container '{}' has sub_kind '{}' in expected but '{}' in inferred",
                            expected.name, expected.sub_kind, actual_sub_kind
                        ),
                    });
                }
            }
        }

        let missing_containers = findings
            .iter()
            .filter(|f| f.kind == DriftKind::MissingContainer)
            .count();
        let extra_containers = findings
            .iter()
            .filter(|f| f.kind == DriftKind::ExtraContainer)
            .count();
        let wrong_sub_kinds = findings
            .iter()
            .filter(|f| f.kind == DriftKind::WrongSubKind)
            .count();

        let summary = if findings.is_empty() {
            "No architecture drift detected".to_string()
        } else {
            format!(
                "Architecture drift: {} missing, {} extra, {} wrong sub_kind",
                missing_containers, extra_containers, wrong_sub_kinds
            )
        };

        Ok(DriftReport {
            findings,
            summary,
            missing_containers,
            extra_containers,
            wrong_sub_kinds,
        })
    }
}

/// Infer the sub_kind of a container from its file path.
fn infer_sub_kind(file: Option<&str>) -> String {
    match file {
        Some(f) => {
            if f.contains("/bin/") || f.ends_with("-bin") {
                "binary".to_string()
            } else {
                "library".to_string()
            }
        }
        None => "library".to_string(),
    }
}

#[cfg(all(test, feature = "multimodal"))]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::{Location, SymbolKind};
    use std::sync::Arc;
    use tempfile::TempDir;

    /// Mock SymbolRepository for architecture tests.
    struct MockSymbolRepo {
        modules: Vec<String>,
        symbols: Vec<ResolvedSymbol>,
    }

    impl MockSymbolRepo {
        fn new(modules: Vec<String>, symbols: Vec<ResolvedSymbol>) -> Self {
            Self { modules, symbols }
        }
    }

    impl SymbolRepository for MockSymbolRepo {
        fn resolve(&self, _id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
            Ok(None)
        }
        fn find_symbols_by_name(&self, _name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn find_symbols_by_file(&self, _file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(vec![])
        }
        fn module_list(&self) -> Vec<String> {
            self.modules.clone()
        }
        fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.symbols.clone())
        }
        fn graph_stats(&self) -> crate::ports::symbol_repository::GraphStats {
            crate::ports::symbol_repository::GraphStats {
                symbol_count: self.symbols.len(),
                relation_count: 0,
            }
        }
    }

    fn make_mock_repo(
        modules: Vec<String>,
        symbols: Vec<ResolvedSymbol>,
    ) -> Arc<dyn SymbolRepository> {
        Arc::new(MockSymbolRepo::new(modules, symbols))
    }

    fn sym(name: &str, file: &str, line: u32) -> ResolvedSymbol {
        ResolvedSymbol {
            id: SymbolId::new(format!("{}:{}:{}", file, name, line)),
            name: name.to_string(),
            kind: SymbolKind::Function,
            file: file.to_string(),
            line,
            signature: Some(format!("fn {}()", name)),
        }
    }

    #[tokio::test]
    async fn build_architecture_returns_system_node() {
        let repo = make_mock_repo(vec![], vec![]);
        let service = GraphServiceImpl::new(repo, None);

        let tmp = TempDir::new().unwrap();
        let workspace_name = tmp
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let expected_system_id = format!("system:{}", workspace_name.to_lowercase());
        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have exactly one system node
        let system_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == "system").collect();
        assert_eq!(system_nodes.len(), 1);
        assert_eq!(system_nodes[0].id, expected_system_id.as_str());
        assert_eq!(system_nodes[0].style_class, "node-system");
    }

    #[tokio::test]
    async fn build_architecture_includes_workspace_containers() {
        // Create a temp workspace with Cargo.toml containing members
        let tmp = TempDir::new().unwrap();
        let workspace_name = tmp
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let expected_system_id = format!("system:{}", workspace_name.to_lowercase());
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(
            &workspace_toml,
            r#"
[workspace]
members = [
    "crates/test-crate"
]
"#,
        )
        .unwrap();

        // Create the member crate directory with Cargo.toml
        let crate_dir = tmp.path().join("crates/test-crate");
        std::fs::create_dir_all(&crate_dir).unwrap();
        let crate_toml = crate_dir.join("Cargo.toml");
        std::fs::write(
            &crate_toml,
            r#"
[package]
name = "test-crate"
version = "0.1.0"

[lib]
path = "lib.rs"
"#,
        )
        .unwrap();
        std::fs::write(crate_dir.join("lib.rs"), "").unwrap();

        let repo = make_mock_repo(vec![], vec![]);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have system + 1 container
        let container_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == "container")
            .collect();
        assert_eq!(container_nodes.len(), 1);
        assert_eq!(container_nodes[0].id, "container:crates/test-crate");
        assert_eq!(container_nodes[0].style_class, "node-container");

        // Should have part_of edge from container to system
        let container_to_system: Vec<_> = result
            .edges
            .iter()
            .filter(|e| {
                e.source == "container:crates/test-crate" && e.target == expected_system_id.as_str()
            })
            .collect();
        assert_eq!(container_to_system.len(), 1);
        assert_eq!(container_to_system[0].relation, "part_of");
    }

    #[tokio::test]
    async fn build_architecture_includes_apps_containers() {
        let tmp = TempDir::new().unwrap();
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(&workspace_toml, "").unwrap();

        // Create apps/my-app with package.json
        let app_dir = tmp.path().join("apps/my-app");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(app_dir.join("package.json"), r#"{"name": "my-app"}"#).unwrap();

        let repo = make_mock_repo(vec![], vec![]);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have system + 1 node-app container
        let container_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == "container")
            .collect();
        assert_eq!(container_nodes.len(), 1);
        assert_eq!(container_nodes[0].id, "container:apps/my-app");
        assert_eq!(container_nodes[0].style_class, "node-container");
    }

    #[tokio::test]
    async fn build_architecture_includes_components() {
        let tmp = TempDir::new().unwrap();
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(
            &workspace_toml,
            r#"
[workspace]
members = ["crates/test-crate"]
"#,
        )
        .unwrap();

        let crate_dir = tmp.path().join("crates/test-crate");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            r#"
[package]
name = "test-crate"
version = "0.1.0"
[lib]
"#,
        )
        .unwrap();

        // Create src directory as a module
        let src_dir = crate_dir.join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let repo = make_mock_repo(vec!["crates/test-crate/src".to_string()], vec![]);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have container + component
        let component_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == "component")
            .collect();
        assert_eq!(component_nodes.len(), 1);
        assert_eq!(component_nodes[0].id, "component:crates/test-crate/src");
        assert_eq!(component_nodes[0].style_class, "node-component");

        // Should have part_of edge from component to container
        let component_to_container: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.source == "component:crates/test-crate/src" && e.relation == "part_of")
            .collect();
        assert_eq!(component_to_container.len(), 1);
    }

    #[tokio::test]
    async fn build_architecture_caps_code_nodes_at_200() {
        let tmp = TempDir::new().unwrap();
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(&workspace_toml, "").unwrap();

        // Create a crate with src directory
        let crate_dir = tmp.path().join("crates/test-crate");
        std::fs::create_dir_all(&crate_dir.join("src")).unwrap();
        std::fs::write(crate_dir.join("Cargo.toml"), "[lib]").unwrap();

        // Create 250 symbols in the module
        let symbols: Vec<ResolvedSymbol> = (0..250)
            .map(|i| sym(&format!("func_{}", i), "crates/test-crate/src/lib.rs", i))
            .collect();

        let repo = make_mock_repo(vec!["crates/test-crate/src".to_string()], symbols);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have exactly 200 code nodes due to cap
        let code_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == "code").collect();
        assert_eq!(code_nodes.len(), 200);
    }

    #[tokio::test]
    async fn build_architecture_creates_part_of_edges() {
        let tmp = TempDir::new().unwrap();
        let workspace_name = tmp
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let expected_system_id = format!("system:{}", workspace_name.to_lowercase());
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(
            &workspace_toml,
            r#"
[workspace]
members = ["crates/test-crate"]
"#,
        )
        .unwrap();

        let crate_dir = tmp.path().join("crates/test-crate");
        std::fs::create_dir_all(&crate_dir.join("src")).unwrap();
        std::fs::write(crate_dir.join("Cargo.toml"), "[lib]").unwrap();

        let symbols = vec![sym("my_func", "crates/test-crate/src/lib.rs", 10)];
        let repo = make_mock_repo(vec!["crates/test-crate/src".to_string()], symbols);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // Should have edges: container->system, component->container, code->component
        let part_of_edges: Vec<_> = result
            .edges
            .iter()
            .filter(|e| e.relation == "part_of")
            .collect();
        assert_eq!(part_of_edges.len(), 3); // 3 part_of edges

        // Verify the edge hierarchy
        let sources: Vec<_> = part_of_edges.iter().map(|e| e.source.as_str()).collect();
        let targets: Vec<_> = part_of_edges.iter().map(|e| e.target.as_str()).collect();
        assert!(sources.contains(&"container:crates/test-crate"));
        assert!(targets.contains(&expected_system_id.as_str()));
    }

    #[tokio::test]
    async fn build_architecture_derives_system_name_from_workspace() {
        let tmp = tempfile::Builder::new().prefix("my-app").tempdir().unwrap();
        let repo = make_mock_repo(vec![], vec![]);
        let service = GraphServiceImpl::new(repo, None);

        let response = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        let system_nodes: Vec<_> = response
            .nodes
            .iter()
            .filter(|n| n.kind == "system")
            .collect();
        assert_eq!(system_nodes.len(), 1);

        let workspace_name = tmp
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(
            system_nodes[0].id,
            format!("system:{}", workspace_name.to_lowercase())
        );
        assert_eq!(system_nodes[0].label, workspace_name);
        assert_ne!(system_nodes[0].id, "system:cognicode");
    }

    /// Regression test: C4 code nodes (leaf symbols inside a component)
    /// must have `kind: "code"`, NOT `format!("{:?}", symbol.kind).to_lowercase()`
    /// which would produce e.g. `"function"` instead of `"code"`.
    /// ADR-039 §C4-code-node-kind governs this invariant.
    #[tokio::test]
    async fn build_architecture_emits_code_kind_for_c4_code_nodes() {
        let tmp = TempDir::new().unwrap();
        let workspace_toml = tmp.path().join("Cargo.toml");
        std::fs::write(&workspace_toml, "").unwrap();

        // Create a crate whose src/ directory maps to a C4 component
        let crate_dir = tmp.path().join("crates/my-crate");
        std::fs::create_dir_all(&crate_dir.join("src")).unwrap();
        std::fs::write(crate_dir.join("Cargo.toml"), "[lib]").unwrap();

        // One function symbol — its kind is Function but inside a C4 component
        // it must surface as kind="code"
        let symbols = vec![sym("my_func", "crates/my-crate/src/lib.rs", 10)];
        let repo = make_mock_repo(vec!["crates/my-crate/src".to_string()], symbols);
        let service = GraphServiceImpl::new(repo, None);

        let result = service
            .build_architecture(tmp.path().to_str().unwrap())
            .await
            .unwrap();

        // There should be exactly one code node (the my_func symbol)
        let code_nodes: Vec<_> = result.nodes.iter().filter(|n| n.kind == "code").collect();
        assert_eq!(
            code_nodes.len(),
            1,
            "expected exactly one code node, got: {:#?}",
            result.nodes
        );
        assert_eq!(code_nodes[0].label, "my_func");

        // Ensure it is NOT emitted as kind="function"
        let function_kind_nodes: Vec<_> = result
            .nodes
            .iter()
            .filter(|n| n.kind == "function")
            .collect();
        assert!(
            function_kind_nodes.is_empty(),
            "C4 code nodes must not have kind='function'; found: {:#?}",
            function_kind_nodes
        );
    }
}
