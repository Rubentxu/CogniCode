//! Execution identity — the ungated vocabulary shared by every kind of run
//! (M7.2, cycle e64).
//!
//! ```text
//! AnalysisScope      which (workspace, snapshot) a run is pinned to
//! ActorRef           who ran it
//! CorrelationId      which logical operation it belongs to
//! ExecutionContext   all of the above + the event that triggered it
//! ```
//!
//! Each of these was introduced by a specific milestone (M6 findings, M7 event
//! log) and each turned out to be general: a scope is not a findings concept, an
//! actor is not an event concept. They live here so the next consumer reuses
//! them instead of writing a third variant — the same move e56 made for the
//! kernel ids, e63 for the namespaced-name grammar and e62.4 for `EvidenceGrade`.
//!
//! **What is deliberately not here:** authority. `DetectorAuthority` and
//! `BehaviorClass` say what an execution was *allowed* to do; they come from
//! admission, are carried beside the context, and are never derived from it.

pub mod actor;
pub mod context;
pub mod correlation;
pub mod scope;

pub use actor::{ActorError, ActorKind, ActorRef};
pub use context::{ExecutionContext, ExecutionContextError};
pub use correlation::{CorrelationError, CorrelationId};
pub use scope::{AnalysisScope, AnalysisScopeError};
