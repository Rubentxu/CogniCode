//! State Machine diagram inference from code analysis
//!
//! Detects state machines from enums with state patterns, transition functions,
//! and state field patterns.



use cognicode_core::domain::aggregates::call_graph::{CallGraph, SymbolId};
use cognicode_core::domain::value_objects::SymbolKind;

use crate::model::state_machine_types::{
    State, StateMachineModel, StateType, Transition, TransitionKind,
};

/// Options for state machine inference
#[derive(Debug, Clone)]
pub struct StateMachineInferenceOptions {
    /// Minimum number of states to consider it a state machine
    pub min_states: usize,
    /// Include entry/exit actions
    pub include_actions: bool,
    /// Include guards
    pub include_guards: bool,
    /// Title for the diagram
    pub title: String,
}

impl Default for StateMachineInferenceOptions {
    fn default() -> Self {
        Self {
            min_states: 2,
            include_actions: true,
            include_guards: true,
            title: String::new(),
        }
    }
}

/// Detect state machines from an enum symbol
pub fn infer_state_machine_from_enum(
    call_graph: &CallGraph,
    enum_name: &str,
    options: &StateMachineInferenceOptions,
) -> Option<StateMachineModel> {
    // Find the enum symbol
    let enum_sym_id = find_symbol_by_name(call_graph, enum_name)?;

    // Check if it's actually an enum
    let enum_symbol = call_graph.get_symbol(&enum_sym_id)?;
    if *enum_symbol.kind() != SymbolKind::Enum {
        return None;
    }

    let mut model = StateMachineModel::new(&options.title, enum_name);

    // The enum variants are the states
    let variants = enum_symbol.name(); // Simplified - would need variant extraction

    // For now, create a basic state machine from the enum
    // In a real implementation, we'd parse the enum variants

    // Look for transition functions that reference this enum
    let transitions = find_transition_functions(call_graph, &enum_sym_id);

    // Create states from enum variants (placeholder)
    // The actual variant extraction would need AST access

    // Add found transitions and ensure states exist for endpoints
    for t in transitions {
        // Create states for transition endpoints
        if !model.has_state(&t.from) && t.from != "unknown" {
            model.add_state(State {
                id: t.from.clone(),
                name: format_state_name(&t.from),
                state_type: infer_state_type(&t.from),
                entry_action: None,
                exit_action: None,
                child_states: Vec::new(),
            });
        }
        if !model.has_state(&t.to) && t.to != "unknown" {
            model.add_state(State {
                id: t.to.clone(),
                name: format_state_name(&t.to),
                state_type: infer_state_type(&t.to),
                entry_action: None,
                exit_action: None,
                child_states: Vec::new(),
            });
        }
        model.add_transition(t);
    }

    if model.states.len() < options.min_states {
        return None;
    }

    model.finalize();
    Some(model)
}

/// Infer state machines from a struct with a state field
pub fn infer_state_machine_from_struct(
    call_graph: &CallGraph,
    struct_name: &str,
    options: &StateMachineInferenceOptions,
) -> Option<StateMachineModel> {
    let struct_sym_id = find_symbol_by_name(call_graph, struct_name)?;
    let struct_symbol = call_graph.get_symbol(&struct_sym_id)?;

    if *struct_symbol.kind() != SymbolKind::Struct {
        return None;
    }

    let mut model = StateMachineModel::new(&options.title, struct_name);

    // Find state field and its type
    // Look for functions that seem like state transitions
    let transitions = find_transition_functions(call_graph, &struct_sym_id);

    for t in transitions {
        // Ensure states exist for transition endpoints
        if !model.has_state(&t.from) {
            model.add_state(State {
                id: t.from.clone(),
                name: format_state_name(&t.from),
                state_type: infer_state_type(&t.from),
                entry_action: None,
                exit_action: None,
                child_states: Vec::new(),
            });
        }
        if !model.has_state(&t.to) {
            model.add_state(State {
                id: t.to.clone(),
                name: format_state_name(&t.to),
                state_type: infer_state_type(&t.to),
                entry_action: None,
                exit_action: None,
                child_states: Vec::new(),
            });
        }
        model.add_transition(t);
    }

    if model.states.len() < options.min_states {
        return None;
    }

    model.finalize();
    Some(model)
}

