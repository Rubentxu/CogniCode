//! Component Inference (L3) — extracts C4 Components from CallGraph module structure
//!
//! Groups symbols by directory/module prefix and classifies them into ComponentTypes
//! based on directory conventions: domain/, infrastructure/, interface/, application/

use std::collections::HashMap;

use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::dependency_type::DependencyType;

use crate::model::c4_types::{Component, ComponentType, ElementId};
use crate::model::relationships::{C4Relationship, C4RelationshipKind};

/// Infers C4 Components (L3) from the CallGraph's module structure
#[derive(Debug, Clone)]
pub struct ComponentInference {
    /// Minimum symbols required to create a component
    min_symbols_per_component: usize,
}

impl ComponentInference {
    pub fn new() -> Self {
        Self {
            min_symbols_per_component: 1,
        }
    }

    /// Infer components from the CallGraph module structure
    ///
    /// `scope` is the root path to analyze (e.g., "src/domain")
    pub fn infer_components(&self, call_graph: &CallGraph, scope: &str) -> Vec<Component> {
        // Group symbols by module prefix
        let mut module_groups: HashMap<String, Vec<SymbolId>> = HashMap::new();

        for (symbol_id, symbol) in call_graph.symbol_ids() {
            let file = symbol.location().file();

            // Skip symbols not in scope
            if !file.contains(scope) {
                continue;
            }

            // Extract module path from file path
            let module_prefix = self.extract_module_prefix(file, scope);

            module_groups
                .entry(module_prefix)
                .or_default()
                .push(symbol_id.clone());
        }

        // Convert groups to components
        let mut components = Vec::new();

        for (module_path, symbol_ids) in module_groups {
            if symbol_ids.len() < self.min_symbols_per_component {
                continue;
            }

            let component_type = Self::classify_module(&module_path);
            let name = Self::module_to_component_name(&module_path);
            let description = format!("{} module", name);

            // Collect code elements for this component
            let mut _code_element_count = 0;
            let mut technology = String::new();

            for symbol_id in &symbol_ids {
                if let Some(symbol) = call_graph.get_symbol(symbol_id) {
                    if symbol.is_type_definition() {
                        _code_element_count += 1;
                    }
                    // Detect technology from symbol kind
                    if technology.is_empty() {
                        technology = Self::detect_technology_from_kind(symbol.kind());
                    }
                }
            }

            let component = Component {
                id: ElementId::new(format!("component-{}", module_path.replace('/', "-"))),
                name,
                component_type,
                technology,
                description,
                path: None,
                code_elements: Vec::new(), // Code elements filled by L4 inference
            };

            components.push(component);
        }

        components
    }

    /// Classify a module path into a ComponentType
    fn classify_module(module: &str) -> ComponentType {
        let lower = module.to_lowercase();

        if lower.contains("domain") {
            ComponentType::Module // Domain layer
        } else if lower.contains("infrastructure") || lower.contains("repo") {
            ComponentType::Repository
        } else if lower.contains("interface") || lower.contains("adapter") || lower.contains("controller") {
            ComponentType::Controller
        } else if lower.contains("application") || lower.contains("service") || lower.contains("usecase") {
            ComponentType::Service
        } else if lower.contains("interface") && lower.contains("trait") {
            ComponentType::Interface
        } else {
            ComponentType::Module
        }
    }

    /// Extract module prefix from file path
    fn extract_module_prefix(&self, file: &str, scope: &str) -> String {
        // Find the scope in the file path and extract the module portion
        if let Some(pos) = file.find(scope) {
            let after_scope = &file[pos + scope.len()..];
            // Remove leading slash and file name
            let module_path = after_scope.trim_start_matches('/');
            // Get directory portion (remove file name)
            if let Some(last_slash) = module_path.rfind('/') {
                module_path[..last_slash].to_string()
            } else {
                module_path.to_string()
            }
        } else {
            file.to_string()
        }
    }

