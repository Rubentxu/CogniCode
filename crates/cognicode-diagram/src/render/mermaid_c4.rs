//! Mermaid C4 diagram renderers for Component and Container diagrams

use crate::model::c4_types::{Container, ContainerType, ElementLocation, ComponentType};
use crate::model::relationships::C4RelationshipKind;
use crate::model::workspace::C4Workspace;

pub struct C4MermaidOptions {
    pub direction: String,
    pub show_technology: bool,
    pub show_component_count: bool,
    pub theme: Option<String>,
}

impl Default for C4MermaidOptions {
    fn default() -> Self {
        Self {
            direction: "TB".to_string(),
            show_technology: true,
            show_component_count: true,
            theme: None,
        }
    }
}

/// Render a C4 Component diagram (L3) as Mermaid flowchart
/// Groups components by container and shows their relationships
pub fn render_component_diagram(
    containers: &[Container],
    relationships: &[crate::model::relationships::C4Relationship],
    options: &C4MermaidOptions,
) -> String {
    let mut lines = vec![
        "flowchart TB".to_string(),
        "    %% Component (L3) Diagram".to_string(),
        format!("    direction {}", options.direction),
    ];

    // Render each container with its components
    for container in containers {
        lines.push(format!(
            "    subgraph {}[\"{}\"]",
            container.id.as_str(),
            container.name
        ));

        for component in &container.components {
            let shape = match component.component_type {
                ComponentType::Service => "[",
                ComponentType::Controller => "[",
                ComponentType::Repository => "[(",
                ComponentType::Interface => "[(",
                ComponentType::Module => "[",
            };
            let closing = match component.component_type {
                ComponentType::Repository | ComponentType::Interface => ")]",
                _ => "]",
            };

            let tech_suffix = if options.show_technology && !component.technology.is_empty() {
                format!("\\n[{}]", component.technology)
            } else {
                String::new()
            };

            lines.push(format!(
                "        {}{}{}{}{}",
                component.id.as_str(),
                shape,
                component.name,
                tech_suffix,
                closing
            ));
        }

        lines.push("    end".to_string());
    }

    // Render relationships
    for rel in relationships {
        let arrow = match rel.kind {
            C4RelationshipKind::Calls => "-->",
            C4RelationshipKind::DependsOn => "-->",
            C4RelationshipKind::Uses => "..>",
            _ => "-->",
        };

        let label = rel.label.as_ref()
            .map(|l| format!(" : {}", l))
            .unwrap_or_default();

        lines.push(format!(
            "    {} {} {}{}",
            rel.source_id.as_str(),
            arrow,
            rel.target_id.as_str(),
            label
        ));
    }

    lines.join("\n")
}

