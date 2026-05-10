//! Structurizr DSL renderer for C4 models
//!
//! Renders a full C4Workspace as Structurizr DSL format, including
//! model elements (people, systems, containers, components) and relationships.

use crate::model::c4_types::{Container, ContainerType, ElementLocation, Component};
use crate::model::workspace::C4Workspace;

/// Options for Structurizr DSL output
#[derive(Debug, Clone)]
pub struct StructurizrDslOptions {
    /// Include styles block (default: true)
    pub include_styles: bool,
    /// Include views block (default: true)
    pub include_views: bool,
    /// Enable autoLayout on views (default: true)
    pub auto_layout: bool,
    /// Remote theme URL (optional)
    pub theme: Option<String>,
}

impl Default for StructurizrDslOptions {
    fn default() -> Self {
        Self {
            include_styles: true,
            include_views: true,
            auto_layout: true,
            theme: None,
        }
    }
}

/// Sanitize a string to be a valid DSL identifier.
/// DSL identifiers cannot have hyphens, spaces, or dots.
fn sanitize_id(name: &str) -> String {
    name.replace('-', "_").replace(' ', "").replace('.', "_")
}

/// Render the full C4Workspace as Structurizr DSL format
pub fn render_structurizr_dsl(
    workspace: &C4Workspace,
    options: &StructurizrDslOptions,
) -> String {
    let mut output = String::new();

    // Workspace block header
    output.push_str(&format!(
        "workspace \"{}\" \"{}\" {{\n",
        workspace.name, workspace.description
    ));

    // Theme if specified
    if let Some(ref theme) = options.theme {
        output.push_str(&format!("    theme {}\n", theme));
    }

    // Model block
    output.push_str("    model {\n");
    output.push_str(&render_model(workspace));
    output.push_str("    }\n\n");

    // Views block
    if options.include_views {
        output.push_str("    views {\n");
        output.push_str(&render_views(workspace, options));
        output.push_str("    }\n");
    }

    // Styles block
    if options.include_styles {
        output.push_str("\n    styles {\n");
        output.push_str(&render_styles());
        output.push_str("    }\n");
    }

    output.push_str("}\n");

    output
}

/// Render the model block contents
fn render_model(workspace: &C4Workspace) -> String {
    let mut output = String::new();

    // Render people
    for person in &workspace.model.people {
        let location_suffix = match person.location {
            ElementLocation::External => " \"External\"",
            ElementLocation::Internal => "",
        };
        output.push_str(&format!(
            "        person \"{}\" \"{}\"{}",
            person.name, person.description, location_suffix
        ));
        output.push('\n');
    }

    // Find the main (internal) system
    let main_system = workspace.model.systems.iter()
        .find(|s| s.location == ElementLocation::Internal);

    // Render external systems first (no children)
    for system in &workspace.model.systems {
        if system.location == ElementLocation::External {
            output.push_str(&format!(
                "        softwareSystem \"{}\" \"{}\" \"External\"\n",
                system.name, system.description
            ));
        }
    }

    // Render main system with containers and components
    if let Some(system) = main_system {
        output.push_str(&format!(
            "        softwareSystem \"{}\" \"{}\" {{\n",
            system.name, system.description
        ));

        // Render containers
        for container in &system.containers {
            output.push_str(&render_container(container));
        }

        output.push_str("        }\n");
    }

    // Render relationships
    for rel in &workspace.model.relationships {
        let source = sanitize_id(rel.source_id.as_str());
        let target = sanitize_id(rel.target_id.as_str());
        let label = rel.label.as_ref()
            .map(|l| format!(" \"{}\"", l))
            .unwrap_or_default();
        let technology = rel.technology.as_ref()
            .map(|t| format!(" \"{}\"", t))
            .unwrap_or_default();

        output.push_str(&format!(
            "        {} -> {}{}{}\n",
            source, target, label, technology
        ));
    }

    output
}

/// Render a container element
fn render_container(container: &Container) -> String {
    let mut output = String::new();

    let (prefix, _type_keyword) = match container.container_type {
        ContainerType::DataStore => ("containerdb", "Container Database"),
        _ => ("container", "Container"),
    };

    output.push_str(&format!(
        "            {} \"{}\" \"{}\" \"{}\" \"{}\"\n",
        prefix,
        container.name,
        container.description,
        container.technology,
        sanitize_id(container.id.as_str())
    ));

    // Render components if any
    for component in &container.components {
        output.push_str(&render_component(component));
    }

    output
}

/// Render a component element
fn render_component(component: &Component) -> String {
    format!(
        "                component \"{}\" \"{}\" \"{}\" \"{}\"\n",
        component.name,
        component.description,
        component.technology,
        sanitize_id(component.id.as_str())
    )
}

