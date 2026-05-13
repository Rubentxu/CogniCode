//! State Machine diagram renderer
//!
//! Renders state machine diagrams to Mermaid stateDiagram-v2 and PlantUML formats.

use crate::model::state_machine_types::{
    StateMachineModel, StateType, TransitionKind,
};

/// Options for state machine rendering
#[derive(Debug, Clone)]
pub struct StateMachineRenderOptions {
    /// Include entry/exit actions
    pub show_actions: bool,
    /// Include guards
    pub show_guards: bool,
    /// Title for the diagram
    pub title: String,
    /// Direction: TB (top-bottom), LR (left-right)
    pub direction: String,
}

impl Default for StateMachineRenderOptions {
    fn default() -> Self {
        Self {
            show_actions: true,
            show_guards: true,
            title: String::new(),
            direction: "LR".to_string(),
        }
    }
}

/// Render a state machine diagram as Mermaid stateDiagram-v2
pub fn render_state_machine_mermaid(
    model: &StateMachineModel,
    options: &StateMachineRenderOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push("stateDiagram-v2".to_string());

    if !options.title.is_empty() {
        lines.push(format!("    title: {}", escape_mermaid(&options.title)));
    }

    if options.direction != "LR" {
        lines.push(format!("    direction {}", options.direction));
    }

    // Render states
    for state in &model.states {
        let state_line = render_mermaid_state(state, options);
        lines.push(state_line);
    }

    // Render transitions
    for transition in &model.transitions {
        let transition_line = render_mermaid_transition(transition, options);
        lines.push(transition_line);
    }

    lines.join("\n")
}

/// Render a single state for Mermaid
fn render_mermaid_state(state: &crate::model::state_machine_types::State, options: &StateMachineRenderOptions) -> String {
    match state.state_type {
        StateType::Initial => {
            format!("    [*] --> {}", escape_mermaid(&state.name))
        }
        StateType::Final => {
            format!("    {} --> [*]", escape_mermaid(&state.name))
        }
        StateType::Choice => {
            format!("    {} : {}", escape_mermaid(&state.name), escape_mermaid("choice"))
        }
        StateType::Composite => {
            let mut lines = Vec::new();
            lines.push(format!("    state {} {{", escape_mermaid(&state.name)));
            if options.show_actions {
                if let Some(ref entry) = state.entry_action {
                    lines.push(format!("        entry / {}", escape_mermaid(entry)));
                }
                if let Some(ref exit) = state.exit_action {
                    lines.push(format!("        exit / {}", escape_mermaid(exit)));
                }
            }
            for child in &state.child_states {
                lines.push(format!("        {}", escape_mermaid(child)));
            }
            lines.push("    }".to_string());
            lines.join("\n")
        }
        StateType::History => {
            format!("    {} : H", escape_mermaid(&state.name))
        }
        StateType::DeepHistory => {
            format!("    {} : H*", escape_mermaid(&state.name))
        }
        _ => {
            if options.show_actions
                && (state.entry_action.is_some() || state.exit_action.is_some())
            {
                let actions = format_actions(state, options);
                format!("    {} {}", escape_mermaid(&state.name), actions)
            } else {
                format!("    {}", escape_mermaid(&state.name))
            }
        }
    }
}

/// Format entry/exit actions
fn format_actions(
    state: &crate::model::state_machine_types::State,
    options: &StateMachineRenderOptions,
) -> String {
    let mut parts = Vec::new();
    if options.show_actions {
        if let Some(ref entry) = state.entry_action {
            parts.push(format!("entry / {}", escape_mermaid(entry)));
        }
        if let Some(ref exit) = state.exit_action {
            parts.push(format!("exit / {}", escape_mermaid(exit)));
        }
    }
    if !parts.is_empty() {
        format!(": {}", parts.join(", "))
    } else {
        String::new()
    }
}

/// Render a transition for Mermaid
fn render_mermaid_transition(
    transition: &crate::model::state_machine_types::Transition,
    options: &StateMachineRenderOptions,
) -> String {
    let from = escape_mermaid(&transition.from);
    let to = escape_mermaid(&transition.to);

    // Build the transition label
    let mut label_parts = Vec::new();

    if let Some(ref event) = transition.event {
        label_parts.push(escape_mermaid(event));
    }

    if options.show_guards {
        if let Some(ref guard) = transition.guard {
            label_parts.push(escape_mermaid(guard));
        }
    }

    if options.show_actions {
        if let Some(ref action) = transition.action {
            label_parts.push(format!("/ {}", escape_mermaid(action)));
        }
    }

    if label_parts.is_empty() {
        format!("    {} --> {}", from, to)
    } else {
        let label = label_parts.join(" / ");
        format!("    {} --> {} : {}", from, to, label)
    }
}

