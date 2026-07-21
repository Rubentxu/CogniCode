//! C4-level Mermaid diagram renderer.
//!
//! Converts a [`SubgraphResponse`] (nodes + edges) into a Mermaid C4Context,
//! C4Container, or C4Component diagram string. Pure function — no I/O,
//! deterministic output.
//!
//! ## C4 Model Overview
//!
//! | Level | Mermaid keyword | Description |
//! |-------|-----------------|-------------|
//! | Context | `C4Context` | Top-level system boundary |
//! | Container | `C4Container` | Deployable units within a system |
//! | Component | `C4Component` | Components within a container |
//!
//! ## ID Sanitisation
//!
//! Mermaid requires alphanumeric identifiers. Special characters (`:`, `/`, `(`, `)`, etc.)
//! are replaced with underscores. Duplicated IDs receive numeric suffixes (`_2`, `_3`).

use crate::dto::{GraphEdge, GraphNode};
use std::fmt::{self, Write};

/// C4 diagram levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum C4Level {
    /// Top-level system boundary.
    Context,
    /// Deployable units within a system.
    Container,
    /// Components within a container.
    Component,
}

impl C4Level {
    /// Parse a level string (case-insensitive).
    pub fn parse(s: &str) -> Result<Self, C4ParseError> {
        match s.to_ascii_lowercase().as_str() {
            "context" => Ok(Self::Context),
            "container" => Ok(Self::Container),
            "component" => Ok(Self::Component),
            other => Err(C4ParseError::UnknownLevel(other.to_string())),
        }
    }

    /// Mermaid keyword for this level.
    pub fn as_mermaid_keyword(&self) -> &'static str {
        match self {
            Self::Context => "C4Context",
            Self::Container => "C4Container",
            Self::Component => "C4Component",
        }
    }
}

/// Parse error for C4Level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum C4ParseError {
    UnknownLevel(String),
}

impl fmt::Display for C4ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLevel(s) => {
                write!(
                    f,
                    "unknown C4 level: {s} (expected: context, container, component)"
                )
            }
        }
    }
}

impl std::error::Error for C4ParseError {}

// Re-export from shared mermaid_util so c4_mermaid remains a drop-in replacement
pub use super::mermaid_util::{deduplicate_ids, sanitize_id};

