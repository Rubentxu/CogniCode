//! The event store port (M7, cycle e63).
//!
//! Small on purpose. e63 needs exactly four questions answered:
//!
//! ```text
//! append            — write
//! by_id             — read one
//! causal_chain      — why did this happen (root → … → this)
//! by_correlation    — what happened as part of this operation
//! replay            — the whole (or partial) history, in order
//! ```
//!
//! There is deliberately **no** `update`, `delete`, `publish` or `subscribe`:
//! the log is history, not a bus. Delivery is a separate concern that will be
//! layered on top of commit, never fused with it — an event must exist even if
//! nobody was listening.
//!
//! ## Ids
//!
//! The **store** assigns [`EventId`]s, exactly as the evidence kernel assigns
//! evidence ids. A producer that numbers its own events collides with the next
//! producer, and the collision would surface as a silent reordering of history.

use async_trait::async_trait;

use super::event::{IntelligenceEvent, NewIntelligenceEvent};
use crate::domain::findings::scope::AnalysisScope;
use crate::domain::kernel_ids::EventId;
use crate::domain::value_objects::WorkspaceId;

/// Why an append or a read failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventStoreError {
    /// `caused_by` names an event that does not exist in this workspace.
    ///
    /// Fail-closed: a chain pointing at nothing is exactly the corruption a
    /// causal log must not allow, and accepting it would make `causal_chain`
    /// silently truncate.
    UnknownCause(EventId),
    /// `caused_by` names an event from a different workspace.
    CauseInAnotherWorkspace {
        /// The cause.
        cause: EventId,
        /// The workspace the append targeted.
        workspace: WorkspaceId,
    },
    /// An event was caused by itself.
    SelfCaused(EventId),
    /// An event's own scope names a different workspace than the append target.
    ///
    /// An event that claims to belong to B must not be recorded as A's: the
    /// store and the event must agree on whose history this is.
    ScopeWorkspaceMismatch {
        /// The workspace the append targeted.
        append_workspace: WorkspaceId,
        /// The workspace the event's own scope names.
        event_workspace: WorkspaceId,
    },
    /// The event failed validation.
    Invalid(super::event::EventError),
    /// The stored history is inconsistent (a cycle, or a dangling cause).
    Corrupt(String),
    /// A caller asked for "caused by the previous event" with no previous event.
    ///
    /// A recorder must never invent a cause to keep going: a plausible history
    /// that is not true is worse than a failed append.
    NoPreviousEvent,
    /// Backend failure.
    Store(String),
}

impl std::fmt::Display for EventStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCause(id) => write!(f, "caused_by names unknown event {id}"),
            Self::CauseInAnotherWorkspace { cause, workspace } => write!(
                f,
                "caused_by names {cause}, which belongs to another workspace than {workspace}"
            ),
            Self::SelfCaused(id) => write!(f, "event {id} cannot be caused by itself"),
            Self::ScopeWorkspaceMismatch {
                append_workspace,
                event_workspace,
            } => write!(
                f,
                "the event's scope names workspace {event_workspace}, but the append targets {append_workspace}"
            ),
            Self::Invalid(err) => write!(f, "invalid event: {err}"),
            Self::Corrupt(msg) => write!(f, "corrupt event history: {msg}"),
            Self::NoPreviousEvent => {
                f.write_str("no previous event to be caused by; record a root event first")
            }
            Self::Store(msg) => write!(f, "event store failure: {msg}"),
        }
    }
}

impl std::error::Error for EventStoreError {}

/// Check a well-known invariant that every implementation must agree on.
///
/// Exposed so adapters do not each invent their own version of it.
pub fn check_not_self_caused(event: &IntelligenceEvent) -> Result<(), super::event::EventError> {
    if event.caused_by == Some(event.id) {
        return Err(super::event::EventError::SelfCaused);
    }
    Ok(())
}

/// Append-only causal history.
#[async_trait]
pub trait IntelligenceEventStore: Send + Sync {
    /// Append events to `workspace`, in order, assigning their ids.
    ///
    /// All-or-nothing: if any event is rejected, none is written, so the log
    /// never contains a partially applied operation. Causes must already exist
    /// in the same workspace.
    async fn append(
        &self,
        workspace: &WorkspaceId,
        events: Vec<NewIntelligenceEvent>,
    ) -> Result<Vec<EventId>, EventStoreError>;

    /// One event by id.
    async fn by_id(
        &self,
        workspace: &WorkspaceId,
        id: EventId,
    ) -> Result<Option<IntelligenceEvent>, EventStoreError>;

    /// The causal chain ending at `id`, in **root-first** order.
    ///
    /// For `SourceDelta ← FactBatch ← Analysis ← Finding` this returns
    /// `[SourceDelta, FactBatch, Analysis, Finding]`: the order in which things
    /// actually happened, which is what makes a chain readable and a replay
    /// comparable.
    async fn causal_chain(
        &self,
        workspace: &WorkspaceId,
        id: EventId,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError>;

    /// Every event of one correlation, in append order.
    async fn by_correlation(
        &self,
        workspace: &WorkspaceId,
        correlation: &super::ids::CorrelationId,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError>;

    /// The history in append order, optionally starting **after** `after`.
    ///
    /// Replay must be deterministic: calling it twice on an unchanged log
    /// returns the same sequence.
    async fn replay(
        &self,
        workspace: &WorkspaceId,
        after: Option<EventId>,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError>;

    /// How many events `workspace` holds.
    async fn len(&self, workspace: &WorkspaceId) -> Result<usize, EventStoreError>;

    /// Whether `workspace` holds no events.
    async fn is_empty(&self, workspace: &WorkspaceId) -> Result<bool, EventStoreError> {
        Ok(self.len(workspace).await? == 0)
    }

    /// The scope an event was recorded in, for callers that only have the id.
    ///
    /// Convenience over [`Self::by_id`] to keep the log's own vocabulary
    /// (`scope`) reachable without unwrapping an optional event.
    async fn scope_of(
        &self,
        workspace: &WorkspaceId,
        id: EventId,
    ) -> Result<Option<AnalysisScope>, EventStoreError> {
        Ok(self.by_id(workspace, id).await?.map(|e| e.scope))
    }
}
