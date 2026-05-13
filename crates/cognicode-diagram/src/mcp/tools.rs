//! MCP tool handlers for cognicode-diagram integration

use std::time::Instant;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use cognicode_core::domain::aggregates::call_graph::CallGraph;

use crate::inference::component_inference::ComponentInference;
use crate::inference::config_parsers::detect_and_parse;
use crate::inference::container_inference::ContainerInference;
use crate::inference::context_inference::ContextInference;
use crate::inference::engine::InferenceEngine;
use crate::model::c4_types::{ElementId, ElementLocation, UmlRelationship};
use crate::model::workspace::C4Workspace;
use crate::render::mermaid::{render_c4_context, render_class_diagram, MermaidOptions};
use crate::render::mermaid_c4::{render_component_diagram, render_container_diagram, C4MermaidOptions};
use crate::render::plantuml::{render_plantuml_c4, PlantUmlOptions, PlantUmlViewType};
use crate::render::structurizr_dsl::{render_structurizr_dsl, StructurizrDslOptions};
use crate::render::sequence::{
    find_entry_points, render_sequence_diagram, render_sequence_diagram_plantuml,
    render_sequence_diagram_svg, SequenceDiagramOptions, SequenceSvgOptions,
};
use crate::render::d2::{render_d2, D2Options};
use crate::inference::state_machine_inference::{
    find_state_machines, infer_state_machine_from_struct, StateMachineInferenceOptions,
};
use crate::inference::activity_inference::{
    find_activities, infer_activity_from_function, ActivityInferenceOptions,
};
use crate::render::state_machine::{
    render_state_machine_mermaid, render_state_machine_plantuml, render_empty_state_machine,
    StateMachineRenderOptions,
};
use crate::render::activity::{
    render_activity_mermaid, render_activity_plantuml, render_empty_activity,
    ActivityRenderOptions,
};

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

// =============================================================================
// Reverse Engineer C4 - Meta Tool
// =============================================================================

/// Input for the `reverse_engineer_c4` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct ReverseEngineerC4Input {
    /// Project directory to analyze (default: ".")
    pub directory: Option<String>,
    /// Which C4 levels to generate: ["L1", "L2", "L3", "L4"]
    /// Default: ["L1", "L2", "L3"]
    pub levels: Option<Vec<String>>,
    /// Output format(s): "mermaid", "plantuml", "dsl", "all"
    /// Default: "mermaid"
    pub format: Option<String>,
    /// Output directory — if provided, writes files (optional)
    pub output_dir: Option<String>,
    /// Maximum depth for L4 code inference (default: 3)
    pub max_depth: Option<usize>,
}

/// Output of the `reverse_engineer_c4` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct ReverseEngineerC4Output {
    /// Generated diagrams keyed by level + format
    pub diagrams: IndexMap<String, DiagramOutput>,
    /// Summary of elements detected per level
    pub element_counts: IndexMap<String, ElementCount>,
    /// Files written (if output_dir was provided)
    pub files_written: Vec<String>,
    /// Total time taken in milliseconds
    pub elapsed_ms: u64,
}

/// A single diagram output
#[derive(Debug, Clone, Serialize)]
pub struct DiagramOutput {
    pub level: String,
    pub format: String,
    pub diagram: String,
    pub element_count: usize,
    pub relationship_count: usize,
}

/// Count of elements detected
#[derive(Debug, Clone, Serialize)]
pub struct ElementCount {
    pub people: usize,
    pub systems: usize,
    pub containers: usize,
    pub components: usize,
    pub code_elements: usize,
}

/// Handle the `reverse_engineer_c4` meta-tool
///
/// Orchestrates the complete reverse engineering pipeline — L1 + L2 + L3 + L4
/// in a single call, outputting multiple formats.
pub fn handle_reverse_engineer_c4(
    input: ReverseEngineerC4Input,
    project_dir: &std::path::Path,
    call_graph: Option<&CallGraph>,
) -> anyhow::Result<ReverseEngineerC4Output> {
    let start_time = Instant::now();

    // Parse input
    let _directory = input.directory.as_deref().unwrap_or(".");
    let levels = input.levels.as_ref().map(|v| v.clone()).unwrap_or_else(|| {
        vec!["L1".to_string(), "L2".to_string(), "L3".to_string()]
    });
    let formats = parse_formats(input.format.as_deref().unwrap_or("mermaid"));
    let max_depth = input.max_depth.unwrap_or(3);

    // Build call graph if not provided
    let call_graph = match call_graph {
        Some(cg) => Some(cg),
        None => {
            // Create empty call graph if none provided
            Some(&CallGraph::new())
        }
    };

    // Initialize result structures
    let mut diagrams: IndexMap<String, DiagramOutput> = IndexMap::new();
    let mut element_counts: IndexMap<String, ElementCount> = IndexMap::new();
    let mut files_written = Vec::new();

    // Detect project config for L2
    let containers = detect_and_parse(project_dir)?;

    // Infer L1 context
    if levels.contains(&"L1".to_string()) {
        let l1_counts = infer_l1(project_dir, call_graph, &mut diagrams, &formats, &mut files_written)?;
        element_counts.insert("L1".to_string(), l1_counts);
    }

    // Infer L2 containers
    if levels.contains(&"L2".to_string()) {
        let l2_counts = infer_l2(project_dir, &containers, call_graph, &mut diagrams, &formats, &mut files_written)?;
        element_counts.insert("L2".to_string(), l2_counts);
    }

    // Infer L3 components
    if levels.contains(&"L3".to_string()) {
        let l3_counts = infer_l3(&containers, call_graph, &mut diagrams, &formats, &mut files_written)?;
        element_counts.insert("L3".to_string(), l3_counts);
    }

    // Infer L4 code elements
    if levels.contains(&"L4".to_string()) {
        let l4_counts = infer_l4(call_graph, max_depth, &mut diagrams, &formats, &mut files_written)?;
        element_counts.insert("L4".to_string(), l4_counts);
    }

    // Write files if output_dir is provided
    if let Some(ref output_dir) = input.output_dir {
        let output_path = std::path::Path::new(output_dir);
        if !output_path.exists() {
            std::fs::create_dir_all(output_path)?;
        }

        for (_key, diagram_output) in &diagrams {
            let extension = match diagram_output.format.as_str() {
                "mermaid" => "mmd",
                "plantuml" => "puml",
                "dsl" => "dsl",
                "d2" => "d2",
                _ => "txt",
            };
            let level = diagram_output.level.to_lowercase();
            let filename = format!("{}_{}.{}", level, diagram_output.format, extension);
            let file_path = output_path.join(&filename);

            std::fs::write(&file_path, &diagram_output.diagram)?;
            files_written.push(file_path.to_string_lossy().to_string());
        }
    }

    let elapsed_ms = start_time.elapsed().as_millis() as u64;

    Ok(ReverseEngineerC4Output {
        diagrams,
        element_counts,
        files_written,
        elapsed_ms,
    })
}

