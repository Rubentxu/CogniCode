//! State Machine diagram model types
//!
//! Represents state machines as an intermediate format agnostic to output.

use serde::{Deserialize, Serialize};

/// A state machine diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineModel {
    /// Name of the state machine
    pub name: String,
    /// Entry point symbol that defines this state machine
    pub entry_point: String,
    /// All states in the machine
    pub states: Vec<State>,
    /// All transitions between states
    pub transitions: Vec<Transition>,
    /// Metadata
    pub metadata: StateMachineMetadata,
}

/// Metadata about the state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachineMetadata {
    /// Number of states
    pub state_count: usize,
    /// Number of transitions
    pub transition_count: usize,
    /// Whether it has entry/exit actions
    pub has_actions: bool,
    /// Whether it has guards
    pub has_guards: bool,
    /// Whether it has choice/decision states
    pub has_choice_states: bool,
}

/// A state in the state machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// State type
    pub state_type: StateType,
    /// Entry action (if any)
    pub entry_action: Option<String>,
    /// Exit action (if any)
    pub exit_action: Option<String>,
    /// Child states (for composite states)
    pub child_states: Vec<String>,
}

/// Type of state
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StateType {
    /// Initial state (filled circle)
    Initial,
    /// Final state (double circle)
    Final,
    /// Regular state (rounded rectangle)
    Regular,
    /// Choice/decision state (diamond)
    Choice,
    /// Fork state (inverted fork)
    Fork,
    /// Join state (fork)
    Join,
    /// Composite state (contains child states)
    Composite,
    /// History state (H)
    History,
    /// Deep history state (H*)
    DeepHistory,
}

impl Default for StateType {
    fn default() -> Self {
        StateType::Regular
    }
}

impl StateType {
    /// Infer state type from name
    pub fn from_name(name: &str) -> Self {
        let name_lower = name.to_lowercase();
        if name_lower == "initial" || name_lower == "start" {
            StateType::Initial
        } else if name_lower == "final" || name_lower == "end" || name_lower == "done" {
            StateType::Final
        } else if name_lower.contains("choice") || name_lower.contains("decision") {
            StateType::Choice
        } else if name_lower.contains("fork") {
            StateType::Fork
        } else if name_lower.contains("join") {
            StateType::Join
        } else if name_lower == "h*" || name_lower.contains("deep") || name_lower.contains("history") && name_lower.contains("*") || name_lower.contains("deep") {
            StateType::DeepHistory
        } else if name_lower == "h" || name_lower.contains("history") {
            StateType::History
        } else {
            StateType::Regular
        }
    }
}

/// A transition between states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// Unique identifier
    pub id: String,
    /// Source state ID
    pub from: String,
    /// Target state ID
    pub to: String,
    /// Event/trigger that fires this transition
    pub event: Option<String>,
    /// Guard condition (if any)
    pub guard: Option<String>,
    /// Action to execute on transition
    pub action: Option<String>,
    /// Transition kind
    pub kind: TransitionKind,
}

/// Kind of transition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransitionKind {
    /// External transition (default)
    External,
    /// Internal transition (does not exit/source state)
    Internal,
    /// Local transition (exits source, enters target)
    Local,
}

impl Default for TransitionKind {
    fn default() -> Self {
        TransitionKind::External
    }
}