    /// Convert module path to a human-readable component name
    fn module_to_component_name(module: &str) -> String {
        if module.is_empty() {
            return "Root".to_string();
        }

        // Take the last segment of the path
        module
            .split('/')
            .last()
            .unwrap_or(module)
            .split('_')
            .map(|s| {
                let mut chars = s.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Detect technology stack from symbol kind
    fn detect_technology_from_kind(kind: &cognicode_core::domain::value_objects::SymbolKind) -> String {
        use cognicode_core::domain::value_objects::SymbolKind;
        match kind {
            SymbolKind::Class | SymbolKind::Struct => "Rust".to_string(),
            SymbolKind::Function | SymbolKind::Method => "Rust".to_string(),
            SymbolKind::Trait | SymbolKind::Interface => "Rust Trait".to_string(),
            SymbolKind::Enum => "Rust Enum".to_string(),
            _ => "Rust".to_string(),
        }
    }

    /// Infer relationships between components based on call graph edges
    pub fn infer_component_relationships(
        &self,
        call_graph: &CallGraph,
        components: &[Component],
    ) -> Vec<C4Relationship> {
        let mut relationships = Vec::new();

        // Build component index by name for quick lookup
        let _component_ids: HashMap<String, &Component> = components
            .iter()
            .map(|c| (c.name.clone(), c))
            .collect();

        // Analyze call graph edges to find inter-component relationships
        for (source_id, target_id, dep_type) in call_graph.all_dependencies() {
            let source_symbol = match call_graph.get_symbol(source_id) {
                Some(s) => s,
                None => continue,
            };

            let target_symbol = match call_graph.get_symbol(target_id) {
                Some(s) => s,
                None => continue,
            };

            // Find which components these symbols belong to
            let source_component = self.find_component_for_symbol(source_symbol, components);
            let target_component = self.find_component_for_symbol(target_symbol, components);

            if let (Some(src_comp), Some(tgt_comp)) = (source_component, target_component) {
                // Don't create self-referential relationships
                if src_comp.id.as_str() == tgt_comp.id.as_str() {
                    continue;
                }

                // Check if relationship already exists
                let rel_exists = relationships.iter().any(|r: &C4Relationship| {
                    r.source_id.as_str() == src_comp.id.as_str()
                        && r.target_id.as_str() == tgt_comp.id.as_str()
                });

                if rel_exists {
                    continue;
                }

                // Map dependency type to relationship kind
                let rel_kind = match dep_type {
                    DependencyType::Calls => C4RelationshipKind::Calls,
                    DependencyType::Imports => C4RelationshipKind::Uses,
                    DependencyType::Contains => continue, // Skip containment relationships
                    DependencyType::Inherits => C4RelationshipKind::Inherits,
                    DependencyType::References => C4RelationshipKind::Uses,
                    DependencyType::UsesGeneric => C4RelationshipKind::Uses,
                    DependencyType::Defines => C4RelationshipKind::Composes,
                    DependencyType::AnnotatedBy => C4RelationshipKind::Aggregates,
                };

                relationships.push(C4Relationship::new(
                    src_comp.id.clone(),
                    tgt_comp.id.clone(),
                    rel_kind,
                ));
            }
        }

        relationships
    }

    /// Find which component a symbol belongs to
    fn find_component_for_symbol<'a>(
        &self,
        symbol: &cognicode_core::domain::aggregates::symbol::Symbol,
        components: &'a [Component],
    ) -> Option<&'a Component> {
        let file = symbol.location().file();

        for component in components {
            // Simple heuristic: check if component name appears in the file path
            if file.contains(&component.name.to_lowercase())
                || component.name.to_lowercase().contains("root")
            {
                return Some(component);
            }
        }

        // Fallback: return first component (root)
        components.first()
    }
}

impl Default for ComponentInference {
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

    fn create_test_callgraph() -> CallGraph {
        let mut graph = CallGraph::new();

        // Add domain symbols
        let domain_loc = Location::new("src/domain/model.rs", 1, 1);
        let domain_symbol = Symbol::new("User", SymbolKind::Struct, domain_loc);
        graph.add_symbol(domain_symbol);

        // Add infrastructure symbols
        let infra_loc = Location::new("src/infrastructure/repository.rs", 10, 5);
        let infra_symbol = Symbol::new("UserRepository", SymbolKind::Struct, infra_loc);
        let infra_id = graph.add_symbol(infra_symbol);

        // Add interface symbols
        let interface_loc = Location::new("src/interface/http.rs", 20, 3);
        let interface_symbol = Symbol::new("UserController", SymbolKind::Struct, interface_loc);
        let interface_id = graph.add_symbol(interface_symbol);

        // Add application symbols
        let app_loc = Location::new("src/application/service.rs", 30, 7);
        let app_symbol = Symbol::new("UserService", SymbolKind::Struct, app_loc);
        let app_id = graph.add_symbol(app_symbol);

        // Add dependencies
        let _ = graph.add_dependency(&interface_id, &app_id, DependencyType::Calls);
        let _ = graph.add_dependency(&app_id, &infra_id, DependencyType::Calls);

        graph
    }

    #[test]
    fn test_infer_components() {
        let graph = create_test_callgraph();
        let inference = ComponentInference::new();

        let components = inference.infer_components(&graph, "src");

        // Should find components for each module
        assert!(!components.is_empty());

        // Check that we have different component types
        let types: Vec<_> = components.iter().map(|c| c.component_type).collect();
        assert!(types.contains(&ComponentType::Repository));
        assert!(types.contains(&ComponentType::Controller));
    }

    #[test]
    fn test_classify_module() {
        assert_eq!(
            ComponentInference::classify_module("domain/model"),
            ComponentType::Module
        );
        assert_eq!(
            ComponentInference::classify_module("infrastructure/repository"),
            ComponentType::Repository
        );
        assert_eq!(
            ComponentInference::classify_module("interface/http"),
            ComponentType::Controller
        );
        assert_eq!(
            ComponentInference::classify_module("application/service"),
            ComponentType::Service
        );
    }

    #[test]
    fn test_module_to_component_name() {
        assert_eq!(ComponentInference::module_to_component_name("domain"), "Domain");
        assert_eq!(ComponentInference::module_to_component_name("user_service"), "UserService");
        assert_eq!(ComponentInference::module_to_component_name(""), "Root");
    }

    #[test]
    fn test_infer_component_relationships() {
        let graph = create_test_callgraph();
        let inference = ComponentInference::new();

        let components = inference.infer_components(&graph, "src");
        let relationships = inference.infer_component_relationships(&graph, &components);

        // Should find some relationships
        assert!(!relationships.is_empty());
    }
}