/// Parse format string into list of formats
fn parse_formats(format: &str) -> Vec<String> {
    match format {
        "all" => vec!["mermaid".to_string(), "plantuml".to_string(), "dsl".to_string(), "d2".to_string()],
        _ => vec![format.to_string()],
    }
}

/// Infer L1 (Context) diagram
fn infer_l1(
    project_dir: &std::path::Path,
    call_graph: Option<&CallGraph>,
    diagrams: &mut IndexMap<String, DiagramOutput>,
    formats: &[String],
    _files_written: &mut Vec<String>,
) -> anyhow::Result<ElementCount> {
    // Get context from ContextInference
    let context_inference = ContextInference::new();
    let system = context_inference.infer_context(project_dir, call_graph)?;

    // Get actors and external systems
    let actors = context_inference.get_detected_actors(project_dir);
    let externals = context_inference.get_detected_external_systems(project_dir);

    // Build L1 workspace
    let mut workspace = C4Workspace::new(
        project_dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Project")
    );
    workspace.description = "System Context".to_string();

    // Add people
    for actor in &actors {
        workspace.model.people.push(actor.clone());
    }

    // Add systems (internal first, then external)
    let mut internal_system = system.clone();
    internal_system.location = ElementLocation::Internal;
    internal_system.containers = Vec::new();
    workspace.model.systems.push(internal_system);

    for external in externals {
        workspace.model.systems.push(external);
    }

    // Infer relationships
    let relationships = context_inference.infer_context_relationships(
        &system,
        &actors,
        &workspace.model.systems.iter().filter(|s| s.location == ElementLocation::External).cloned().collect::<Vec<_>>()
    );
    workspace.model.relationships = relationships;

    // Count elements
    let counts = ElementCount {
        people: actors.len(),
        systems: workspace.model.systems.len(),
        containers: 0,
        components: 0,
        code_elements: 0,
    };

    // Render in requested formats
    for format in formats {
        let diagram = render_diagram_for_workspace(&workspace, "L1", format)?;
        let relationship_count = workspace.model.relationships.len();

        diagrams.insert(format!("L1_{}", format), DiagramOutput {
            level: "L1".to_string(),
            format: format.clone(),
            diagram,
            element_count: counts.people + counts.systems,
            relationship_count,
        });
    }

    Ok(counts)
}

/// Infer L2 (Container) diagram
fn infer_l2(
    project_dir: &std::path::Path,
    containers: &[crate::model::c4_types::Container],
    call_graph: Option<&CallGraph>,
    diagrams: &mut IndexMap<String, DiagramOutput>,
    formats: &[String],
    _files_written: &mut Vec<String>,
) -> anyhow::Result<ElementCount> {
    // Build L2 workspace
    let project_name = project_dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");

    let mut workspace = C4Workspace::new(project_name);
    workspace.description = "Container View".to_string();

    // Create system with containers
    let system = crate::model::c4_types::SoftwareSystem {
        id: ElementId::new("sys_main"),
        name: project_name.to_string(),
        description: "Main system".to_string(),
        location: ElementLocation::Internal,
        containers: containers.to_vec(),
    };
    workspace.model.systems.push(system);

    // Infer relationships if call graph available
    if let Some(cg) = call_graph {
        let container_inference = ContainerInference::new();
        let relationships = container_inference.infer_container_relationships(containers, cg);
        workspace.model.relationships = relationships;
    }

    let container_count = containers.len();
    let counts = ElementCount {
        people: 0,
        systems: 1,
        containers: container_count,
        components: 0,
        code_elements: 0,
    };

    // Render in requested formats
    for format in formats {
        let diagram = render_diagram_for_workspace(&workspace, "L2", format)?;
        let relationship_count = workspace.model.relationships.len();

        diagrams.insert(format!("L2_{}", format), DiagramOutput {
            level: "L2".to_string(),
            format: format.clone(),
            diagram,
            element_count: container_count,
            relationship_count,
        });
    }

    Ok(counts)
}

/// Infer L3 (Component) diagram
fn infer_l3(
    containers: &[crate::model::c4_types::Container],
    call_graph: Option<&CallGraph>,
    diagrams: &mut IndexMap<String, DiagramOutput>,
    formats: &[String],
    _files_written: &mut Vec<String>,
) -> anyhow::Result<ElementCount> {
    // Use ComponentInference to get components
    let component_inference = ComponentInference::new();

    // Infer components from call graph (if available)
    let mut all_components: Vec<crate::model::c4_types::Component> = Vec::new();

    if let Some(cg) = call_graph {
        for container in containers {
            // Use container path as scope
            let scope = container.path
                .as_ref()
                .and_then(|p| p.parent())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "src".to_string());

            let components = component_inference.infer_components(cg, &scope);
            all_components.extend(components);
        }
    }

    let component_count = all_components.len();
    let counts = ElementCount {
        people: 0,
        systems: 0,
        containers: containers.len(),
        components: component_count,
        code_elements: 0,
    };

    // Build a minimal workspace for rendering
    let mut workspace = C4Workspace::new("Components");
    workspace.description = "Component View".to_string();

    // Create containers with components
    for container in containers {
        let mut c = container.clone();
        c.components = all_components.iter()
            .filter(|comp| {
                // Filter components that belong to this container
                comp.id.as_str().contains(&container.name) || comp.id.as_str().contains("component")
            })
            .cloned()
            .collect();
        workspace.model.systems.push(crate::model::c4_types::SoftwareSystem {
            id: ElementId::new(format!("sys_{}", container.name.to_lowercase())),
            name: container.name.clone(),
            description: container.description.clone(),
            location: ElementLocation::Internal,
            containers: vec![c],
        });
    }

    // Infer component relationships
    let relationships = if let Some(cg) = call_graph {
        component_inference.infer_component_relationships(cg, &all_components)
    } else {
        Vec::new()
    };

    // Render in requested formats
    for format in formats {
        let diagram = render_diagram_for_workspace(&workspace, "L3", format)?;
        let relationship_count = relationships.len();

        diagrams.insert(format!("L3_{}", format), DiagramOutput {
            level: "L3".to_string(),
            format: format.clone(),
            diagram,
            element_count: component_count,
            relationship_count,
        });
    }

    Ok(counts)
}

