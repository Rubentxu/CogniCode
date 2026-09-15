//! The minimal behavior runtime (M7.3, cycle e64, ADR-044).
//!
//! ## What this is
//!
//! ```text
//! permit ──► start execution ──► produce/attempt effect
//!                                      │
//!                              authorize (policy table)
//!                                      │
//!                        accept ──► adapter ;  reject ──► policy event
//! ```
//!
//! ## What this deliberately is not
//!
//! No subscription matching, no scheduler, no retry, no backoff, no workers, no
//! parallel behaviors, no durable queue, no transport. None of it is needed to
//! answer the question this cut exists to answer — *is the authority boundary
//! real?* — and building it first would mean building it on an unproven boundary.
//!
//! ## The shape of the guarantee
//!
//! The runtime accepts **only** a [`BehaviorPermit`], never a definition, and it
//! authorizes every effect with [`BehaviorAuthorityPolicy`] against the
//! *effective* class before the effect sink is reached. The rejection is
//! therefore structural: there is no branch in which an unauthorized effect is
//! handed to an adapter and undone afterwards.
//!
//! ## Two records of every run
//!
//! `behavior.started` is caused by the execution's `trigger_event` — the event
//! that originated the execution, not the previous log entry — and the
//! completion or the rejection is caused by the start. That is what makes
//! `causal_chain(rejection)` read as
//! `[trigger, behavior.started, policy.behavior_output_rejected]`.
//!
//! Pure domain: the log and the effect sink are ports.

use super::admission::BehaviorPermit;
use super::class::{BehaviorAuthorityPolicy, BehaviorClass, BehaviorEffectKind};
use crate::domain::execution::ExecutionContext;
use crate::domain::intelligence_log::event::{EventTime, IntelligenceEvent, NewIntelligenceEvent};
use crate::domain::intelligence_log::kind::{ActorRef, EventKind, EventKinds};
use crate::domain::intelligence_log::payload::{
    BoundedEventPayload, EventPayloadRef, PayloadError,
};
use crate::domain::intelligence_log::store::{EventStoreError, IntelligenceEventStore};
use crate::domain::kernel_ids::{EntityId, EventId, SnapshotId};

/// Why a behavior run failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorRuntimeError {
    /// The event log refused an append.
    Log(EventStoreError),
    /// A payload could not be built.
    Payload(PayloadError),
    /// The effect sink refused an authorized effect.
    Sink(String),
}

impl std::fmt::Display for BehaviorRuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Log(err) => write!(f, "behavior event log: {err}"),
            Self::Payload(err) => write!(f, "behavior event payload: {err}"),
            Self::Sink(msg) => write!(f, "effect sink refused an authorized effect: {msg}"),
        }
    }
}

impl std::error::Error for BehaviorRuntimeError {}

/// A fact a behavior *proposes* to commit.
///
/// Deliberately **not** a kernel `Fact`: a behavior does not own canonical
/// truth, it asks for it. Whether a draft ever becomes a fact is decided by the
/// policy above and by the kernel's own rules below, and the draft carrying only
/// what is needed to make that decision is what keeps the behavior domain free
/// of the gated kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactDraft {
    /// The entity the assertion is about.
    pub subject: EntityId,
    /// The namespaced predicate.
    pub predicate: String,
    /// A short, bounded description of the object.
    pub object: String,
    /// The snapshot the assertion would belong to.
    pub snapshot: SnapshotId,
}

impl FactDraft {
    /// Construct a draft.
    pub fn new(
        subject: EntityId,
        predicate: impl Into<String>,
        object: impl Into<String>,
        snapshot: SnapshotId,
    ) -> Self {
        Self {
            subject,
            predicate: predicate.into(),
            object: object.into(),
            snapshot,
        }
    }
}

