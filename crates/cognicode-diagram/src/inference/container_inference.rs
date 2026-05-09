//! Container Inference (L2) — combines Cargo.toml and CallGraph analysis
//!
//! Infers C4 Containers (L2) by combining project configuration (Cargo.toml)
//! with call graph analysis to determine container boundaries and relationships.

use std::collections::HashMap;
use std::path::Path;

use cognicode_core::domain::aggregates::call_graph::CallGraph;

use crate::inference::config_parsers::{self, WorkspaceInfo};
use crate::model::c4_types::{Container, ContainerType, ElementId};
use crate::model::relationships::{C4Relationship, C4RelationshipKind};

/// Container inference engine that combines Cargo.toml and CallGraph analysis
#[derive(Debug, Clone)]
pub struct ContainerInference {
    /// Minimum symbols required to create a separate container
    min_symbols_per_container: usize,
}

impl ContainerInference {
    pub fn new() -> Self {
        Self {
            min_symbols_per_container: 5,
        }
    }

    /// Infer containers from both Cargo.toml and CallGraph
    ///
    /// First parses Cargo.toml to get container structure, then enriches
    /// with information from the call graph.
    pub fn infer_from_workspace(&self, project_dir: &Path, call_graph: &CallGraph) -> anyhow::Result<Vec<Container>> {
        // Try to parse Cargo.toml first
        if let Some(mut workspace_info) = config_parsers::parse_project(project_dir)? {
            // Enrich containers with call graph data
            self.enrich_containers_with_callgraph(&mut workspace_info.containers, call_graph);
            Ok(workspace_info.containers)
        } else {
            // Fallback: infer containers purely from call graph
            Ok(self.infer_containers_from_callgraph(call_graph, project_dir))
        }
    }

    /// Enrich containers with information from the call graph
    fn enrich_containers_with_callgraph(&self, containers: &mut [Container], call_graph: &CallGraph) {
        // First pass: build mapping of container_id -> (prefix, original_index)
        let container_prefixes: Vec<(String, usize, String)> = containers
            .iter()
            .enumerate()
            .filter_map(|(idx, container)| {
                container.path.as_ref().map(|path| {
                    let path_str = path.to_string_lossy();
                    let prefix = path_str.find("/src/").map(|pos| path_str[..pos].to_string());
                    (prefix.unwrap_or_default(), idx, container.id.as_str().to_string())
                })
            })
            .collect();

        // Update container descriptions based on symbol counts
        for (prefix, idx, _container_id) in &container_prefixes {
            let mut symbol_count = 0;
            let mut public_functions = 0;

            for (_symbol_id, symbol) in call_graph.symbol_ids() {
                let file = symbol.location().file();
                // Check if file belongs to this container
                if file.starts_with(prefix) {
                    symbol_count += 1;
                    if symbol.is_callable() {
                        public_functions += 1;
                    }
                }
            }

            if symbol_count > 0 {
                let desc = format!(
                    " ({} symbols, {} public functions)",
                    symbol_count, public_functions
                );
                containers[*idx].description.push_str(&desc);
            }
        }
    }

    /// Infer containers purely from call graph (fallback when no Cargo.toml)
    fn infer_containers_from_callgraph(&self, call_graph: &CallGraph, _project_dir: &Path) -> Vec<Container> {
        // Group symbols by top-level directory
        let mut dir_groups: HashMap<String, Vec<cognicode_core::domain::aggregates::call_graph::SymbolId>> = HashMap::new();

        for (symbol_id, symbol) in call_graph.symbol_ids() {
            let file = symbol.location().file();

            // Extract top-level directory (first segment after project root)
            let top_dir = if let Some(slash_pos) = file.find('/') {
                let after_slash = &file[slash_pos + 1..];
                if let Some(next_slash) = after_slash.find('/') {
                    after_slash[..next_slash].to_string()
                } else {
                    after_slash.to_string()
                }
            } else {
                "root".to_string()
            };

            dir_groups.entry(top_dir).or_default().push(symbol_id.clone());
        }

        // Convert groups to containers
        let mut containers = Vec::new();

        for (dir_name, symbol_ids) in dir_groups {
            if symbol_ids.len() < self.min_symbols_per_container {
                continue;
            }

            let container_type = Self::classify_by_directory(&dir_name);

            let mut technology = Vec::new();
            let mut symbol_count = 0;

            for symbol_id in &symbol_ids {
                if let Some(symbol) = call_graph.get_symbol(symbol_id) {
                    symbol_count += 1;
                    if technology.is_empty() {
                        // Detect technology from symbol kinds
                        let kind = symbol.kind();
                        technology.push(Self::detect_technology(kind));
                    }
                }
            }

            containers.push(Container {
                id: ElementId::new(format!("container-{}", dir_name)),
                name: dir_name.clone(),
                container_type,
                technology: technology.join(", "),
                description: format!("{} container with {} symbols", dir_name, symbol_count),
                path: None,
                components: Vec::new(),
            });
        }

        containers
    }

