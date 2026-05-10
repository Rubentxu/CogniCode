//! MCP tool handlers for cognicode-diagram integration

use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::call_graph::CallGraph;

use crate::inference::component_inference::ComponentInference;
use crate::inference::config_parsers::detect_and_parse;
use crate::inference::container_inference::ContainerInference;
use crate::inference::engine::InferenceEngine;
use crate::model::c4_types::UmlRelationship;
use crate::render::mermaid::{render_class_diagram, MermaidOptions};
use crate::render::mermaid_c4::{render_component_diagram, render_container_diagram, C4MermaidOptions};

/// Input for the `generate_c4_code` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateC4CodeInput {
    /// Module scope to infer (e.g. "src/domain")
    pub scope: String,
    /// Maximum dependency traversal depth (default: 3)
    pub max_depth: Option<usize>,
    /// Output format: "mermaid" (default), future: "plantuml", "d2", "svg"
    pub format: Option<String>,
    /// Whether to show methods in the diagram (default: true)
    pub show_methods: Option<bool>,
    /// Whether to show attributes in the diagram (default: true)
    pub show_attributes: Option<bool>,
}

/// Output of the `generate_c4_code` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateC4CodeOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// Number of elements in the diagram
    pub element_count: usize,
    /// Number of relationships in the diagram
    pub relationship_count: usize,
}

/// Extract UML relationships from code elements via the inference engine
fn extract_uml_relationships(
    engine: &InferenceEngine,
    elements: &[crate::model::c4_types::CodeElement],
) -> Vec<UmlRelationship> {
    use std::collections::HashMap;
    use crate::model::c4_types::ElementId;

    let element_ids: HashMap<String, ElementId> = elements
        .iter()
        .map(|e| (e.id.as_str().to_string(), e.id.clone()))
        .collect();

    let mut relationships = Vec::new();

    // Use the UML rule engine approach: iterate call graph edges
    for (source_sym_id, target_sym_id, dep_type) in engine.call_graph().all_dependencies() {
        if let (Some(_source), Some(_target)) = (
            element_ids.get(source_sym_id.as_str()),
            element_ids.get(target_sym_id.as_str()),
        ) {
            let (kind, confidence) = match crate::inference::uml_rules::UmlRuleEngine::map_dependency(*dep_type) {
                Some(r) => r,
                None => continue,
            };

            relationships.push(UmlRelationship {
                target_id: _target.clone(),
                kind,
                label: None,
                confidence,
            });
        }
    }

    relationships
}

/// Handle the `generate_c4_code` MCP tool request
///
/// Orchestrates: InferenceEngine → UML relationships → Mermaid renderer
pub fn handle_generate_c4_code(
    input: GenerateC4CodeInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateC4CodeOutput> {
    let max_depth = input.max_depth.unwrap_or(3);
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    // Build inference engine
    let engine = InferenceEngine::new(call_graph);

    // Infer code elements within scope
    let elements = engine.infer_code_elements(&input.scope, max_depth);

    // Extract UML relationships for rendering
    let relationships = extract_uml_relationships(&engine, &elements);

    // Build render options
    let options = MermaidOptions {
        title: format!("C4 Code — {}", input.scope),
        show_methods: input.show_methods.unwrap_or(true),
        show_attributes: input.show_attributes.unwrap_or(true),
        ..MermaidOptions::default()
    };

    // Render (only mermaid supported in Phase 1)
    let diagram = match format.as_str() {
        "mermaid" => render_class_diagram(&elements, &relationships, &options),
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported format '{}'. Only 'mermaid' is supported in Phase 1.",
                other
            ))
        }
    };

    let element_count = elements.len();
    let relationship_count = relationships.len();

    Ok(GenerateC4CodeOutput {
        diagram,
        format,
        element_count,
        relationship_count,
    })
}

// =============================================================================
// L2 Container Diagram Tools
// =============================================================================

/// Input for the `generate_c4_containers` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateC4ContainersInput {
    /// Project directory to analyze (default: ".")
    pub directory: Option<String>,
    /// Output format: "mermaid" (default)
    pub format: Option<String>,
    /// Show coupling scores between containers
    pub show_coupling: Option<bool>,
    /// Show technology stack labels
    pub show_technology: Option<bool>,
}

/// Output of the `generate_c4_containers` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateC4ContainersOutput {
    pub diagram: String,
    pub format: String,
    pub container_count: usize,
    pub relationship_count: usize,
}

