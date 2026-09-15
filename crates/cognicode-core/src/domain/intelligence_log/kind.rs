//! Event kinds (M7, cycle e63).
//!
//! ## Why not an enum
//!
//! An `enum EventKind { FactBatchCommitted, AnalysisStarted, … }` is the
//! obvious first move and the wrong one: every future milestone would reopen
//! it, every stored event would need a migration when a variant is renamed,
//! and an open vocabulary (`plugin.*`, `company.*`) would be impossible.
//!
//! A kind is therefore a **validated namespaced name** (`namespace.name`, the
//! shared grammar), with the kinds actually emitted published as constructors.
//! Adding a kind is adding a string, not changing a type.
//!
//! Actors ([`ActorRef`]) now live in [`crate::domain::execution::actor`], since
//! they identify an *execution*, not an event; they are re-exported here so
//! event-log call sites keep working.

use serde::{Deserialize, Serialize};

use crate::domain::naming::{NamespacedError, NamespacedName};

pub use crate::domain::execution::actor::{ActorError as KindError, ActorKind, ActorRef};

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

/// The event kinds the platform emits, and the ones its contracts reserve.
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

    /// A behavior execution started.
    pub fn behavior_started() -> EventKind {
        Self::expect_valid("behavior.started")
    }

    /// A behavior completed, having produced its accepted effects.
    pub fn behavior_completed() -> EventKind {
        Self::expect_valid("behavior.completed")
    }

    /// A behavior's output was rejected by policy (ADR-044).
    pub fn behavior_output_rejected() -> EventKind {
        Self::expect_valid("policy.behavior_output_rejected")
    }

    /// A behavior exhausted its budget (M7.4).
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
            Self::behavior_completed(),
            Self::behavior_output_rejected(),
            Self::behavior_budget_exhausted(),
        ]
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
        // Open vocabulary: a plugin may publish `company.audit_recorded`
        // without changing a core type.
        assert!(EventKind::new("company.audit_recorded").is_ok());
    }

    #[test]
    fn actors_are_re_exported_from_their_real_home() {
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
    }

    #[test]
    fn kinds_round_trip() {
        let kind = EventKinds::analysis_completed();
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"analysis.completed\"");
        assert_eq!(serde_json::from_str::<EventKind>(&json).unwrap(), kind);
        assert!(serde_json::from_str::<EventKind>("\"nons\"").is_err());
    }
}
