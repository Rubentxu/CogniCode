//! The event itself (M7, cycle e63).

use serde::{Deserialize, Serialize};

use super::ids::CorrelationId;
use super::kind::{ActorRef, EventKind};
use super::payload::{BoundedEventPayload, EventPayloadRef};
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::kernel_ids::EventId;

/// When an event occurred, in milliseconds since the Unix epoch.
///
/// Supplied by the caller, never read from a clock: the domain must be
/// deterministic, and a replayed log has to reproduce the same values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventTime(i64);

impl EventTime {
    /// Construct an instant from epoch milliseconds.
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// The epoch milliseconds.
    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

impl std::fmt::Display for EventTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Why an event was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventError {
    /// `caused_by` pointed at the event's own id.
    SelfCaused,
    /// The kind was structurally unusable (empty namespace or name).
    MalformedKind(String),
    /// The event's scope is not pinned to a real snapshot (`SnapshotId::NONE`).
    InvalidScope,
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SelfCaused => f.write_str("an event cannot be caused by itself"),
            Self::MalformedKind(kind) => write!(f, "malformed event kind: `{kind}`"),
            Self::InvalidScope => f.write_str("an event's scope must be pinned to a real snapshot"),
        }
    }
}

impl std::error::Error for EventError {}

/// An event that has not been appended yet: everything except its id.
///
/// The store assigns the id, exactly as the evidence kernel assigns evidence
/// ids — a producer that numbers its own records collides with the next
/// producer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewIntelligenceEvent {
    /// The `(workspace, snapshot)` this event belongs to.
    pub scope: AnalysisScope,
    /// Who caused it.
    pub actor: ActorRef,
    /// Which logical operation it belongs to.
    pub correlation: CorrelationId,
    /// The event that caused this one, if any.
    pub caused_by: Option<EventId>,
    /// What happened.
    pub kind: EventKind,
    /// Where the payload lives.
    pub payload: EventPayloadRef,
    /// When it happened.
    pub occurred_at: EventTime,
}

impl NewIntelligenceEvent {
    /// Construct an event, defaulting to an inline summary payload.
    ///
    /// `summary` is turned into a bounded inline payload, so the common case
    /// cannot accidentally embed something large.
    pub fn new(
        scope: AnalysisScope,
        actor: ActorRef,
        correlation: CorrelationId,
        caused_by: Option<EventId>,
        kind: EventKind,
        summary: impl Into<String>,
        occurred_at: EventTime,
    ) -> Result<Self, super::payload::PayloadError> {
        Ok(Self {
            scope,
            actor,
            correlation,
            caused_by,
            kind,
            payload: EventPayloadRef::inline(BoundedEventPayload::new(summary)?),
            occurred_at,
        })
    }

    /// Replace the payload (e.g. with an artifact reference).
    pub fn with_payload(mut self, payload: EventPayloadRef) -> Self {
        self.payload = payload;
        self
    }

    /// Validate what can be checked before an id exists.
    ///
    /// `scope` and `kind` are public fields, so a struct literal (or a
    /// deserialized value) can carry values the constructors would have
    /// refused; this is the gate the store runs before committing anything.
    pub fn validate(&self) -> Result<(), EventError> {
        if !self.scope.is_valid() {
            return Err(EventError::InvalidScope);
        }
        if self.kind.as_str().trim().is_empty() {
            return Err(EventError::MalformedKind(self.kind.as_str().to_string()));
        }
        Ok(())
    }
}

/// An appended event: a [`NewIntelligenceEvent`] that now has an id.
///
/// Immutable by construction: there is no setter, and the store only ever
/// appends. A log whose entries can change is not a history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntelligenceEvent {
    /// The id the store assigned.
    pub id: EventId,
    /// The `(workspace, snapshot)` this event belongs to.
    pub scope: AnalysisScope,
    /// Who caused it.
    pub actor: ActorRef,
    /// Which logical operation it belongs to.
    pub correlation: CorrelationId,
    /// The event that caused this one, if any.
    pub caused_by: Option<EventId>,
    /// What happened.
    pub kind: EventKind,
    /// Where the payload lives.
    pub payload: EventPayloadRef,
    /// When it happened.
    pub occurred_at: EventTime,
}

