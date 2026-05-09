//! Main inference engine — orchestrates C4 model extraction from CallGraph

use std::collections::HashMap;

use cognicode_core::domain::aggregates::call_graph::CallGraph;
use cognicode_core::domain::value_objects::dependency_type::DependencyType;
use cognicode_core::domain::value_objects::symbol_kind::SymbolKind;

use crate::model::c4_types::{
    CodeElement, CodeElementKind, Container, ContainerType,
    ElementId, ElementLocation, Person, SoftwareSystem, Visibility,
};
use crate::model::relationships::{C4Relationship, C4RelationshipKind};
use crate::model::workspace::C4Workspace;

use super::code_inference::CodeInference;
use super::uml_rules::UmlRuleEngine;

/// Main inference engine — extracts C4 model elements from code analysis
pub struct InferenceEngine<'a> {
    call_graph: &'a CallGraph,
    code_inference: CodeInference,
    uml_rules: UmlRuleEngine,
}

impl<'a> InferenceEngine<'a> {
    /// Create a new inference engine from a CallGraph reference
    pub fn new(call_graph: &'a CallGraph) -> Self {
        Self {
            call_graph,
            code_inference: CodeInference::new(),
            uml_rules: UmlRuleEngine::new(),
        }
    }

    /// Access the underlying CallGraph reference
    pub fn call_graph(&self) -> &CallGraph {
        self.call_graph
    }

    /// Full inference pipeline — produces a complete C4Workspace
    pub fn infer_workspace(&self, project_name: &str) -> C4Workspace {
        let mut workspace = C4Workspace::new(project_name);

        // Infer L1 — People and external systems
        self.infer_people(&mut workspace);
        self.infer_systems(&mut workspace);

        // Infer L2 — Containers (crates)
        self.infer_containers(&mut workspace);

        // Infer relationships
        self.infer_workspace_relationships(&mut workspace);

        workspace
    }

    /// Infer code elements (L4) within a scope
    pub fn infer_code_elements(&self, scope: &str, max_depth: usize) -> Vec<CodeElement> {
        self.code_inference.infer_scope(scope, self.call_graph, max_depth)
    }

    /// Infer C4Relationships between CodeElements
    pub fn infer_relationships(&self, elements: &[CodeElement]) -> Vec<C4Relationship> {
        let element_ids: HashMap<String, ElementId> = elements
            .iter()
            .map(|e| (e.id.as_str().to_string(), e.id.clone()))
            .collect();

        let mut relationships = Vec::new();

        // Iterate all edges in the call graph
        for (source_sym_id, target_sym_id, dep_type) in self.call_graph.all_dependencies() {
            if let Some(source_id) = element_ids.get(source_sym_id.as_str()) {
                if let Some(target_id) = element_ids.get(target_sym_id.as_str()) {
                    if let Some(c4_kind) = Self::map_dependency_type(*dep_type) {
                        let confidence = Self::dependency_confidence(*dep_type);
                        relationships.push(
                            C4Relationship::new(
                                source_id.clone(),
                                target_id.clone(),
                                c4_kind,
                            )
                            .with_confidence(confidence),
                        );
                    }
                }
            }
        }

        relationships
    }

    /// Map SymbolKind from cognicode-core to CodeElementKind
    pub fn map_symbol_kind(kind: SymbolKind) -> CodeElementKind {
        CodeInference::map_symbol_kind(kind)
    }

    /// Map DependencyType to C4RelationshipKind
    pub fn map_dependency_type(dep: DependencyType) -> Option<C4RelationshipKind> {
        match dep {
            DependencyType::Calls => Some(C4RelationshipKind::Calls),
            DependencyType::Imports => Some(C4RelationshipKind::DependsOn),
            DependencyType::Inherits => Some(C4RelationshipKind::Inherits),
            DependencyType::UsesGeneric => Some(C4RelationshipKind::Uses),
            DependencyType::References => Some(C4RelationshipKind::DependsOn),
            DependencyType::Defines => None,
            DependencyType::AnnotatedBy => Some(C4RelationshipKind::Uses),
            DependencyType::Contains => Some(C4RelationshipKind::Composes),
        }
    }

    /// Get confidence score for a dependency type
    fn dependency_confidence(dep: DependencyType) -> f64 {
        match dep {
            DependencyType::Inherits => 1.0,
            DependencyType::Contains => 0.9,
            DependencyType::UsesGeneric => 0.8,
            DependencyType::References => 0.7,
            DependencyType::Calls => 0.6,
            DependencyType::Imports => 0.5,
            DependencyType::AnnotatedBy => 0.4,
            DependencyType::Defines => 0.0,
        }
    }

    /// Map visibility string to Visibility enum
    pub fn map_visibility(vis: Option<&str>) -> Visibility {
        CodeInference::map_visibility(vis)
    }

    // --- Private inference methods ---

    /// Infer people (actors) from entry points and external dependencies
    fn infer_people(&self, workspace: &mut C4Workspace) {
        // Entry points (roots) represent external actors
        let roots = self.call_graph.roots();
        if !roots.is_empty() {
            let person = Person {
                id: ElementId::new("actor_user"),
                name: "User".to_string(),
                description: "End user of the system".to_string(),
                location: ElementLocation::External,
            };
            workspace.model.people.push(person);
        }
    }

