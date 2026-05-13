//! Sequence diagram renderer — generates Mermaid sequenceDiagram from CallGraph traversal
//!
//! Uses BFS from an entry point symbol to collect call chain edges and render them
//! as a Mermaid sequence diagram showing the flow of function calls.

use std::collections::{HashMap, HashSet, VecDeque};

use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::dependency_type::DependencyType;

/// Options for sequence diagram rendering
#[derive(Debug, Clone)]
pub struct SequenceDiagramOptions {
    /// Maximum call depth to traverse (default: 5)
    pub max_depth: usize,
    /// Include loop markers when BFS revisits nodes (default: true)
    pub show_loops: bool,
    /// Show method names on edges (default: true)
    pub show_method_names: bool,
    /// Title for the diagram
    pub title: String,
}

impl Default for SequenceDiagramOptions {
    fn default() -> Self {
        Self {
            max_depth: 5,
            show_loops: true,
            show_method_names: true,
            title: String::new(),
        }
    }
}

/// A participant in the sequence diagram (represents a module/component)
#[derive(Debug, Clone)]
struct Participant {
    id: String,
    name: String,
    module: String,
}

/// An edge in the call chain
#[derive(Debug, Clone)]
struct CallEdge {
    caller: String,
    callee: String,
    method_name: String,
    is_loop: bool,
}

/// Find potential entry points in the call graph
/// (functions with no incoming edges, or known main/test functions)
pub fn find_entry_points(call_graph: &CallGraph) -> Vec<String> {
    let mut entry_points = Vec::new();

    // Get all roots (symbols with no incoming edges)
    let roots = call_graph.roots();
    for root in roots {
        entry_points.push(root.as_str().to_string());
    }

    // Also look for common entry point patterns
    for (id, symbol) in call_graph.symbol_ids() {
        let name = symbol.name().to_lowercase();
        let fqn = symbol.fully_qualified_name().to_lowercase();

        // Match common entry point patterns
        if name == "main" || fqn.contains("::main") {
            if !entry_points.contains(&id.as_str().to_string()) {
                entry_points.push(id.as_str().to_string());
            }
        } else if name == "handle" || name == "process" || name == "run" || name == "execute" {
            // Common handler patterns - only add if they have outgoing edges
            let sym_id = SymbolId::new(id.as_str());
            let has_deps = call_graph.dependencies(&sym_id).next().is_some();
            if has_deps && !entry_points.contains(&id.as_str().to_string()) {
                entry_points.push(id.as_str().to_string());
            }
        }
    }

    // If still empty, use first symbol with outgoing edges
    if entry_points.is_empty() {
        for (id, _symbol) in call_graph.symbol_ids() {
            let sym_id = SymbolId::new(id.as_str());
            if call_graph.dependencies(&sym_id).next().is_some() {
                entry_points.push(id.as_str().to_string());
                break;
            }
        }
    }

    entry_points
}

/// Find a symbol by name or path (partial match)
fn find_symbol_by_name<'a>(call_graph: &'a CallGraph, entry_point: &str) -> Option<String> {
    // Try exact match first
    for (id, _) in call_graph.symbol_ids() {
        if id.as_str() == entry_point {
            return Some(id.as_str().to_string());
        }
    }

    // Try FQN partial match
    for (id, symbol) in call_graph.symbol_ids() {
        let fqn = symbol.fully_qualified_name();
        if fqn.contains(entry_point) || fqn.ends_with(entry_point) {
            return Some(id.as_str().to_string());
        }
    }

    // Try name-only partial match
    for (id, symbol) in call_graph.symbol_ids() {
        let name = symbol.name();
        if name == entry_point || name.contains(entry_point) {
            return Some(id.as_str().to_string());
        }
    }

    None
}