impl IntelligenceEvent {
    /// Build an appended event from a new one and the id the store assigned.
    pub fn from_new(id: EventId, new: NewIntelligenceEvent) -> Self {
        Self {
            id,
            scope: new.scope,
            actor: new.actor,
            correlation: new.correlation,
            caused_by: new.caused_by,
            kind: new.kind,
            payload: new.payload,
            occurred_at: new.occurred_at,
        }
    }

    /// Whether this event is the root of its causal chain.
    pub fn is_root(&self) -> bool {
        self.caused_by.is_none()
    }

    /// The causal depth this event claims: 0 for a root, `1 + parent` otherwise.
    pub fn depth(&self) -> u32 {
        if self.is_root() { 0 } else { 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intelligence_log::kind::EventKinds;
    use crate::domain::kernel_ids::SnapshotId;
    use crate::domain::value_objects::WorkspaceId;

    fn scope() -> AnalysisScope {
        AnalysisScope::new(WorkspaceId::try_new("ws").unwrap(), SnapshotId::new(1))
    }

    fn new_event(caused_by: Option<EventId>) -> NewIntelligenceEvent {
        NewIntelligenceEvent::new(
            scope(),
            ActorRef::kernel(),
            CorrelationId::new("c1").unwrap(),
            caused_by,
            EventKinds::fact_batch_committed(),
            "1000 facts committed",
            EventTime::from_millis(1_700_000_000_000),
        )
        .unwrap()
    }

    #[test]
    fn a_new_event_is_well_formed_and_reports_its_root() {
        let event = IntelligenceEvent::from_new(EventId::new(1), new_event(None));
        assert_eq!(event.id, EventId::new(1));
        assert!(event.is_root());
        assert!(event.payload.is_inline());
        assert_eq!(event.scope.snapshot, SnapshotId::new(1));
    }

    #[test]
    fn a_caused_event_is_not_a_root() {
        let event = IntelligenceEvent::from_new(EventId::new(2), new_event(Some(EventId::new(1))));
        assert!(!event.is_root());
        assert_eq!(event.caused_by, Some(EventId::new(1)));
    }

    #[test]
    fn self_causation_is_rejected_when_checked() {
        let new = new_event(Some(EventId::new(7)));
        let event = IntelligenceEvent::from_new(EventId::new(7), new);
        assert!(!event.is_root());
        // The store is what refuses a self-caused append; the domain exposes
        // the predicate so the check is not duplicated.
        assert_eq!(
            super::super::store::check_not_self_caused(&event).unwrap_err(),
            EventError::SelfCaused
        );
    }

    #[test]
    fn events_round_trip() {
        let event = IntelligenceEvent::from_new(EventId::new(3), new_event(None));
        let json = serde_json::to_string(&event).unwrap();
        let parsed: IntelligenceEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, event);
    }

    /// A scope pinned to the invalid sentinel is refused, however the event
    /// was built.
    #[test]
    fn an_unpinned_scope_is_rejected() {
        use crate::domain::kernel_ids::SnapshotId;
        let mut event = new_event(None);
        assert!(event.validate().is_ok());
        event.scope = crate::domain::findings::scope::AnalysisScope::new(
            crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
            SnapshotId::NONE,
        );
        assert_eq!(event.validate().unwrap_err(), EventError::InvalidScope);
    }

    #[test]
    fn event_time_is_an_opaque_millisecond_instant() {
        let t = EventTime::from_millis(42);
        assert_eq!(t.as_millis(), 42);
        assert_eq!(t.to_string(), "42");
        assert!(EventTime::from_millis(1) < EventTime::from_millis(2));
    }
}