/// Render a C4 diagram as a Mermaid string.
///
/// `nodes` and `edges` come from [`GraphService::build_architecture`].
/// `level` selects the Mermaid diagram type (C4Context / C4Container / C4Component).
///
/// ## Behaviour
///
/// - Self-loop edges are silently omitted.
/// - Empty levels (no nodes) produce a placeholder comment.
/// - IDs are sanitised; duplicates are resolved with `_2`, `_3`, … suffixes.
/// - Person/external actors use `C4Context` notation; containers/components use
///   the appropriate `C4Container`/`C4Component` notation.
/// - `part_of` and `depends_on` edges are rendered as `Rel_U`, `Rel_D` variants.
pub fn c4_to_mermaid(nodes: &[GraphNode], edges: &[GraphEdge], level: C4Level) -> String {
    if nodes.is_empty() {
        return format!(
            "%% {}\n%% No nodes at this level — nothing to render\n",
            level.as_mermaid_keyword()
        );
    }

    // Determine which node IDs are actually used in non-self-loop edges.
    let used_targets: std::collections::HashSet<&str> = edges
        .iter()
        .filter(|e| e.source != e.target)
        .map(|e| e.target.as_str())
        .collect();

    // Collect all IDs and deduplicate
    let all_ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let id_map: Vec<String> = deduplicate_ids(&all_ids);
    let id_to_sanitised: std::collections::HashMap<&str, &str> = all_ids
        .iter()
        .zip(id_map.iter())
        .map(|(orig, san)| (*orig, san.as_str()))
        .collect();

    // Filter to only nodes that appear in non-self-loop edges OR are at context level
    // First zip nodes with their sanitised IDs, then filter
    let all_zipped: Vec<(&GraphNode, &str)> = nodes
        .iter()
        .zip(id_map.iter())
        .map(|(n, san)| (n, san.as_str()))
        .collect();

    // Collect all node IDs that appear as source OR target in non-self-loop edges
    let used_in_edges: std::collections::HashSet<&str> = edges
        .iter()
        .filter(|e| e.source != e.target)
        .flat_map(|e| [e.source.as_str(), e.target.as_str()])
        .collect();

    let relevant_nodes: Vec<(&GraphNode, &str)> = if level == C4Level::Context {
        all_zipped
    } else {
        // At Container/Component level, include all nodes that appear in edges
        // (either as source or target)
        all_zipped
            .into_iter()
            .filter(|(n, _)| used_in_edges.contains(n.id.as_str()))
            .collect()
    };

    let mut lines = Vec::with_capacity(nodes.len() + edges.len() + 4);

    // Header
    lines.push(format!("{}\n", level.as_mermaid_keyword()));
    lines.push("".to_string());

    // Render nodes
    for (node, sanitised_id) in &relevant_nodes {
        let label = &node.label;
        let kind = &node.kind;
        let style_class = &node.style_class;

        match level {
            C4Level::Context => {
                // At context level, system/person nodes
                match style_class.as_str() {
                    "node-system" => {
                        lines.push(format!("System({}, \"{}\", \"\")", sanitised_id, label));
                    }
                    "node-container" => {
                        lines.push(format!(
                            "Container({}, \"{}\", \"\", \"\")",
                            sanitised_id, label
                        ));
                    }
                    _ => {
                        // Generic node
                        lines.push(format!("System_Ext({}, \"{}\", \"\")", sanitised_id, label));
                    }
                }
            }
            C4Level::Container => match style_class.as_str() {
                "node-system" => {
                    lines.push(format!("System({}, \"{}\", \"\")", sanitised_id, label));
                }
                "node-container" => {
                    lines.push(format!(
                        "Container({}, \"{}\", \"\", \"\")",
                        sanitised_id, label
                    ));
                }
                "node-component" => {
                    lines.push(format!(
                        "Component({}, \"{}\", \"\", \"\")",
                        sanitised_id, label
                    ));
                }
                _ => {
                    lines.push(format!(
                        "Container({}, \"{}\", \"\", \"\")",
                        sanitised_id, label
                    ));
                }
            },
            C4Level::Component => match kind.as_str() {
                "function" | "method" | "fn" => {
                    lines.push(format!(
                        "Component({}, \"{}\", \"\", \"\")",
                        sanitised_id, label
                    ));
                }
                _ => {
                    lines.push(format!(
                        "Component({}, \"{}\", \"\", \"\")",
                        sanitised_id, label
                    ));
                }
            },
        }
    }

    lines.push("".to_string());

    // Render edges (skip self-loops)
    for edge in edges {
        if edge.source == edge.target {
            continue; // skip self-loop
        }

        let Some(&src_san) = id_to_sanitised.get(edge.source.as_str()) else {
            continue;
        };
        let Some(&tgt_san) = id_to_sanitised.get(edge.target.as_str()) else {
            continue;
        };

        let rel_tag = match edge.relation.as_str() {
            "part_of" => "Rel_U",
            "depends_on" | "depends-on" | "dependency" => "Rel_D",
            "calls" => "Rel",
            _ => "Rel",
        };

        lines.push(format!("{}({}, {}, \"\")", rel_tag, src_san, tgt_san));
    }

    lines.join("\n")
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_node(id: &str, label: &str, kind: &str, style_class: &str) -> GraphNode {
        GraphNode {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            file: None,
            line: None,
            style_class: style_class.to_string(),
        }
    }

    fn make_edge(source: &str, target: &str, relation: &str) -> GraphEdge {
        GraphEdge {
            source: source.to_string(),
            target: target.to_string(),
            relation: relation.to_string(),
            style_class: format!("edge-{}", relation),
        }
    }

    // ------------------------------------------------------------------------
    // C4Level::parse
    // ------------------------------------------------------------------------

    #[test]
    fn c4_level_parse_context() {
        assert_eq!(C4Level::parse("context").unwrap(), C4Level::Context);
        assert_eq!(C4Level::parse("CONTEXT").unwrap(), C4Level::Context);
        assert_eq!(C4Level::parse("Context").unwrap(), C4Level::Context);
    }

    #[test]
    fn c4_level_parse_container() {
        assert_eq!(C4Level::parse("container").unwrap(), C4Level::Container);
        assert_eq!(C4Level::parse("CONTAINER").unwrap(), C4Level::Container);
    }

    #[test]
    fn c4_level_parse_component() {
        assert_eq!(C4Level::parse("component").unwrap(), C4Level::Component);
    }

    #[test]
    fn c4_level_parse_unknown() {
        let result = C4Level::parse("unknown");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("unknown C4 level"));
    }

    // ------------------------------------------------------------------------
    // sanitize_id
    // ------------------------------------------------------------------------

    #[test]
    fn sanitize_id_colons() {
        assert_eq!(sanitize_id("symbol:foo:bar"), "symbol_foo_bar");
    }

    #[test]
    fn sanitize_id_slashes() {
        assert_eq!(sanitize_id("path/to/file"), "path_to_file");
    }

    #[test]
    fn sanitize_id_parens() {
        assert_eq!(sanitize_id("fn (arg)"), "fn_arg");
    }

    #[test]
    fn sanitize_id_dots() {
        assert_eq!(sanitize_id("crate.module"), "crate_module");
    }

    #[test]
    fn sanitize_id_alphanumeric_unchanged() {
        assert_eq!(sanitize_id("valid_id_123"), "valid_id_123");
    }

    #[test]
    fn sanitize_id_leading_trailing_underscores_trimmed() {
        assert_eq!(sanitize_id("_foo_"), "foo");
    }

    // ------------------------------------------------------------------------
    // deduplicate_ids
    // ------------------------------------------------------------------------

    #[test]
    fn deduplicate_ids_no_dupes() {
        let ids = ["a", "b", "c"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn deduplicate_ids_one_dup() {
        let ids = ["a", "b", "a"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "a_2"]);
    }

    #[test]
    fn deduplicate_ids_multiple_dups() {
        let ids = ["a", "b", "a", "a", "c", "b"];
        let result = deduplicate_ids(&ids);
        assert_eq!(result, vec!["a", "b", "a_2", "a_3", "c", "b_2"]);
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — empty
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_empty_nodes_context() {
        let nodes: Vec<GraphNode> = vec![];
        let edges: Vec<GraphEdge> = vec![];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Context);
        assert!(out.contains("C4Context"));
        assert!(out.contains("No nodes at this level"));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — context level
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_context_single_system() {
        let nodes = vec![make_node("system:myapp", "MyApp", "system", "node-system")];
        let edges: Vec<GraphEdge> = vec![];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Context);
        assert!(out.contains("C4Context"));
        assert!(out.contains("System(system_myapp"));
        assert!(out.contains("\"MyApp\""));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — container level
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_container_with_containers() {
        let nodes = vec![
            make_node("system:myapp", "MyApp", "system", "node-system"),
            make_node("container:web", "Web App", "container", "node-container"),
        ];
        let edges = vec![make_edge("system:myapp", "container:web", "part_of")];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Container);
        assert!(out.contains("C4Container"));
        assert!(out.contains("System(system_myapp"));
        assert!(out.contains("Container(container_web"));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — self-loop edges omitted
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_self_loop_omitted() {
        let nodes = vec![
            make_node("a", "A", "system", "node-system"),
            make_node("b", "B", "system", "node-system"),
        ];
        let edges = vec![
            make_edge("a", "a", "calls"), // self-loop
            make_edge("a", "b", "calls"), // valid
        ];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Context);
        // Should NOT contain a self-loop relation for a->a
        assert!(!out.contains("Rel(a, a"));
        assert!(out.contains("Rel"));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — deduplicated sanitised IDs
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_id_deduplication() {
        let nodes = vec![
            make_node("a:b", "First", "system", "node-system"),
            make_node("a:b", "Second", "system", "node-system"), // duplicate
        ];
        let edges: Vec<GraphEdge> = vec![];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Context);
        // First a:b -> a_b, Second a:b -> a_b_2
        assert!(out.contains("System(a_b"));
        assert!(out.contains("System(a_b_2"));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — part_of edge rendered as Rel_U
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_part_of_becomes_rel_u() {
        let nodes = vec![
            make_node("system:myapp", "MyApp", "system", "node-system"),
            make_node("container:web", "Web", "container", "node-container"),
        ];
        let edges = vec![make_edge("system:myapp", "container:web", "part_of")];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Container);
        assert!(out.contains("Rel_U(system_myapp, container_web"));
    }

    // ------------------------------------------------------------------------
    // c4_to_mermaid — depends_on edge rendered as Rel_D
    // ------------------------------------------------------------------------

    #[test]
    fn c4_to_mermaid_depends_on_becomes_rel_d() {
        let nodes = vec![
            make_node("container:api", "API", "container", "node-container"),
            make_node("container:db", "Database", "container", "node-container"),
        ];
        let edges = vec![make_edge("container:api", "container:db", "depends_on")];
        let out = c4_to_mermaid(&nodes, &edges, C4Level::Container);
        assert!(out.contains("Rel_D(container_api, container_db"));
    }
}
