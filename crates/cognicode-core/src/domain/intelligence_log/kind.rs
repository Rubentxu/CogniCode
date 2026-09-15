//! Event kinds and actors (M7, cycle e63).
//!
//! ## Why not an enum
//!
//! An `enum EventKind { FactBatchCommitted, AnalysisStarted, … }` is the
//! obvious first move and the wrong one: every future milestone would reopen
//! it, every stored event would need a migration when a variant is renamed,
//! and an open vocabulary (`plugin.*`, `company.*`) would be impossible.
//!
//! A kind is therefore a **validated namespaced name** (`namespace.name`,
//! the shared grammar), with the kinds that e63 actually emits published as
//! constants. Adding a kind is adding a string, not changing a type.
//!
//! ## Actors
//!
//! An [`ActorRef`] says *who* caused an event. The kind is what the future
//! behavior-authority model (ADR-044) will key on — an agent and a detector are
//! different actors even when they produce the same evidence.

use serde::{Deserialize, Serialize};

use crate::domain::naming::{NamespacedError, NamespacedName};

/// Why an actor reference was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindError {
    /// The actor id was empty.
    EmptyActor,
    /// The kind name is not a valid `namespace.name`.
    Kind(NamespacedError),
}

impl std::fmt::Display for KindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyActor => f.write_str("an actor id must not be empty"),
            Self::Kind(err) => write!(f, "invalid event kind: {err}"),
        }
    }
}

impl std::error::Error for KindError {}

/// What kind of thing an event kind is, as a validated namespaced name.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EventKind(NamespacedName);

impl EventKind {
    /// Validate and construct an event kind.
    pub fn new(value: impl Into<String>) -> Result<Self, NamespacedError> {
        NamespacedName::new(value).map(Self)
    }

    /// Borrow the raw value.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The namespace segment (`kernel`, `analysis`, `finding`, `behavior`, …).
    pub fn namespace(&self) -> &str {
        self.0.namespace()
    }

    /// The local name.
    pub fn name(&self) -> &str {
        self.0.name()
    }
}

impl std::fmt::Display for EventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl TryFrom<String> for EventKind {
    type Error = NamespacedError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<EventKind> for String {
    fn from(value: EventKind) -> Self {
        value.0.as_str().to_string()
    }
}

/// The event kinds e63 emits, and the ones reserved by the umbrella contracts.
///
/// Published as constructors rather than an enum: the vocabulary has to stay
/// open, but the *names actually in use* must be greppable and typo-proof.
pub struct EventKinds;

impl EventKinds {
    /// A source delta was observed (the root of a causal chain).
    pub fn source_delta() -> EventKind {
        Self::expect_valid("kernel.source_delta")
    }

    /// A batch of facts was committed to the evidence kernel.
    ///
    /// One event per **batch**, carrying its size and digest — never one event
    /// per fact.
    pub fn fact_batch_committed() -> EventKind {
        Self::expect_valid("kernel.fact_batch_committed")
    }

    /// An analysis run started.
    pub fn analysis_started() -> EventKind {
        Self::expect_valid("analysis.started")
    }

    /// An analysis run completed, producing findings.
    pub fn analysis_completed() -> EventKind {
        Self::expect_valid("analysis.completed")
    }

    /// A finding was produced.
    pub fn finding_produced() -> EventKind {
        Self::expect_valid("finding.produced")
    }

    /// A behavior started (reserved: no behavior runtime exists yet).
    pub fn behavior_started() -> EventKind {
        Self::expect_valid("behavior.started")
    }

    /// A behavior's output was rejected by policy (reserved, ADR-044).
    pub fn behavior_output_rejected() -> EventKind {
        Self::expect_valid("policy.behavior_output_rejected")
    }

    /// A behavior exhausted its budget (reserved, M7.4).
    pub fn behavior_budget_exhausted() -> EventKind {
        Self::expect_valid("behavior.budget_exhausted")
    }

    fn expect_valid(value: &str) -> EventKind {
        EventKind::new(value).expect("a published event kind must be well formed")
    }

    /// Every kind this module publishes, for the invariant test below.
    pub fn published() -> Vec<EventKind> {
        vec![
            Self::source_delta(),
            Self::fact_batch_committed(),
            Self::analysis_started(),
            Self::analysis_completed(),
            Self::finding_produced(),
            Self::behavior_started(),
            Self::behavior_output_rejected(),
            Self::behavior_budget_exhausted(),
        ]
    }
}

/// Who caused an event.
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
    pub fn new(kind: ActorKind, id: impl Into<String>) -> Result<Self, KindError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(KindError::EmptyActor);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_published_kind_is_well_formed() {
        // The constructors `expect` internally; this test is what makes that
        // expectation visible rather than a latent panic.
        for kind in EventKinds::published() {
            assert!(kind.as_str().contains('.'));
            assert!(!kind.namespace().is_empty());
            assert!(!kind.name().is_empty());
        }
    }

    #[test]
    fn kinds_are_namespaced_and_validated() {
        let kind = EventKinds::fact_batch_committed();
        assert_eq!(kind.as_str(), "kernel.fact_batch_committed");
        assert_eq!(kind.namespace(), "kernel");
        assert_eq!(kind.name(), "fact_batch_committed");

        for bad in ["", "nons", "a.", ".b"] {
            assert!(EventKind::new(bad).is_err(), "`{bad}` must be rejected");
        }
    }

    #[test]
    fn an_unknown_namespace_is_allowed() {
        // Open vocabulary: a plugin may publish `company.foo.thing` without
        // changing a core type.
        assert!(EventKind::new("company.audit_recorded").is_ok());
    }

    #[test]
    fn actors_carry_kind_and_id() {
        let kernel = ActorRef::kernel();
        assert_eq!(kernel.kind, ActorKind::Kernel);
        assert_eq!(kernel.to_string(), "kernel:kernel");
        assert_eq!(
            ActorRef::detector("security.weak_hash").to_string(),
            "detector:security.weak_hash"
        );
        assert_eq!(
            ActorRef::new(ActorKind::Agent, "  "),
            Err(KindError::EmptyActor)
        );
        assert_eq!(
            ActorRef::new(ActorKind::Human, "").unwrap_err(),
            KindError::EmptyActor
        );
    }

    #[test]
    fn kinds_and_actors_round_trip() {
        let kind = EventKinds::analysis_completed();
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"analysis.completed\"");
        assert_eq!(serde_json::from_str::<EventKind>(&json).unwrap(), kind);
        assert!(serde_json::from_str::<EventKind>("\"nons\"").is_err());

        let actor = ActorRef::agent("reviewer-1");
        let json = serde_json::to_string(&actor).unwrap();
        assert_eq!(serde_json::from_str::<ActorRef>(&json).unwrap(), actor);
    }
}
