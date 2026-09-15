//! Intelligence Event Log — the causal history of what happened (M7, cycle e63).
//!
//! ## What this is, and what it is not
//!
//! ```text
//! Evidence Kernel  = truth          (what is)
//! Intelligence Log = history        (what happened)
//! Event Bus        = delivery       (how it reaches someone)  — NOT here
//! Reactive Runtime = consumer       (what acts on it)         — NOT here
//! ```
//!
//! The log is **append-only and causal**. It is deliberately not an
//! event-sourcing store for facts: a batch of a million facts produces **one**
//! event that references the batch by content digest, not a million events.
//! Canonical truth stays in the evidence kernel; the log only records that
//! something happened, who did it, why, and where the payload lives.
//!
//! ## Shape
//!
//! ```text
//! NewIntelligenceEvent ──append──► IntelligenceEvent { id, scope, actor,
//!                                                      correlation,
//!                                                      caused_by, kind,
//!                                                      payload, occurred_at }
//! ```
//!
//! - `kind` is a validated namespaced name (`kernel.fact_batch_committed`,
//!   `analysis.completed`, …), not a growing enum that every milestone would
//!   have to reopen;
//! - `caused_by` points at exactly one predecessor, so `causal_chain` is a
//!   walk, not a graph search;
//! - `correlation` groups a logical operation across actors;
//! - `payload` is either a bounded inline summary or a reference to an
//!   artifact by digest (see [`payload`]).
//!
//! ## Pure domain
//!
//! No I/O, no clock: `occurred_at` is supplied by the caller, which keeps the
//! log deterministic and therefore replayable. Persistence is a port
//! ([`IntelligenceEventStore`]).

pub mod event;
pub mod ids;
pub mod kind;
pub mod payload;
pub mod store;

pub use event::{EventError, EventTime, IntelligenceEvent, NewIntelligenceEvent};
pub use ids::CorrelationId;
pub use kind::{EventKind, EventKinds};
// Re-exported from their real home (`domain::execution`) so event-log call
// sites keep resolving.
pub use crate::domain::execution::{ActorKind, ActorRef, CorrelationId as Correlation};
pub use payload::{
    BoundedEventPayload, ContentDigest, EventPayloadRef, MAX_INLINE_PAYLOAD_BYTES, PayloadError,
};
pub use store::{EventStoreError, IntelligenceEventStore};
