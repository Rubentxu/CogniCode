//! In-memory, append-only event log — the reference oracle (M7, cycle e63).
//!
//! Same role the in-memory evidence store plays for the kernel: a small,
//! obviously-correct implementation that defines the *semantics* of the port,
//! so a durable adapter later has something to be tested against.
//!
//! ## Semantics fixed here
//!
//! - **Ids are allocated by the store**, from one global sequence in append
//!   order, shared by every workspace. Nothing else may choose an id. One sequence (rather than one per
//!   workspace, as the evidence kernel uses) is what makes the workspace check
//!   on a cause *meaningful*: with per-workspace numbering the same id exists in
//!   every workspace and "this cause is another tenant's event" would be
//!   undetectable.
//! - **Append is all-or-nothing.** Every event is validated (including its
//!   cause) before anything is written, so a rejected batch leaves the log
//!   exactly as it was.
//! - **A cause must already exist in the same workspace**, either stored or
//!   earlier in the same batch (a batch of related events is naturally caused
//!   intra-batch). A chain that points at nothing is corruption, and accepting
//!   it would make `causal_chain` silently truncate.
//! - **Nothing is ever mutated or removed.** There is no update or delete API;
//!   the value returned by `by_id` cannot change.
//! - **Workspaces are isolated**: an event's own scope must name the workspace
//!   it is appended to, and a cause from another workspace is refused, so one
//!   tenant's history can never be reached from another's.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::domain::findings::scope::AnalysisScope;
use crate::domain::intelligence_log::event::{IntelligenceEvent, NewIntelligenceEvent};
use crate::domain::intelligence_log::ids::CorrelationId;
use crate::domain::intelligence_log::store::{
    EventStoreError, IntelligenceEventStore, check_not_self_caused,
};
use crate::domain::kernel_ids::EventId;
use crate::domain::value_objects::WorkspaceId;

#[derive(Default)]
struct Log {
    /// Append order, per workspace.
    events: HashMap<WorkspaceId, Vec<IntelligenceEvent>>,
    /// Which workspace owns each id, so a cross-tenant cause is detectable.
    owners: HashMap<EventId, WorkspaceId>,
    /// Next id to allocate (one global sequence).
    next_id: u64,
}

/// An append-only event log held in memory.
#[derive(Default)]
pub struct InMemoryEventLog {
    log: Mutex<Log>,
}

impl InMemoryEventLog {
    /// Create an empty log.
    pub fn new() -> Self {
        Self::default()
    }

    fn events_of<'g>(log: &'g Log, workspace: &WorkspaceId) -> Option<&'g Vec<IntelligenceEvent>> {
        log.events.get(workspace)
    }
}

#[async_trait]
impl IntelligenceEventStore for InMemoryEventLog {
    async fn append(
        &self,
        workspace: &WorkspaceId,
        events: Vec<NewIntelligenceEvent>,
    ) -> Result<Vec<EventId>, EventStoreError> {
        let mut log = self.log.lock().expect("event log lock");

        // ── Validate everything first: an append is all-or-nothing ──────────
        let mut next = log.next_id.max(1);
        let mut planned: Vec<(EventId, NewIntelligenceEvent)> = Vec::with_capacity(events.len());
        for event in events {
            event.validate().map_err(EventStoreError::Invalid)?;
            // The store and the event must agree on whose history this is.
            if &event.scope.workspace != workspace {
                return Err(EventStoreError::ScopeWorkspaceMismatch {
                    append_workspace: workspace.clone(),
                    event_workspace: event.scope.workspace.clone(),
                });
            }
            let id = EventId::new(next);
            next += 1;
            // A cause must exist in this workspace — either already stored, or
            // earlier **in this same batch** (a batch of related events is
            // naturally caused intra-batch) — and cannot be this event.
            if let Some(cause) = event.caused_by {
                if cause == id {
                    return Err(EventStoreError::SelfCaused(id));
                }
                let owner = log.owners.get(&cause).cloned();
                let earlier_in_batch = planned.iter().any(|(assigned, _)| *assigned == cause);
                match owner {
                    Some(owner) if &owner != workspace => {
                        return Err(EventStoreError::CauseInAnotherWorkspace {
                            cause,
                            workspace: workspace.clone(),
                        });
                    }
                    Some(_) => {}
                    None if earlier_in_batch => {}
                    None => return Err(EventStoreError::UnknownCause(cause)),
                }
            }
            planned.push((id, event));
        }

        // ── Commit ─────────────────────────────────────────────────────────
        let mut ids = Vec::with_capacity(planned.len());
        let mut committed = Vec::with_capacity(planned.len());
        for (id, new) in planned {
            let event = IntelligenceEvent::from_new(id, new);
            check_not_self_caused(&event).map_err(EventStoreError::Invalid)?;
            ids.push(id);
            committed.push(event);
        }
        for event in &committed {
            log.owners.insert(event.id, workspace.clone());
        }
        log.events
            .entry(workspace.clone())
            .or_default()
            .extend(committed);
        log.next_id = next;
        Ok(ids)
    }

