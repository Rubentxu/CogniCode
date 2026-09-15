//! Recording causal slices into the Intelligence Event Log (M7, cycle e63).
//!
//! The domain owns the log's *shape*; this is the thin application helper that
//! real code uses at real boundaries. It exists to make the causal edge
//! explicit and hard to get wrong:
//!
//! ```text
//! let mut rec = CausalRecorder::new(store, ws, ActorRef::kernel(), correlation, now);
//! let delta = rec.record(scope, EventKinds::source_delta(), "delta").await?;
//! //   ^ root: no cause
//! let batch = rec.caused_by(delta).record(scope, EventKinds::fact_batch_committed(), "…").await?;
//! ```
//!
//! [`CausalRecorder`] never guesses a cause: the root case
//! ([`record_root`](CausalRecorder::record_root)) and the "caused by the
//! previous event" case ([`record_next`](CausalRecorder::record_next)) are
//! separate calls, and the latter refuses to run when there is no previous
//! event. Guessing is what produces a plausible history that is not true.
//!
//! Note what this is *not*: there is no `publish`, no subscriber list and no
//! delivery. Committing an event and telling somebody about it are different
//! operations, and the log has to survive the process dying between them.

use crate::domain::findings::scope::AnalysisScope;
use crate::domain::intelligence_log::event::{EventTime, NewIntelligenceEvent};
use crate::domain::intelligence_log::ids::CorrelationId;
use crate::domain::intelligence_log::kind::{ActorRef, EventKind};
use crate::domain::intelligence_log::payload::{
    BoundedEventPayload, EventPayloadRef, PayloadError,
};
use crate::domain::intelligence_log::store::{EventStoreError, IntelligenceEventStore};
use crate::domain::kernel_ids::EventId;
use crate::domain::value_objects::WorkspaceId;

/// Records one logical operation's causal chain.
pub struct CausalRecorder<'a> {
    store: &'a dyn IntelligenceEventStore,
    workspace: WorkspaceId,
    actor: ActorRef,
    correlation: CorrelationId,
    occurred_at: EventTime,
    previous: Option<EventId>,
}

impl<'a> CausalRecorder<'a> {
    /// Start a recorder for one correlation.
    ///
    /// `occurred_at` is supplied rather than read from a clock: the domain must
    /// stay deterministic so a replayed log reproduces the same values.
    pub fn new(
        store: &'a dyn IntelligenceEventStore,
        workspace: WorkspaceId,
        actor: ActorRef,
        correlation: CorrelationId,
        occurred_at: EventTime,
    ) -> Self {
        Self {
            store,
            workspace,
            actor,
            correlation,
            occurred_at,
            previous: None,
        }
    }

    /// The last event recorded through this recorder, if any.
    pub fn last(&self) -> Option<EventId> {
        self.previous
    }

    /// Record a **root** event (nothing caused it).
    pub async fn record_root(
        &mut self,
        scope: AnalysisScope,
        kind: EventKind,
        payload: EventPayloadRef,
    ) -> Result<EventId, EventStoreError> {
        self.record_with_cause(None, scope, kind, payload).await
    }

    /// Record an event caused by the previous one this recorder wrote.
    ///
    /// Fails with [`EventStoreError::NoPreviousEvent`] if nothing has been
    /// recorded yet: there is no previous event, and inventing one is exactly
    /// the failure mode this API avoids.
    pub async fn record_next(
        &mut self,
        scope: AnalysisScope,
        kind: EventKind,
        payload: EventPayloadRef,
    ) -> Result<EventId, EventStoreError> {
        let Some(cause) = self.previous else {
            return Err(EventStoreError::NoPreviousEvent);
        };
        self.record_with_cause(Some(cause), scope, kind, payload)
            .await
    }

    /// Record an event caused by a specific event (e.g. one written elsewhere).
    pub async fn record_caused_by(
        &mut self,
        cause: EventId,
        scope: AnalysisScope,
        kind: EventKind,
        payload: EventPayloadRef,
    ) -> Result<EventId, EventStoreError> {
        self.record_with_cause(Some(cause), scope, kind, payload)
            .await
    }

    async fn record_with_cause(
        &mut self,
        caused_by: Option<EventId>,
        scope: AnalysisScope,
        kind: EventKind,
        payload: EventPayloadRef,
    ) -> Result<EventId, EventStoreError> {
        let new = NewIntelligenceEvent {
            scope,
            actor: self.actor.clone(),
            correlation: self.correlation.clone(),
            caused_by,
            kind,
            payload,
            occurred_at: self.occurred_at,
        };
        let ids = self.store.append(&self.workspace, vec![new]).await?;
        let id = *ids.first().expect("append returns one id per event");
        self.previous = Some(id);
        Ok(id)
    }
}

/// A one-line inline payload, for events whose content is just a description.
pub fn summary_payload(summary: impl Into<String>) -> Result<EventPayloadRef, PayloadError> {
    Ok(EventPayloadRef::inline(BoundedEventPayload::new(summary)?))
}

/// An inline payload carrying a few bounded counts/metadata fields.
pub fn counted_payload(
    summary: impl Into<String>,
    fields: &[(&str, String)],
) -> Result<EventPayloadRef, PayloadError> {
    let mut payload = BoundedEventPayload::new(summary)?;
    for (key, value) in fields {
        payload = payload.with_field(*key, value.clone())?;
    }
    Ok(EventPayloadRef::inline(payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intelligence_log::kind::EventKinds;
    use crate::domain::kernel_ids::SnapshotId;
    use crate::infrastructure::intelligence_log::InMemoryEventLog;

    fn ws() -> WorkspaceId {
        WorkspaceId::try_new("ws").unwrap()
    }

    fn scope() -> AnalysisScope {
        AnalysisScope::new(ws(), SnapshotId::new(1))
    }

    #[tokio::test]
    async fn a_recorder_builds_a_chain_in_order() {
        let log = InMemoryEventLog::new();
        let mut rec = CausalRecorder::new(
            &log,
            ws(),
            ActorRef::kernel(),
            CorrelationId::new("c1").unwrap(),
            EventTime::from_millis(5),
        );
        assert!(rec.last().is_none());

        let delta = rec
            .record_root(
                scope(),
                EventKinds::source_delta(),
                summary_payload("delta").unwrap(),
            )
            .await
            .unwrap();
        let batch = rec
            .record_next(
                scope(),
                EventKinds::fact_batch_committed(),
                counted_payload("batch", &[("count", "10".to_string())]).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rec.last(), Some(batch));

        let chain = log.causal_chain(&ws(), batch).await.unwrap();
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].id, delta);
        assert_eq!(chain[0].caused_by, None);
        assert_eq!(chain[1].caused_by, Some(delta));
    }

    #[tokio::test]
    async fn record_next_without_a_previous_event_fails_loud() {
        let log = InMemoryEventLog::new();
        let mut rec = CausalRecorder::new(
            &log,
            ws(),
            ActorRef::kernel(),
            CorrelationId::new("c1").unwrap(),
            EventTime::from_millis(5),
        );
        assert_eq!(
            rec.record_next(
                scope(),
                EventKinds::source_delta(),
                summary_payload("x").unwrap()
            )
            .await
            .unwrap_err(),
            EventStoreError::NoPreviousEvent,
            "there is no previous event, and none may be invented"
        );
        assert_eq!(rec.last(), None);
    }

    #[test]
    fn counted_payloads_are_bounded() {
        assert!(counted_payload("s", &[("a", "1".to_string())]).is_ok());
        assert!(summary_payload("").is_err());
    }
}