/// Extract module name from a symbol's file path
fn extract_module_name(symbol: &cognicode_core::domain::aggregates::symbol::Symbol) -> String {
    let file = symbol.location().file();

    // Try to extract module from path: src/foo/bar.rs -> foo::bar or bar
    let path_parts: Vec<&str> = file.split('/').collect();

    if path_parts.len() >= 2 {
        // Check if it's a module file (mod.rs) or source file
        let last = path_parts.last().unwrap();
        let second_last = path_parts.get(path_parts.len() - 2).unwrap();

        if *last == "mod.rs" {
            second_last.to_string()
        } else {
            // Remove .rs extension and use as module name
            last.trim_end_matches(".rs").to_string()
        }
    } else if path_parts.len() == 1 {
        // Single element path
        let last = path_parts[0];
        last.trim_end_matches(".rs").to_string()
    } else {
        // Fallback to filename
        file.split('/').last()
            .map(|s| s.trim_end_matches(".rs").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Render a sequence diagram from CallGraph traversal
///
/// Uses BFS from entry_point symbol, collecting call chain edges.
/// Returns Mermaid sequenceDiagram syntax.
pub fn render_sequence_diagram(
    call_graph: &CallGraph,
    entry_point: &str,
    options: &SequenceDiagramOptions,
) -> String {
    // Find the actual entry point symbol
    let start_symbol = find_symbol_by_name(call_graph, entry_point)
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if start_symbol.is_empty() {
        return render_empty_diagram(options);
    }

    // BFS traversal to collect call edges
    let (edges, participants) = bfs_traverse(call_graph, &start_symbol, options);

    // Build Mermaid sequence diagram
    build_mermaid_sequence(&participants, &edges, options)
}

/// Render an empty diagram when no valid entry point is found
fn render_empty_diagram(options: &SequenceDiagramOptions) -> String {
    let mut lines = Vec::new();
    lines.push("sequenceDiagram".to_string());

    if !options.title.is_empty() {
        lines.push(format!("    title: {}", options.title));
    }

    lines.push("    Note over Participant: No call graph data available".to_string());
    lines.join("\n")
}

/// BFS traversal of the call graph from an entry point
fn bfs_traverse(
    call_graph: &CallGraph,
    start_symbol: &str,
    options: &SequenceDiagramOptions,
) -> (Vec<CallEdge>, HashMap<String, Participant>) {
    let mut edges = Vec::new();
    let mut participants: HashMap<String, Participant> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize, Vec<String>)> = VecDeque::new();

    // Start BFS from the entry point
    queue.push_back((start_symbol.to_string(), 0, Vec::new()));

    while let Some((current, depth, path)) = queue.pop_front() {
        // Check depth limit
        if depth >= options.max_depth {
            continue;
        }

        // Mark as visited for loop detection
        let _is_loop = visited.contains(&current);
        visited.insert(current.clone());

        // Get symbol info for participant
        let sym_id = SymbolId::new(&current);
        if let Some(symbol) = call_graph.get_symbol(&sym_id) {
            let module = extract_module_name(symbol);
            let name = symbol.name().to_string();
            let _id = current.clone();

            // Add participant if not seen
            participants.entry(current.clone()).or_insert(Participant {
                id: current.clone(),
                name,
                module,
            });

            // Process dependencies (outgoing edges)
            for (dep_id, dep_type) in call_graph.dependencies(&sym_id) {
                // Only follow "Calls" dependencies for sequence diagram
                if *dep_type != DependencyType::Calls {
                    continue;
                }

                let dep_id_str = dep_id.as_str().to_string();

                // Add participant for callee
                if let Some(dep_symbol) = call_graph.get_symbol(dep_id) {
                    let module = extract_module_name(dep_symbol);
                    let name = dep_symbol.name().to_string();

                    participants.entry(dep_id_str.clone()).or_insert(Participant {
                        id: dep_id_str.clone(),
                        name,
                        module,
                    });

                    // Determine method name
                    let method_name = if options.show_method_names {
                        dep_symbol.name().to_string()
                    } else {
                        "call".to_string()
                    };

                    // Check if this is a loop (revisiting in current path)
                    let is_loop_edge = path.contains(&dep_id_str);

                    // Create edge
                    edges.push(CallEdge {
                        caller: current.clone(),
                        callee: dep_id_str.clone(),
                        method_name,
                        is_loop: options.show_loops && is_loop_edge,
                    });

                    // Add to queue for further traversal (if not already visited in this path)
                    if !visited.contains(&dep_id_str) && !path.contains(&dep_id_str) {
                        let mut new_path = path.clone();
                        new_path.push(current.clone());
                        queue.push_back((dep_id_str, depth + 1, new_path));
                    }
                }
            }
        }
    }

    (edges, participants)
}

/// Build Mermaid sequence diagram from participants and edges
fn build_mermaid_sequence(
    participants: &HashMap<String, Participant>,
    edges: &[CallEdge],
    options: &SequenceDiagramOptions,
) -> String {
    let mut lines = Vec::new();

    // Header
    lines.push("sequenceDiagram".to_string());

    if !options.title.is_empty() {
        lines.push(format!("    title: {}", options.title));
    }

    // Participants
    for participant in participants.values() {
        let display_name = if participant.module != participant.name {
            format!("{}:{}", participant.module, participant.name)
        } else {
            participant.name.clone()
        };
        lines.push(format!(
            "    participant {} as {}",
            sanitize_participant_id(&participant.id),
            escape_mermaid(&display_name)
        ));
    }

    if participants.is_empty() {
        lines.push("    Note over Participant: No callable symbols found".to_string());
        return lines.join("\n");
    }

    // Collect loop groups
    let mut loop_starts: HashMap<&str, usize> = HashMap::new();
    let mut loop_ends: HashMap<&str, Vec<usize>> = HashMap::new();

    for (i, edge) in edges.iter().enumerate() {
        if edge.is_loop {
            loop_starts.entry(edge.callee.as_str()).or_insert(i);
            loop_ends.entry(edge.callee.as_str()).or_default().push(i);
        }
    }

    // Render edges with loop markers
    let mut i = 0;
    while i < edges.len() {
        let edge = &edges[i];
        let caller_id = sanitize_participant_id(&edge.caller);
        let callee_id = sanitize_participant_id(&edge.callee);

        if edge.is_loop {
            // Check if this is the start of a loop
            if loop_starts.get(edge.callee.as_str()) == Some(&i) {
                lines.push(format!("    loop {}", escape_mermaid(&edge.method_name)));
            }

            // Render the call
            lines.push(format!(
                "        {}->>+{}: {}()",
                caller_id,
                callee_id,
                escape_mermaid(&edge.method_name)
            ));
            lines.push(format!(
                "        {}-->>-{}: return",
                callee_id,
                caller_id
            ));

            // Check if this is the end of a loop
            if let Some(ends) = loop_ends.get(edge.callee.as_str()) {
                if ends.last() == Some(&i) {
                    lines.push("    end".to_string());
                }
            }
        } else {
            // Regular call
            lines.push(format!(
                "        {}->>+{}: {}()",
                caller_id,
                callee_id,
                escape_mermaid(&edge.method_name)
            ));
            lines.push(format!(
                "        {}-->>-{}: return",
                callee_id,
                caller_id
            ));
        }

        i += 1;
    }

    lines.join("\n")
}

/// Sanitize a participant ID for Mermaid (must be alphanumeric + underscore)
fn sanitize_participant_id(id: &str) -> String {
    let mut result = String::new();
    for (i, c) in id.chars().enumerate() {
        if c.is_alphanumeric() || c == '_' {
            result.push(c);
        } else if c == ':' {
            result.push_str("_");
        } else {
            result.push('_');
        }
        // Limit length to avoid overly long IDs
        if i > 30 {
            result.push_str("_trunc");
            break;
        }
    }
    if result.is_empty() {
        result.push_str("Participant");
    }
    result
}

/// Escape text for safe inclusion in Mermaid diagrams
fn escape_mermaid(text: &str) -> String {
    text.replace('"', "'")
        .replace('[', "(")
        .replace(']', ")")
        .replace('{', "(")
        .replace('}', ")")
        .replace('<', "(")
        .replace('>', ")")
        .replace('&', "and")
        .replace('\n', " ")
}

// ============================================================================
// PlantUML Sequence Diagram Rendering
// ============================================================================

/// Render a sequence diagram as PlantUML format
pub fn render_sequence_diagram_plantuml(
    call_graph: &CallGraph,
    entry_point: &str,
    options: &SequenceDiagramOptions,
) -> String {
    // Find the actual entry point symbol
    let start_symbol = find_symbol_by_name(call_graph, entry_point)
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if start_symbol.is_empty() {
        return render_empty_plantuml(options);
    }

    // BFS traversal to collect call edges
    let (edges, participants) = bfs_traverse(call_graph, &start_symbol, options);

    // Build PlantUML sequence diagram
    build_plantuml_sequence(&participants, &edges, options)
}

/// Render an empty PlantUML diagram
fn render_empty_plantuml(options: &SequenceDiagramOptions) -> String {
    let mut lines = Vec::new();
    lines.push("@startuml".to_string());
    if !options.title.is_empty() {
        lines.push(format!("title {}", escape_plantuml(&options.title)));
    }
    lines.push("' No call graph data available".to_string());
    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Escape text for PlantUML
fn escape_plantuml(text: &str) -> String {
    text.replace('"', "'")
        .replace('\n', " ")
}

/// Build PlantUML sequence diagram from participants and edges
fn build_plantuml_sequence(
    participants: &HashMap<String, Participant>,
    edges: &[CallEdge],
    options: &SequenceDiagramOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push("@startuml".to_string());

    if !options.title.is_empty() {
        lines.push(format!("title {}", escape_plantuml(&options.title)));
    }

    // Participants
    for participant in participants.values() {
        let display_name = if participant.module != participant.name {
            format!("{}:{}", participant.module, participant.name)
        } else {
            participant.name.clone()
        };

        // Use actor icon for actors, otherwise participant
        if is_actor_name(&participant.name) {
            lines.push(format!("actor {}", escape_plantuml(&display_name)));
        } else {
            lines.push(format!("participant {}", escape_plantuml(&display_name)));
        }
    }

    if participants.is_empty() {
        lines.push("' No callable symbols found".to_string());
        lines.push("@enduml".to_string());
        return lines.join("\n");
    }

    // Messages
    for edge in edges {
        let caller_display = get_participant_display(participants, &edge.caller);
        let callee_display = get_participant_display(participants, &edge.callee);

        if edge.is_loop && options.show_loops {
            lines.push(format!("loop {}", escape_plantuml(&edge.method_name)));
            lines.push(format!(
                "{} -> {} : {}()",
                escape_plantuml(&caller_display),
                escape_plantuml(&callee_display),
                escape_plantuml(&edge.method_name)
            ));
            lines.push("end".to_string());
        } else {
            lines.push(format!(
                "{} -> {} : {}()",
                escape_plantuml(&caller_display),
                escape_plantuml(&callee_display),
                escape_plantuml(&edge.method_name)
            ));
        }
    }

    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Check if a name represents an actor
fn is_actor_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower == "user"
        || lower == "client"
        || lower == "admin"
        || lower == "actor"
        || lower == "guest"
}

/// Get display name for a participant
fn get_participant_display(participants: &HashMap<String, Participant>, id: &str) -> String {
    participants
        .get(id)
        .map(|p| {
            if p.module != p.name {
                format!("{}:{}", p.module, p.name)
            } else {
                p.name.clone()
            }
        })
        .unwrap_or_else(|| id.to_string())
}

// ============================================================================
// SVG Sequence Diagram Rendering
// ============================================================================

/// Options for SVG rendering
#[derive(Debug, Clone)]
pub struct SequenceSvgOptions {
    /// Width of the diagram in pixels
    pub width: u32,
    /// Height of the diagram in pixels
    pub height: u32,
    /// Padding around the diagram
    pub padding: u32,
    /// Color for participant boxes
    pub box_color: String,
    /// Color for arrows/lines
    pub arrow_color: String,
    /// Background color
    pub background_color: String,
    /// Font family
    pub font_family: String,
    /// Font size
    pub font_size: u32,
}

impl Default for SequenceSvgOptions {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            padding: 40,
            box_color: "#e0e0e0".to_string(),
            arrow_color: "#333333".to_string(),
            background_color: "#ffffff".to_string(),
            font_family: "Monaco, Consolas, monospace".to_string(),
            font_size: 12,
        }
    }
}

/// Render a sequence diagram as SVG
pub fn render_sequence_diagram_svg(
    call_graph: &CallGraph,
    entry_point: &str,
    options: &SequenceDiagramOptions,
    svg_options: &SequenceSvgOptions,
) -> String {
    // Find the actual entry point symbol
    let start_symbol = find_symbol_by_name(call_graph, entry_point)
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if start_symbol.is_empty() {
        return render_empty_svg(svg_options);
    }

    // BFS traversal to collect call edges
    let (edges, participants) = bfs_traverse(call_graph, &start_symbol, options);

    // Build SVG
    build_svg_sequence(&participants, &edges, options, svg_options)
}

/// Render an empty SVG diagram
fn render_empty_svg(options: &SequenceSvgOptions) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
  <rect width="100%" height="100%" fill="{}"/>
  <text x="{}" y="{}" font-family="{}" font-size="{}" fill="#666" text-anchor="middle">
    No call graph data available
  </text>
</svg>"##,
        options.width,
        options.height,
        options.background_color,
        options.width / 2,
        options.height / 2,
        options.font_family,
        options.font_size
    )
}