    async fn by_id(
        &self,
        workspace: &WorkspaceId,
        id: EventId,
    ) -> Result<Option<IntelligenceEvent>, EventStoreError> {
        let log = self.log.lock().expect("event log lock");
        Ok(Self::events_of(&log, workspace)
            .and_then(|events| events.iter().find(|e| e.id == id))
            .cloned())
    }

    async fn causal_chain(
        &self,
        workspace: &WorkspaceId,
        id: EventId,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError> {
        let log = self.log.lock().expect("event log lock");
        let Some(events) = Self::events_of(&log, workspace) else {
            return Ok(Vec::new());
        };

        let mut chain: Vec<IntelligenceEvent> = Vec::new();
        let mut seen: Vec<EventId> = Vec::new();
        let mut cursor = Some(id);
        while let Some(current) = cursor {
            if seen.contains(&current) {
                // A cycle can only exist if something bypassed `append`; fail
                // loud rather than loop forever.
                return Err(EventStoreError::Corrupt(format!(
                    "causal chain contains a cycle at {current}"
                )));
            }
            seen.push(current);
            let Some(event) = events.iter().find(|e| e.id == current) else {
                return Err(EventStoreError::Corrupt(format!(
                    "causal chain references unknown event {current}"
                )));
            };
            cursor = event.caused_by;
            chain.push(event.clone());
        }
        // Collected tip-first; a chain reads (and replays) root-first.
        chain.reverse();
        Ok(chain)
    }

    async fn by_correlation(
        &self,
        workspace: &WorkspaceId,
        correlation: &CorrelationId,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError> {
        let log = self.log.lock().expect("event log lock");
        Ok(Self::events_of(&log, workspace)
            .map(|events| {
                events
                    .iter()
                    .filter(|e| &e.correlation == correlation)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn replay(
        &self,
        workspace: &WorkspaceId,
        after: Option<EventId>,
    ) -> Result<Vec<IntelligenceEvent>, EventStoreError> {
        let log = self.log.lock().expect("event log lock");
        let Some(events) = Self::events_of(&log, workspace) else {
            return Ok(Vec::new());
        };
        let start = match after {
            None => 0,
            Some(id) => {
                let position = events
                    .iter()
                    .position(|e| e.id == id)
                    .ok_or(EventStoreError::UnknownCause(id))?;
                position + 1
            }
        };
        Ok(events.get(start..).unwrap_or_default().to_vec())
    }

    async fn len(&self, workspace: &WorkspaceId) -> Result<usize, EventStoreError> {
        let log = self.log.lock().expect("event log lock");
        Ok(Self::events_of(&log, workspace)
            .map(|events| events.len())
            .unwrap_or(0))
    }
}

/// The scope an event was recorded in, for tests and adapters that want it
/// without the trait import.
pub fn scope_of_event(event: &IntelligenceEvent) -> &AnalysisScope {
    &event.scope
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::intelligence_log::event::EventTime;
    use crate::domain::intelligence_log::kind::{ActorRef, EventKinds};
    use crate::domain::intelligence_log::payload::ContentDigest;
    use crate::domain::kernel_ids::SnapshotId;

    fn ws(name: &str) -> WorkspaceId {
        WorkspaceId::try_new(name).unwrap()
    }

    fn scope_of(name: &str, snapshot: u64) -> AnalysisScope {
        AnalysisScope::new(ws(name), SnapshotId::new(snapshot))
    }

    fn scope(snapshot: u64) -> AnalysisScope {
        scope_of("ws-a", snapshot)
    }

    fn correlation() -> CorrelationId {
        CorrelationId::new("c1").unwrap()
    }

    fn event_in(
        name: &str,
        kind: std::string::String,
        caused_by: Option<EventId>,
    ) -> NewIntelligenceEvent {
        NewIntelligenceEvent::new(
            scope_of(name, 1),
            ActorRef::kernel(),
            correlation(),
            caused_by,
            crate::domain::intelligence_log::kind::EventKind::new(kind).unwrap(),
            "happened",
            EventTime::from_millis(1),
        )
        .unwrap()
    }

    fn event(kind: std::string::String, caused_by: Option<EventId>) -> NewIntelligenceEvent {
        event_in("ws-a", kind, caused_by)
    }

    fn chain_event(caused_by: Option<EventId>) -> NewIntelligenceEvent {
        event("kernel.fact_batch_committed".to_string(), caused_by)
    }

    #[tokio::test]
    async fn append_allocates_sequential_ids_from_one_global_sequence() {
        let log = InMemoryEventLog::new();
        let a = log
            .append(&ws("ws-a"), vec![chain_event(None), chain_event(None)])
            .await
            .unwrap();
        assert_eq!(a, vec![EventId::new(1), EventId::new(2)]);

        let mut b_event = chain_event(None);
        b_event.scope = scope_of("ws-b", 1);
        let b = log.append(&ws("ws-b"), vec![b_event]).await.unwrap();
        assert_eq!(
            b,
            vec![EventId::new(3)],
            "one sequence, so an id identifies exactly one workspace"
        );
        assert_eq!(log.len(&ws("ws-a")).await.unwrap(), 2);
        assert_eq!(log.len(&ws("ws-b")).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn a_causal_chain_reads_root_first() {
        let log = InMemoryEventLog::new();
        let ids = log
            .append(
                &ws("ws-a"),
                vec![
                    event("kernel.source_delta".to_string(), None),
                    chain_event(Some(EventId::new(1))),
                    event("analysis.completed".to_string(), Some(EventId::new(2))),
                    event("finding.produced".to_string(), Some(EventId::new(3))),
                ],
            )
            .await
            .unwrap();
        assert_eq!(ids.len(), 4);

        let chain = log
            .causal_chain(&ws("ws-a"), EventId::new(4))
            .await
            .unwrap();
        let kinds: Vec<&str> = chain.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "kernel.source_delta",
                "kernel.fact_batch_committed",
                "analysis.completed",
                "finding.produced"
            ],
            "the chain must read in the order things happened"
        );
        // A root's chain is itself.
        assert_eq!(
            log.causal_chain(&ws("ws-a"), EventId::new(1))
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn an_unknown_cause_is_refused_and_nothing_is_written() {
        let log = InMemoryEventLog::new();
        let err = log
            .append(
                &ws("ws-a"),
                vec![chain_event(None), chain_event(Some(EventId::new(99)))],
            )
            .await
            .unwrap_err();
        assert_eq!(err, EventStoreError::UnknownCause(EventId::new(99)));
        assert_eq!(
            log.len(&ws("ws-a")).await.unwrap(),
            0,
            "a rejected batch must leave the log untouched"
        );
    }

    #[tokio::test]
    async fn a_cause_from_another_workspace_is_refused() {
        let log = InMemoryEventLog::new();
        let mut in_b = chain_event(None);
        in_b.scope = scope_of("ws-b", 1);
        log.append(&ws("ws-b"), vec![in_b]).await.unwrap();
        let err = log
            .append(&ws("ws-a"), vec![chain_event(Some(EventId::new(1)))])
            .await
            .unwrap_err();
        assert_eq!(
            err,
            EventStoreError::CauseInAnotherWorkspace {
                cause: EventId::new(1),
                workspace: ws("ws-a"),
            },
            "one tenant's history must be unreachable from another's"
        );
        assert_eq!(log.len(&ws("ws-a")).await.unwrap(), 0);
        assert_eq!(
            log.causal_chain(&ws("ws-a"), EventId::new(1))
                .await
                .unwrap(),
            Vec::new(),
            "and the foreign event is not visible through a chain either"
        );
    }

    /// An event whose scope names B must not be recorded as A's history, even
    /// though the store could happily file it under A.
    #[tokio::test]
    async fn an_event_may_not_claim_another_workspaces_scope() {
        let log = InMemoryEventLog::new();
        let mut foreign = chain_event(None);
        foreign.scope = scope_of("ws-b", 1);

        let err = log.append(&ws("ws-a"), vec![foreign]).await.unwrap_err();
        assert_eq!(
            err,
            EventStoreError::ScopeWorkspaceMismatch {
                append_workspace: ws("ws-a"),
                event_workspace: ws("ws-b"),
            }
        );
        assert_eq!(
            log.len(&ws("ws-a")).await.unwrap(),
            0,
            "and nothing is written"
        );

        // The same shape is fine when it is appended to its own workspace.
        let mut own = chain_event(None);
        own.scope = scope_of("ws-b", 1);
        assert!(log.append(&ws("ws-b"), vec![own]).await.is_ok());
    }

    /// A scope pinned to the invalid sentinel is refused before anything is
    /// written, however the event was constructed.
    #[tokio::test]
    async fn an_unpinned_scope_is_refused() {
        let log = InMemoryEventLog::new();
        let mut bad = chain_event(None);
        bad.scope = AnalysisScope::new(ws("ws-a"), SnapshotId::NONE);
        assert_eq!(
            log.append(&ws("ws-a"), vec![bad]).await.unwrap_err(),
            EventStoreError::Invalid(crate::domain::intelligence_log::EventError::InvalidScope)
        );
        assert_eq!(log.len(&ws("ws-a")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn correlation_groups_one_operation_in_append_order() {
        let log = InMemoryEventLog::new();
        let other = CorrelationId::new("c2").unwrap();
        let mut unrelated = chain_event(None);
        unrelated.correlation = other.clone();
        log.append(
            &ws("ws-a"),
            vec![
                event("kernel.source_delta".to_string(), None),
                unrelated,
                chain_event(None),
            ],
        )
        .await
        .unwrap();

        let mine = log
            .by_correlation(&ws("ws-a"), &correlation())
            .await
            .unwrap();
        assert_eq!(mine.len(), 2);
        assert!(mine.iter().all(|e| e.correlation == correlation()));
        assert_eq!(
            log.by_correlation(&ws("ws-a"), &other).await.unwrap().len(),
            1
        );
        assert_eq!(
            log.by_correlation(&ws("ws-b"), &correlation())
                .await
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn replay_is_deterministic_and_incremental() {
        let log = InMemoryEventLog::new();
        log.append(
            &ws("ws-a"),
            vec![
                event("kernel.source_delta".to_string(), None),
                chain_event(Some(EventId::new(1))),
                event("analysis.completed".to_string(), Some(EventId::new(2))),
            ],
        )
        .await
        .unwrap();

        let first = log.replay(&ws("ws-a"), None).await.unwrap();
        let second = log.replay(&ws("ws-a"), None).await.unwrap();
        assert_eq!(first, second, "replay must be reproducible");
        assert_eq!(first.len(), 3);

        let tail = log
            .replay(&ws("ws-a"), Some(EventId::new(1)))
            .await
            .unwrap();
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].id, EventId::new(2));

        assert_eq!(
            log.replay(&ws("ws-a"), Some(EventId::new(99)))
                .await
                .unwrap_err(),
            EventStoreError::UnknownCause(EventId::new(99))
        );
    }

    #[tokio::test]
    async fn append_is_all_or_nothing_across_a_whole_batch() {
        let log = InMemoryEventLog::new();
        let mut bad = chain_event(Some(EventId::new(50)));
        bad.kind = EventKinds::finding_produced();
        let err = log
            .append(&ws("ws-a"), vec![chain_event(None), bad])
            .await
            .unwrap_err();
        assert_eq!(err, EventStoreError::UnknownCause(EventId::new(50)));
        assert_eq!(log.len(&ws("ws-a")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn an_event_is_immutable_once_appended() {
        let log = InMemoryEventLog::new();
        log.append(&ws("ws-a"), vec![chain_event(None)])
            .await
            .unwrap();
        let first = log.by_id(&ws("ws-a"), EventId::new(1)).await.unwrap();
        // Appending more does not change what an existing id resolves to.
        log.append(&ws("ws-a"), vec![chain_event(Some(EventId::new(1)))])
            .await
            .unwrap();
        let second = log.by_id(&ws("ws-a"), EventId::new(1)).await.unwrap();
        assert_eq!(first, second);
        assert!(
            log.by_id(&ws("ws-a"), EventId::new(99))
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn a_bounded_payload_and_an_artifact_reference_both_round_trip() {
        let log = InMemoryEventLog::new();
        let digest = ContentDigest::of(&[7u8; 4096]);
        let mut big = chain_event(None);
        big.payload = crate::domain::intelligence_log::payload::EventPayloadRef::artifact(
            digest.clone(),
            "application/json",
            4096,
        )
        .unwrap();
        log.append(&ws("ws-a"), vec![big]).await.unwrap();

        let stored = log
            .by_id(&ws("ws-a"), EventId::new(1))
            .await
            .unwrap()
            .unwrap();
        assert!(!stored.payload.is_inline());
        assert_eq!(stored.payload.digest(), Some(&digest));
    }

    #[tokio::test]
    async fn scope_of_reports_where_an_event_was_recorded() {
        let log = InMemoryEventLog::new();
        log.append(&ws("ws-a"), vec![chain_event(None)])
            .await
            .unwrap();
        let scope = log
            .scope_of(&ws("ws-a"), EventId::new(1))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(scope.snapshot, SnapshotId::new(1));
        assert_eq!(
            scope_of_event(
                &log.by_id(&ws("ws-a"), EventId::new(1))
                    .await
                    .unwrap()
                    .unwrap()
            )
            .workspace,
            ws("ws-a")
        );
    }
}