/// Escape text for Mermaid
fn escape_mermaid(text: &str) -> String {
    text.replace('"', "'")
        .replace('<', "(")
        .replace('>', ")")
        .replace('{', "(")
        .replace('}', ")")
}

/// Render a state machine diagram as PlantUML
pub fn render_state_machine_plantuml(
    model: &StateMachineModel,
    options: &StateMachineRenderOptions,
) -> String {
    let mut lines = Vec::new();

    lines.push("@startuml".to_string());

    if !options.title.is_empty() {
        lines.push(format!("title {}", escape_plantuml(&options.title)));
    }

    // Define states
    for state in &model.states {
        let state_line = render_plantuml_state(state, options);
        lines.push(state_line);
    }

    // Define transitions
    for transition in &model.transitions {
        let transition_line = render_plantuml_transition(transition, options);
        lines.push(transition_line);
    }

    lines.push("@enduml".to_string());
    lines.join("\n")
}

/// Render a single state for PlantUML
fn render_plantuml_state(
    state: &crate::model::state_machine_types::State,
    _options: &StateMachineRenderOptions,
) -> String {
    match state.state_type {
        StateType::Initial => {
            "[*] --> ".to_string() + &escape_plantuml(&state.name)
        }
        StateType::Final => {
            escape_plantuml(&state.name) + " --> [*]"
        }
        StateType::Choice => {
            format!("state {} <<choice>>", escape_plantuml(&state.name))
        }
        StateType::Fork => {
            format!("state {} <<fork>>", escape_plantuml(&state.name))
        }
        StateType::Join => {
            format!("state {} <<join>>", escape_plantuml(&state.name))
        }
        StateType::History => {
            format!("state {} <<history>>", escape_plantuml(&state.name))
        }
        StateType::DeepHistory => {
            format!("state {} <<deepHistory>>", escape_plantuml(&state.name))
        }
        StateType::Composite => {
            let mut lines = Vec::new();
            lines.push(format!("state {} {{", escape_plantuml(&state.name)));
            for child in &state.child_states {
                lines.push(format!("  {}", escape_plantuml(child)));
            }
            lines.push("}".to_string());
            lines.join("\n")
        }
        _ => {
            if state.entry_action.is_some() || state.exit_action.is_some() {
                let mut note = String::new();
                if let Some(ref entry) = state.entry_action {
                    note += &format!("entry / {}\n", escape_plantuml(entry));
                }
                if let Some(ref exit) = state.exit_action {
                    note += &format!("exit / {}", escape_plantuml(exit));
                }
                format!("state {} <<entry>> {}", escape_plantuml(&state.name), note.trim())
            } else {
                format!("state {}", escape_plantuml(&state.name))
            }
        }
    }
}

/// Render a transition for PlantUML
fn render_plantuml_transition(
    transition: &crate::model::state_machine_types::Transition,
    options: &StateMachineRenderOptions,
) -> String {
    let from = escape_plantuml(&transition.from);
    let to = escape_plantuml(&transition.to);

    let transition_str = match transition.kind {
        TransitionKind::Local => "-->",
        TransitionKind::Internal => "-[#blue]->",
        TransitionKind::External => "-->",
    };

    if let Some(ref event) = transition.event {
        let mut label = event.clone();
        if options.show_guards {
            if let Some(ref guard) = transition.guard {
                label = format!("{} [{}]", label, guard);
            }
        }
        if options.show_actions {
            if let Some(ref action) = transition.action {
                label = format!("{} / {}", label, action);
            }
        }
        format!("{} {} {} : {}", from, transition_str, to, escape_plantuml(&label))
    } else {
        if options.show_actions {
            if let Some(ref action) = transition.action {
                return format!("{} {} {} / {}", from, transition_str, to, escape_plantuml(action));
            }
        }
        format!("{} {} {}", from, transition_str, to)
    }
}

/// Escape text for PlantUML
fn escape_plantuml(text: &str) -> String {
    text.replace('"', "'")
}

