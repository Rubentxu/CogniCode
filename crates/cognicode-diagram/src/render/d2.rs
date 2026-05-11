//! D2 diagram renderer for C4 models
//!
//! Renders C4 models using the D2 diagram language.

use crate::model::c4_types::{ContainerType, ElementLocation};
use crate::model::workspace::C4Workspace;

/// D2 theme options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D2Theme {
    Classic,
    Dark,
    Default,
}

impl Default for D2Theme {
    fn default() -> Self {
        Self::Default
    }
}

/// D2 layout direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D2Direction {
    Down,
    Right,
}

impl Default for D2Direction {
    fn default() -> Self {
        Self::Down
    }
}

impl D2Direction {
    fn as_d2_string(&self) -> &'static str {
        match self {
            D2Direction::Down => "down",
            D2Direction::Right => "right",
        }
    }
}

/// Options for D2 output
#[derive(Debug, Clone)]
pub struct D2Options {
    /// Theme for colors and styling
    pub theme: D2Theme,
    /// Layout direction
    pub direction: D2Direction,
    /// Whether to use sketch mode
    pub sketch: bool,
    /// Padding between elements
    pub pad: u32,
}

impl Default for D2Options {
    fn default() -> Self {
        Self {
            theme: D2Theme::Default,
            direction: D2Direction::Down,
            sketch: false,
            pad: 100,
        }
    }
}

/// Renders a C4 workspace as D2 diagram source.
///
/// D2 is a modern diagram language that compiles to SVG and allows for
/// flexible layout and styling of diagrams.
///
/// # Examples
///
/// ```
/// use cognicode_diagram::model::workspace::C4Workspace;
/// use cognicode_diagram::render::d2::{render_d2, D2Options, D2Theme, D2Direction};
///
/// # let workspace = C4Workspace::new("MySystem");
/// let options = D2Options {
///     theme: D2Theme::Default,
///     direction: D2Direction::Down,
///     sketch: false,
///     pad: 100,
/// };
/// let d2_source = render_d2(&workspace, &options);
/// assert!(d2_source.contains("direction"));
/// ```
pub fn render_d2(workspace: &C4Workspace, options: &D2Options) -> String {
    let mut lines = vec![];

    // Direction
    lines.push(format!("direction: {}", options.direction.as_d2_string()));
    lines.push(String::new());

    // Theme settings via shape styles
    let (fill_person, fill_system, fill_container, fill_database, fill_queue, fill_hexagon, fill_external) =
        match options.theme {
            D2Theme::Classic => (
                "#08427B", // Person fill (blue)
                "#1168BD", // System fill (lighter blue)
                "#438DD5", // Container fill
                "#85BBF0", // Database fill (light blue)
                "#85BBF0", // Queue fill
                "#85BBF0", // Hexagon fill
                "#999999", // External system fill (grey)
            ),
            D2Theme::Dark => (
                "#0F4C81",
                "#1A3A5C",
                "#2D5A87",
                "#4A7FB5",
                "#4A7FB5",
                "#4A7FB5",
                "#666666",
            ),
            D2Theme::Default => (
                "#1168BD",
                "#438DD5",
                "#85BBF0",
                "#85BBF0",
                "#85BBF0",
                "#85BBF0",
                "#999999",
            ),
        };

    // L1: People
    for person in &workspace.model.people {
        let fill = if person.location == ElementLocation::External {
            fill_external
        } else {
            fill_person
        };
        lines.push(format!(
            "{}: {{\n  shape: person\n  style.fill: \"{}\"\n  label: \"{} ({})\"\n}}",
            sanitize_id(&person.id.to_string()),
            fill,
            person.name,
            person.description
        ));
    }

    // L1: Systems
    for system in &workspace.model.systems {
        if system.containers.is_empty() {
            // L1 system (no containers) - render as simple rectangle
            let fill = if system.location == ElementLocation::External {
                fill_external
            } else {
                fill_system
            };
            lines.push(format!(
                "{}: {{\n  shape: rectangle\n  style.fill: \"{}\"\n  label: \"{} ({})\"\n}}",
                sanitize_id(&system.id.to_string()),
                fill,
                system.name,
                system.description
            ));
        } else {
            // L2+ system - render as group with containers
            let fill = if system.location == ElementLocation::External {
                fill_external
            } else {
                fill_system
            };
            lines.push(format!(
                "{}: {{\n  label: \"{}\"\n  style.fill: \"{}\"",
                sanitize_id(&system.id.to_string()),
                system.name,
                fill
            ));

            // Containers within system
            for container in &system.containers {
                let (shape, container_fill) = match container.container_type {
                    ContainerType::DataStore => ("cylinder", fill_database),
                    ContainerType::Queue => ("queue", fill_queue),
                    _ => ("rectangle", fill_container),
                };

                if container.components.is_empty() {
                    // L2 container
                    lines.push(format!(
                        "  {}: {{\n    shape: {}\n    style.fill: \"{}\"\n    label: \"{} ({})\"\n  }}",
                        sanitize_id(&container.id.to_string()),
                        shape,
                        container_fill,
                        container.name,
                        container.description
                    ));
                } else {
                    // L3 component view - container with components
                    lines.push(format!(
                        "  {}: {{\n    label: \"{} ({})\"\n    shape: {}",
                        sanitize_id(&container.id.to_string()),
                        container.name,
                        container.description,
                        shape
                    ));

                    for component in &container.components {
                        lines.push(format!(
                            "    {}: {{\n      shape: hexagon\n      style.fill: \"{}\"\n      label: \"{} ({})\"\n    }}",
                            sanitize_id(&component.id.to_string()),
                            fill_hexagon,
                            component.name,
                            component.description
                        ));
                    }
                    lines.push("  }".to_string());
                }
            }
            lines.push("}".to_string());
        }
    }

    lines.push(String::new());

    // Relationships
    for rel in &workspace.model.relationships {
        let source = sanitize_id(&rel.source_id.to_string());
        let target = sanitize_id(&rel.target_id.to_string());
        let label = rel.label.as_deref().unwrap_or("");
        let arrow = if rel.kind.is_async() { "-->" } else { "->" };

        if label.is_empty() {
            lines.push(format!("{} {} {}", source, arrow, target));
        } else {
            lines.push(format!("{} {} {}: \"{}\"", source, arrow, target, label));
        }
    }

    lines.join("\n")
}