/// Handle `generate_c4_containers` — L2 container diagram
pub fn handle_generate_c4_containers(
    input: GenerateC4ContainersInput,
    project_dir: &std::path::Path,
    call_graph: Option<&CallGraph>,
) -> anyhow::Result<GenerateC4ContainersOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    // Detect and parse containers from project config files
    let mut containers = detect_and_parse(project_dir)?;

    // If CallGraph provided, enrich with symbol counts
    if let Some(cg) = call_graph {
        let inference = ContainerInference::new();
        inference.enrich_containers_with_callgraph(&mut containers, cg);
    }

    // Build a minimal C4Workspace for rendering
    let project_name = project_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");

    let mut workspace = crate::model::workspace::C4Workspace::new(project_name);
    workspace.description = format!("{} containers", containers.len());

    // Create system with all containers
    let system = crate::model::c4_types::SoftwareSystem {
        id: crate::model::c4_types::ElementId::new("sys_main"),
        name: project_name.to_string(),
        description: "Main system".to_string(),
        location: crate::model::c4_types::ElementLocation::Internal,
        containers: containers.clone(),
    };
    workspace.model.systems.push(system);

    // Infer relationships if CallGraph available
    let relationships = if let Some(cg) = call_graph {
        let inference = ContainerInference::new();
        inference.infer_container_relationships(&containers, cg)
    } else {
        Vec::new()
    };

    let options = C4MermaidOptions {
        show_technology: input.show_technology.unwrap_or(true),
        show_component_count: input.show_coupling.unwrap_or(false),
        ..C4MermaidOptions::default()
    };

    let diagram = render_container_diagram(&workspace, &options);

    Ok(GenerateC4ContainersOutput {
        diagram,
        format,
        container_count: containers.len(),
        relationship_count: relationships.len(),
    })
}

// =============================================================================
// L3 Component Diagram Tools
// =============================================================================

/// Input for the `generate_c4_components` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateC4ComponentsInput {
    /// Module scope to analyze (e.g. "src/domain")
    pub scope: String,
    /// Container name to group components (optional)
    pub container_name: Option<String>,
    /// Output format: "mermaid" (default)
    pub format: Option<String>,
    /// Detail level: "high" (default) shows methods/fields
    pub detail_level: Option<String>,
}

/// Output of the `generate_c4_components` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateC4ComponentsOutput {
    pub diagram: String,
    pub format: String,
    pub component_count: usize,
    pub relationship_count: usize,
}

/// Handle `generate_c4_components` — L3 component diagram
pub fn handle_generate_c4_components(
    input: GenerateC4ComponentsInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateC4ComponentsOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    let inference = ComponentInference::new();
    let components = inference.infer_components(call_graph, &input.scope);
    let relationships = inference.infer_component_relationships(call_graph, &components);

    // Wrap components in a container for rendering
    let container = crate::model::c4_types::Container {
        id: crate::model::c4_types::ElementId::new("container_default"),
        name: input.container_name.unwrap_or_else(|| "Default Container".to_string()),
        container_type: crate::model::c4_types::ContainerType::Service,
        technology: "Rust".to_string(),
        description: format!("Components in {}", input.scope),
        path: None,
        components: components.clone(),
    };

    let options = C4MermaidOptions {
        show_technology: true,
        show_component_count: true,
        ..C4MermaidOptions::default()
    };

    let diagram = render_component_diagram(&[container], &relationships, &options);

    Ok(GenerateC4ComponentsOutput {
        diagram,
        format,
        component_count: components.len(),
        relationship_count: relationships.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    #[test]
    fn test_handle_empty_call_graph() {
        let call_graph = CallGraph::new();
        let input = GenerateC4CodeInput {
            scope: "src".to_string(),
            max_depth: None,
            format: None,
            show_methods: None,
            show_attributes: None,
        };

        let result = handle_generate_c4_code(input, &call_graph).unwrap();
        assert_eq!(result.format, "mermaid");
        assert_eq!(result.element_count, 0);
        assert!(result.diagram.contains("classDiagram"));
    }

    #[test]
    fn test_unsupported_format() {
        let call_graph = CallGraph::new();
        let input = GenerateC4CodeInput {
            scope: "src".to_string(),
            max_depth: None,
            format: Some("plantuml".to_string()),
            show_methods: None,
            show_attributes: None,
        };

        let result = handle_generate_c4_code(input, &call_graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported format"));
    }
}