/// Something a behavior asks the platform to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BehaviorEffect {
    /// Commit a fact into canonical truth.
    CommitCanonicalFact(FactDraft),
    /// Record evidence.
    RecordEvidence {
        /// What the evidence is about.
        summary: String,
    },
    /// Record a hypothesis (explicitly not canonical truth).
    RecordHypothesis {
        /// What is hypothesized.
        summary: String,
    },
    /// Propose a change for someone else to accept.
    ProposeChange {
        /// What is proposed.
        summary: String,
    },
    /// Ask the platform to run an analysis that admission already permitted.
    ///
    /// Carries no authority: the detector id here is a *request*, and the
    /// `ExecutionPermit` that would let it run cannot be minted by a behavior.
    ExecuteAdmittedAnalysis {
        /// The detector to run.
        detector_id: String,
    },
}

impl BehaviorEffect {
    /// The effect kind, for authorization and diagnostics.
    pub fn kind(&self) -> BehaviorEffectKind {
        match self {
            Self::CommitCanonicalFact(_) => BehaviorEffectKind::CommitCanonicalFact,
            Self::RecordEvidence { .. } => BehaviorEffectKind::RecordEvidence,
            Self::RecordHypothesis { .. } => BehaviorEffectKind::RecordHypothesis,
            Self::ProposeChange { .. } => BehaviorEffectKind::ProposeChange,
            Self::ExecuteAdmittedAnalysis { .. } => BehaviorEffectKind::ExecuteAdmittedAnalysis,
        }
    }

    /// A bounded description for the rejection payload.
    pub fn describe(&self) -> String {
        match self {
            Self::CommitCanonicalFact(d) => {
                format!("fact draft {} about entity {}", d.predicate, d.subject)
            }
            Self::RecordEvidence { summary }
            | Self::RecordHypothesis { summary }
            | Self::ProposeChange { summary } => summary.clone(),
            Self::ExecuteAdmittedAnalysis { detector_id } => format!("run {detector_id}"),
        }
    }
}

/// A behavior's logic. Deterministic and synchronous: the runtime owns the
/// world-facing part.
pub trait Behavior {
    /// Produce the effects this execution asks for.
    fn execute(&self, context: &ExecutionContext) -> Vec<BehaviorEffect>;
}

/// Applies an **authorized** effect.
///
/// The runtime never calls this for a refused effect, so an adapter never has
/// to defend against one.
pub trait BehaviorEffectSink {
    /// Apply `effect`, or explain why it could not be applied.
    fn apply(&mut self, effect: &BehaviorEffect) -> Result<(), String>;
}

/// A refused effect, kept so the caller can report exactly what was denied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyViolation {
    /// The behavior that asked.
    pub behavior_id: String,
    /// The class it ran as.
    pub class: BehaviorClass,
    /// The effect it asked for.
    pub effect: BehaviorEffectKind,
    /// A bounded description of what was asked.
    pub detail: String,
}

impl PolicyViolation {
    /// A one-line, stable diagnostic.
    pub fn reason(&self) -> String {
        format!(
            "a {} behavior may not {}",
            self.class.name(),
            self.effect.name()
        )
    }
}

/// What a behavior run produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorOutcome {
    /// The class the run executed under (from the permit, never the claim).
    pub effective_class: BehaviorClass,
    /// The `behavior.started` event.
    pub started_event: EventId,
    /// The effects that were authorized and applied.
    pub accepted: Vec<BehaviorEffectKind>,
    /// The effects that were refused, in order.
    pub rejected: Vec<PolicyViolation>,
    /// The rejection events, aligned with [`Self::rejected`].
    pub rejection_events: Vec<EventId>,
    /// The `behavior.completed` event, if the run finished cleanly.
    pub completed_event: Option<EventId>,
}

impl BehaviorOutcome {
    /// Whether any effect was refused.
    pub fn has_violations(&self) -> bool {
        !self.rejected.is_empty()
    }
}

/// The minimal governed runtime.
pub struct BehaviorRuntime<'a> {
    log: &'a dyn IntelligenceEventStore,
    now: EventTime,
}

impl<'a> BehaviorRuntime<'a> {
    /// Construct a runtime over an event log.
    pub fn new(log: &'a dyn IntelligenceEventStore, now: EventTime) -> Self {
        Self { log, now }
    }

