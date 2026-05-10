//! PlantUML C4 diagram renderer
//!
//! Renders C4 models using the C4-PlantUML stdlib macros.
//! Supports System Context (L1), Container (L2), and Component (L3) views.

use crate::model::c4_types::{ContainerType, ElementLocation};
use crate::model::workspace::C4Workspace;

/// Which C4 view to render
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlantUmlViewType {
    SystemContext,
    Container,
    Component,
}

/// Options for PlantUML output
#[derive(Debug, Clone)]
pub struct PlantUmlOptions {
    /// Add LAYOUT_WITH_LEGEND() at the end (default: true)
    pub include_legend: bool,
    /// Show technology labels on containers/components (default: true)
    pub show_technology: bool,
    /// Layout direction: "top to bottom" or "left to right" (default: "top to bottom")
    pub direction: String,
}

impl Default for PlantUmlOptions {
    fn default() -> Self {
        Self {
            include_legend: true,
            show_technology: true,
            direction: "top to bottom".to_string(),
        }
    }
}

/// Sanitize a string to be a valid PlantUML identifier.
/// Replaces problematic characters: `-` -> `_`, spaces -> `_`, dots removed.
fn sanitize_alias(id: &str) -> String {
    id.replace('-', "_")
        .replace(' ', "_")
        .replace('.', "")
}

/// Render C4Workspace as PlantUML with C4 macros
pub fn render_plantuml_c4(
    workspace: &C4Workspace,
    view_type: PlantUmlViewType,
    options: &PlantUmlOptions,
) -> String {
    match view_type {
        PlantUmlViewType::SystemContext => render_system_context(workspace, options),
        PlantUmlViewType::Container => render_container_view(workspace, options),
        PlantUmlViewType::Component => render_component_view(workspace, options),
    }
}

fn layout_directive(direction: &str) -> &'static str {
    match direction {
        "left to right" | "LR" => "LAYOUT_LEFT_RIGHT()",
        _ => "LAYOUT_TOP_DOWN()",
    }
}

fn render_system_context(workspace: &C4Workspace, options: &PlantUmlOptions) -> String {
    let mut lines = vec![
        "@startuml CogniCode_SystemContext".to_string(),
        "!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Context.puml".to_string(),
        "".to_string(),
        layout_directive(&options.direction).to_string(),
        "".to_string(),
    ];

    // People (actors)
    for person in &workspace.model.people {
        let alias = sanitize_alias(person.id.as_str());
        lines.push(format!(
            "Person({}, \"{}\", \"{}\")",
            alias,
            person.name,
            person.description
        ));
    }

    // Internal systems
    for system in &workspace.model.systems {
        if system.location == ElementLocation::Internal {
            let alias = sanitize_alias(system.id.as_str());
            lines.push(format!(
                "System({}, \"{}\", \"{}\")",
                alias, system.name, system.description
            ));
        }
    }

    // External systems
    for system in &workspace.model.systems {
        if system.location == ElementLocation::External {
            let alias = sanitize_alias(system.id.as_str());
            if system.containers.iter().any(|c| c.container_type == ContainerType::DataStore) {
                lines.push(format!(
                    "SystemDb_Ext({}, \"{}\", \"{}\")",
                    alias, system.name, system.description
                ));
            } else {
                lines.push(format!(
                    "System_Ext({}, \"{}\", \"{}\")",
                    alias, system.name, system.description
                ));
            }
        }
    }

    lines.push("".to_string());

    // Relationships
    for rel in &workspace.model.relationships {
        let source = sanitize_alias(rel.source_id.as_str());
        let target = sanitize_alias(rel.target_id.as_str());
        let label = rel.label.as_deref().unwrap_or("");
        let tech = rel.technology.as_deref().unwrap_or("");

        if tech.is_empty() {
            lines.push(format!("Rel({}, {}, \"{}\")", source, target, label));
        } else {
            lines.push(format!("Rel({}, {}, \"{}\", \"{}\")", source, target, label, tech));
        }
    }

    lines.push("".to_string());

    if options.include_legend {
        lines.push("LAYOUT_WITH_LEGEND()".to_string());
    }

    lines.push("@enduml".to_string());

    lines.join("\n")
}

