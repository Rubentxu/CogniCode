//! Behaviors — governed, auditable executions (M7.3, cycle e64, ADR-044).
//!
//! ```text
//! BehaviorDefinition          what the author declares
//!         │
//!         ▼
//! BehaviorAdmission           the single trust boundary
//!         │  AiGenerated/Imported ⇒ AgentBehavior (a claim never escalates)
//!         ▼
//! BehaviorPermit              sealed: private fields, no serde, no public ctor
//!         │
//!         ▼
//! BehaviorRuntime             authorizes every effect before any adapter
//!         │
//!         ├─ accepted ──► BehaviorEffectSink (the adapter)
//!         └─ refused  ──► policy.behavior_output_rejected event
//! ```
//!
//! The class a behavior runs under is decided by admission, never by the
//! behavior and never by the actor's kind. An AI-proposed behavior that declares
//! `PureDerivation` runs as `AgentBehavior` and therefore cannot commit canonical
//! facts: a declaration never escalates, exactly as an AI detector is a
//! `Candidate`.
//!
//! Because a behavior may only *ask*, the M6 seam into findings is untouched: an
//! `ExecuteAdmittedAnalysis` request cannot mint an `ExecutionPermit`, so the
//! only route to a `Finding` is still `DetectorExecutor → FindingAssembler →
//! FindingVerifier`.
//!
//! Pure domain: the event log and the effect sink are ports.

pub mod admission;
pub mod class;
pub mod runtime;

pub use admission::{
    AdmittedBehavior, BehaviorAdmission, BehaviorAdmissionError, BehaviorDefinition, BehaviorId,
    BehaviorPermit,
};
pub use class::{BehaviorAuthorityPolicy, BehaviorClass, BehaviorEffectKind};
pub use runtime::{
    Behavior, BehaviorEffect, BehaviorEffectSink, BehaviorOutcome, BehaviorRuntime,
    BehaviorRuntimeError, FactDraft, PolicyViolation,
};