    /// Classify container type based on directory name
    fn classify_by_directory(dir: &str) -> ContainerType {
        let lower = dir.to_lowercase();

        if lower.contains("api") || lower.contains("service") || lower.contains("server") {
            ContainerType::Service
        } else if lower.contains("db") || lower.contains("data") || lower.contains("store") {
            ContainerType::DataStore
        } else if lower.contains("cli") || lower.contains("cmd") {
            ContainerType::Executable
        } else if lower.contains("queue") || lower.contains("mq") {
            ContainerType::Queue
        } else {
            ContainerType::Library
        }
    }

    /// Detect technology from symbol kind
    fn detect_technology(kind: &cognicode_core::domain::value_objects::SymbolKind) -> String {
        use cognicode_core::domain::value_objects::SymbolKind;
        match kind {
            SymbolKind::Class | SymbolKind::Struct => "Rust".to_string(),
            SymbolKind::Function | SymbolKind::Method => "Rust".to_string(),
            SymbolKind::Trait | SymbolKind::Interface => "Rust Trait".to_string(),
            SymbolKind::Enum => "Rust Enum".to_string(),
            _ => "Rust".to_string(),
        }
    }

    /// Infer container-level relationships from call graph
    pub fn infer_container_relationships(
        &self,
        containers: &[Container],
        call_graph: &CallGraph,
    ) -> Vec<C4Relationship> {
        let mut relationships = Vec::new();

        // Analyze call graph edges for inter-container relationships
        for (source_id, target_id, _dep_type) in call_graph.all_dependencies() {
            let source_symbol = match call_graph.get_symbol(source_id) {
                Some(s) => s,
                None => continue,
            };

            let target_symbol = match call_graph.get_symbol(target_id) {
                Some(s) => s,
                None => continue,
            };

            // Find source and target containers
            let source_file = source_symbol.location().file();
            let target_file = target_symbol.location().file();

            // Find containers for these files
            let source_container = Self::find_container_for_file(source_file, containers);
            let target_container = Self::find_container_for_file(target_file, containers);

            if let (Some(src), Some(tgt)) = (source_container, target_container) {
                // Skip self-references
                if src.id.as_str() == tgt.id.as_str() {
                    continue;
                }

                // Check if relationship already exists
                let exists = relationships.iter().any(|r: &C4Relationship| {
                    r.source_id.as_str() == src.id.as_str()
                        && r.target_id.as_str() == tgt.id.as_str()
                });

                if !exists {
                    relationships.push(C4Relationship::new(
                        src.id.clone(),
                        tgt.id.clone(),
                        C4RelationshipKind::Calls,
                    ));
                }
            }
        }

        relationships
    }

    /// Find which container a file belongs to
    fn find_container_for_file<'a>(file: &str, containers: &'a [Container]) -> Option<&'a Container> {
        for container in containers {
            if let Some(path) = &container.path {
                let container_path = path.to_string_lossy();
                if file.starts_with(&container_path[..container_path.len().saturating_sub(10)]) {
                    return Some(container);
                }
            }
        }
        containers.first()
    }

    /// Get workspace info from project directory
    pub fn get_workspace_info(&self, project_dir: &Path) -> anyhow::Result<Option<WorkspaceInfo>> {
        config_parsers::parse_project(project_dir)
    }
}

impl Default for ContainerInference {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::symbol::Symbol;
    use cognicode_core::domain::value_objects::Location;
    use cognicode_core::domain::value_objects::SymbolKind;
    use cognicode_core::domain::value_objects::DependencyType;

    fn create_test_callgraph() -> CallGraph {
        let mut graph = CallGraph::new();

        // Add symbols from different modules
        let loc1 = Location::new("src/domain/model.rs", 1, 1);
        let sym1 = Symbol::new("User", SymbolKind::Struct, loc1);
        graph.add_symbol(sym1);

        let loc2 = Location::new("src/service/mod.rs", 5, 2);
        let sym2 = Symbol::new("UserService", SymbolKind::Struct, loc2);
        graph.add_symbol(sym2);

        let loc3 = Location::new("src/repository/mod.rs", 10, 3);
        let sym3 = Symbol::new("UserRepository", SymbolKind::Struct, loc3);
        let repo_id = graph.add_symbol(sym3);

        // Service calls repository
        let _ = graph.add_dependency(
            &cognicode_core::domain::aggregates::call_graph::SymbolId::new("src/service/mod.rs:UserService:5:2"),
            &repo_id,
            DependencyType::Calls,
        );

        graph
    }

    #[test]
    fn test_classify_by_directory() {
        assert_eq!(ContainerInference::classify_by_directory("api"), ContainerType::Service);
        assert_eq!(ContainerInference::classify_by_directory("user-service"), ContainerType::Service);
        assert_eq!(ContainerInference::classify_by_directory("database"), ContainerType::DataStore);
        assert_eq!(ContainerInference::classify_by_directory("cli"), ContainerType::Executable);
        assert_eq!(ContainerInference::classify_by_directory("queue"), ContainerType::Queue);
        assert_eq!(ContainerInference::classify_by_directory("library"), ContainerType::Library);
    }

    #[test]
    fn test_infer_containers_from_callgraph() {
        let graph = create_test_callgraph();

        // Create inference with lower threshold for testing
        let inference = ContainerInference {
            min_symbols_per_container: 1,
        };

        let containers = inference.infer_containers_from_callgraph(&graph, Path::new("/test"));

        // Should have grouped by top-level directory
        assert!(!containers.is_empty());
    }
}