fn render_container_view(workspace: &C4Workspace, options: &PlantUmlOptions) -> String {
    let mut lines = vec![
        "@startuml CogniCode_Containers".to_string(),
        "!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Container.puml".to_string(),
        "".to_string(),
        layout_directive(&options.direction).to_string(),
        "".to_string(),
    ];

    // People
    for person in &workspace.model.people {
        let alias = sanitize_alias(person.id.as_str());
        lines.push(format!(
            "Person({}, \"{}\", \"{}\")",
            alias,
            person.name,
            person.description
        ));
    }

    // Find the main internal system boundary
    for system in &workspace.model.systems {
        if system.location == ElementLocation::Internal {
            let alias = sanitize_alias(system.id.as_str());
            lines.push(format!("System_Boundary({}, \"{}\") {{", alias, system.name));

            // Containers within this system
            for container in &system.containers {
                let alias = sanitize_alias(container.id.as_str());
                let tech = if options.show_technology && !container.technology.is_empty() {
                    container.technology.clone()
                } else {
                    String::new()
                };

                match container.container_type {
                    ContainerType::DataStore => {
                        if tech.is_empty() {
                            lines.push(format!("    ContainerDb({}, \"{}\", \"{}\")", alias, container.name, container.description));
                        } else {
                            lines.push(format!("    ContainerDb({}, \"{}\", \"{}\", \"{}\")", alias, container.name, tech, container.description));
                        }
                    }
                    _ => {
                        if tech.is_empty() {
                            lines.push(format!("    Container({}, \"{}\", \"{}\")", alias, container.name, container.description));
                        } else {
                            lines.push(format!("    Container({}, \"{}\", \"{}\", \"{}\")", alias, container.name, tech, container.description));
                        }
                    }
                }
            }

            lines.push("}".to_string());
            break; // Only one internal system boundary for now
        }
    }

    // External systems
    for system in &workspace.model.systems {
        if system.location == ElementLocation::External {
            let alias = sanitize_alias(system.id.as_str());
            lines.push(format!(
                "System_Ext({}, \"{}\", \"{}\")",
                alias, system.name, system.description
            ));
        }
    }

    lines.push("".to_string());

    // Relationships
    for rel in &workspace.model.relationships {
        let source = sanitize_alias(rel.source_id.as_str());
        let target = sanitize_alias(rel.target_id.as_str());
        let label = rel.label.as_deref().unwrap_or("");
        let tech = rel.technology.as_deref().unwrap_or("");

        if tech.is_empty() {
            lines.push(format!("Rel({}, {}, \"{}\")", source, target, label));
        } else {
            lines.push(format!("Rel({}, {}, \"{}\", \"{}\")", source, target, label, tech));
        }
    }

    lines.push("".to_string());

    if options.include_legend {
        lines.push("LAYOUT_WITH_LEGEND()".to_string());
    }

    lines.push("@enduml".to_string());

    lines.join("\n")
}

