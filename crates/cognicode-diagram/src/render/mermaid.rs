//! Mermaid diagram renderer for C4 and UML diagrams

use crate::model::c4_types::{
    CodeElement, CodeElementKind, Container, ContainerType, Person, SoftwareSystem, UmlRelationKind,
    UmlRelationship, Visibility,
};
use crate::model::workspace::C4Workspace;

/// Options for Mermaid rendering
#[derive(Debug, Clone)]
pub struct MermaidOptions {
    /// Diagram title
    pub title: String,
    /// Mermaid theme (default, dark, forest, neutral)
    pub theme: Option<String>,
    /// Diagram direction: "TB", "BT", "LR", "RL"
    pub direction: String,
    /// Maximum depth for relationship traversal
    pub max_depth: usize,
    /// Whether to show methods in class diagrams
    pub show_methods: bool,
    /// Whether to show attributes in class diagrams
    pub show_attributes: bool,
    /// Whether to show visibility markers (+, -, #)
    pub show_visibility: bool,
}

impl Default for MermaidOptions {
    fn default() -> Self {
        Self {
            title: String::new(),
            theme: None,
            direction: "TB".to_string(),
            max_depth: 3,
            show_methods: true,
            show_attributes: true,
            show_visibility: true,
        }
    }
}

/// Escape text for safe inclusion in Mermaid diagrams
pub fn escape_mermaid(text: &str) -> String {
    text.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('{', "(")
        .replace('}', ")")
        .replace('<', "(")
        .replace('>', ")")
        .replace('&', "and")
        .replace('#', "")
        .replace('|', "")
        .replace('\n', " ")
}

/// Render a visibility marker for Mermaid class diagrams
fn visibility_marker(vis: Visibility) -> &'static str {
    match vis {
        Visibility::Public => "+",
        Visibility::Private => "-",
        Visibility::Protected => "#",
        Visibility::Package => "~",
    }
}

/// Render the Mermaid arrow syntax for a UML relationship kind
fn relation_arrow(kind: UmlRelationKind) -> &'static str {
    match kind {
        UmlRelationKind::Inheritance => "<|--",
        UmlRelationKind::Realization => "..|>",
        UmlRelationKind::Composition => "*--",
        UmlRelationKind::Aggregation => "o--",
        UmlRelationKind::Association => "-->",
        UmlRelationKind::Dependency => "..>",
    }
}