/// Infer L4 (Code) diagram
fn infer_l4(
    call_graph: Option<&CallGraph>,
    max_depth: usize,
    diagrams: &mut IndexMap<String, DiagramOutput>,
    formats: &[String],
    _files_written: &mut Vec<String>,
) -> anyhow::Result<ElementCount> {
    let mut code_element_count = 0;

    if let Some(cg) = call_graph {
        let engine = InferenceEngine::new(cg);

        // Infer code elements in "src" scope
        let elements = engine.infer_code_elements("src", max_depth);
        code_element_count = elements.len();

        // Extract UML relationships using the proper conversion
        let uml_relationships = extract_uml_relationships(&engine, &elements);

        // Build workspace for L4
        let mut workspace = C4Workspace::new("Code");
        workspace.description = "Code View".to_string();

        // Create a single container with all code elements
        let container = crate::model::c4_types::Container {
            id: ElementId::new("code_container"),
            name: "Code".to_string(),
            container_type: crate::model::c4_types::ContainerType::Library,
            technology: "Rust".to_string(),
            description: "Code elements".to_string(),
            path: Some(std::path::PathBuf::from("src")),
            components: Vec::new(),
        };

        let system = crate::model::c4_types::SoftwareSystem {
            id: ElementId::new("code_system"),
            name: "Code System".to_string(),
            description: "Code elements and relationships".to_string(),
            location: ElementLocation::Internal,
            containers: vec![container],
        };
        workspace.model.systems.push(system);

        // Render in requested formats (only mermaid for L4)
        for format in formats {
            if format == "mermaid" {
                let options = MermaidOptions {
                    title: "Code View (L4)".to_string(),
                    show_methods: true,
                    show_attributes: true,
                    ..Default::default()
                };
                let diagram = render_class_diagram(&elements, &uml_relationships, &options);

                diagrams.insert(format!("L4_{}", format), DiagramOutput {
                    level: "L4".to_string(),
                    format: format.clone(),
                    diagram,
                    element_count: code_element_count,
                    relationship_count: uml_relationships.len(),
                });
            }
        }
    }

    Ok(ElementCount {
        people: 0,
        systems: 0,
        containers: 0,
        components: 0,
        code_elements: code_element_count,
    })
}

/// Render a workspace diagram in the specified format
fn render_diagram_for_workspace(
    workspace: &C4Workspace,
    level: &str,
    format: &str,
) -> anyhow::Result<String> {
    match format {
        "mermaid" => {
            match level {
                "L1" => Ok(render_c4_context(workspace)),
                "L2" => {
                    let options = C4MermaidOptions::default();
                    Ok(render_container_diagram(workspace, &options))
                }
                "L3" => {
                    let containers: Vec<_> = workspace.model.systems
                        .iter()
                        .flat_map(|s| s.containers.clone())
                        .collect();
                    let options = C4MermaidOptions::default();
                    Ok(render_component_diagram(&containers, &workspace.model.relationships, &options))
                }
                _ => Ok(render_c4_context(workspace)),
            }
        }
        "plantuml" => {
            let view_type = match level {
                "L1" => PlantUmlViewType::SystemContext,
                "L2" => PlantUmlViewType::Container,
                "L3" => PlantUmlViewType::Component,
                _ => PlantUmlViewType::SystemContext,
            };
            let options = PlantUmlOptions::default();
            Ok(render_plantuml_c4(workspace, view_type, &options))
        }
        "dsl" => {
            let options = StructurizrDslOptions::default();
            Ok(render_structurizr_dsl(workspace, &options))
        }
        "d2" => {
            let options = D2Options::default();
            Ok(render_d2(workspace, &options))
        }
        other => Err(anyhow::anyhow!("Unsupported format: {}", other)),
    }
}

// =============================================================================
// Dynamic/Sequence Diagram Tools
// =============================================================================

/// Input for the `generate_c4_dynamic` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateC4DynamicInput {
    /// Entry point symbol name or path (default: auto-detect first entry point)
    pub entry_point: Option<String>,
    /// Maximum call depth (default: 5)
    pub max_depth: Option<usize>,
    /// Output format: "mermaid" only for now
    pub format: Option<String>,
}

/// Output of the `generate_c4_dynamic` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateC4DynamicOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// The entry point used
    pub entry_point: String,
    /// Number of call edges in the diagram
    pub call_count: usize,
}

/// Handle `generate_c4_dynamic` — sequence/dynamic diagram
pub fn handle_generate_c4_dynamic(
    input: GenerateC4DynamicInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateC4DynamicOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    if format != "mermaid" {
        return Err(anyhow::anyhow!(
            "Unsupported format '{}'. Only 'mermaid' is supported for dynamic diagrams.",
            format
        ));
    }

    let max_depth = input.max_depth.unwrap_or(5);

    // Determine entry point
    let entry_point = input.entry_point
        .clone()
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if entry_point.is_empty() {
        return Ok(GenerateC4DynamicOutput {
            diagram: "sequenceDiagram\n    Note over Participant: No entry point found".to_string(),
            format,
            entry_point: String::new(),
            call_count: 0,
        });
    }

    // Build sequence diagram options
    let options = SequenceDiagramOptions {
        max_depth,
        show_loops: true,
        show_method_names: true,
        title: format!("Call Sequence: {}", entry_point),
    };

    // Render the sequence diagram
    let diagram = render_sequence_diagram(call_graph, &entry_point, &options);

    // Count call edges (rough estimate from diagram lines)
    let call_count = diagram.lines()
        .filter(|line| line.contains("->>") || line.contains("-->>"))
        .count();

    Ok(GenerateC4DynamicOutput {
        diagram,
        format,
        entry_point,
        call_count,
    })
}

// =============================================================================
// Sequence Diagram Tools (T6.1)
// =============================================================================

