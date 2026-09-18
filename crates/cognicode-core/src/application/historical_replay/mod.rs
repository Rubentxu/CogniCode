//! Historical replay with an immutable OPTIMIZE/CONFIRM split (e81 — M13).
//!
//! ```text
//! historical cases
//!         ↓
//! immutable corpus
//!         ↓
//! immutable split
//!
//!      ┌───────────┴───────────┐
//!      ↓                       ↓
//!   OPTIMIZE                 CONFIRM
//!   visible for iteration     held-out dataset
//! ```
//!
//! ## The invariant
//!
//! ```text
//! OPTIMIZE ∩ CONFIRM = ∅
//! ```
//!
//! An overlapping (or unknown-id) configuration is a **configuration error**
//! rejected before any predictor can run, so it produces **zero executions**.
//! The assignment is fixed before evaluation begins and cannot change during it.
//!
//! ## What e81 is, and is not
//!
//! e81 is evidence/measurement infrastructure. It replays one analyzer against
//! historical cases and reports counts per role. It does NOT compare a current
//! analyzer with a candidate (e82), does not classify failures (e82), does not
//! promote anything (e83), and never decides what is "good enough".
//!
//! The authority boundary is absolute: production code in this module does not
//! import any promotion, approval, or fix-agent surface, and grants zero
//! authority. See `historical_replay_tests.rs` for the audit that enforces it.
//!
//! ## Reuse of e76
//!
//! The e76 self-hosting foundation is reused unchanged:
//! [`SealedPrediction`](crate::application::self_hosting::prediction::SealedPrediction),
//! [`Observation`](crate::application::self_hosting::prediction::Observation),
//! [`ScoreMatrix`](crate::application::self_hosting::prediction::ScoreMatrix),
//! `score`, and
//! [`HistoricalReplay`](crate::application::self_hosting::platform_equivalence::HistoricalReplay).
//! `HistoricalReplay` stays an observation container; it does not own case
//! identity or dataset policy.

pub mod case;
pub mod corpus;
pub mod plan;
pub mod replay;
pub mod report;
pub mod split;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "historical_replay_tests.rs"]
mod historical_replay_tests;

pub use case::{
    ContentRef, HistoricalCase, HistoricalCaseError, HistoricalCaseId, HistoricalPredictionInput,
};
pub use corpus::{CorpusError, HistoricalCorpus};
pub use plan::{HistoricalReplayPlan, PlanError};
pub use replay::{HistoricalPredictor, ReplayCaseOutcome, ReplayCaseResult, ReplayIncomplete};
pub use report::{ReplayReport, ReplayReports};
pub use split::{DatasetRole, DatasetSplit, SplitError};