/// Render an empty state machine diagram
pub fn render_empty_state_machine(title: &str, format: &str) -> String {
    match format {
        "plantuml" => {
            format!(
                "@startuml\ntitle {}\n' No state machine detected\n@enduml",
                title
            )
        }
        _ => {
            // Mermaid default
            format!("stateDiagram-v2\n    title {}\n    [*] --> Empty\n    Empty --> [*]", title)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::state_machine_types::{State, StateType, Transition};

    #[test]
    fn test_render_empty_state_machine_mermaid() {
        let result = render_empty_state_machine("Test", "mermaid");
        assert!(result.contains("stateDiagram-v2"));
        assert!(result.contains("Empty"));
    }

    #[test]
    fn test_render_empty_state_machine_plantuml() {
        let result = render_empty_state_machine("Test", "plantuml");
        assert!(result.contains("@startuml"));
        assert!(result.contains("@enduml"));
    }

    #[test]
    fn test_escape_mermaid() {
        assert_eq!(escape_mermaid("hello \"world\""), "hello 'world'");
        assert_eq!(escape_mermaid("a < b > c"), "a ( b ) c");
    }

    #[test]
    fn test_render_mermaid_state_initial() {
        let state = State {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Initial,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        };
        let result = render_mermaid_state(&state, &StateMachineRenderOptions::default());
        assert_eq!(result, "    [*] --> Start");
    }

    #[test]
    fn test_render_mermaid_state_final() {
        let state = State {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::Final,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        };
        let result = render_mermaid_state(&state, &StateMachineRenderOptions::default());
        assert_eq!(result, "    End --> [*]");
    }

    #[test]
    fn test_render_mermaid_state_with_actions() {
        let state = State {
            id: "running".to_string(),
            name: "Running".to_string(),
            state_type: StateType::Regular,
            entry_action: Some("onEnter()".to_string()),
            exit_action: Some("onExit()".to_string()),
            child_states: Vec::new(),
        };
        let options = StateMachineRenderOptions::default();
        let result = render_mermaid_state(&state, &options);
        assert!(result.contains("Running"));
        assert!(result.contains("entry"));
        assert!(result.contains("exit"));
    }

    #[test]
    fn test_render_mermaid_transition_simple() {
        let transition = Transition {
            id: "t1".to_string(),
            from: "idle".to_string(),
            to: "running".to_string(),
            event: Some("start".to_string()),
            guard: None,
            action: None,
            kind: TransitionKind::External,
        };
        let options = StateMachineRenderOptions::default();
        let result = render_mermaid_transition(&transition, &options);
        assert!(result.contains("idle --> running"));
        assert!(result.contains("start"));
    }

    #[test]
    fn test_render_mermaid_transition_with_guard() {
        let transition = Transition {
            id: "t1".to_string(),
            from: "idle".to_string(),
            to: "running".to_string(),
            event: Some("start".to_string()),
            guard: Some("[isReady]".to_string()),
            action: None,
            kind: TransitionKind::External,
        };
        let options = StateMachineRenderOptions::default();
        let result = render_mermaid_transition(&transition, &options);
        assert!(result.contains("[isReady]"));
    }

    #[test]
    fn test_render_full_state_machine() {
        let mut model = StateMachineModel::new("TestSM", "main");

        model.add_state(State {
            id: "idle".to_string(),
            name: "Idle".to_string(),
            state_type: StateType::Initial,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });

        model.add_state(State {
            id: "running".to_string(),
            name: "Running".to_string(),
            state_type: StateType::Regular,
            entry_action: Some("startWork()".to_string()),
            exit_action: None,
            child_states: Vec::new(),
        });

        model.add_state(State {
            id: "done".to_string(),
            name: "Done".to_string(),
            state_type: StateType::Final,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });

        model.add_transition(Transition {
            id: "t1".to_string(),
            from: "idle".to_string(),
            to: "running".to_string(),
            event: Some("start".to_string()),
            guard: None,
            action: None,
            kind: TransitionKind::External,
        });

        model.add_transition(Transition {
            id: "t2".to_string(),
            from: "running".to_string(),
            to: "done".to_string(),
            event: Some("complete".to_string()),
            guard: None,
            action: None,
            kind: TransitionKind::External,
        });

        let options = StateMachineRenderOptions::default();
        let result = render_state_machine_mermaid(&model, &options);

        assert!(result.contains("stateDiagram-v2"));
        assert!(result.contains("[*] --> Idle"));
        assert!(result.contains("Running"));
        assert!(result.contains("Done --> [*]"));
        assert!(result.contains("idle --> running : start"));
        assert!(result.contains("running --> done : complete"));
    }
}