/// Input for the `generate_sequence_diagram` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateSequenceDiagramInput {
    /// Entry point symbol name or path (default: auto-detect first entry point)
    pub entry_point: Option<String>,
    /// Maximum call depth (default: 5)
    pub max_depth: Option<usize>,
    /// Output format: "mermaid" (default), "plantuml", "svg"
    pub format: Option<String>,
    /// SVG-specific: width in pixels (default: 800)
    pub width: Option<u32>,
    /// SVG-specific: height in pixels (default: 600)
    pub height: Option<u32>,
    /// Include loop markers (default: true)
    pub show_loops: Option<bool>,
    /// Show method names on edges (default: true)
    pub show_method_names: Option<bool>,
    /// Title for the diagram
    pub title: Option<String>,
}

/// Output of the `generate_sequence_diagram` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateSequenceDiagramOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// The entry point used
    pub entry_point: String,
    /// Number of participants detected
    pub participant_count: usize,
    /// Number of messages/calls in the diagram
    pub message_count: usize,
    /// Maximum depth traversed
    pub max_depth: usize,
}

/// Handle `generate_sequence_diagram` — sequence diagram in multiple formats
pub fn handle_generate_sequence_diagram(
    input: GenerateSequenceDiagramInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateSequenceDiagramOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());
    let max_depth = input.max_depth.unwrap_or(5);
    let show_loops = input.show_loops.unwrap_or(true);
    let show_method_names = input.show_method_names.unwrap_or(true);
    let title = input.title.unwrap_or_else(|| "Sequence Diagram".to_string());

    // Determine entry point
    let entry_point = input
        .entry_point
        .clone()
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if entry_point.is_empty() {
        return Ok(GenerateSequenceDiagramOutput {
            diagram: "sequenceDiagram\n    Note over Participant: No entry point found".to_string(),
            format,
            entry_point: String::new(),
            participant_count: 0,
            message_count: 0,
            max_depth: 0,
        });
    }

    // Build sequence diagram options
    let options = SequenceDiagramOptions {
        max_depth,
        show_loops,
        show_method_names,
        title,
    };

    // Render based on format
    let diagram = match format.as_str() {
        "mermaid" => render_sequence_diagram(call_graph, &entry_point, &options),
        "plantuml" => render_sequence_diagram_plantuml(call_graph, &entry_point, &options),
        "svg" => {
            let svg_options = SequenceSvgOptions {
                width: input.width.unwrap_or(800),
                height: input.height.unwrap_or(600),
                ..SequenceSvgOptions::default()
            };
            render_sequence_diagram_svg(call_graph, &entry_point, &options, &svg_options)
        }
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported format '{}'. Supported: 'mermaid', 'plantuml', 'svg'",
                other
            ))
        }
    };

    // Count participants and messages from the rendered diagram
    let participant_count = if format == "mermaid" {
        diagram.lines().filter(|l| l.contains("participant")).count()
    } else if format == "plantuml" {
        diagram.lines().filter(|l| l.contains("participant") || l.contains("actor")).count()
    } else {
        // For SVG, count participant rectangles
        diagram.matches("<rect").count()
    };

    let message_count = diagram
        .lines()
        .filter(|line| line.contains("->>") || line.contains("-->>"))
        .count();

    Ok(GenerateSequenceDiagramOutput {
        diagram,
        format,
        entry_point,
        participant_count,
        message_count,
        max_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_reverse_engineer_basic() {
        let temp_dir = TempDir::new().unwrap();

        let input = ReverseEngineerC4Input {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            levels: Some(vec!["L1".to_string()]),
            format: Some("mermaid".to_string()),
            output_dir: None,
            max_depth: None,
        };

        let result = handle_reverse_engineer_c4(input, temp_dir.path(), None);

        // Should succeed even with empty directory
        assert!(result.is_ok(), "Expected ok, got: {:?}", result);
        let output = result.unwrap();
        assert!(output.diagrams.contains_key("L1_mermaid"));
        assert!(output.elapsed_ms >= 0, "elapsed_ms should not be negative");
    }

    #[test]
    fn test_reverse_engineer_all_levels() {
        let temp_dir = TempDir::new().unwrap();

        // Create a minimal Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"

[dependencies]
"#,
        )
        .unwrap();

        let input = ReverseEngineerC4Input {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            levels: Some(vec!["L1".to_string(), "L2".to_string(), "L3".to_string()]),
            format: Some("all".to_string()),
            output_dir: None,
            max_depth: None,
        };

        let result = handle_reverse_engineer_c4(input, temp_dir.path(), None);
        assert!(result.is_ok(), "Expected ok, got: {:?}", result);

        let output = result.unwrap();
        assert!(output.diagrams.contains_key("L1_mermaid"));
        assert!(output.diagrams.contains_key("L1_plantuml"));
        assert!(output.diagrams.contains_key("L1_dsl"));
        assert!(output.diagrams.contains_key("L2_mermaid"));
        assert!(output.diagrams.contains_key("L3_mermaid"));
        assert_eq!(output.element_counts.len(), 3);
    }

    #[test]
    fn test_reverse_engineer_writes_files() {
        let temp_dir = TempDir::new().unwrap();
        let output_dir = TempDir::new().unwrap();

        // Create a minimal Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test-project"
version = "0.1.0"

[dependencies]
"#,
        )
        .unwrap();

        let input = ReverseEngineerC4Input {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            levels: Some(vec!["L1".to_string()]),
            format: Some("mermaid".to_string()),
            output_dir: Some(output_dir.path().to_string_lossy().to_string()),
            max_depth: None,
        };

        let result = handle_reverse_engineer_c4(input, temp_dir.path(), None);
        assert!(result.is_ok(), "Expected ok, got: {:?}", result);

        let output = result.unwrap();
        assert!(!output.files_written.is_empty());

        // Verify files exist
        for file in &output.files_written {
            assert!(std::path::Path::new(file).exists(), "File not found: {}", file);
        }
    }

    #[test]
    fn test_parse_formats() {
        assert_eq!(parse_formats("mermaid"), vec!["mermaid"]);
        assert_eq!(
            parse_formats("all"),
            vec!["mermaid", "plantuml", "dsl", "d2"]
        );
    }

    #[test]
    fn test_element_count_default() {
        let counts = ElementCount {
            people: 0,
            systems: 0,
            containers: 0,
            components: 0,
            code_elements: 0,
        };
        assert_eq!(counts.people, 0);
    }

    #[test]
    fn test_generate_c4_dynamic_empty_graph() {
        let call_graph = CallGraph::new();
        let input = GenerateC4DynamicInput {
            entry_point: None,
            max_depth: None,
            format: None,
        };

        let result = handle_generate_c4_dynamic(input, &call_graph).unwrap();
        assert_eq!(result.format, "mermaid");
        assert!(result.diagram.contains("sequenceDiagram"));
        assert!(result.call_count == 0);
    }

    #[test]
    fn test_generate_c4_dynamic_unsupported_format() {
        let call_graph = CallGraph::new();
        let input = GenerateC4DynamicInput {
            entry_point: None,
            max_depth: None,
            format: Some("plantuml".to_string()),
        };

        let result = handle_generate_c4_dynamic(input, &call_graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported format"));
    }

    #[test]
    fn test_generate_c4_dynamic_custom_max_depth() {
        let call_graph = CallGraph::new();
        let input = GenerateC4DynamicInput {
            entry_point: Some("main".to_string()),
            max_depth: Some(3),
            format: None,
        };

        let result = handle_generate_c4_dynamic(input, &call_graph).unwrap();
        assert_eq!(result.format, "mermaid");
        assert!(result.diagram.contains("sequenceDiagram"));
    }
}