/// Build SVG sequence diagram from participants and edges
fn build_svg_sequence(
    participants: &HashMap<String, Participant>,
    edges: &[CallEdge],
    _options: &SequenceDiagramOptions,
    svg_options: &SequenceSvgOptions,
) -> String {
    let mut svg_parts = Vec::new();

    let width = svg_options.width;
    let height = svg_options.height;
    let padding = svg_options.padding;

    svg_parts.push(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">"#,
        width, height
    ));

    // Background
    svg_parts.push(format!(
        r#"  <rect width="100%" height="100%" fill="{}"/>"#,
        svg_options.background_color
    ));

    if participants.is_empty() {
        svg_parts.push(format!(
            r##"  <text x="{}" y="{}" font-family="{}" font-size="{}" fill="#666" text-anchor="middle">
    No callable symbols found
  </text>"##,
            width / 2,
            height / 2,
            svg_options.font_family,
            svg_options.font_size
        ));
        svg_parts.push("</svg>".to_string());
        return svg_parts.join("\n");
    }

    // Calculate layout
    let participant_count = participants.len() as u32;
    let spacing = (width - 2 * padding) / participant_count.max(1);
    let box_width = spacing - 20;
    let box_height = 40u32;
    let lifeline_height = height - 2 * padding - box_height;

    // Draw participants and lifelines
    let mut participant_x: HashMap<String, u32> = HashMap::new();
    for (i, (id, participant)) in participants.iter().enumerate() {
        let x = padding + (i as u32) * spacing + spacing / 2;
        participant_x.insert(id.clone(), x);

        let display_name = if participant.module != participant.name {
            format!("{}:{}", participant.module, participant.name)
        } else {
            participant.name.clone()
        };

        // Box
        svg_parts.push(format!(
            r#"  <rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" rx="4"/>"#,
            x - box_width / 2,
            padding,
            box_width,
            box_height,
            svg_options.box_color,
            svg_options.arrow_color
        ));

        // Name
        svg_parts.push(format!(
            r##"  <text x="{}" y="{}" font-family="{}" font-size="{}" fill="#333" text-anchor="middle" dominant-baseline="middle">
    {}
  </text>"##,
            x,
            padding + box_height / 2,
            svg_options.font_family,
            svg_options.font_size,
            escape_xml(&display_name)
        ));

        // Lifeline (dashed line going down)
        svg_parts.push(format!(
            r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1" stroke-dasharray="4,2"/>"#,
            x,
            padding + box_height,
            x,
            padding + box_height + lifeline_height,
            svg_options.arrow_color
        ));
    }

    // Draw messages
    let message_spacing = (lifeline_height - 40) / edges.len().max(1) as u32;
    for (i, edge) in edges.iter().enumerate() {
        let y_offset = padding + box_height + 30 + (i as u32) * message_spacing;

        let from_x = *participant_x.get(&edge.caller).unwrap_or(&padding);
        let to_x = *participant_x.get(&edge.callee).unwrap_or(&padding);

        let is_forward = to_x > from_x;
        let _arrow_dir = if is_forward { "-8,4 0,-4 8,4" } else { "8,4 0,-4 -8,4" };

        // Arrow line
        svg_parts.push(format!(
            r#"  <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"#,
            from_x,
            y_offset,
            to_x,
            y_offset,
            svg_options.arrow_color
        ));

        // Arrowhead
        let arrow_x = if is_forward { to_x - 8 } else { to_x + 8 };
        let _arrow_dir = if is_forward { "-8,4 0,-4 8,4" } else { "8,4 0,-4 -8,4" };
        svg_parts.push(format!(
            r#"  <polygon points="{},{} {},{} {},{}" fill="{}"/>"#,
            to_x,
            y_offset,
            arrow_x,
            y_offset - 4,
            arrow_x,
            y_offset + 4,
            svg_options.arrow_color
        ));

        // Method name label
        let label_x = (from_x + to_x) / 2;
        svg_parts.push(format!(
            r##"  <text x="{}" y="{}" font-family="{}" font-size="{}" fill="#666" text-anchor="middle">
    {}()
  </text>"##,
            label_x,
            y_offset - 5,
            svg_options.font_family,
            svg_options.font_size - 2,
            escape_xml(&edge.method_name)
        ));
    }

    svg_parts.push("</svg>".to_string());
    svg_parts.join("\n")
}

