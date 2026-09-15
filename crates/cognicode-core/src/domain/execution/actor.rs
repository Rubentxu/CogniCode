//! Actors — who caused something.
//!
//! **Ungated general execution vocabulary (cycle e64).** An `ActorRef` says who
//! performed an *execution*; the event log is one consumer, not the owner.
//! `intelligence_log::kind` re-exports it.
//!
//! The pair `(kind, id)` is deliberate: "who" is not just a string, because the
//! authority model (ADR-044) will treat the same string differently depending on
//! whether a detector or an agent said it.

use serde::{Deserialize, Serialize};

/// Why an actor reference was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorError {
    /// The actor id was empty.
    EmptyActor,
}

impl std::fmt::Display for ActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an actor id must not be empty")
    }
}

impl std::error::Error for ActorError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    /// The kernel itself (a fact batch was committed).
    Kernel,
    /// A human acting directly.
    Human,
    /// A deterministic detector or analyzer.
    Detector,
    /// A behavior (M7.3; no behavior runtime exists yet).
    Behavior,
    /// An AI agent.
    Agent,
    /// A component that is none of the above.
    System,
}

impl ActorKind {
    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Human => "human",
            Self::Detector => "detector",
            Self::Behavior => "behavior",
            Self::Agent => "agent",
            Self::System => "system",
        }
    }
}

/// An identified actor.
///
/// The pair `(kind, id)` is deliberate: "who" is not just a string, because the
/// authority model (ADR-044) will treat the same string differently depending
/// on whether a detector or an agent said it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorRef {
    /// What kind of actor.
    pub kind: ActorKind,
    /// Stable identifier within that kind.
    pub id: String,
}

impl ActorRef {
    /// Construct an actor reference, rejecting an empty id.
    pub fn new(kind: ActorKind, id: impl Into<String>) -> Result<Self, ActorError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ActorError::EmptyActor);
        }
        Ok(Self { kind, id })
    }

    /// The kernel itself.
    pub fn kernel() -> Self {
        Self {
            kind: ActorKind::Kernel,
            id: "kernel".to_string(),
        }
    }

    /// A deterministic detector.
    pub fn detector(id: impl Into<String>) -> Self {
        Self::new(ActorKind::Detector, id).expect("detector id must not be empty")
    }

    /// A human.
    pub fn human(id: impl Into<String>) -> Self {
        Self::new(ActorKind::Human, id).expect("human id must not be empty")
    }

    /// An AI agent.
    pub fn agent(id: impl Into<String>) -> Self {
        Self::new(ActorKind::Agent, id).expect("agent id must not be empty")
    }

    /// A behavior.
    pub fn behavior(id: impl Into<String>) -> Self {
        Self::new(ActorKind::Behavior, id).expect("behavior id must not be empty")
    }

    /// A generic component.
    pub fn system(id: impl Into<String>) -> Self {
        Self::new(ActorKind::System, id).expect("system id must not be empty")
    }
}

impl std::fmt::Display for ActorRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.kind.name(), self.id)
    }
}