// =============================================================================
// Deployment Diagram Tools
// =============================================================================

/// Input for the `generate_c4_deployment` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateC4DeploymentInput {
    /// Project directory to analyze (default: ".")
    pub directory: Option<String>,
    /// Output format: "mermaid" (default), "d2"
    pub format: Option<String>,
    /// Show port mappings (default: true)
    pub show_ports: Option<bool>,
    /// Show environment variables (default: false)
    pub show_environment: Option<bool>,
}

/// Output of the `generate_c4_deployment` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateC4DeploymentOutput {
    pub diagram: String,
    pub format: String,
    pub node_count: usize,
    pub network_count: usize,
    pub relationship_count: usize,
}

/// Handle `generate_c4_deployment` — deployment/infrastructure diagram
pub fn handle_generate_c4_deployment(
    input: GenerateC4DeploymentInput,
    project_dir: &std::path::Path,
) -> anyhow::Result<GenerateC4DeploymentOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    // Infer deployment from Docker files
    let model = crate::inference::deployment_inference::infer_deployment(project_dir)?;

    // Render diagram
    let diagram = match format.as_str() {
        "mermaid" => crate::render::deployment::render_deployment_mermaid(&model),
        "d2" => {
            let options = crate::render::d2::D2Options::default();
            crate::render::deployment::render_deployment_d2(&model, &options)
        }
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported format '{}'. Supported: 'mermaid', 'd2'",
                other
            ))
        }
    };

    Ok(GenerateC4DeploymentOutput {
        diagram,
        format,
        node_count: model.nodes.len(),
        network_count: model.networks.len(),
        relationship_count: model.relationships.len(),
    })
}

// =============================================================================
// ER Diagram Tools
// =============================================================================

/// Input for the `generate_er_diagram` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateErDiagramInput {
    /// Project directory to analyze (default: ".")
    pub directory: Option<String>,
    /// Output format: "mermaid" (default), "d2"
    pub format: Option<String>,
    /// Include relationships (default: true)
    pub show_relationships: Option<bool>,
}

/// Output of the `generate_er_diagram` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateErDiagramOutput {
    pub diagram: String,
    pub format: String,
    pub entity_count: usize,
    pub relationship_count: usize,
}

/// Handle `generate_er_diagram` — entity-relationship diagram
pub fn handle_generate_er_diagram(
    input: GenerateErDiagramInput,
    project_dir: &std::path::Path,
) -> anyhow::Result<GenerateErDiagramOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    // Infer ER from SQL files
    let model = crate::inference::er_inference::infer_er_diagram(project_dir)?;

    // Render diagram
    let diagram = match format.as_str() {
        "mermaid" => crate::render::er::render_er_mermaid(&model),
        "d2" => {
            let options = crate::render::d2::D2Options::default();
            crate::render::er::render_er_d2(&model, &options)
        }
        other => {
            return Err(anyhow::anyhow!(
                "Unsupported format '{}'. Supported: 'mermaid', 'd2'",
                other
            ))
        }
    };

    Ok(GenerateErDiagramOutput {
        diagram,
        format,
        entity_count: model.entities.len(),
        relationship_count: model.relationships.len(),
    })
}

// =============================================================================
// State Machine Diagram Tools (T6.2)
// =============================================================================

/// Input for the `generate_state_machine` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateStateMachineInput {
    /// Symbol name to analyze (enum or struct with state pattern)
    /// If not provided, auto-detects state machines in the codebase
    pub symbol_name: Option<String>,
    /// Output format: "mermaid" (default), "plantuml"
    pub format: Option<String>,
    /// Show entry/exit actions (default: true)
    pub show_actions: Option<bool>,
    /// Show guards on transitions (default: true)
    pub show_guards: Option<bool>,
    /// Title for the diagram
    pub title: Option<String>,
    /// Direction: "LR" (left-right, default) or "TB" (top-bottom)
    pub direction: Option<String>,
}

/// Output of the `generate_state_machine` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateStateMachineOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// Name of the state machine detected
    pub name: String,
    /// Number of states detected
    pub state_count: usize,
    /// Number of transitions detected
    pub transition_count: usize,
}