/// Render the views block
fn render_views(workspace: &C4Workspace, options: &StructurizrDslOptions) -> String {
    let mut output = String::new();

    // Find main system
    let main_system = workspace.model.systems.iter()
        .find(|s| s.location == ElementLocation::Internal);

    // System Context view
    if let Some(system) = main_system {
        let layout = if options.auto_layout { "autoLayout lr" } else { "" };
        output.push_str(&format!(
            "        systemContext \"{}\" \"SystemContext\" {{\n",
            sanitize_id(system.id.as_str())
        ));
        output.push_str("            include *\n");
        if !layout.is_empty() {
            output.push_str(&format!("            {}\n", layout));
        }
        output.push_str("        }\n");
    }

    // Container view for main system
    if let Some(system) = main_system {
        let layout = if options.auto_layout { "autoLayout lr" } else { "" };
        output.push_str(&format!(
            "        container \"{}\" \"Containers\" {{\n",
            sanitize_id(system.id.as_str())
        ));
        output.push_str("            include *\n");
        if !layout.is_empty() {
            output.push_str(&format!("            {}\n", layout));
        }
        output.push_str("        }\n");
    }

    // Component views for each container with components
    if let Some(system) = main_system {
        for container in &system.containers {
            if !container.components.is_empty() {
                let layout = if options.auto_layout { "autoLayout tb" } else { "" };
                output.push_str(&format!(
                    "        component \"{}\" \"CoreComponents\" {{\n",
                    sanitize_id(container.id.as_str())
                ));
                output.push_str("            include *\n");
                if !layout.is_empty() {
                    output.push_str(&format!("            {}\n", layout));
                }
                output.push_str("        }\n");
            }
        }
    }

    output
}

/// Render the styles block with standard C4 colors
fn render_styles() -> String {
    // Using r## to avoid issues with # in color codes
    let software_system = r##"        element "Software System" {
            background "#1168bd"
            color "#ffffff"
        }
"##;
    let container = r##"        element "Container" {
            background "#438dd5"
            color "#ffffff"
        }
"##;
    let container_db = r##"        element "Container Database" {
            shape cylinder
            background "#438dd5"
            color "#ffffff"
        }
"##;
    let component = r##"        element "Component" {
            background "#85bbf0"
            color "#000000"
        }
"##;
    let person = r##"        element "Person" {
            shape person
            background "#08427b"
            color "#ffffff"
        }