    /// Infer software systems from module structure
    fn infer_systems(&self, workspace: &mut C4Workspace) {
        // The project itself is the internal system
        let system = SoftwareSystem {
            id: ElementId::new("sys_main"),
            name: workspace.name.clone(),
            description: "Main software system".to_string(),
            location: ElementLocation::Internal,
            containers: Vec::new(),
        };

        // External dependencies are external systems
        let modules = self.call_graph.modules();
        for module in &modules {
            // Simple heuristic: std/core/alloc are external
            if module.contains("std") || module.contains("core") || module.contains("alloc") {
                let ext_id = ElementId::new(format!("sys_ext_{}", module.replace("::", "_")));
                let ext_system = SoftwareSystem {
                    id: ext_id,
                    name: module.clone(),
                    description: format!("External dependency: {}", module),
                    location: ElementLocation::External,
                    containers: Vec::new(),
                };
                workspace.model.systems.push(ext_system);
            }
        }

        workspace.model.systems.insert(0, system);
    }

    /// Infer containers (crates/modules) from module structure
    fn infer_containers(&self, workspace: &mut C4Workspace) {
        let (modules_deps, _, _) = self.call_graph.find_module_dependencies();

        // Each top-level module becomes a container in the main system
        if let Some(main_system) = workspace.model.systems.first_mut() {
            for (module_name, _deps, _dependents, size) in &modules_deps {
                let container_id = ElementId::new(format!("container_{}", module_name.replace("::", "_")));
                let container = Container {
                    id: container_id,
                    name: module_name.clone(),
                    container_type: ContainerType::Library,
                    technology: "Rust".to_string(),
                    description: format!("Module: {} ({} symbols)", module_name, size),
                    path: None,
                    components: Vec::new(),
                };
                main_system.containers.push(container);
            }
        }
    }

    /// Infer relationships between workspace elements
    fn infer_workspace_relationships(&self, workspace: &mut C4Workspace) {
        let (modules_deps, _, dep_counts) = self.call_graph.find_module_dependencies();

        // Add container-level relationships
        for (module, deps, _, _) in &modules_deps {
            let source_key = format!("container_{}", module.replace("::", "_"));
            let source_id = ElementId::new(&source_key);

            for dep in deps {
                let target_key = format!("container_{}", dep.replace("::", "_"));
                let target_id = ElementId::new(&target_key);

                let count = dep_counts
                    .get(&(module.clone(), dep.clone()))
                    .copied()
                    .unwrap_or(1);

                workspace.model.relationships.push(
                    C4Relationship::new(source_id.clone(), target_id, C4RelationshipKind::DependsOn)
                        .with_label(format!("{} deps", count))
                        .with_confidence(0.8),
                );
            }
        }

        // Person → System relationship
        if !workspace.model.people.is_empty() && !workspace.model.systems.is_empty() {
            let person_id = workspace.model.people[0].id.clone();
            let system_id = workspace.model.systems[0].id.clone();
            workspace.model.relationships.push(
                C4Relationship::new(person_id, system_id, C4RelationshipKind::Uses)
                    .with_label("interacts with"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_dependency_type() {
        assert_eq!(
            InferenceEngine::map_dependency_type(DependencyType::Calls),
            Some(C4RelationshipKind::Calls)
        );
        assert_eq!(
            InferenceEngine::map_dependency_type(DependencyType::Inherits),
            Some(C4RelationshipKind::Inherits)
        );
        assert_eq!(
            InferenceEngine::map_dependency_type(DependencyType::Defines),
            None
        );
    }

    #[test]
    fn test_map_symbol_kind() {
        assert_eq!(
            InferenceEngine::map_symbol_kind(SymbolKind::Class),
            CodeElementKind::Class
        );
        assert_eq!(
            InferenceEngine::map_symbol_kind(SymbolKind::Struct),
            CodeElementKind::Struct
        );
        assert_eq!(
            InferenceEngine::map_symbol_kind(SymbolKind::Enum),
            CodeElementKind::Enum
        );
    }

    #[test]
    fn test_infer_empty_workspace() {
        let cg = CallGraph::new();
        let engine = InferenceEngine::new(&cg);
        let workspace = engine.infer_workspace("EmptyProject");
        assert_eq!(workspace.name, "EmptyProject");
        // No roots → no people
        assert!(workspace.model.people.is_empty());
        // Main system always present
        assert!(!workspace.model.systems.is_empty());
        assert_eq!(workspace.model.systems[0].name, "EmptyProject");
    }

    #[test]
    fn test_infer_code_elements_empty() {
        let cg = CallGraph::new();
        let engine = InferenceEngine::new(&cg);
        let elements = engine.infer_code_elements("src", 3);
        assert!(elements.is_empty());
    }

    #[test]
    fn test_infer_relationships_empty() {
        let cg = CallGraph::new();
        let engine = InferenceEngine::new(&cg);
        let elements = vec![];
        let rels = engine.infer_relationships(&elements);
        assert!(rels.is_empty());
    }
}