/// Sanitize string to be a valid D2 identifier
fn sanitize_id(id: &str) -> String {
    id.replace('-', "_")
        .replace(' ', "_")
        .replace('.', "_")
        .replace(':', "_")
}

/// Check if a relationship kind is async
trait AsyncMarker {
    fn is_async(&self) -> bool;
}

impl AsyncMarker for crate::model::relationships::C4RelationshipKind {
    fn is_async(&self) -> bool {
        matches!(self, crate::model::relationships::C4RelationshipKind::ReadsFrom
            | crate::model::relationships::C4RelationshipKind::WritesTo)
    }
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
    fn test_render_empty_workspace() {
        let workspace = C4Workspace::new("Empty");
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        assert!(result.contains("direction:"));
        assert!(result.contains("down"));
    }

    #[test]
    fn test_render_l1_with_people() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Should contain person shapes for user and developer
        assert!(result.contains("shape: person"));
        assert!(result.contains("user"));
        assert!(result.contains("developer"));
    }

    #[test]
    fn test_render_l1_systems() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Internal system should be in a group with containers
        assert!(result.contains("cognicode"));
        // External system should be a rectangle
        assert!(result.contains("otel_collector"));
    }

    #[test]
    fn test_render_l2_containers() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Container shapes
        assert!(result.contains("cognicode_core"));
        assert!(result.contains("cognicode_mcp"));
        // Database should be cylinder
        assert!(result.contains("shape: cylinder"));
        assert!(result.contains("sqlite"));
    }

    #[test]
    fn test_render_l3_components() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Components should be hexagons
        assert!(result.contains("shape: hexagon"));
        assert!(result.contains("domain"));
        assert!(result.contains("infrastructure"));
    }

    #[test]
    fn test_render_relationships_sync() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Synchronous relationship (Uses)
        assert!(result.contains("->"));
    }

    #[test]
    fn test_render_relationships_async() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Async relationship (ReadsFrom)
        assert!(result.contains("-->"));
    }

    #[test]
    fn test_render_relationship_labels() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // Label should appear in quotes
        assert!(result.contains("\"reads from\""));
    }

    #[test]
    fn test_render_dark_theme() {
        let workspace = create_test_workspace();
        let options = D2Options {
            theme: D2Theme::Dark,
            direction: D2Direction::Down,
            sketch: false,
            pad: 100,
        };
        let result = render_d2(&workspace, &options);

        // Dark theme should use dark colors
        assert!(result.contains("0F4C81") || result.contains("1A3A5C") || result.contains("2D5A87"));
    }

    #[test]
    fn test_render_classic_theme() {
        let workspace = create_test_workspace();
        let options = D2Options {
            theme: D2Theme::Classic,
            direction: D2Direction::Down,
            sketch: false,
            pad: 100,
        };
        let result = render_d2(&workspace, &options);

        // Classic theme should use blue colors
        assert!(result.contains("08427B") || result.contains("1168BD") || result.contains("438DD5"));
    }

    #[test]
    fn test_render_direction_right() {
        let workspace = create_test_workspace();
        let options = D2Options {
            theme: D2Theme::Default,
            direction: D2Direction::Right,
            sketch: false,
            pad: 100,
        };
        let result = render_d2(&workspace, &options);

        assert!(result.contains("direction: right"));
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("my-id"), "my_id");
        assert_eq!(sanitize_id("my id"), "my_id");
        assert_eq!(sanitize_id("my.id"), "my_id");
        assert_eq!(sanitize_id("my:id"), "my_id");
    }

    #[test]
    fn test_d2_options_default() {
        let options = D2Options::default();
        assert_eq!(options.theme, D2Theme::Default);
        assert_eq!(options.direction, D2Direction::Down);
        assert!(!options.sketch);
        assert_eq!(options.pad, 100);
    }

    #[test]
    fn test_d2_theme_default() {
        let theme = D2Theme::default();
        assert_eq!(theme, D2Theme::Default);
    }

    #[test]
    fn test_d2_direction_default() {
        let direction = D2Direction::default();
        assert_eq!(direction, D2Direction::Down);
    }

    #[test]
    fn test_d2_direction_as_string() {
        assert_eq!(D2Direction::Down.as_d2_string(), "down");
        assert_eq!(D2Direction::Right.as_d2_string(), "right");
    }

    #[test]
    fn test_container_database_shape() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // SQLite container is a DataStore, should be cylinder
        assert!(result.contains("sqlite: {"));
        assert!(result.contains("shape: cylinder"));
    }

    #[test]
    fn test_external_system_fill() {
        let workspace = create_test_workspace();
        let options = D2Options::default();
        let result = render_d2(&workspace, &options);

        // External system should have grey fill
        assert!(result.contains("otel_collector: {"));
        assert!(result.contains("999999"));
    }
}