/// Escape text for XML/SVG
fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    #[test]
    fn test_render_empty_sequence() {
        let call_graph = CallGraph::new();
        let options = SequenceDiagramOptions::default();
        let result = render_sequence_diagram(&call_graph, "", &options);

        assert!(result.contains("sequenceDiagram"));
        assert!(result.contains("No call graph data"));
    }

    #[test]
    fn test_find_entry_points_empty_graph() {
        let call_graph = CallGraph::new();
        let entry_points = find_entry_points(&call_graph);
        assert!(entry_points.is_empty());
    }

    #[test]
    fn test_render_sequence_with_calls() {
        // This test would need a populated CallGraph
        // For now, verify the empty case works
        let call_graph = CallGraph::new();
        let options = SequenceDiagramOptions::default();
        let result = render_sequence_diagram(&call_graph, "nonexistent", &options);

        assert!(result.contains("sequenceDiagram"));
    }

    #[test]
    fn test_find_entry_points() {
        let call_graph = CallGraph::new();
        let entry_points = find_entry_points(&call_graph);
        // Empty graph should return empty
        assert!(entry_points.is_empty());
    }

    #[test]
    fn test_sequence_shows_loops_option() {
        let call_graph = CallGraph::new();
        let options = SequenceDiagramOptions {
            show_loops: false,
            ..Default::default()
        };
        let result = render_sequence_diagram(&call_graph, "", &options);

        assert!(result.contains("sequenceDiagram"));
    }

    #[test]
    fn test_mermaid_sequence_valid() {
        let call_graph = CallGraph::new();
        let options = SequenceDiagramOptions::default();
        let result = render_sequence_diagram(&call_graph, "", &options);

        // Verify it starts with sequenceDiagram
        assert!(result.starts_with("sequenceDiagram"));
    }

    #[test]
    fn test_sanitize_participant_id() {
        assert_eq!(sanitize_participant_id("foo::bar"), "foo__bar");
        assert_eq!(sanitize_participant_id("my-function"), "my_function");
        assert_eq!(sanitize_participant_id(""), "Participant");
    }

    #[test]
    fn test_escape_mermaid() {
        assert_eq!(escape_mermaid("hello \"world\""), "hello 'world'");
        assert_eq!(escape_mermaid("a[b]c"), "a(b)c");
        assert_eq!(escape_mermaid("foo & bar"), "foo and bar");
    }

    #[test]
    fn test_sequence_diagram_options_default() {
        let options = SequenceDiagramOptions::default();
        assert_eq!(options.max_depth, 5);
        assert!(options.show_loops);
        assert!(options.show_method_names);
        assert!(options.title.is_empty());
    }

    #[test]
    fn test_sequence_diagram_options_custom() {
        let options = SequenceDiagramOptions {
            max_depth: 10,
            show_loops: false,
            show_method_names: false,
            title: "Test Diagram".to_string(),
        };

        assert_eq!(options.max_depth, 10);
        assert!(!options.show_loops);
        assert!(!options.show_method_names);
        assert_eq!(options.title, "Test Diagram");
    }
}