/// Handle `generate_state_machine` — state machine diagram
pub fn handle_generate_state_machine(
    input: GenerateStateMachineInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateStateMachineOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());
    let title = input.title.unwrap_or_else(|| "State Machine".to_string());

    // Build inference options
    let inference_options = StateMachineInferenceOptions {
        min_states: 2,
        include_actions: input.show_actions.unwrap_or(true),
        include_guards: input.show_guards.unwrap_or(true),
        title: title.clone(),
    };

    // Build render options
    let render_options = StateMachineRenderOptions {
        show_actions: input.show_actions.unwrap_or(true),
        show_guards: input.show_guards.unwrap_or(true),
        title: title.clone(),
        direction: input.direction.unwrap_or_else(|| "LR".to_string()),
    };

    // Try to find state machines
    let model = if let Some(ref symbol_name) = input.symbol_name {
        // Try to infer from specific symbol
        infer_state_machine_from_struct(call_graph, symbol_name, &inference_options)
            .or_else(|| {
                // If not found, try to find state machines with this name
                let machines = find_state_machines(call_graph, &inference_options);
                let name_to_find = symbol_name.as_str();
                machines.into_iter().find(|m| m.name.as_str() == name_to_find || m.entry_point.as_str() == name_to_find)
            })
    } else {
        // Auto-detect: find first state machine
        let machines = find_state_machines(call_graph, &inference_options);
        machines.into_iter().next()
    };

    // Render the diagram
    let (diagram, model_name, state_count, transition_count) = match model {
        Some(ref model) => {
            let diagram = match format.as_str() {
                "plantuml" => render_state_machine_plantuml(model, &render_options),
                _ => render_state_machine_mermaid(model, &render_options),
            };
            (diagram, model.name.clone(), model.states.len(), model.transitions.len())
        }
        None => {
            // No state machine found, render empty diagram
            let diagram = render_empty_state_machine(&title, &format);
            (diagram, "Unknown".to_string(), 0, 0)
        }
    };

    Ok(GenerateStateMachineOutput {
        diagram,
        format,
        name: model_name,
        state_count,
        transition_count,
    })
}

// =============================================================================
// Activity Diagram Tools (T6.3)
// =============================================================================

/// Input for the `generate_activity_diagram` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateActivityDiagramInput {
    /// Function name to analyze
    /// If not provided, auto-detects activities in the codebase
    pub symbol_name: Option<String>,
    /// Output format: "mermaid" (default), "plantuml"
    pub format: Option<String>,
    /// Title for the diagram
    pub title: Option<String>,
    /// Direction: "TB" (top-bottom, default) or "LR" (left-right)
    pub direction: Option<String>,
    /// Include loop detection (default: true)
    pub include_loops: Option<bool>,
}

/// Output of the `generate_activity_diagram` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateActivityDiagramOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// Name of the activity function
    pub name: String,
    /// Number of nodes in the diagram
    pub node_count: usize,
    /// Number of edges in the diagram
    pub edge_count: usize,
}

/// Handle `generate_activity_diagram` — activity/flow diagram
pub fn handle_generate_activity_diagram(
    input: GenerateActivityDiagramInput,
    call_graph: &CallGraph,
) -> anyhow::Result<GenerateActivityDiagramOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());
    let title = input.title.unwrap_or_else(|| "Activity Diagram".to_string());

    // Build inference options
    let inference_options = ActivityInferenceOptions {
        min_actions: 2,
        include_loops: input.include_loops.unwrap_or(true),
        title: title.clone(),
    };

    // Build render options
    let render_options = ActivityRenderOptions {
        show_ids: false,
        title: title.clone(),
        direction: input.direction.unwrap_or_else(|| "TB".to_string()),
        show_guards: true,
    };

    // Try to find activity
    let model = if let Some(ref symbol_name) = input.symbol_name {
        // Try to infer from specific function
        infer_activity_from_function(call_graph, symbol_name, &inference_options)
            .or_else(|| {
                // Try to find activities with this name
                let activities = find_activities(call_graph, &inference_options);
                let name_to_find = symbol_name.as_str();
                activities.into_iter().find(|a| a.entry_point.as_str() == name_to_find)
            })
    } else {
        // Auto-detect: find first activity
        let activities = find_activities(call_graph, &inference_options);
        activities.into_iter().next()
    };

    // Render the diagram
    let (diagram, model_name, node_count, edge_count) = match model {
        Some(ref model) => {
            let diagram = match format.as_str() {
                "plantuml" => render_activity_plantuml(model, &render_options),
                _ => render_activity_mermaid(model, &render_options),
            };
            (diagram, model.entry_point.clone(), model.nodes.len(), model.edges.len())
        }
        None => {
            // No activity found, render empty diagram
            let diagram = render_empty_activity(&title, &format);
            (diagram, "Unknown".to_string(), 0, 0)
        }
    };

    Ok(GenerateActivityDiagramOutput {
        diagram,
        format,
        name: model_name,
        node_count,
        edge_count,
    })
}

// =============================================================================
// AI Summarization Tools (T6.6)
// =============================================================================

use crate::summarization::{
    summarize_workspace, SummaryStyle as DiagramSummaryStyle,
    DiagramSummary as SummarizedDiagram, DiagramStatistics, ArchitectureRisk, RiskSeverity,
};

/// Input for the `summarize_diagram` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct SummarizeDiagramInput {
    /// C4 Workspace as JSON string
    pub workspace_json: String,
    /// Summary style: "executive", "technical", or "risk"
    pub style: Option<String>,
}

/// Output of the `summarize_diagram` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct SummarizeDiagramOutput {
    /// Title of the summary
    pub title: String,
    /// The summary text
    pub text: String,
    /// Style used
    pub style: String,
    /// Key highlights
    pub highlights: Vec<String>,
    /// Risk count by severity (only for risk assessment)
    pub risk_counts: Option<RiskCounts>,
    /// Diagram statistics
    pub statistics: DiagramStatisticsDto,
}

/// Risk counts by severity
#[derive(Debug, Clone, Serialize)]
pub struct RiskCounts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
}

/// DTO for diagram statistics
#[derive(Debug, Clone, Serialize)]
pub struct DiagramStatisticsDto {
    pub system_count: usize,
    pub container_count: usize,
    pub component_count: usize,
    pub relationship_count: usize,
    pub person_count: usize,
    pub technologies: Vec<String>,
}

impl From<DiagramStatistics> for DiagramStatisticsDto {
    fn from(stats: DiagramStatistics) -> Self {
        Self {
            system_count: stats.system_count,
            container_count: stats.container_count,
            component_count: stats.component_count,
            relationship_count: stats.relationship_count,
            person_count: stats.person_count,
            technologies: stats.technologies,
        }
    }
}