"##;

    format!("{}{}{}{}{}", software_system, container, container_db, component, person)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{Component, ComponentType, Container, ElementId, Person, SoftwareSystem};
    use crate::model::relationships::{C4Relationship, C4RelationshipKind};

    fn create_test_workspace() -> C4Workspace {
        let core_container = Container {
            id: ElementId::new("cognicode-core"),
            name: "cognicode-core".to_string(),
            container_type: ContainerType::Library,
            technology: "Rust".to_string(),
            description: "Core library".to_string(),
            path: None,
            components: vec![
                Component {
                    id: ElementId::new("call-graph"),
                    name: "CallGraph".to_string(),
                    component_type: ComponentType::Module,
                    technology: "Rust".to_string(),
                    description: "Call graph analysis".to_string(),
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
            description: "Main MCP server binary".to_string(),
            path: None,
            components: vec![],
        };

        let sqlite_container = Container {
            id: ElementId::new("sqlite"),
            name: "SQLite".to_string(),
            container_type: ContainerType::DataStore,
            technology: "SQLite".to_string(),
            description: "Analysis cache".to_string(),
            path: None,
            components: vec![],
        };

        let main_system = SoftwareSystem {
            id: ElementId::new("cognicode"),
            name: "CogniCode".to_string(),
            description: "Code quality analysis engine".to_string(),
            location: ElementLocation::Internal,
            containers: vec![mcp_container, core_container, sqlite_container],
        };

        let developer = Person {
            id: ElementId::new("developer"),
            name: "Developer".to_string(),
            description: "CLI user".to_string(),
            location: ElementLocation::External,
        };

        let ai_agent = Person {
            id: ElementId::new("ai-agent"),
            name: "AI Agent".to_string(),
            description: "MCP protocol user".to_string(),
            location: ElementLocation::External,
        };

        let relationships = vec![
            C4Relationship::new(
                ElementId::new("developer"),
                ElementId::new("cognicode-mcp"),
                C4RelationshipKind::Uses,
            ).with_label("Uses CLI"),
            C4Relationship::new(
                ElementId::new("ai-agent"),
                ElementId::new("cognicode-mcp"),
                C4RelationshipKind::Uses,
            ).with_label("Uses MCP protocol"),
            C4Relationship::new(
                ElementId::new("cognicode-mcp"),
                ElementId::new("sqlite"),
                C4RelationshipKind::ReadsFrom,
            ).with_label("Reads/Writes").with_technology("SQL"),
        ];

        C4Workspace {
            name: "CogniCode".to_string(),
            description: "Code quality analysis platform".to_string(),
            model: crate::model::workspace::C4Model {
                people: vec![developer, ai_agent],
                systems: vec![main_system],
                relationships,
            },
            views: vec![],
        }
    }

    #[test]
    fn test_render_empty_workspace() {
        let workspace = C4Workspace::new("Empty");
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        assert!(result.starts_with("workspace \"Empty\""));
        assert!(result.contains("model {"));
        assert!(result.contains("}"));
    }

    #[test]
    fn test_render_workspace_with_people() {
        let workspace = create_test_workspace();
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        assert!(result.contains("person \"Developer\" \"CLI user\" \"External\""));
        assert!(result.contains("person \"AI Agent\" \"MCP protocol user\" \"External\""));
    }

    #[test]
    fn test_render_workspace_with_containers() {
        let workspace = create_test_workspace();
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        // Check container declarations
        assert!(result.contains("container \"cognicode-mcp\""));
        assert!(result.contains("container \"cognicode-core\""));
        // DataStore should use containerdb
        assert!(result.contains("containerdb \"SQLite\""));
    }

    #[test]
    fn test_dsl_has_valid_structure() {
        let workspace = create_test_workspace();
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        // Verify output starts with workspace
        assert!(result.starts_with("workspace \"CogniCode\""));

        // Verify model block exists
        assert!(result.contains("model {"));
        assert!(result.contains("softwareSystem"));

        // Verify views block exists
        assert!(result.contains("views {"));
        assert!(result.contains("systemContext"));
        assert!(result.contains("container"));

        // Verify styles block exists
        assert!(result.contains("styles {"));
        assert!(result.contains("element \"Software System\""));
    }

    #[test]
    fn test_sanitize_id() {
        assert_eq!(sanitize_id("my-id"), "my_id");
        assert_eq!(sanitize_id("hello world"), "helloworld");
        assert_eq!(sanitize_id("foo.bar"), "foo_bar");
        assert_eq!(sanitize_id("already_clean"), "already_clean");
    }

    #[test]
    fn test_include_styles_flag() {
        let workspace = create_test_workspace();

        // With styles
        let options_with_styles = StructurizrDslOptions {
            include_styles: true,
            ..Default::default()
        };
        let result_with = render_structurizr_dsl(&workspace, &options_with_styles);
        assert!(result_with.contains("styles {"));

        // Without styles
        let options_without_styles = StructurizrDslOptions {
            include_styles: false,
            ..Default::default()
        };
        let result_without = render_structurizr_dsl(&workspace, &options_without_styles);
        assert!(!result_without.contains("styles {"));
    }

    #[test]
    fn test_include_views_flag() {
        let workspace = create_test_workspace();

        // With views
        let options_with_views = StructurizrDslOptions {
            include_views: true,
            ..Default::default()
        };
        let result_with = render_structurizr_dsl(&workspace, &options_with_views);
        assert!(result_with.contains("views {"));

        // Without views
        let options_without_views = StructurizrDslOptions {
            include_views: false,
            ..Default::default()
        };
        let result_without = render_structurizr_dsl(&workspace, &options_without_views);
        assert!(!result_without.contains("views {"));
    }

    #[test]
    fn test_auto_layout_flag() {
        let workspace = create_test_workspace();

        // With auto_layout
        let options_with_layout = StructurizrDslOptions {
            auto_layout: true,
            ..Default::default()
        };
        let result_with = render_structurizr_dsl(&workspace, &options_with_layout);
        assert!(result_with.contains("autoLayout lr") || result_with.contains("autoLayout tb"));

        // Without auto_layout
        let options_without_layout = StructurizrDslOptions {
            auto_layout: false,
            ..Default::default()
        };
        let result_without = render_structurizr_dsl(&workspace, &options_without_layout);
        assert!(!result_without.contains("autoLayout"));
    }

    #[test]
    fn test_relationships_render() {
        let workspace = create_test_workspace();
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        // Check relationship rendering
        assert!(result.contains("developer -> cognicode_mcp"));
        assert!(result.contains("ai_agent -> cognicode_mcp"));
        assert!(result.contains("\"Uses CLI\""));
        assert!(result.contains("\"Uses MCP protocol\""));
    }

    #[test]
    fn test_components_render() {
        let workspace = create_test_workspace();
        let options = StructurizrDslOptions::default();
        let result = render_structurizr_dsl(&workspace, &options);

        // Check component inside container
        assert!(result.contains("component \"CallGraph\""));
        // Check component view is generated
        assert!(result.contains("CoreComponents"));
    }

    #[test]
    fn test_theme_option() {
        let workspace = C4Workspace::new("Test");

        let options_with_theme = StructurizrDslOptions {
            theme: Some("https://example.com/theme".to_string()),
            ..Default::default()
        };
        let result = render_structurizr_dsl(&workspace, &options_with_theme);

        assert!(result.contains("theme https://example.com/theme"));
    }

    #[test]
    fn test_structurizr_dsl_options_default() {
        let options = StructurizrDslOptions::default();

        assert!(options.include_styles);
        assert!(options.include_views);
        assert!(options.auto_layout);
        assert!(options.theme.is_none());
    }
}