/// Render a C4 Container diagram (L2) as Mermaid flowchart
/// Shows containers with their technologies
pub fn render_container_diagram(
    workspace: &C4Workspace,
    options: &C4MermaidOptions,
) -> String {
    let mut lines = vec![
        "flowchart TB".to_string(),
        "    %% Container (L2) Diagram".to_string(),
        format!("    direction {}", options.direction),
        format!("    subgraph system[\"{}\"]", workspace.name),
    ];

    // Internal containers
    for system in &workspace.model.systems {
        if system.location == ElementLocation::Internal {
            for container in &system.containers {
                let shape = match container.container_type {
                    ContainerType::Service | ContainerType::Executable => "[",
                    ContainerType::DataStore => "[(",
                    ContainerType::Queue => "([",
                    ContainerType::Library => "[[",
                };
                let closing = match container.container_type {
                    ContainerType::DataStore => ")]",
                    ContainerType::Queue => "])",
                    ContainerType::Library => "]]",
                    _ => "]",
                };

                let tech_suffix = if options.show_technology && !container.technology.is_empty() {
                    format!("\\n[{}]", container.technology)
                } else {
                    String::new()
                };

                lines.push(format!(
                    "        {}{}{}{}{}",
                    container.id.as_str(),
                    shape,
                    container.name,
                    tech_suffix,
                    closing
                ));
            }
        }
    }

    lines.push("    end".to_string());

    // External systems
    for system in &workspace.model.systems {
        if system.location == ElementLocation::External {
            lines.push(format!(
                "    {}{{ \"{} \\n[External]\" }}",
                system.id.as_str(),
                system.name
            ));
        }
    }

    // People
    for person in &workspace.model.people {
        lines.push(format!(
            "    {}((\"{}\"))",
            person.id.as_str(),
            person.name
        ));
    }

    // Relationships
    for rel in &workspace.model.relationships {
        let arrow = match rel.kind {
            C4RelationshipKind::Uses => "-->",
            C4RelationshipKind::Calls => "-->",
            C4RelationshipKind::DependsOn => "-->",
            _ => "-->",
        };

        let label = rel.label.as_ref()
            .map(|l| format!(" : {}", l))
            .unwrap_or_default();

        lines.push(format!(
            "    {} {} {}{}",
            rel.source_id.as_str(),
            arrow,
            rel.target_id.as_str(),
            label
        ));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{Component, ElementId, Person, SoftwareSystem};
    use crate::model::relationships::C4Relationship;

    fn create_test_workspace() -> C4Workspace {
        let web_app = Container {
            id: ElementId::new("web-app"),
            name: "Web Application".to_string(),
            container_type: ContainerType::Service,
            technology: "React".to_string(),
            description: "Main web interface".to_string(),
            path: None,
            components: vec![
                Component {
                    id: ElementId::new("comp-user-service"),
                    name: "UserService".to_string(),
                    component_type: ComponentType::Service,
                    technology: "TypeScript".to_string(),
                    description: "Handles user operations".to_string(),
                    path: None,
                    code_elements: vec![],
                },
                Component {
                    id: ElementId::new("comp-user-repo"),
                    name: "UserRepository".to_string(),
                    component_type: ComponentType::Repository,
                    technology: "TypeScript".to_string(),
                    description: "Data access".to_string(),
                    path: None,
                    code_elements: vec![],
                },
            ],
        };

        let api_service = Container {
            id: ElementId::new("api-service"),
            name: "API Service".to_string(),
            container_type: ContainerType::Service,
            technology: "Node.js".to_string(),
            description: "REST API".to_string(),
            path: None,
            components: vec![
                Component {
                    id: ElementId::new("comp-auth"),
                    name: "AuthController".to_string(),
                    component_type: ComponentType::Controller,
                    technology: "Node.js".to_string(),
                    description: "Authentication".to_string(),
                    path: None,
                    code_elements: vec![],
                },
            ],
        };

        let database = Container {
            id: ElementId::new("database"),
            name: "Database".to_string(),
            container_type: ContainerType::DataStore,
            technology: "PostgreSQL".to_string(),
            description: "Primary database".to_string(),
            path: None,
            components: vec![],
        };

        let internal_system = SoftwareSystem {
            id: ElementId::new("internal-system"),
            name: "Internal System".to_string(),
            description: "Main system".to_string(),
            location: ElementLocation::Internal,
            containers: vec![web_app, api_service, database],
        };

        let external_system = SoftwareSystem {
            id: ElementId::new("external-system"),
            name: "External API".to_string(),
            description: "Third party service".to_string(),
            location: ElementLocation::External,
            containers: vec![],
        };

        let user = Person {
            id: ElementId::new("user"),
            name: "User".to_string(),
            description: "End user".to_string(),
            location: ElementLocation::Internal,
        };

        let relationships = vec![
            C4Relationship::new(
                ElementId::new("comp-user-service"),
                ElementId::new("comp-user-repo"),
                C4RelationshipKind::Calls,
            ),
            C4Relationship::new(
                ElementId::new("comp-user-repo"),
                ElementId::new("database"),
                C4RelationshipKind::Uses,
            ),
        ];

        C4Workspace {
            name: "Test System".to_string(),
            description: "Test workspace".to_string(),
            model: crate::model::workspace::C4Model {
                people: vec![user],
                systems: vec![internal_system, external_system],
                relationships,
            },
            views: vec![],
        }
    }

    #[test]
    fn test_render_component_diagram_basic() {
        let workspace = create_test_workspace();
        let containers: Vec<Container> = workspace.model.systems
            .iter()
            .flat_map(|s| s.containers.clone())
            .collect();

        let relationships = vec![
            C4Relationship::new(
                ElementId::new("comp-user-service"),
                ElementId::new("comp-user-repo"),
                C4RelationshipKind::Calls,
            ),
        ];

        let options = C4MermaidOptions::default();
        let result = render_component_diagram(&containers, &relationships, &options);

        assert!(result.contains("flowchart TB"));
        assert!(result.contains("subgraph web-app"));
        assert!(result.contains("comp-user-service"));
        assert!(result.contains("UserService"));
        assert!(result.contains("comp-user-repo"));
        assert!(result.contains("UserRepository"));
        assert!(result.contains("comp-user-service --> comp-user-repo"));
    }

    #[test]
    fn test_render_component_diagram_without_technology() {
        let workspace = create_test_workspace();
        let containers: Vec<Container> = workspace.model.systems
            .iter()
            .flat_map(|s| s.containers.clone())
            .collect();

        let options = C4MermaidOptions {
            show_technology: false,
            ..Default::default()
        };
        let result = render_component_diagram(&containers, &[], &options);

        // Technology should not appear in brackets
        assert!(!result.contains("\\n[TypeScript]"));
        assert!(!result.contains("\\n[Node.js]"));
    }

    #[test]
    fn test_render_component_diagram_direction() {
        let workspace = create_test_workspace();
        let containers: Vec<Container> = workspace.model.systems
            .iter()
            .flat_map(|s| s.containers.clone())
            .collect();

        let options = C4MermaidOptions {
            direction: "LR".to_string(),
            ..Default::default()
        };
        let result = render_component_diagram(&containers, &[], &options);

        assert!(result.contains("direction LR"));
    }

    #[test]
    fn test_render_container_diagram_basic() {
        let workspace = create_test_workspace();
        let options = C4MermaidOptions::default();
        let result = render_container_diagram(&workspace, &options);

        assert!(result.contains("flowchart TB"));
        assert!(result.contains("subgraph system"));
        assert!(result.contains("Container (L2) Diagram"));
        assert!(result.contains("web-app"));
        assert!(result.contains("Web Application"));
        assert!(result.contains("api-service"));
        assert!(result.contains("database"));
        assert!(result.contains("external-system"));
        assert!(result.contains("[External]"));
        assert!(result.contains("user"));
    }

    #[test]
    fn test_render_container_diagram_shows_technology() {
        let workspace = create_test_workspace();
        let options = C4MermaidOptions::default();
        let result = render_container_diagram(&workspace, &options);

        // Technology should be shown
        assert!(result.contains("\\n[React]"));
        assert!(result.contains("\\n[Node.js]"));
        assert!(result.contains("\\n[PostgreSQL]"));
    }

    #[test]
    fn test_render_container_diagram_hides_technology() {
        let workspace = create_test_workspace();
        let options = C4MermaidOptions {
            show_technology: false,
            ..Default::default()
        };
        let result = render_container_diagram(&workspace, &options);

        // Technology should not appear
        assert!(!result.contains("\\n[React]"));
        assert!(!result.contains("\\n[PostgreSQL]"));
    }

    #[test]
    fn test_render_container_diagram_relationships() {
        let workspace = create_test_workspace();
        let options = C4MermaidOptions::default();
        let result = render_container_diagram(&workspace, &options);

        // Check relationships are rendered
        assert!(result.contains("comp-user-service --> comp-user-repo"));
    }

    #[test]
    fn test_container_shapes() {
        let mut workspace = create_test_workspace();

        // Add a queue container
        let queue = Container {
            id: ElementId::new("message-queue"),
            name: "Message Queue".to_string(),
            container_type: ContainerType::Queue,
            technology: "RabbitMQ".to_string(),
            description: "Async messaging".to_string(),
            path: None,
            components: vec![],
        };

        if let Some(system) = workspace.model.systems.iter_mut().find(|s| s.id.as_str() == "internal-system") {
            system.containers.push(queue);
        }

        let options = C4MermaidOptions::default();
        let result = render_container_diagram(&workspace, &options);

        // Queue should use ([  ])
        assert!(result.contains("message-queue(["));
        assert!(result.contains("])"));
    }

    #[test]
    fn test_c4_mermaid_options_default() {
        let options = C4MermaidOptions::default();

        assert_eq!(options.direction, "TB");
        assert!(options.show_technology);
        assert!(options.show_component_count);
        assert!(options.theme.is_none());
    }

    #[test]
    fn test_component_shapes() {
        let containers = vec![Container {
            id: ElementId::new("test-container"),
            name: "Test Container".to_string(),
            container_type: ContainerType::Service,
            technology: "Rust".to_string(),
            description: "Test".to_string(),
            path: None,
            components: vec![
                Component {
                    id: ElementId::new("svc"),
                    name: "ServiceComp".to_string(),
                    component_type: ComponentType::Service,
                    technology: "Rust".to_string(),
                    description: "".to_string(),
                    path: None,
                    code_elements: vec![],
                },
                Component {
                    id: ElementId::new("repo"),
                    name: "RepoComp".to_string(),
                    component_type: ComponentType::Repository,
                    technology: "Rust".to_string(),
                    description: "".to_string(),
                    path: None,
                    code_elements: vec![],
                },
                Component {
                    id: ElementId::new("ctrl"),
                    name: "CtrlComp".to_string(),
                    component_type: ComponentType::Controller,
                    technology: "Rust".to_string(),
                    description: "".to_string(),
                    path: None,
                    code_elements: vec![],
                },
            ],
        }];

        let result = render_component_diagram(&containers, &[], &C4MermaidOptions::default());

        // Service uses [ ]
        assert!(result.contains("svc["));
        assert!(result.contains("]"));

        // Repository uses [(  )]
        assert!(result.contains("repo[("));
        assert!(result.contains(")]"));

        // Controller uses [ ]
        assert!(result.contains("ctrl["));
    }
}
