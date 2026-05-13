//! Activity diagram renderer
//!
//! Renders activity diagrams to Mermaid flowchart and PlantUML activity formats.

use crate::model::activity_types::{
    ActivityEdge, ActivityModel, ActivityNode, ActivityNodeType,
};

/// Options for activity diagram rendering
#[derive(Debug, Clone)]
pub struct ActivityRenderOptions {
    /// Include node IDs in labels
    pub show_ids: bool,
    /// Title for the diagram
    pub title: String,
    /// Direction: TB (top-bottom), LR (left-right)
    pub direction: String,
    /// Show guards on edges
    pub show_guards: bool,
}

impl Default for ActivityRenderOptions {
    fn default() -> Self {
        Self {
            show_ids: false,
            title: String::new(),
            direction: "TB".to_string(),
            show_guards: true,
        }
    }
}

/// Render an activity diagram as Mermaid flowchart
pub fn render_activity_mermaid(
    model: &ActivityModel,
    options: &ActivityRenderOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push("flowchart TD".to_string());

    if options.show_ids {
        lines.push("    force_ltr".to_string());
    }

    if !options.title.is_empty() {
        lines.push(format!("    title: {}", escape_mermaid(&options.title)));
    }

    if options.direction != "TB" {
        lines.push(format!("    direction {}", options.direction));
    }

    // Render nodes
    for node in &model.nodes {
        let node_line = render_mermaid_node(node, options);
        lines.push(node_line);
    }

    // Render edges
    for edge in &model.edges {
        let edge_line = render_mermaid_edge(edge, options);
        lines.push(edge_line);
    }

    lines.join("\n")
}

/// Render a single node for Mermaid
fn render_mermaid_node(node: &ActivityNode, options: &ActivityRenderOptions) -> String {
    let id = escape_mermaid(&node.id);
    let label = if options.show_ids {
        format!("{}: {}", node.id, escape_mermaid(&node.name))
    } else {
        escape_mermaid(&node.name)
    };

    match node.node_type {
        ActivityNodeType::Initial => {
            format!("    {}[({})]", id, label)
        }
        ActivityNodeType::Final => {
            format!("    {}(({}))", id, label)
        }
        ActivityNodeType::Decision => {
            format!("    {}{{{}}}", id, label)
        }
        ActivityNodeType::Merge => {
            format!("    {}{{{}}}", id, label)
        }
        ActivityNodeType::Fork => {
            format!("    {}-->{}", id, label)
        }
        ActivityNodeType::Join => {
            format!("    {}-->{}", id, label)
        }
        ActivityNodeType::Loop => {
            if let Some(ref var) = node.loop_variable {
                format!("    {}(({} : {}))", id, label, escape_mermaid(var))
            } else {
                format!("    {}(({}))", id, label)
            }
        }
        ActivityNodeType::Action | ActivityNodeType::Call => {
            format!("    {}[{}]", id, label)
        }
    }
}

/// Render an edge for Mermaid
fn render_mermaid_edge(edge: &ActivityEdge, options: &ActivityRenderOptions) -> String {
    let from = escape_mermaid(&edge.from);
    let to = escape_mermaid(&edge.to);

    if let Some(ref guard) = edge.guard {
        if options.show_guards {
            return format!("    {} --{}--> {}", from, escape_mermaid(guard), to);
        }
    }

    if let Some(ref label) = edge.label {
        return format!("    {} --{}--> {}", from, escape_mermaid(label), to);
    }

    format!("    {} --> {}", from, to)
}

/// Escape text for Mermaid
fn escape_mermaid(text: &str) -> String {
    text.replace('"', "'")
        .replace('<', "(")
        .replace('>', ")")
        .replace('{', "(")
        .replace('}', ")")
}