/// Find all potential state machines in a call graph
pub fn find_state_machines(
    call_graph: &CallGraph,
    options: &StateMachineInferenceOptions,
) -> Vec<StateMachineModel> {
    let mut machines = Vec::new();

    // Look for enums that might be state machines
    for (sym_id, symbol) in call_graph.symbol_ids() {
        if *symbol.kind() == SymbolKind::Enum {
            let name = symbol.name();
            // State machine enums often have specific naming patterns
            if is_state_enum_name(name) {
                if let Some(mut sm) = infer_state_machine_from_enum(call_graph, name, options) {
                    sm.name = name.to_string();
                    machines.push(sm);
                }
            }
        }
    }

    // Look for structs with state patterns
    for (sym_id, symbol) in call_graph.symbol_ids() {
        if *symbol.kind() == SymbolKind::Struct {
            let name = symbol.name();
            if is_state_struct_name(name) {
                if let Some(mut sm) = infer_state_machine_from_struct(call_graph, name, options) {
                    sm.name = name.to_string();
                    machines.push(sm);
                }
            }
        }
    }

    machines
}

/// Find symbol by name
fn find_symbol_by_name<'a>(call_graph: &'a CallGraph, name: &str) -> Option<SymbolId> {
    // Try exact match
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

/// Check if an enum name suggests it's a state machine
fn is_state_enum_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("state")
        || name_lower.contains("status")
        || name_lower.contains("mode")
        || name_lower.contains("phase")
}

/// Check if a struct name suggests it holds state
fn is_state_struct_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("machine")
        || name_lower.contains("controller")
        || name_lower.contains("handler")
}

/// Find functions that look like state transitions
fn find_transition_functions(
    call_graph: &CallGraph,
    state_holder_id: &SymbolId,
) -> Vec<Transition> {
    let mut transitions = Vec::new();
    let mut transition_id = 0usize;

    // Look for functions that call this state holder
    // and seem to be transition functions
    for (caller_id, symbol) in call_graph.symbol_ids() {
        let name = symbol.name().to_lowercase();

        // Look for transition-like function names
        if is_transition_function_name(&name) {
            let caller_str = caller_id.as_str().to_string();

            // Try to determine from/to states
            let (from_state, to_state) = parse_transition_function_name(&name);

            if from_state.is_some() || to_state.is_some() {
                transitions.push(Transition {
                    id: format!("t{}", transition_id),
                    from: from_state.unwrap_or_else(|| "unknown".to_string()),
                    to: to_state.unwrap_or_else(|| "unknown".to_string()),
                    event: Some(name.clone()),
                    guard: None,
                    action: None,
                    kind: TransitionKind::External,
                });
                transition_id += 1;
            }
        }
    }

    // Also look at the edges in the call graph for patterns
    for (source, target, _dep_type) in call_graph.all_dependencies() {
        let source_name = call_graph
            .get_symbol(&source)
            .map(|s| s.name().to_lowercase())
            .unwrap_or_default();
        let target_name = call_graph
            .get_symbol(&target)
            .map(|s| s.name().to_lowercase())
            .unwrap_or_default();

        // If source and target seem to be state-related
        if is_state_related_name(&source_name) && is_state_related_name(&target_name) {
            // This might be a state transition
            if !transitions.iter().any(|t| t.from == source.as_str() && t.to == target.as_str()) {
                transitions.push(Transition {
                    id: format!("t{}", transition_id),
                    from: source.as_str().to_string(),
                    to: target.as_str().to_string(),
                    event: None,
                    guard: None,
                    action: None,
                    kind: TransitionKind::External,
                });
                transition_id += 1;
            }
        }
    }

    transitions
}

/// Check if a function name suggests it's a transition
fn is_transition_function_name(name: &str) -> bool {
    name.starts_with("transition_to")
        || name.starts_with("set_state")
        || name.starts_with("change_state")
        || name.starts_with("move_to")
        || name.starts_with("go_to")
        || name.starts_with("enter_")
        || name.starts_with("exit_")
        || name.contains("_to_")
}

