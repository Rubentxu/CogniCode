//! Execution context — what a concrete execution *is*, and where it sits in the
//! causal history (M7.2, cycle e64).
//!
//! ```text
//!                         ExecutionContext
//!                      ┌─────────────────────┐
//!                      │ execution_id        │
//!                      │ scope               │
//!                      │ actor               │
//!                      │ correlation         │
//!                      │ trigger_event       │
//!                      └──────────┬──────────┘
//!                                 │
//!                  ┌──────────────┴──────────────┐
//!                  ▼                             ▼
//!         DetectorExecutionRef          BehaviorExecutionRef
//!                  │                             │
//!          DetectorAuthority                 BehaviorClass
//! ```
//!
//! ## Two different questions, two different types
//!
//! ```text
//! ExecutionContext    which execution is this, and where in history
//! DetectorAuthority   what authority that execution held
//! BehaviorClass       what authority that execution held
//! ```
//!
//! This module answers only the first. Authority is decided by the admission
//! that minted the permit and is carried *beside* the context, never derived
//! from it — an agent acting is not more or less authorized because it is an
//! agent; the class says what it may do, and the class comes from admission.
//!
//! ## `trigger_event`, not `caused_by`
//!
//! `caused_by` belongs to **events**: it is the edge between two log records.
//! `trigger_event` says *the event that originated this execution*. They are
//! different relations and using one word for both is how a causal graph
//! acquires edges that were never true.
//!
//! ```text
//! SourceDelta event
//!       │ trigger_event
//!       ▼
//! ExecutionContext ──► behavior.started ──caused_by──► policy.behavior_output_rejected
//! ```

use serde::{Deserialize, Serialize};

use super::actor::ActorRef;
use super::correlation::CorrelationId;
use super::scope::AnalysisScope;
use crate::domain::kernel_ids::{EventId, ExecutionId};

/// Why an execution context was rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionContextError {
    /// The scope is not pinned to a real snapshot.
    InvalidScope,
}

impl std::fmt::Display for ExecutionContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("an execution context needs a scope pinned to a real snapshot")
    }
}

impl std::error::Error for ExecutionContextError {}

/// The identity of one concrete execution and its place in the causal history.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Which execution this is.
    pub execution_id: ExecutionId,
    /// The `(workspace, snapshot)` it ran against.
    pub scope: AnalysisScope,
    /// Who ran it.
    pub actor: ActorRef,
    /// The logical operation it belongs to.
    pub correlation: CorrelationId,
    /// The event that originated this execution, if any.
    ///
    /// Not the same as an event's `caused_by`: this points *into* the execution
    /// from history, not between two events.
    pub trigger_event: Option<EventId>,
}

impl ExecutionContext {
    /// Construct a context, rejecting an unpinned scope.
    pub fn try_new(
        execution_id: ExecutionId,
        scope: AnalysisScope,
        actor: ActorRef,
        correlation: CorrelationId,
        trigger_event: Option<EventId>,
    ) -> Result<Self, ExecutionContextError> {
        if !scope.is_valid() {
            return Err(ExecutionContextError::InvalidScope);
        }
        Ok(Self {
            execution_id,
            scope,
            actor,
            correlation,
            trigger_event,
        })
    }

    /// Whether the context is well formed.
    pub fn is_valid(&self) -> bool {
        self.scope.is_valid()
    }

    /// Whether this execution was triggered by an event.
    pub fn is_triggered(&self) -> bool {
        self.trigger_event.is_some()
    }

    /// The label used in finding ids (`exec5`), kept next to the id so the two
    /// cannot drift.
    pub fn id_label(&self) -> String {
        format!("exec{}", self.execution_id.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::kernel_ids::SnapshotId;
    use crate::domain::value_objects::WorkspaceId;

    fn scope(snapshot: u64) -> AnalysisScope {
        AnalysisScope::new(
            WorkspaceId::try_new("ws").unwrap(),
            SnapshotId::new(snapshot),
        )
    }

    fn context(trigger: Option<EventId>) -> ExecutionContext {
        ExecutionContext::try_new(
            ExecutionId::new(5),
            scope(1),
            ActorRef::detector("security.weak_hash"),
            CorrelationId::new("corr-1").unwrap(),
            trigger,
        )
        .unwrap()
    }

    #[test]
    fn a_context_names_the_execution_and_its_actor() {
        let ctx = context(Some(EventId::new(3)));
        assert_eq!(ctx.execution_id, ExecutionId::new(5));
        assert_eq!(ctx.scope.snapshot, SnapshotId::new(1));
        assert_eq!(ctx.actor, ActorRef::detector("security.weak_hash"));
        assert_eq!(ctx.correlation.as_str(), "corr-1");
        assert!(ctx.is_triggered());
        assert!(ctx.is_valid());
    }

    #[test]
    fn an_untriggered_execution_is_legitimate() {
        let ctx = context(None);
        assert!(!ctx.is_triggered());
        assert_eq!(ctx.trigger_event, None);
    }

    #[test]
    fn an_unpinned_scope_is_rejected() {
        assert_eq!(
            ExecutionContext::try_new(
                ExecutionId::new(1),
                AnalysisScope::new(WorkspaceId::try_new("ws").unwrap(), SnapshotId::NONE),
                ActorRef::kernel(),
                CorrelationId::new("c").unwrap(),
                None,
            )
            .unwrap_err(),
            ExecutionContextError::InvalidScope
        );
    }

    #[test]
    fn the_finding_id_label_comes_from_the_execution_id() {
        assert_eq!(context(None).id_label(), "exec5");
    }

    #[test]
    fn contexts_round_trip() {
        let ctx = context(Some(EventId::new(9)));
        let json = serde_json::to_string(&ctx).unwrap();
        assert_eq!(
            serde_json::from_str::<ExecutionContext>(&json).unwrap(),
            ctx
        );
    }
}
