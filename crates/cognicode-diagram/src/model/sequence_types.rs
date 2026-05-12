//! Sequence diagram model types
//!
//! Represents sequence diagrams as an intermediate format agnostic to output.
//! Used for inference (from CallGraph) and rendering (to Mermaid, PlantUML, SVG).

use serde::{Deserialize, Serialize};

/// A sequence diagram showing call flow between components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceModel {
    /// Diagram title
    pub title: String,
    /// Entry point symbol that started this sequence
    pub entry_point: String,
    /// All participants in the sequence
    pub participants: Vec<SequenceParticipant>,
    /// All messages/edges in the sequence
    pub messages: Vec<SequenceMessage>,
    /// Metadata about the sequence
    pub metadata: SequenceMetadata,
}

/// Metadata about the sequence diagram
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceMetadata {
    /// Maximum depth traversed
    pub max_depth: usize,
    /// Total number of unique participants
    pub participant_count: usize,
    /// Total number of messages
    pub message_count: usize,
    /// Number of async messages
    pub async_count: usize,
    /// Number of loop/self-calls detected
    pub loop_count: usize,
}

/// A participant (component/module) in the sequence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceParticipant {
    /// Unique identifier
    pub id: String,
    /// Display name
    pub name: String,
    /// Module/container name
    pub module: String,
    /// Source file path (if available)
    pub location: Option<String>,
    /// Participant type
    pub participant_type: ParticipantType,
}

/// Type of participant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParticipantType {
    /// External actor (person)
    Actor,
    /// Internal system/component
    System,
    /// Database or data store
    Database,
    /// External service
    ExternalService,
    /// Generic participant
    Generic,
}

impl Default for ParticipantType {
    fn default() -> Self {
        ParticipantType::Generic
    }
}

impl ParticipantType {
    /// Infer participant type from name/module
    pub fn from_name_and_module(name: &str, module: &str) -> Self {
        let name_lower = name.to_lowercase();
        let module_lower = module.to_lowercase();

        if name_lower == "user"
            || name_lower == "client"
            || name_lower == "admin"
            || name_lower == "actor"
        {
            return ParticipantType::Actor;
        }

        if module_lower.contains("db")
            || module_lower.contains("database")
            || module_lower.contains("sqlite")
            || module_lower.contains("postgres")
            || module_lower.contains("mysql")
            || name_lower.contains("repository")
            || name_lower.contains("store")
        {
            return ParticipantType::Database;
        }

        if module_lower.contains("service")
            || module_lower.contains("api")
            || module_lower.contains("http")
        {
            return ParticipantType::ExternalService;
        }

        if module_lower == "std" || module_lower == "core" || module_lower == "unknown" {
            return ParticipantType::ExternalService;
        }

        ParticipantType::System
    }
}

/// A message (call/return) between participants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceMessage {
    /// Unique message ID
    pub id: String,
    /// Caller participant ID
    pub from: String,
    /// Callee participant ID
    pub to: String,
    /// Method/function name being called
    pub method_name: String,
    /// Message type
    pub message_type: MessageType,
    /// Whether this is part of a loop
    pub is_loop: bool,
    /// Loop label (if is_loop)
    pub loop_label: Option<String>,
    /// Whether this is a self-call
    pub is_self_call: bool,
    /// Sequence number for ordering
    pub seq: usize,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

/// Type of message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// Synchronous call with return
    SynchronousCall,
    /// Asynchronous message (no immediate return expected)
    AsynchronousCall,
    /// Return value/message
    Return,
    /// Self-call (recursive)
    SelfCall,
    /// Signal (one-way message)
    Signal,
    /// Create instance
    Create,
    /// Delete/destroy instance
    Delete,
}

impl Default for MessageType {
    fn default() -> Self {
        MessageType::SynchronousCall
    }
}

impl MessageType {
    /// Infer message type from symbol names and call patterns
    pub fn infer(method_name: &str, is_self_call: bool, is_loop: bool) -> Self {
        if is_self_call || is_loop {
            return MessageType::SelfCall;
        }

        let name_lower = method_name.to_lowercase();
        if name_lower.starts_with("new_")
            || name_lower.starts_with("create_")
            || name_lower.starts_with("init_")
        {
            return MessageType::Create;
        }

        if name_lower.starts_with("delete_")
            || name_lower.starts_with("drop_")
            || name_lower.starts_with("destroy_")
        {
            return MessageType::Delete;
        }

        if name_lower.starts_with("send_")
            || name_lower.starts_with("emit_")
            || name_lower.starts_with("publish_")
        {
            return MessageType::AsynchronousCall;
        }

        if name_lower.starts_with("on_")
            || name_lower.starts_with("handle_")
            || name_lower.starts_with("notify_")
        {
            return MessageType::Signal;
        }

        MessageType::SynchronousCall
    }
}