/// Parse state names from a transition function name
fn parse_transition_function_name(name: &str) -> (Option<String>, Option<String>) {
    // transition_to_running -> from=current, to=running
    if name.starts_with("transition_to_") {
        return (None, Some(name.strip_prefix("transition_to_").unwrap().to_string()));
    }
    if name.starts_with("set_state_") {
        return (None, Some(name.strip_prefix("set_state_").unwrap().to_string()));
    }
    if name.starts_with("change_state_") {
        return (None, Some(name.strip_prefix("change_state_").unwrap().to_string()));
    }
    if name.starts_with("move_to_") {
        return (None, Some(name.strip_prefix("move_to_").unwrap().to_string()));
    }
    if name.starts_with("go_to_") {
        return (None, Some(name.strip_prefix("go_to_").unwrap().to_string()));
    }

    // foo_to_bar -> from=foo, to=bar
    if let Some(pos) = name.find("_to_") {
        let from = name[..pos].to_string();
        let to = name[pos + 4..].to_string();
        return (Some(from), Some(to));
    }

    (None, None)
}

/// Check if a name is state-related
fn is_state_related_name(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    name_lower.contains("state")
        || name_lower.contains("status")
        || name_lower.contains("mode")
        || name_lower.contains("idle")
        || name_lower.contains("running")
        || name_lower.contains("waiting")
        || name_lower.contains("active")
        || name_lower.contains("inactive")
        || name_lower.contains("error")
        || name_lower.contains("pending")
        || name_lower.contains("ready")
}

/// Format a state name for display
fn format_state_name(name: &str) -> String {
    // Convert snake_case to Title Case
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Infer state type from name
fn infer_state_type(name: &str) -> StateType {
    let name_lower = name.to_lowercase();
    if name_lower.contains("initial")
        || name_lower.contains("start")
        || name_lower == "s0"
        || name_lower == "s1"
    {
        StateType::Initial
    } else if name_lower.contains("final")
        || name_lower.contains("end")
        || name_lower.contains("done")
        || name_lower == "sf"
    {
        StateType::Final
    } else if name_lower.contains("choice")
        || name_lower.contains("decision")
        || name_lower.contains("?")
    {
        StateType::Choice
    } else {
        StateType::Regular
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_state_enum_name() {
        assert!(is_state_enum_name("ConnectionState"));
        assert!(is_state_enum_name("Status"));
        assert!(is_state_enum_name("Mode"));
        assert!(!is_state_enum_name("User"));
        assert!(!is_state_enum_name("Config"));
    }

    #[test]
    fn test_is_transition_function_name() {
        assert!(is_transition_function_name("transition_to_running"));
        assert!(is_transition_function_name("set_state_idle"));
        assert!(is_transition_function_name("foo_to_bar"));
        assert!(!is_transition_function_name("calculate"));
        assert!(!is_transition_function_name("get_state"));
    }

    #[test]
    fn test_parse_transition_function_name() {
        assert_eq!(
            parse_transition_function_name("transition_to_running"),
            (None, Some("running".to_string()))
        );
        assert_eq!(
            parse_transition_function_name("foo_to_bar"),
            (Some("foo".to_string()), Some("bar".to_string()))
        );
        assert_eq!(parse_transition_function_name("calculate"), (None, None));
    }

    #[test]
    fn test_format_state_name() {
        assert_eq!(format_state_name("idle"), "Idle");
        assert_eq!(format_state_name("connection_established"), "Connection Established");
        assert_eq!(format_state_name("error"), "Error");
    }

    #[test]
    fn test_infer_state_type() {
        assert_eq!(infer_state_type("Initial"), StateType::Initial);
        assert_eq!(infer_state_type("start"), StateType::Initial);
        assert_eq!(infer_state_type("s0"), StateType::Initial);
        assert_eq!(infer_state_type("Final"), StateType::Final);
        assert_eq!(infer_state_type("end"), StateType::Final);
        assert_eq!(infer_state_type("choice_state"), StateType::Choice);
        assert_eq!(infer_state_type("running"), StateType::Regular);
    }
}
