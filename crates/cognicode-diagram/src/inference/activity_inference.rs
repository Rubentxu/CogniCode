//! Activity diagram inference from code analysis
//!
//! Infers activity diagrams from function control flow analysis.

use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};

use crate::model::activity_types::{
    ActivityEdge, ActivityModel, ActivityNode, ActivityNodeType,
};

/// Options for activity diagram inference
#[derive(Debug, Clone)]
pub struct ActivityInferenceOptions {
    /// Minimum number of actions to consider it an activity
    pub min_actions: usize,
    /// Include loop detection
    pub include_loops: bool,
    /// Title for the diagram
    pub title: String,
}

impl Default for ActivityInferenceOptions {
    fn default() -> Self {
        Self {
            min_actions: 2,
            include_loops: true,
            title: String::new(),
        }
    }
}

/// Find symbol by name in call graph
fn find_symbol_by_name<'a>(call_graph: &'a CallGraph, name: &str) -> Option<SymbolId> {
    // Try exact match first
    for (id, _) in call_graph.symbol_ids() {
        if id.as_str() == name {
            return Some(SymbolId::new(id.as_str()));
        }
    }

    // Try partial match
    for (id, symbol) in call_graph.symbol_ids() {
        let sym_name = symbol.name();
        if sym_name == name || sym_name.ends_with(name) {
            return Some(SymbolId::new(id.as_str()));
        }
    }

    None
}