fn render_component_view(workspace: &C4Workspace, options: &PlantUmlOptions) -> String {
    let mut lines = vec![
        "@startuml cognicode_core_Components".to_string(),
        "!include https://raw.githubusercontent.com/plantuml-stdlib/C4-PlantUML/master/C4_Component.puml".to_string(),
        "".to_string(),
        layout_directive(&options.direction).to_string(),
        "".to_string(),
    ];

    // Find the main internal system and render its containers with components
    for system in &workspace.model.systems {
        if system.location == ElementLocation::Internal {
            for container in &system.containers {
                let alias = sanitize_alias(container.id.as_str());
                lines.push(format!("Container_Boundary({}, \"{}\") {{", alias, container.name));

                for component in &container.components {
                    let comp_alias = sanitize_alias(component.id.as_str());
                    let tech = if options.show_technology && !component.technology.is_empty() {
                        format!(", \"{}\"", component.technology)
                    } else {
                        String::new()
                    };
                    lines.push(format!(
                        "    Component({}, \"{}\"{})",
                        comp_alias, component.name, tech
                    ));
                }

                lines.push("}".to_string());
            }
            break; // Only one internal system for now
        }
    }

    lines.push("".to_string());

    // Relationships between components
    for rel in &workspace.model.relationships {
        let source = sanitize_alias(rel.source_id.as_str());
        let target = sanitize_alias(rel.target_id.as_str());
        let label = rel.label.as_deref().unwrap_or("");

        if label.is_empty() {
            lines.push(format!("Rel({}, {}, \"\")", source, target));
        } else {
            lines.push(format!("Rel({}, {}, \"{}\")", source, target, label));
        }
    }

    lines.push("".to_string());

    if options.include_legend {
        lines.push("LAYOUT_WITH_LEGEND()".to_string());
    }

    lines.push("@enduml".to_string());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{Component, ComponentType, Container, ContainerType, ElementId, Person, SoftwareSystem};
    use crate::model::relationships::{C4Relationship, C4RelationshipKind};

    fn create_test_workspace() -> C4Workspace {
        let user = Person {
            id: ElementId::new("user"),
            name: "User".to_string(),
            description: "End user".to_string(),
            location: ElementLocation::Internal,
        };

        let developer = Person {
            id: ElementId::new("developer"),
            name: "Developer".to_string(),
            description: "CLI user".to_string(),
            location: ElementLocation::Internal,
        };

        let core_container = Container {
            id: ElementId::new("cognicode-core"),
            name: "cognicode-core".to_string(),
            container_type: ContainerType::Library,
            technology: "Rust".to_string(),
            description: "Core analysis library".to_string(),
            path: None,
            components: vec![
                Component {
                    id: ElementId::new("domain"),
                    name: "domain".to_string(),
                    component_type: ComponentType::Module,
                    technology: "Rust".to_string(),
                    description: "Domain aggregates and traits".to_string(),
                    path: None,
                    code_elements: vec![],
                },
                Component {
                    id: ElementId::new("infrastructure"),
                    name: "infrastructure".to_string(),
                    component_type: ComponentType::Module,
                    technology: "Rust".to_string(),
                    description: "Infrastructure implementations".to_string(),
                    path: None,
                    code_elements: vec![],
                },
            ],
        };

        let mcp_container = Container {
            id: ElementId::new("cognicode-mcp"),
            name: "cognicode-mcp".to_string(),
            container_type: ContainerType::Service,
            technology: "Rust, rmcp".to_string(),
            description: "MCP server binary".to_string(),
            path: None,
            components: vec![],
        };

        let sqlite = Container {
            id: ElementId::new("sqlite"),
            name: "SQLite".to_string(),
            container_type: ContainerType::DataStore,
            technology: "SQLite".to_string(),
            description: "Local analysis cache".to_string(),
            path: None,
            components: vec![],
        };

        let internal_system = SoftwareSystem {
            id: ElementId::new("cognicode"),
            name: "CogniCode".to_string(),
            description: "Code quality analysis engine".to_string(),
            location: ElementLocation::Internal,
            containers: vec![core_container, mcp_container, sqlite],
        };

        let external_otel = SoftwareSystem {
            id: ElementId::new("otel-collector"),
            name: "OTel Collector".to_string(),
            description: "Telemetry backend".to_string(),
            location: ElementLocation::External,
            containers: vec![],
        };

        let relationships = vec![
            C4Relationship::new(
                ElementId::new("developer"),
                ElementId::new("cognicode"),
                C4RelationshipKind::Uses,
            ),
            C4Relationship::new(
                ElementId::new("cognicode"),
                ElementId::new("sqlite"),
                C4RelationshipKind::ReadsFrom,
            )
            .with_label("reads from")
            .with_technology("SQL"),
        ];

        C4Workspace {
            name: "CogniCode".to_string(),
            description: "Code quality analysis engine".to_string(),
            model: crate::model::workspace::C4Model {
                people: vec![user, developer],
                systems: vec![internal_system, external_otel],
                relationships,
            },
            views: vec![],
        }
    }

    #[test]
    fn test_render_empty_context() {
        let workspace = C4Workspace::new("Empty");
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        assert!(result.starts_with("@startuml"));
        assert!(result.ends_with("@enduml"));
        assert!(result.contains("C4_Context.puml"));
    }

    #[test]
    fn test_render_context_with_actors() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        assert!(result.contains("Person(developer, \"Developer\", \"CLI user\")"));
        assert!(result.contains("Person(user, \"User\", \"End user\")"));
        assert!(result.contains("System(cognicode, \"CogniCode\", \"Code quality analysis engine\")"));
        assert!(result.contains("System_Ext(otel_collector, \"OTel Collector\", \"Telemetry backend\")"));
    }

    #[test]
    fn test_render_container_view() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::Container, &options);

        assert!(result.contains("C4_Container.puml"));
        assert!(result.contains("System_Boundary(cognicode, \"CogniCode\")"));
        assert!(result.contains("Container(cognicode_mcp, \"cognicode-mcp\", \"Rust, rmcp\", \"MCP server binary\")"));
        assert!(result.contains("ContainerDb(sqlite, \"SQLite\", \"SQLite\", \"Local analysis cache\")"));
    }

    #[test]
    fn test_render_component_view() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::Component, &options);

        assert!(result.contains("C4_Component.puml"));
        assert!(result.contains("Container_Boundary(cognicode_core, \"cognicode-core\")"));
        // Components have technology "Rust" so they are rendered with it
        assert!(result.contains("Component(domain, \"domain\", \"Rust\")"));
        assert!(result.contains("Component(infrastructure, \"infrastructure\", \"Rust\")"));
    }

    #[test]
    fn test_plantuml_is_valid() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        // Verify output starts with @startuml and ends with @enduml
        assert!(result.starts_with("@startuml CogniCode_SystemContext"));
        assert!(result.ends_with("@enduml"));

        // Verify it includes the correct C4 library
        assert!(result.contains("C4_Context.puml"));

        // Verify LAYOUT_WITH_LEGEND is present
        assert!(result.contains("LAYOUT_WITH_LEGEND()"));
    }

    #[test]
    fn test_sanitize_alias() {
        assert_eq!(sanitize_alias("my-id"), "my_id");
        assert_eq!(sanitize_alias("my id"), "my_id");
        assert_eq!(sanitize_alias("my.id"), "myid");
        assert_eq!(sanitize_alias("my-id-123"), "my_id_123");
    }

    #[test]
    fn test_plantuml_options_default() {
        let options = PlantUmlOptions::default();
        assert!(options.include_legend);
        assert!(options.show_technology);
        assert_eq!(options.direction, "top to bottom");
    }

    #[test]
    fn test_plantuml_options_no_legend() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions {
            include_legend: false,
            ..Default::default()
        };
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        assert!(!result.contains("LAYOUT_WITH_LEGEND()"));
    }

    #[test]
    fn test_plantuml_options_left_to_right() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions {
            direction: "left to right".to_string(),
            ..Default::default()
        };
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        assert!(result.contains("LAYOUT_LEFT_RIGHT()"));
    }

    #[test]
    fn test_plantuml_container_without_technology() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions {
            show_technology: false,
            ..Default::default()
        };
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::Container, &options);

        // Container should not show technology
        assert!(result.contains("Container(cognicode_mcp, \"cognicode-mcp\", \"MCP server binary\")"));
    }

    #[test]
    fn test_render_system_context_external_db() {
        let ext_db = SoftwareSystem {
            id: ElementId::new("external-db"),
            name: "External DB".to_string(),
            description: "External database".to_string(),
            location: ElementLocation::External,
            containers: vec![Container {
                id: ElementId::new("ext-db-container"),
                name: "DB".to_string(),
                container_type: ContainerType::DataStore,
                technology: "PostgreSQL".to_string(),
                description: "Database".to_string(),
                path: None,
                components: vec![],
            }],
        };

        let workspace = C4Workspace {
            name: "Test".to_string(),
            description: "".to_string(),
            model: crate::model::workspace::C4Model {
                people: vec![],
                systems: vec![ext_db],
                relationships: vec![],
            },
            views: vec![],
        };

        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        assert!(result.contains("SystemDb_Ext(external_db, \"External DB\", \"External database\")"));
    }

    #[test]
    fn test_render_relationships_with_technology() {
        let workspace = create_test_workspace();
        let options = PlantUmlOptions::default();
        let result = render_plantuml_c4(&workspace, PlantUmlViewType::SystemContext, &options);

        // Check relationship with technology
        assert!(result.contains("Rel(cognicode, sqlite, \"reads from\", \"SQL\")"));
    }
}