impl StateMachineModel {
    /// Create a new empty state machine
    pub fn new(name: impl Into<String>, entry_point: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            entry_point: entry_point.into(),
            states: Vec::new(),
            transitions: Vec::new(),
            metadata: StateMachineMetadata {
                state_count: 0,
                transition_count: 0,
                has_actions: false,
                has_guards: false,
                has_choice_states: false,
            },
        }
    }

    /// Add a state
    pub fn add_state(&mut self, state: State) {
        if !self.states.iter().any(|s| s.id == state.id) {
            self.states.push(state);
        }
    }

    /// Add a transition
    pub fn add_transition(&mut self, transition: Transition) {
        self.transitions.push(transition);
    }

    /// Get state by ID
    pub fn get_state(&self, id: &str) -> Option<&State> {
        self.states.iter().find(|s| s.id == id)
    }

    /// Finalize the model (compute metadata)
    pub fn finalize(&mut self) {
        let has_actions = self
            .states
            .iter()
            .any(|s| s.entry_action.is_some() || s.exit_action.is_some());
        let has_guards = self.transitions.iter().any(|t| t.guard.is_some());
        let has_choice_states = self
            .states
            .iter()
            .any(|s| s.state_type == StateType::Choice);

        self.metadata = StateMachineMetadata {
            state_count: self.states.len(),
            transition_count: self.transitions.len(),
            has_actions,
            has_guards,
            has_choice_states,
        };
    }

    /// Check if a state exists
    pub fn has_state(&self, id: &str) -> bool {
        self.states.iter().any(|s| s.id == id)
    }

    /// Get initial states (usually just one)
    pub fn initial_states(&self) -> Vec<&State> {
        self.states
            .iter()
            .filter(|s| s.state_type == StateType::Initial)
            .collect()
    }

    /// Get final states
    pub fn final_states(&self) -> Vec<&State> {
        self.states
            .iter()
            .filter(|s| s.state_type == StateType::Final)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_machine_new() {
        let sm = StateMachineModel::new("TestSM", "main");
        assert_eq!(sm.name, "TestSM");
        assert_eq!(sm.entry_point, "main");
        assert!(sm.states.is_empty());
        assert!(sm.transitions.is_empty());
    }

    #[test]
    fn test_add_state() {
        let mut sm = StateMachineModel::new("Test", "main");
        sm.add_state(State {
            id: "idle".to_string(),
            name: "Idle".to_string(),
            state_type: StateType::Initial,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });
        assert_eq!(sm.states.len(), 1);
    }

    #[test]
    fn test_add_duplicate_state() {
        let mut sm = StateMachineModel::new("Test", "main");
        sm.add_state(State {
            id: "idle".to_string(),
            name: "Idle".to_string(),
            state_type: StateType::Initial,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });
        sm.add_state(State {
            id: "idle".to_string(),
            name: "Idle2".to_string(),
            state_type: StateType::Final,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });
        assert_eq!(sm.states.len(), 1);
    }

    #[test]
    fn test_finalize() {
        let mut sm = StateMachineModel::new("Test", "main");
        sm.add_state(State {
            id: "idle".to_string(),
            name: "Idle".to_string(),
            state_type: StateType::Initial,
            entry_action: Some("on_enter".to_string()),
            exit_action: None,
            child_states: Vec::new(),
        });
        sm.add_transition(Transition {
            id: "t1".to_string(),
            from: "idle".to_string(),
            to: "running".to_string(),
            event: Some("start".to_string()),
            guard: Some("[isReady]".to_string()),
            action: None,
            kind: TransitionKind::External,
        });
        sm.finalize();

        assert_eq!(sm.metadata.state_count, 1);
        assert_eq!(sm.metadata.transition_count, 1);
        assert!(sm.metadata.has_actions);
        assert!(sm.metadata.has_guards);
    }

    #[test]
    fn test_state_type_inference() {
        assert_eq!(StateType::from_name("Initial"), StateType::Initial);
        assert_eq!(StateType::from_name("start"), StateType::Initial);
        assert_eq!(StateType::from_name("Final"), StateType::Final);
        assert_eq!(StateType::from_name("end"), StateType::Final);
        assert_eq!(StateType::from_name("choice_state"), StateType::Choice);
        assert_eq!(StateType::from_name("fork_node"), StateType::Fork);
        assert_eq!(StateType::from_name("join_node"), StateType::Join);
        assert_eq!(StateType::from_name("H"), StateType::History);
        assert_eq!(StateType::from_name("H*"), StateType::DeepHistory);
        assert_eq!(StateType::from_name("running"), StateType::Regular);
    }

    #[test]
    fn test_initial_and_final_states() {
        let mut sm = StateMachineModel::new("Test", "main");
        sm.add_state(State {
            id: "start".to_string(),
            name: "Start".to_string(),
            state_type: StateType::Initial,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });
        sm.add_state(State {
            id: "end".to_string(),
            name: "End".to_string(),
            state_type: StateType::Final,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });
        sm.add_state(State {
            id: "running".to_string(),
            name: "Running".to_string(),
            state_type: StateType::Regular,
            entry_action: None,
            exit_action: None,
            child_states: Vec::new(),
        });

        assert_eq!(sm.initial_states().len(), 1);
        assert_eq!(sm.final_states().len(), 1);
    }
}