/// Infer an activity diagram from a function symbol
pub fn infer_activity_from_function(
    call_graph: &CallGraph,
    function_name: &str,
    options: &ActivityInferenceOptions,
) -> Option<ActivityModel> {
    let func_sym_id = find_symbol_by_name(call_graph, function_name)?;

    let func_symbol = call_graph.get_symbol(&func_sym_id)?;
    let kind = func_symbol.kind();

    // Only work with functions/methods
    let is_function = matches!(
        kind,
        cognicode_core::domain::value_objects::SymbolKind::Function
            | cognicode_core::domain::value_objects::SymbolKind::Method
    );

    if !is_function {
        return None;
    }

    let mut model = ActivityModel::new(&options.title, function_name);

    // Add initial node
    model.add_node(ActivityNode {
        id: "start".to_string(),
        name: format!("Start {}", func_symbol.name()),
        node_type: ActivityNodeType::Initial,
        guard: None,
        loop_variable: None,
        location: None,
    });

    // Build activity from function name patterns and call graph
    let mut node_counter = 0usize;

    // Analyze function name for control flow patterns
    let func_name_lower = func_symbol.name().to_lowercase();

    if func_name_lower.contains("if")
        || func_name_lower.contains("check")
        || func_name_lower.contains("validate")
        || func_name_lower.contains("verify")
    {
        // Add decision node
        node_counter += 1;
        let node_id = format!("decision_{}", node_counter);
        model.add_node(ActivityNode {
            id: node_id.clone(),
            name: format!("Check {}", extract_base_name(func_symbol.name())),
            node_type: ActivityNodeType::Decision,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_edge(ActivityEdge {
            id: format!("e{}_to_decision", node_counter),
            from: "start".to_string(),
            to: node_id.clone(),
            label: None,
            guard: None,
        });

        // Add "then" branch (action)
        node_counter += 1;
        let then_id = format!("action_{}", node_counter);
        model.add_node(ActivityNode {
            id: then_id.clone(),
            name: "Process".to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_edge(ActivityEdge {
            id: format!("e{}_{}_true", node_counter, node_id),
            from: node_id,
            to: then_id.clone(),
            label: None,
            guard: Some("[condition]".to_string()),
        });
    } else {
        // Add initial action
        node_counter += 1;
        let action_id = format!("action_{}", node_counter);
        model.add_node(ActivityNode {
            id: action_id.clone(),
            name: func_symbol.name().to_string(),
            node_type: ActivityNodeType::Action,
            guard: None,
            loop_variable: None,
            location: None,
        });
        model.add_edge(ActivityEdge {
            id: format!("e{}_start_action", node_counter),
            from: "start".to_string(),
            to: action_id.clone(),
            label: None,
            guard: None,
        });
    }

    // Look for loop patterns in callees
    if options.include_loops {
        let loop_patterns = find_loop_patterns(call_graph, &func_sym_id);
        for loop_info in loop_patterns {
            node_counter += 1;
            let loop_id = format!("loop_{}", node_counter);
            model.add_node(ActivityNode {
                id: loop_id.clone(),
                name: loop_info.name,
                node_type: ActivityNodeType::Loop,
                guard: None,
                loop_variable: loop_info.variable,
                location: None,
            });
            // Connect from previous node or start
            let prev_id = if node_counter == 1 { "start" } else { "action_1" };
            model.add_edge(ActivityEdge {
                id: format!("e{}_to_loop", node_counter),
                from: prev_id.to_string(),
                to: loop_id.clone(),
                label: None,
                guard: None,
            });
        }
    }

    // Look for async/fork patterns in callees
    let parallel_patterns = find_parallel_patterns(call_graph, &func_sym_id);
    if !parallel_patterns.is_empty() {
        // Add fork node
        node_counter += 1;
        let fork_id = format!("fork_{}", node_counter);
        model.add_node(ActivityNode {
            id: fork_id.clone(),
            name: "Parallel Tasks".to_string(),
            node_type: ActivityNodeType::Fork,
            guard: None,
            loop_variable: None,
            location: None,
        });

        // Add join node
        node_counter += 1;
        let join_id = format!("join_{}", node_counter);
        model.add_node(ActivityNode {
            id: join_id.clone(),
            name: "Sync".to_string(),
            node_type: ActivityNodeType::Join,
            guard: None,
            loop_variable: None,
            location: None,
        });

        // Connect fork to join
        model.add_edge(ActivityEdge {
            id: format!("e{}_fork_join", node_counter),
            from: fork_id,
            to: join_id,
            label: None,
            guard: None,
        });
    }

    // Add final node
    node_counter += 1;
    let end_id = "end".to_string();
    model.add_node(ActivityNode {
        id: end_id.clone(),
        name: format!("End {}", func_symbol.name()),
        node_type: ActivityNodeType::Final,
        guard: None,
        loop_variable: None,
        location: None,
    });

    // Connect last action to end
    model.add_edge(ActivityEdge {
        id: format!("e{}_to_end", node_counter),
        from: if node_counter > 1 { format!("action_{}", node_counter - 1) } else { "start".to_string() },
        to: end_id,
        label: None,
        guard: None,
    });

    if model.nodes.len() < options.min_actions + 2 {
        // Need at least start, one action, and end
        return None;
    }

    model.finalize();
    Some(model)
}

/// Find all potential activities in a call graph
pub fn find_activities(
    call_graph: &CallGraph,
    options: &ActivityInferenceOptions,
) -> Vec<ActivityModel> {
    let mut activities = Vec::new();

    for (sym_id, symbol) in call_graph.symbol_ids() {
        let kind = symbol.kind();
        let is_function = matches!(
            kind,
            cognicode_core::domain::value_objects::SymbolKind::Function
                | cognicode_core::domain::value_objects::SymbolKind::Method
        );

        if is_function && is_activity_function_name(symbol.name()) {
            let name = symbol.name().to_string();
            if let Some(mut activity) = infer_activity_from_function(call_graph, &name, options) {
                activity.title = name.clone();
                activities.push(activity);
            }
        }
    }

    activities
}

/// Check if a function name suggests an activity/workflow
fn is_activity_function_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("process")
        || name_lower.contains("workflow")
        || name_lower.contains("handle")
        || name_lower.contains("execute")
        || name_lower.contains("run")
        || name_lower.contains("perform")
        || name_lower.contains("do_")
        || name_lower.contains("if_")
        || name_lower.contains("check_")
        || name_lower.contains("validate_")
}

/// Extract base name from function (remove prefix like "do_", "process_")
fn extract_base_name(name: &str) -> String {
    let name_lower = name.to_lowercase();
    if let Some(pos) = name_lower.find("do_") {
        name[pos + 3..].to_string()
    } else if let Some(pos) = name_lower.find("process_") {
        name[pos + 8..].to_string()
    } else if let Some(pos) = name_lower.find("handle_") {
        name[pos + 7..].to_string()
    } else {
        name.to_string()
    }
}

/// Loop info from inference
struct LoopInfo {
    name: String,
    variable: Option<String>,
}

/// Find loop patterns in a function's callees
fn find_loop_patterns(call_graph: &CallGraph, func_id: &SymbolId) -> Vec<LoopInfo> {
    let mut loops = Vec::new();

    for (caller_id, symbol) in call_graph.symbol_ids() {
        let name = symbol.name().to_lowercase();

        if name.contains("for_")
            || name.contains("foreach_")
            || name.contains("while_")
            || name.contains("loop_")
            || name.contains("iterate_")
        {
            loops.push(LoopInfo {
                name: symbol.name().to_string(),
                variable: Some(extract_loop_variable(&name)),
            });
        }
    }

    loops
}

/// Extract loop variable from name
fn extract_loop_variable(name: &str) -> String {
    let name_lower = name.to_lowercase();
    if let Some(pos) = name_lower.find("foreach_") {
        let rest = &name[pos + 8..];
        if let Some(end) = rest.find('_') {
            return rest[..end].to_string();
        }
        return rest.to_string();
    }
    if let Some(pos) = name_lower.find("for_") {
        let rest = &name[pos + 4..];
        if let Some(end) = rest.find('_') {
            return rest[..end].to_string();
        }
        return rest.to_string();
    }
    "item".to_string()
}

/// Parallel task info
struct ParallelInfo {
    name: String,
}

/// Find parallel execution patterns
fn find_parallel_patterns(call_graph: &CallGraph, func_id: &SymbolId) -> Vec<ParallelInfo> {
    let mut parallel = Vec::new();

    // Look for functions that suggest parallel execution
    for (_, symbol) in call_graph.symbol_ids() {
        let name = symbol.name().to_lowercase();
        if name.contains("join_")
            || name.contains("wait_")
            || name.contains("sync_")
            || name.contains("collect_")
        {
            parallel.push(ParallelInfo {
                name: symbol.name().to_string(),
            });
        }
    }

    parallel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_activity_function_name() {
        assert!(is_activity_function_name("process_order"));
        assert!(is_activity_function_name("do_cleanup"));
        assert!(is_activity_function_name("handle_request"));
        assert!(!is_activity_function_name("get_user"));
        assert!(!is_activity_function_name("calculate"));
    }

    #[test]
    fn test_extract_base_name() {
        assert_eq!(extract_base_name("do_cleanup"), "cleanup");
        assert_eq!(extract_base_name("process_order"), "order");
        assert_eq!(extract_base_name("handle_request"), "request");
    }

    #[test]
    fn test_extract_loop_variable() {
        assert_eq!(extract_loop_variable("for_items"), "items");
        assert_eq!(extract_loop_variable("foreach_users"), "users");
    }
}