    /// Run an admitted behavior, authorizing every effect before the sink.
    pub async fn run(
        &self,
        permit: &BehaviorPermit,
        context: &ExecutionContext,
        behavior: &dyn Behavior,
        sink: &mut dyn BehaviorEffectSink,
    ) -> Result<BehaviorOutcome, BehaviorRuntimeError> {
        let admitted = permit.admitted();
        let class = admitted.effective_class();

        // The actor is the behavior, not the caller: the log must attribute the
        // act to whatever actually did it.
        let actor = ActorRef::behavior(admitted.id().as_str());

        let started_event = self
            .append(
                context,
                &actor,
                EventKinds::behavior_started(),
                payload(&[
                    ("behavior_id", admitted.id().as_str().to_string()),
                    ("class", class.name().to_string()),
                    (
                        "declared_class",
                        admitted.declared_class().name().to_string(),
                    ),
                    ("execution_id", context.execution_id.to_string()),
                ])?,
                context.trigger_event,
            )
            .await?;

        let mut outcome = BehaviorOutcome {
            effective_class: class,
            started_event,
            accepted: Vec::new(),
            rejected: Vec::new(),
            rejection_events: Vec::new(),
            completed_event: None,
        };

        for effect in behavior.execute(context) {
            let kind = effect.kind();
            if BehaviorAuthorityPolicy::allows(class, kind) {
                sink.apply(&effect).map_err(BehaviorRuntimeError::Sink)?;
                outcome.accepted.push(kind);
                continue;
            }

            // Refused *before* the sink: nothing for an adapter to undo.
            let violation = PolicyViolation {
                behavior_id: admitted.id().as_str().to_string(),
                class,
                effect: kind,
                detail: effect.describe(),
            };
            let event = self
                .append(
                    context,
                    &actor,
                    EventKinds::behavior_output_rejected(),
                    payload(&[
                        ("behavior_id", violation.behavior_id.clone()),
                        ("class", class.name().to_string()),
                        ("effect", kind.name().to_string()),
                        ("reason", violation.reason()),
                        ("detail", violation.detail.clone()),
                        ("execution_id", context.execution_id.to_string()),
                    ])?,
                    Some(started_event),
                )
                .await?;
            outcome.rejection_events.push(event);
            outcome.rejected.push(violation);
        }

        if !outcome.has_violations() {
            let completed = self
                .append(
                    context,
                    &actor,
                    EventKinds::behavior_completed(),
                    payload(&[
                        ("behavior_id", admitted.id().as_str().to_string()),
                        ("class", class.name().to_string()),
                        ("accepted", outcome.accepted.len().to_string()),
                        ("execution_id", context.execution_id.to_string()),
                    ])?,
                    Some(started_event),
                )
                .await?;
            outcome.completed_event = Some(completed);
        }

        Ok(outcome)
    }

    async fn append(
        &self,
        context: &ExecutionContext,
        actor: &ActorRef,
        kind: EventKind,
        payload: EventPayloadRef,
        caused_by: Option<EventId>,
    ) -> Result<EventId, BehaviorRuntimeError> {
        let new = NewIntelligenceEvent {
            scope: context.scope.clone(),
            actor: actor.clone(),
            correlation: context.correlation.clone(),
            caused_by,
            kind,
            payload,
            occurred_at: self.now,
        };
        let ids = self
            .log
            .append(&context.scope.workspace, vec![new])
            .await
            .map_err(BehaviorRuntimeError::Log)?;
        Ok(ids[0])
    }
}

/// The bounded payload of a behavior event.
fn payload(fields: &[(&str, String)]) -> Result<EventPayloadRef, BehaviorRuntimeError> {
    let mut built =
        BoundedEventPayload::new("behavior execution").map_err(BehaviorRuntimeError::Payload)?;
    for (key, value) in fields {
        built = built
            .with_field(*key, value.clone())
            .map_err(BehaviorRuntimeError::Payload)?;
    }
    Ok(EventPayloadRef::inline(built))
}

/// Read the bounded payload of a behavior event, for assertions and reporting.
pub fn event_fields(event: &IntelligenceEvent) -> std::collections::BTreeMap<String, String> {
    event
        .payload
        .inline_payload()
        .map(|p| p.fields().clone())
        .unwrap_or_default()
}