/// Render an activity diagram as PlantUML activity
pub fn render_activity_plantuml(
    model: &ActivityModel,
    options: &ActivityRenderOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push("@startuml".to_string());

    if !options.title.is_empty() {
        lines.push(format!("title {}", escape_plantuml(&options.title)));
    }

    // Render nodes
    for node in &model.nodes {
        let node_line = render_plantuml_node(node, options);
        lines.push(node_line);
    }

    // Render edges
    for edge in &model.edges {
        let edge_line = render_plantuml_edge(edge, options);
        lines.push(edge_line);
    }

    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Render a single node for PlantUML
fn render_plantuml_node(node: &ActivityNode, _options: &ActivityRenderOptions) -> String {
    let label = escape_plantuml(&node.name);

    match node.node_type {
        ActivityNodeType::Initial => {
            format!("(*) --> {}", label)
        }
        ActivityNodeType::Final => {
            format!("{} --> (*)", label)
        }
        ActivityNodeType::Decision => {
            if let Some(ref guard) = node.guard {
                format!("if ({}) then ({})", escape_plantuml(guard), label)
            } else {
                format!("if (condition) then ({})", label)
            }
        }
        ActivityNodeType::Merge => {
            format!("{} --> merge", label)
        }
        ActivityNodeType::Fork => {
            format!("fork {}", label)
        }
        ActivityNodeType::Join => {
            format!("{} -left-> join", label)
        }
        ActivityNodeType::Loop => {
            if let Some(ref var) = node.loop_variable {
                format!("while ({}) is ({})", escape_plantuml(var), label)
            } else {
                format!("while (loop) is ({})", label)
            }
        }
        ActivityNodeType::Call => {
            format!(":{};", label)
        }
        ActivityNodeType::Action => {
            format!(":{};", label)
        }
    }
}

/// Render an edge for PlantUML
fn render_plantuml_edge(edge: &ActivityEdge, _options: &ActivityRenderOptions) -> String {
    let from = escape_plantuml(&edge.from);
    let to = escape_plantuml(&edge.to);

    if let Some(ref label) = edge.label {
        return format!("{} -right-> {} : {}", from, to, escape_plantuml(label));
    }

    format!("{} --> {}", from, to)
}

/// Escape text for PlantUML
fn escape_plantuml(text: &str) -> String {
    text.replace('"', "'")
}

/// Render an empty activity diagram
pub fn render_empty_activity(title: &str, format: &str) -> String {
    match format {
        "plantuml" => {
            format!(
                "@startuml\ntitle {}\n' No activity detected\n(*) -->> (*)\n@enduml",
                title
            )
        }
        _ => {
            // Mermaid default
            format!(
                "flowchart TD\n    title: {}\n    Start[Start] --> End[End]",
                title
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::activity_types::{ActivityEdge, ActivityModel, ActivityNode, ActivityNodeType};

    #[test]
    fn test_render_empty_activity_mermaid() {
        let result = render_empty_activity("Test", "mermaid");
        assert!(result.contains("flowchart TD"));
        assert!(result.contains("Start"));
    }

    #[test]
    fn test_render_empty_activity_plantuml() {
        let result = render_empty_activity("Test", "plantuml");
        assert!(result.contains("@startuml"));
        assert!(result.contains("@enduml"));
    }

    #[test]
    fn test_escape_mermaid() {
        assert_eq!(escape_mermaid("hello \"world\""), "hello 'world'");
        assert_eq!(escape_mermaid("a < b > c"), "a ( b ) c");
    }

    #[test]
    fn test_render_mermaid_node_initial() {
        let node = ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        };
        let result = render_mermaid_node(&node, &ActivityRenderOptions::default());
        assert_eq!(result, "    start[(Start)]");
    }

    #[test]
    fn test_render_mermaid_node_final() {
        let node = ActivityNode {
            id: "end".to_string(),
            name: "End".to_string(),
            node_type: ActivityNodeType::Final,
            guard: None,
            loop_variable: None,
            location: None,
        };
        let result = render_mermaid_node(&node, &ActivityRenderOptions::default());
        assert_eq!(result, "    end((End))");
    }

    #[test]
    fn test_render_mermaid_node_decision() {
        let node = ActivityNode {
            id: "choice".to_string(),
            name: "Is Valid?".to_string(),
            node_type: ActivityNodeType::Decision,
            guard: None,
            loop_variable: None,
            location: None,
        };
        let result = render_mermaid_node(&node, &ActivityRenderOptions::default());
        assert_eq!(result, "    choice{Is Valid?}");
    }

    #[test]
    fn test_render_mermaid_node_action() {
        let node = ActivityNode {
            id: "action1".to_string(),
            name: "Do Something".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        };
        let result = render_mermaid_node(&node, &ActivityRenderOptions::default());
        assert_eq!(result, "    action1[Do Something]");
    }

    #[test]
    fn test_render_mermaid_edge() {
        let edge = ActivityEdge {
            id: "e1".to_string(),
            from: "start".to_string(),
            to: "action1".to_string(),
            label: None,
            guard: None,
        };
        let result = render_mermaid_edge(&edge, &ActivityRenderOptions::default());
        assert_eq!(result, "    start --> action1");
    }

    #[test]
    fn test_render_mermaid_edge_with_guard() {
        let edge = ActivityEdge {
            id: "e1".to_string(),
            from: "choice".to_string(),
            to: "action1".to_string(),
            label: None,
            guard: Some("[isValid]".to_string()),
        };
        let result = render_mermaid_edge(&edge, &ActivityRenderOptions::default());
        assert!(result.contains("[isValid]"));
    }

    #[test]
    fn test_render_full_activity() {
        let mut model = ActivityModel::new("Process Flow", "main");

        model.add_node(ActivityNode {
            id: "start".to_string(),
            name: "Start".to_string(),
            node_type: ActivityNodeType::Initial,
            guard: None,
            loop_variable: None,
            location: None,
        });

        model.add_node(ActivityNode {
            id: "validate".to_string(),
            name: "Validate Input".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });

        model.add_node(ActivityNode {
            id: "choice".to_string(),
            name: "Is Valid?".to_string(),
            node_type: ActivityNodeType::Decision,
            guard: None,
            loop_variable: None,
            location: None,
        });

        model.add_node(ActivityNode {
            id: "process".to_string(),
            name: "Process Data".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });

        model.add_node(ActivityNode {
            id: "end".to_string(),
            name: "End".to_string(),
            node_type: ActivityNodeType::Final,
            guard: None,
            loop_variable: None,
            location: None,
        });

        model.add_edge(ActivityEdge {
            id: "e1".to_string(),
            from: "start".to_string(),
            to: "validate".to_string(),
            label: None,
            guard: None,
        });

        model.add_edge(ActivityEdge {
            id: "e2".to_string(),
            from: "validate".to_string(),
            to: "choice".to_string(),
            label: None,
            guard: None,
        });

        model.add_edge(ActivityEdge {
            id: "e3".to_string(),
            from: "choice".to_string(),
            to: "process".to_string(),
            label: None,
            guard: Some("[valid]".to_string()),
        });

        model.add_edge(ActivityEdge {
            id: "e4".to_string(),
            from: "process".to_string(),
            to: "end".to_string(),
            label: None,
            guard: None,
        });

        model.finalize();

        let options = ActivityRenderOptions::default();
        let result = render_activity_mermaid(&model, &options);

        assert!(result.contains("flowchart TD"));
        assert!(result.contains("Start"));
        assert!(result.contains("Validate Input"));
        assert!(result.contains("Is Valid?"));
        assert!(result.contains("Process Data"));
        assert!(result.contains("End"));
        assert!(result.contains("-->"));
    }
}