impl SequenceModel {
    /// Create a new empty sequence model
    pub fn new(title: impl Into<String>, entry_point: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            entry_point: entry_point.into(),
            participants: Vec::new(),
            messages: Vec::new(),
            metadata: SequenceMetadata {
                max_depth: 0,
                participant_count: 0,
                message_count: 0,
                async_count: 0,
                loop_count: 0,
            },
        }
    }

    /// Add a participant
    pub fn add_participant(&mut self, participant: SequenceParticipant) {
        if !self.participants.iter().any(|p| p.id == participant.id) {
            self.participants.push(participant);
        }
    }

    /// Add a message
    pub fn add_message(&mut self, message: SequenceMessage) {
        self.messages.push(message);
    }

    /// Finalize the model (compute metadata)
    pub fn finalize(&mut self) {
        let max_depth = self
            .messages
            .iter()
            .map(|m| m.seq)
            .max()
            .unwrap_or(0);

        let loop_count = self.messages.iter().filter(|m| m.is_loop).count();
        let async_count = self
            .messages
            .iter()
            .filter(|m| m.message_type == MessageType::AsynchronousCall)
            .count();

        self.metadata = SequenceMetadata {
            max_depth,
            participant_count: self.participants.len(),
            message_count: self.messages.len(),
            async_count,
            loop_count,
        };
    }

    /// Get participant by ID
    pub fn get_participant(&self, id: &str) -> Option<&SequenceParticipant> {
        self.participants.iter().find(|p| p.id == id)
    }

    /// Check if the diagram has loops
    pub fn has_loops(&self) -> bool {
        self.metadata.loop_count > 0
    }

    /// Check if the diagram has async messages
    pub fn has_async(&self) -> bool {
        self.metadata.async_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_model_new() {
        let model = SequenceModel::new("Test", "main");
        assert_eq!(model.title, "Test");
        assert_eq!(model.entry_point, "main");
        assert!(model.participants.is_empty());
        assert!(model.messages.is_empty());
    }

    #[test]
    fn test_add_participant() {
        let mut model = SequenceModel::new("Test", "main");
        model.add_participant(SequenceParticipant {
            id: "foo".to_string(),
            name: "Foo".to_string(),
            module: "bar".to_string(),
            location: None,
            participant_type: ParticipantType::System,
        });
        assert_eq!(model.participants.len(), 1);
    }

    #[test]
    fn test_add_duplicate_participant() {
        let mut model = SequenceModel::new("Test", "main");
        model.add_participant(SequenceParticipant {
            id: "foo".to_string(),
            name: "Foo".to_string(),
            module: "bar".to_string(),
            location: None,
            participant_type: ParticipantType::System,
        });
        model.add_participant(SequenceParticipant {
            id: "foo".to_string(),
            name: "Foo2".to_string(),
            module: "baz".to_string(),
            location: None,
            participant_type: ParticipantType::Database,
        });
        // Should not add duplicate
        assert_eq!(model.participants.len(), 1);
    }

    #[test]
    fn test_finalize() {
        let mut model = SequenceModel::new("Test", "main");
        model.add_participant(SequenceParticipant {
            id: "a".to_string(),
            name: "A".to_string(),
            module: "mod".to_string(),
            location: None,
            participant_type: ParticipantType::System,
        });
        model.add_participant(SequenceParticipant {
            id: "b".to_string(),
            name: "B".to_string(),
            module: "mod".to_string(),
            location: None,
            participant_type: ParticipantType::System,
        });
        model.add_message(SequenceMessage {
            id: "1".to_string(),
            from: "a".to_string(),
            to: "b".to_string(),
            method_name: "call".to_string(),
            message_type: MessageType::SynchronousCall,
            is_loop: false,
            loop_label: None,
            is_self_call: false,
            seq: 1,
            confidence: 1.0,
        });
        model.finalize();

        assert_eq!(model.metadata.participant_count, 2);
        assert_eq!(model.metadata.message_count, 1);
        assert_eq!(model.metadata.max_depth, 1);
    }

    #[test]
    fn test_participant_type_inference() {
        assert_eq!(
            ParticipantType::from_name_and_module("user", "main"),
            ParticipantType::Actor
        );
        assert_eq!(
            ParticipantType::from_name_and_module("UserRepository", "db"),
            ParticipantType::Database
        );
        assert_eq!(
            ParticipantType::from_name_and_module("HttpService", "service"),
            ParticipantType::ExternalService
        );
    }

    #[test]
    fn test_message_type_infer() {
        assert_eq!(
            MessageType::infer("call_foo", false, false),
            MessageType::SynchronousCall
        );
        assert_eq!(
            MessageType::infer("new_foo", false, false),
            MessageType::Create
        );
        assert_eq!(
            MessageType::infer("send_email", false, false),
            MessageType::AsynchronousCall
        );
        assert_eq!(
            MessageType::infer("handle_click", false, false),
            MessageType::Signal
        );
        assert_eq!(MessageType::infer("call_foo", true, false), MessageType::SelfCall);
    }
}
