//! Edge-level diffing for incremental graph updates (ADR-022, Sprint 4).
//!
//! Extends `GraphDiffCalculator` with edge-level diffing so that incremental
//! scans can emit `DependencyAdded`/`DependencyRemoved` events (not just
//! symbol-level events).

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::events::graph_event::{DependencyEvent, GraphEvent};
use crate::domain::value_objects::DependencyType;

/// Calculate the diff between old and new edges for a set of changed files.
/// Produces `DependencyAdded` and `DependencyRemoved` events.
///
/// `old_edges`: edges from the previous scan for the changed files.
/// `new_edges`: edges from the current scan for the changed files.
pub fn calculate_edge_diff(
    old_edges: &[(String, String, DependencyType)], // (source_name, target_name, dep_type)
    new_edges: &[(String, String, DependencyType, String)], // + file
) -> Vec<GraphEvent> {
    let old_set: std::collections::HashSet<(String, String, DependencyType)> =
        old_edges.iter().cloned().collect();
    let new_set: std::collections::HashSet<(String, String, DependencyType)> = new_edges
        .iter()
        .map(|(s, t, d, _)| (s.clone(), t.clone(), *d))
        .collect();

    let mut events = Vec::new();

    // Removed: in old but not in new
    for (source, target, dep_type) in old_edges {
        if !new_set.contains(&(source.clone(), target.clone(), *dep_type)) {
            events.push(GraphEvent::DependencyRemoved(DependencyEvent {
                file: String::new(),
                source_name: source.clone(),
                target_name: target.clone(),
                dependency_type: *dep_type,
            }));
        }
    }

    // Added: in new but not in old
    for (source, target, dep_type, file) in new_edges {
        if !old_set.contains(&(source.clone(), target.clone(), *dep_type)) {
            events.push(GraphEvent::DependencyAdded(DependencyEvent {
                file: file.clone(),
                source_name: source.clone(),
                target_name: target.clone(),
                dependency_type: *dep_type,
            }));
        }
    }

    events
}

/// Collect edges from a CallGraph that belong to a list of changed file paths.
/// Returns flat `(source_file:name, target_file:name, dep_type)` tuples.
pub fn collect_edges_for_files(
    graph: &CallGraph,
    changed_files: &[String],
) -> Vec<(String, String, DependencyType, String)> {
    let mut edges = Vec::new();
    for (src_id, tgt_id, dep_type) in graph.all_dependencies() {
        let src_name = src_id.as_str().to_string();
        let tgt_name = tgt_id.as_str().to_string();

        // Determine which file this edge belongs to (source file)
        if let Some(sym) = graph.get_symbol(src_id) {
            let file = sym.location().file().to_string();
            if changed_files.iter().any(|f| file.contains(f.as_str())) {
                edges.push((src_name, tgt_name, *dep_type, file));
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

    #[test]
    fn test_calculate_edge_diff_added() {
        let old: Vec<(String, String, DependencyType)> = vec![];
        let new = vec![(
            "main".to_string(),
            "helper".to_string(),
            DependencyType::Calls,
            "main.rs".to_string(),
        )];
        let events = calculate_edge_diff(&old, &new);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], GraphEvent::DependencyAdded(_)));
    }

    #[test]
    fn test_calculate_edge_diff_removed() {
        let old = vec![(
            "main".to_string(),
            "helper".to_string(),
            DependencyType::Calls,
        )];
        let new: Vec<(String, String, DependencyType, String)> = vec![];
        let events = calculate_edge_diff(&old, &new);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], GraphEvent::DependencyRemoved(_)));
    }

    #[test]
    fn test_calculate_edge_diff_unchanged() {
        let old = vec![(
            "main".to_string(),
            "helper".to_string(),
            DependencyType::Calls,
        )];
        let new = vec![(
            "main".to_string(),
            "helper".to_string(),
            DependencyType::Calls,
            "main.rs".to_string(),
        )];
        let events = calculate_edge_diff(&old, &new);
        assert_eq!(events.len(), 0);
    }

    #[test]
    fn test_collect_edges_for_files() {
        let mut graph = CallGraph::new();
        let sym_main = Symbol::new(
            "main",
            SymbolKind::Function,
            Location::new("src/main.rs", 1, 1),
        );
        let sym_helper = Symbol::new(
            "helper",
            SymbolKind::Function,
            Location::new("src/main.rs", 5, 1),
        );
        graph.add_symbol(sym_main);
        graph.add_symbol(sym_helper);
        // Add dependency
        let sid_main = SymbolId::new("src/main.rs:main:1");
        let sid_helper = SymbolId::new("src/main.rs:helper:5");
        graph
            .add_dependency(&sid_main, &sid_helper, DependencyType::Calls)
            .unwrap();

        let edges = collect_edges_for_files(&graph, &["src/main.rs".to_string()]);
        assert_eq!(edges.len(), 1);
    }
}