/// Render a class diagram from CodeElements and UML relationships
pub fn render_class_diagram(
    elements: &[CodeElement],
    relationships: &[UmlRelationship],
    options: &MermaidOptions,
) -> String {
    let mut lines = Vec::new();

    // Header
    if !options.title.is_empty() {
        lines.push(format!("---",));
        lines.push(format!("title: {}", escape_mermaid(&options.title)));
        lines.push("---".to_string());
    }
    lines.push("classDiagram".to_string());

    // Render each code element as a class
    for element in elements {
        let class_name = escape_mermaid(&element.name);

        // Class annotation for kind
        let annotation = match element.kind {
            CodeElementKind::Struct => "    <<struct>>",
            CodeElementKind::Enum => "    <<enum>>",
            CodeElementKind::Interface => "    <<interface>>",
            _ => "",
        };

        if !annotation.is_empty() {
            lines.push(format!("    class {} {{", class_name));
            lines.push(annotation.to_string());
        } else {
            lines.push(format!("    class {} {{", class_name));
        }

        // Attributes
        if options.show_attributes {
            for attr in &element.attributes {
                let vis = if options.show_visibility {
                    visibility_marker(attr.visibility).to_string()
                } else {
                    String::new()
                };
                let type_str = attr
                    .type_annotation
                    .as_ref()
                    .map(|t| format!(": {}", escape_mermaid(t)))
                    .unwrap_or_default();
                lines.push(format!("        {}{}{}", vis, escape_mermaid(&attr.name), type_str));
            }
        }

        // Methods
        if options.show_methods {
            for method in &element.methods {
                let vis = if options.show_visibility {
                    visibility_marker(method.visibility).to_string()
                } else {
                    String::new()
                };
                let params = method
                    .parameters
                    .iter()
                    .map(|(name, t)| {
                        t.as_ref()
                            .map(|ty| format!("{}: {}", escape_mermaid(name), escape_mermaid(ty)))
                            .unwrap_or_else(|| escape_mermaid(name))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = method
                    .return_type
                    .as_ref()
                    .map(|t| format!(": {}", escape_mermaid(t)))
                    .unwrap_or_default();
                lines.push(format!(
                    "        {}{}({}){}",
                    vis,
                    escape_mermaid(&method.name),
                    params,
                    ret
                ));
            }
        }

        lines.push("    }".to_string());
    }

    // Render relationships
    for rel in relationships {
        let target_name = escape_mermaid(&rel.target_id.as_str());
        // We need to find the source name from the element list
        // For now, we use the target_id directly since relationships are attached to elements
        let label = rel
            .label
            .as_ref()
            .map(|l| format!(" : {}", escape_mermaid(l)))
            .unwrap_or_default();
        let arrow = relation_arrow(rel.kind);
        lines.push(format!("    {} {} {}{}", target_name, arrow, target_name, label));
    }

    lines.join("\n")
}

/// Render a C4 Context diagram (L1) as a Mermaid flowchart
pub fn render_c4_context(workspace: &C4Workspace) -> String {
    let mut lines = Vec::new();

    lines.push("flowchart TB".to_string());

    if !workspace.name.is_empty() {
        lines.push(format!("    %% {}", escape_mermaid(&workspace.name)));
    }

    // Internal system
    let system_id = "sys_internal";
    lines.push(format!(
        "    {}[\"{}\\n{}\"]",
        system_id,
        escape_mermaid(&workspace.name),
        "Software System"
    ));
    lines.push(format!("    style {} fill:#1168bd,color:#fff", system_id));

    // People
    for person in &workspace.model.people {
        let pid = escape_mermaid(person.id.as_str());
        lines.push(format!(
            "    {}(\"{}\\n{}\")",
            pid,
            escape_mermaid(&person.name),
            "Person"
        ));
        lines.push(format!("    style {} fill:#08427b,color:#fff", pid));
    }

    // External systems
    for system in &workspace.model.systems {
        let sid = escape_mermaid(system.id.as_str());
        lines.push(format!(
            "    {}{{\"{}\\n{}\"}}",
            sid,
            escape_mermaid(&system.name),
            "External System"
        ));
        lines.push(format!("    style {} fill:#999,color:#fff", sid));
    }

    // Relationships
    for rel in &workspace.model.relationships {
        let source = escape_mermaid(rel.source_id.as_str());
        let target = escape_mermaid(rel.target_id.as_str());
        let label = rel
            .label
            .as_ref()
            .map(|l| format!("|{}|", escape_mermaid(l)))
            .unwrap_or_default();
        lines.push(format!("    {} --> {} {}", source, target, label));
    }

    lines.join("\n")
}

/// Render a C4 Container diagram (L2) as a Mermaid flowchart
pub fn render_c4_containers(workspace: &C4Workspace) -> String {
    let mut lines = Vec::new();

    lines.push("flowchart TB".to_string());
    lines.push(format!("    %% {} — Container View", escape_mermaid(&workspace.name)));

    for system in &workspace.model.systems {
        let system_label = escape_mermaid(&system.name);
        lines.push(format!("    subgraph {}", system_label));
        lines.push(format!("        direction TB"));

        for container in &system.containers {
            let cid = escape_mermaid(container.id.as_str());
            let container_shape = match container.container_type {
                ContainerType::DataStore => ("[(", ")]"),
                ContainerType::Queue => ("([", "])"),
                _ => ("[", "]"),
            };
            lines.push(format!(
                "        {}{}\"{}\\n[{}]\"{}",
                cid,
                container_shape.0,
                escape_mermaid(&container.name),
                escape_mermaid(&container.technology),
                container_shape.1
            ));
            lines.push(format!(
                "        style {} fill:{}",
                cid,
                container_color(&container.container_type)
            ));
        }

        lines.push("    end".to_string());
    }

    // Relationships
    for rel in &workspace.model.relationships {
        let source = escape_mermaid(rel.source_id.as_str());
        let target = escape_mermaid(rel.target_id.as_str());
        let label = rel
            .label
            .as_ref()
            .map(|l| format!("|{}|", escape_mermaid(l)))
            .unwrap_or_default();
        lines.push(format!("    {} --> {} {}", source, target, label));
    }

    lines.join("\n")
}

fn container_color(ct: &ContainerType) -> &'static str {
    match ct {
        ContainerType::Service => "#438dd5",
        ContainerType::Library => "#85bbf0",
        ContainerType::DataStore => "#6c6c6c",
        ContainerType::Executable => "#438dd5",
        ContainerType::Queue => "#999",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::c4_types::{ElementId, ElementLocation, Method, UmlRelationship};

    #[test]
    fn test_escape_mermaid() {
        assert_eq!(escape_mermaid("hello \"world\""), "hello 'world'");
        assert_eq!(escape_mermaid("a[b]c"), "a(b)c");
        assert_eq!(escape_mermaid("foo & bar"), "foo and bar");
    }

    #[test]
    fn test_render_empty_class_diagram() {
        let elements = vec![];
        let relationships = vec![];
        let options = MermaidOptions::default();
        let result = render_class_diagram(&elements, &relationships, &options);
        assert!(result.contains("classDiagram"));
    }

    #[test]
    fn test_render_class_diagram_with_element() {
        let element = CodeElement {
            id: ElementId::new("test::MyClass"),
            name: "MyClass".to_string(),
            kind: CodeElementKind::Class,
            visibility: Visibility::Public,
            path: Some("src/lib.rs".to_string()),
            attributes: vec![],
            methods: vec![Method {
                name: "new".to_string(),
                parameters: vec![],
                return_type: Some("Self".to_string()),
                visibility: Visibility::Public,
                is_async: false,
            }],
            relationships: vec![],
        };

        let result = render_class_diagram(&[element], &[], &MermaidOptions::default());
        assert!(result.contains("class MyClass"));
        assert!(result.contains("+new()"));
    }

    #[test]
    fn test_relation_arrow() {
        assert_eq!(relation_arrow(UmlRelationKind::Inheritance), "<|--");
        assert_eq!(relation_arrow(UmlRelationKind::Realization), "..|>");
        assert_eq!(relation_arrow(UmlRelationKind::Composition), "*--");
        assert_eq!(relation_arrow(UmlRelationKind::Dependency), "..>");
    }

    #[test]
    fn test_render_c4_context() {
        let workspace = C4Workspace::new("TestSystem");
        let result = render_c4_context(&workspace);
        assert!(result.contains("flowchart TB"));
        assert!(result.contains("TestSystem"));
    }
}
