//! Sequence diagram inference from CallGraph
//!
//! Takes a CallGraph and produces a SequenceModel by traversing call chains
//! from an entry point symbol.

use std::collections::{HashMap, HashSet, VecDeque};

use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::dependency_type::DependencyType;

use crate::model::sequence_types::{
    MessageType, ParticipantType, SequenceMessage, SequenceModel, SequenceParticipant,
};

/// Options for sequence diagram inference
#[derive(Debug, Clone)]
pub struct SequenceInferenceOptions {
    /// Maximum call depth to traverse (default: 5)
    pub max_depth: usize,
    /// Include loop markers when BFS revisits nodes (default: true)
    pub show_loops: bool,
    /// Show method names on edges (default: true)
    pub show_method_names: bool,
    /// Title for the diagram
    pub title: String,
    /// Minimum confidence threshold (0.0 - 1.0)
    pub min_confidence: f64,
}

impl Default for SequenceInferenceOptions {
    fn default() -> Self {
        Self {
            max_depth: 5,
            show_loops: true,
            show_method_names: true,
            title: String::new(),
            min_confidence: 0.5,
        }
    }
}

/// Infer a sequence diagram from a CallGraph starting from an entry point
pub fn infer_sequence(
    call_graph: &CallGraph,
    entry_point: &str,
    options: &SequenceInferenceOptions,
) -> SequenceModel {
    let mut model = SequenceModel::new(&options.title, entry_point);

    // Find the actual entry point symbol
    let start_symbol = find_symbol_by_name(call_graph, entry_point)
        .or_else(|| find_entry_points(call_graph).first().cloned())
        .unwrap_or_default();

    if start_symbol.is_empty() {
        return model;
    }

    // BFS traversal to collect call edges and participants
    let (messages, participants) = bfs_traverse(call_graph, &start_symbol, options);

    // Add participants and messages to model
    for participant in participants.into_values() {
        model.add_participant(participant);
    }

    for message in messages {
        model.add_message(message);
    }

    // Finalize to compute metadata
    model.finalize();

    model
}

/// Find potential entry points in the call graph
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
        file.split('/')
            .last()
            .map(|s| s.trim_end_matches(".rs").to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// BFS traversal of the call graph from an entry point
fn bfs_traverse(
    call_graph: &CallGraph,
    start_symbol: &str,
    options: &SequenceInferenceOptions,
) -> (Vec<SequenceMessage>, HashMap<String, SequenceParticipant>) {
    let mut messages = Vec::new();
    let mut participants: HashMap<String, SequenceParticipant> = HashMap::new();
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize, Vec<String>)> = VecDeque::new();
    let mut message_id = 0usize;

    // Start BFS from the entry point
    queue.push_back((start_symbol.to_string(), 0, Vec::new()));

    while let Some((current, depth, path)) = queue.pop_front() {
        // Check depth limit
        if depth >= options.max_depth {
            continue;
        }

        // Mark as visited for loop detection
        visited.insert(current.clone());

        // Get symbol info for participant
        let sym_id = SymbolId::new(&current);
        if let Some(symbol) = call_graph.get_symbol(&sym_id) {
            let module = extract_module_name(symbol);
            let name = symbol.name().to_string();
            let location = Some(symbol.location().file().to_string());

            // Add participant if not seen
            participants.entry(current.clone()).or_insert_with(|| {
                SequenceParticipant {
                    id: current.clone(),
                    name: name.clone(),
                    module: module.clone(),
                    location,
                    participant_type: ParticipantType::from_name_and_module(&name, &module),
                }
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
                    let location = Some(dep_symbol.location().file().to_string());

                    participants
                        .entry(dep_id_str.clone())
                        .or_insert_with(|| SequenceParticipant {
                            id: dep_id_str.clone(),
                            name: name.clone(),
                            module: module.clone(),
                            location,
                            participant_type: ParticipantType::from_name_and_module(&name, &module),
                        });

                    // Determine method name
                    let method_name = if options.show_method_names {
                        dep_symbol.name().to_string()
                    } else {
                        "call".to_string()
                    };

                    // Check if this is a loop (revisiting in current path)
                    let is_loop_edge = path.contains(&dep_id_str);
                    let is_self_call = current == dep_id_str;

                    // Create message
                    let message_type = MessageType::infer(&method_name, is_self_call, is_loop_edge);

                    message_id += 1;
                    messages.push(SequenceMessage {
                        id: format!("msg_{}", message_id),
                        from: current.clone(),
                        to: dep_id_str.clone(),
                        method_name,
                        message_type,
                        is_loop: options.show_loops && is_loop_edge,
                        loop_label: if is_loop_edge {
                            Some("loop".to_string())
                        } else {
                            None
                        },
                        is_self_call,
                        seq: depth + 1,
                        confidence: 1.0,
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

    (messages, participants)
}

/// Infer a sequence diagram from a CallGraph with default options
pub fn infer_sequence_default(call_graph: &CallGraph, entry_point: &str) -> SequenceModel {
    infer_sequence(call_graph, entry_point, &SequenceInferenceOptions::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::call_graph::CallGraph;

    #[test]
    fn test_infer_empty_sequence() {
        let call_graph = CallGraph::new();
        let options = SequenceInferenceOptions::default();
        let model = infer_sequence(&call_graph, "", &options);

        assert!(model.participants.is_empty());
        assert!(model.messages.is_empty());
    }

    #[test]
    fn test_infer_sequence_with_options() {
        let call_graph = CallGraph::new();
        let options = SequenceInferenceOptions {
            max_depth: 3,
            show_loops: true,
            show_method_names: true,
            title: "Test Diagram".to_string(),
            min_confidence: 0.5,
        };
        let model = infer_sequence(&call_graph, "main", &options);

        assert_eq!(model.title, "Test Diagram");
        assert_eq!(model.entry_point, "main");
    }

    #[test]
    fn test_find_entry_points_empty_graph() {
        let call_graph = CallGraph::new();
        let entry_points = find_entry_points(&call_graph);
        assert!(entry_points.is_empty());
    }

    #[test]
    fn test_sequence_metadata() {
        let mut model = SequenceModel::new("Test", "main");
        model.add_participant(SequenceParticipant {
            id: "a".to_string(),
            name: "A".to_string(),
            module: "mod".to_string(),
            location: None,
            participant_type: ParticipantType::System,
        });
        model.add_message(SequenceMessage {
            id: "1".to_string(),
            from: "a".to_string(),
            to: "a".to_string(),
            method_name: "call".to_string(),
            message_type: MessageType::SelfCall,
            is_loop: true,
            loop_label: Some("loop".to_string()),
            is_self_call: true,
            seq: 1,
            confidence: 1.0,
        });
        model.finalize();

        assert_eq!(model.metadata.participant_count, 1);
        assert_eq!(model.metadata.message_count, 1);
        assert_eq!(model.metadata.loop_count, 1);
    }
}