/// Handle `summarize_diagram` — generate AI summary of a diagram
pub fn handle_summarize_diagram(
    input: SummarizeDiagramInput,
) -> anyhow::Result<SummarizeDiagramOutput> {
    // Parse style
    let style = match input.style.as_deref().unwrap_or("technical") {
        "executive" => DiagramSummaryStyle::Executive,
        "risk" | "risk_assessment" => DiagramSummaryStyle::RiskAssessment,
        _ => DiagramSummaryStyle::Technical,
    };

    // Parse workspace
    let workspace: crate::model::workspace::C4Workspace =
        serde_json::from_str(&input.workspace_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse workspace JSON: {}", e))?;

    // Generate summary
    let summary = summarize_workspace(&workspace, style);

    // Build risk counts if risk assessment
    let risk_counts = if style == DiagramSummaryStyle::RiskAssessment {
        Some(RiskCounts {
            critical: summary.risks.iter().filter(|r| r.severity == RiskSeverity::Critical).count(),
            high: summary.risks.iter().filter(|r| r.severity == RiskSeverity::High).count(),
            medium: summary.risks.iter().filter(|r| r.severity == RiskSeverity::Medium).count(),
            low: summary.risks.iter().filter(|r| r.severity == RiskSeverity::Low).count(),
        })
    } else {
        None
    };

    Ok(SummarizeDiagramOutput {
        title: summary.title,
        text: summary.text,
        style: format!("{:?}", style),
        highlights: summary.highlights,
        risk_counts,
        statistics: summary.statistics.into(),
    })
}

// =============================================================================
// Multi-Language Workspace Tools (T6.5)
// =============================================================================

use crate::inference::multi_lang_engine::MultiLangEngine;

/// Input for the `generate_multi_lang_workspace` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct GenerateMultiLangWorkspaceInput {
    /// Project directory path (default: current directory)
    pub directory: Option<String>,
    /// Output format: "mermaid" (default), "plantuml", "structurizr"
    pub format: Option<String>,
    /// Whether to include code details (default: false)
    pub include_code: Option<bool>,
}

/// Output of the `generate_multi_lang_workspace` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct GenerateMultiLangWorkspaceOutput {
    /// The generated diagram source
    pub diagram: String,
    /// Output format used
    pub format: String,
    /// Detected languages
    pub languages: Vec<String>,
    /// Number of containers in the workspace
    pub container_count: usize,
    /// Number of relationships in the workspace
    pub relationship_count: usize,
}

/// Handle `generate_multi_lang_workspace` — multi-language C4 workspace
pub fn handle_generate_multi_lang_workspace(
    input: GenerateMultiLangWorkspaceInput,
    project_dir: &std::path::Path,
) -> anyhow::Result<GenerateMultiLangWorkspaceOutput> {
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    // Create multi-language engine
    let mut engine = MultiLangEngine::new();

    // Detect languages
    let detected_languages = engine.detect_languages(project_dir);
    let language_names: Vec<String> = detected_languages
        .iter()
        .map(|l| match l {
            crate::inference::multi_lang_engine::Language::Rust => "Rust",
            crate::inference::multi_lang_engine::Language::TypeScript => "TypeScript",
            crate::inference::multi_lang_engine::Language::JavaScript => "JavaScript",
            crate::inference::multi_lang_engine::Language::Python => "Python",
            crate::inference::multi_lang_engine::Language::Go => "Go",
            crate::inference::multi_lang_engine::Language::Unknown => "Unknown",
        })
        .map(String::from)
        .collect();

    // Build workspace
    let workspace = engine.build_workspace(project_dir);

    let container_count = workspace.model.systems.iter()
        .map(|s| s.containers.len())
        .sum();

    let relationship_count = workspace.model.relationships.len();

    // Render the workspace
    let diagram = match format.as_str() {
        "structurizr" => {
            use crate::render::structurizr_dsl::render_structurizr_dsl;
            let options = crate::render::structurizr_dsl::StructurizrDslOptions::default();
            render_structurizr_dsl(&workspace, &options)
        }
        "plantuml" => {
            use crate::render::plantuml::{render_plantuml_c4, PlantUmlOptions, PlantUmlViewType};
            let options = PlantUmlOptions::default();
            render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options)
        }
        _ => {
            // Default to Mermaid
            render_c4_context(&workspace)
        }
    };

    Ok(GenerateMultiLangWorkspaceOutput {
        diagram,
        format,
        languages: language_names,
        container_count,
        relationship_count,
    })
}

// =============================================================================
// Diagram Diff Tools (T6.7)
// =============================================================================

use crate::diff::{
    diff_workspaces, render_diff_mermaid, WorkspaceDiff, DiffSummary,
    ContainerDiff, RelationshipDiff,
};

/// Input for the `diff_diagrams` MCP tool
#[derive(Debug, Clone, Deserialize)]
pub struct DiffDiagramsInput {
    /// First workspace as JSON string
    pub workspace_a_json: String,
    /// Second workspace as JSON string
    pub workspace_b_json: String,
    /// Output format: "mermaid" (default), "mermaid_state", "mermaid_class", "json"
    pub format: Option<String>,
}

/// Output of the `diff_diagrams` MCP tool
#[derive(Debug, Clone, Serialize)]
pub struct DiffDiagramsOutput {
    /// The diff diagram source (Mermaid) or JSON
    pub diff_output: String,
    /// Output format used
    pub format: String,
    /// Summary of changes
    pub summary: DiffSummaryDto,
    /// Detailed container diffs
    pub containers_added: Vec<ContainerSummaryDto>,
    pub containers_removed: Vec<ContainerSummaryDto>,
    pub containers_modified: Vec<ContainerDiffDto>,
    pub relationships_added_count: usize,
    pub relationships_removed_count: usize,
}

/// DTO for diff summary
#[derive(Debug, Clone, Serialize)]
pub struct DiffSummaryDto {
    pub systems_added: usize,
    pub systems_removed: usize,
    pub containers_added: usize,
    pub containers_removed: usize,
    pub containers_modified: usize,
    pub relationships_added: usize,
    pub relationships_removed: usize,
    pub total_changes: usize,
}

impl From<&DiffSummary> for DiffSummaryDto {
    fn from(summary: &DiffSummary) -> Self {
        Self {
            systems_added: summary.systems_added,
            systems_removed: summary.systems_removed,
            containers_added: summary.containers_added,
            containers_removed: summary.containers_removed,
            containers_modified: summary.containers_modified,
            relationships_added: summary.relationships_added,
            relationships_removed: summary.relationships_removed,
            total_changes: summary.total_changes,
        }
    }
}

/// DTO for container summary
#[derive(Debug, Clone, Serialize)]
pub struct ContainerSummaryDto {
    pub id: String,
    pub name: String,
    pub technology: String,
    pub description: String,
}

/// DTO for container diff
#[derive(Debug, Clone, Serialize)]
pub struct ContainerDiffDto {
    pub id: String,
    pub name_before: String,
    pub name_after: String,
    pub technology_before: String,
    pub technology_after: String,
    pub description_before: String,
    pub description_after: String,
}

/// Handle `diff_diagrams` — compare two diagrams
pub fn handle_diff_diagrams(
    input: DiffDiagramsInput,
) -> anyhow::Result<DiffDiagramsOutput> {
    // Parse workspaces
    let workspace_a: crate::model::workspace::C4Workspace =
        serde_json::from_str(&input.workspace_a_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse workspace A: {}", e))?;

    let workspace_b: crate::model::workspace::C4Workspace =
        serde_json::from_str(&input.workspace_b_json)
            .map_err(|e| anyhow::anyhow!("Failed to parse workspace B: {}", e))?;

    // Compute diff
    let diff = diff_workspaces(&workspace_a, &workspace_b);

    // Format output
    let format = input.format.unwrap_or_else(|| "mermaid".to_string());

    let diff_output = if format == "json" {
        serde_json::to_string_pretty(&diff)
            .map_err(|e| anyhow::anyhow!("Failed to serialize diff: {}", e))?
    } else {
        render_diff_mermaid(&diff, &format)
    };

    // Build container dtos
    let containers_added: Vec<ContainerSummaryDto> = diff
        .containers_added
        .iter()
        .map(|c| ContainerSummaryDto {
            id: c.id.as_str().to_string(),
            name: c.name.clone(),
            technology: c.technology.clone(),
            description: c.description.clone(),
        })
        .collect();

    let containers_removed: Vec<ContainerSummaryDto> = diff
        .containers_removed
        .iter()
        .map(|c| ContainerSummaryDto {
            id: c.id.as_str().to_string(),
            name: c.name.clone(),
            technology: c.technology.clone(),
            description: c.description.clone(),
        })
        .collect();

    let containers_modified: Vec<ContainerDiffDto> = diff
        .containers_modified
        .iter()
        .map(|c| ContainerDiffDto {
            id: c.id.clone(),
            name_before: get_modified_value(&c.name_diff),
            name_after: get_modified_value(&c.name_diff),
            technology_before: get_modified_value(&c.technology_diff),
            technology_after: get_modified_value(&c.technology_diff),
            description_before: get_modified_value(&c.description_diff),
            description_after: get_modified_value(&c.description_diff),
        })
        .collect();

    Ok(DiffDiagramsOutput {
        diff_output,
        format: format.clone(),
        summary: (&diff.summary).into(),
        containers_added,
        containers_removed,
        containers_modified,
        relationships_added_count: diff.summary.relationships_added,
        relationships_removed_count: diff.summary.relationships_removed,
    })
}

fn get_modified_value<T: Clone + PartialEq>(diff: &crate::diff::ElementDiff<T>) -> String
where
    T: std::fmt::Display,
{
    match diff {
        crate::diff::ElementDiff::Unchanged(v) => format!("{}", v),
        crate::diff::ElementDiff::Modified { after, .. } => format!("{}", after),
        crate::diff::ElementDiff::Added(v) => format!("+{}", v),
        crate::diff::ElementDiff::Removed(v) => format!("-{}", v),
    }
}

#[cfg(test)]
mod deployment_er_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_generate_c4_deployment_empty() {
        let temp_dir = TempDir::new().unwrap();
        let input = GenerateC4DeploymentInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("mermaid".to_string()),
            show_ports: None,
            show_environment: None,
        };

        let result = handle_generate_c4_deployment(input, temp_dir.path()).unwrap();
        assert_eq!(result.format, "mermaid");
        assert_eq!(result.node_count, 0);
    }

    #[test]
    fn test_generate_er_diagram_empty() {
        let temp_dir = TempDir::new().unwrap();
        let input = GenerateErDiagramInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("mermaid".to_string()),
            show_relationships: None,
        };

        let result = handle_generate_er_diagram(input, temp_dir.path()).unwrap();
        assert_eq!(result.format, "mermaid");
        assert_eq!(result.entity_count, 0);
    }

    #[test]
    fn test_generate_c4_deployment_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let input = GenerateC4DeploymentInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("plantuml".to_string()),
            show_ports: None,
            show_environment: None,
        };

        let result = handle_generate_c4_deployment(input, temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported format"));
    }

    #[test]
    fn test_generate_er_diagram_unsupported_format() {
        let temp_dir = TempDir::new().unwrap();
        let input = GenerateErDiagramInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("plantuml".to_string()),
            show_relationships: None,
        };

        let result = handle_generate_er_diagram(input, temp_dir.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported format"));
    }

    #[test]
    fn test_generate_multi_lang_workspace() {
        let temp_dir = TempDir::new().unwrap();

        // Create a mixed workspace with Rust and TypeScript
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"backend\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        std::fs::write(
            temp_dir.path().join("package.json"),
            r#"{"name": "frontend", "dependencies": {"react": "^18.0.0"}}"#,
        )
        .unwrap();

        let input = GenerateMultiLangWorkspaceInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("mermaid".to_string()),
            include_code: Some(false),
        };

        let result = handle_generate_multi_lang_workspace(input, temp_dir.path()).unwrap();
        assert_eq!(result.format, "mermaid");
        assert!(result.languages.contains(&"Rust".to_string()));
        assert!(result.languages.contains(&"JavaScript".to_string()));
        assert!(result.container_count >= 2);
    }

    #[test]
    fn test_generate_multi_lang_workspace_single_language() {
        let temp_dir = TempDir::new().unwrap();

        // Create a Rust-only workspace
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            "[package]\nname = \"backend\"\nversion = \"0.1.0\"",
        )
        .unwrap();

        let input = GenerateMultiLangWorkspaceInput {
            directory: Some(temp_dir.path().to_string_lossy().to_string()),
            format: Some("mermaid".to_string()),
            include_code: Some(false),
        };

        let result = handle_generate_multi_lang_workspace(input, temp_dir.path()).unwrap();
        assert_eq!(result.format, "mermaid");
        assert!(result.languages.contains(&"Rust".to_string()));
        assert_eq!(result.languages.len(), 1);
    }
}
