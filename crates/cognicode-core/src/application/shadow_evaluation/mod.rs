//! Analyzer shadow evaluation and failure-regime diagnosis (e82 — M13 tasks 13.3/13.4).
//!
//! ```text
//!                    frozen HistoricalReplayPlan
//!                             |
//!                +------------+------------+
//!                |                         |
//!                v                         v
//!         AnalyzerCurrent           AnalyzerCandidate
//!                |                         |
//!                v                         v
//!           ReplayReport               ReplayReport
//!                |                         |
//!                +------------+------------+
//!                             |
//!                             v
//!                    ShadowComparison
//!                             |
//!                +------------+------------+
//!                |                         |
//!                v                         v
//!            raw deltas             FailureRegimes
//!                |
//!                v
//!         measurement only
//! ```
//!
//! ## The load-bearing invariant
//!
//! ```text
//! candidate improves something  !=  candidate may be promoted
//! ```
//!
//! e82 is **measurement + diagnosis**. It is not selection, promotion, or
//! authority. It never decides that a candidate is better, never invokes the
//! FixAgent, and never issues a `PromotionAuthorization` or `PromotionPermit`.
//! A module audit enforces that (see `shadow_evaluation_tests.rs`).
//!
//! ## Composition, not duplication
//!
//! e81 owns historical replay; e82 composes it. The replay engine is reused
//! unchanged through [`HistoricalReplayPlan::run_role`], and both analyzer sides
//! reuse the single [`HistoricalPredictor`] contract. `Current` vs `Candidate`
//! is an evaluation **role**, not a different analyzer type.
//!
//! ## Future-proofing for e83
//!
//! [`ShadowEvaluator::evaluate_role`] runs one role, so e83 can sequence
//! `OPTIMIZE -> freeze candidate -> CONFIRM`. [`ShadowEvaluationPlan::evaluation_digest`]
//! binds corpus + split + both revisions so e83 can prove that the candidate
//! which passed CONFIRM is exactly the candidate being promoted.
//!
//! e82 does NOT guarantee that nobody inspected CONFIRM while developing a
//! candidate: e83 establishes candidate freeze and held-out promotion
//! discipline.

pub mod analyzer;
pub mod classifiers;
pub mod compare;
pub mod plan;
pub mod regime;
pub mod report;

#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "shadow_evaluation_tests.rs"]
mod shadow_evaluation_tests;

pub use analyzer::{AnalyzerDescriptor, AnalyzerError, AnalyzerSide, AnalyzerUnderTest};
pub use classifiers::BuiltinFailureRegimeClassifier;
pub use compare::{
    ScoreDelta, ShadowAggregateDelta, ShadowCaseComparison, ShadowError, aggregate_delta,
    pair_case_results,
};
pub use plan::ShadowEvaluationPlan;
pub use regime::{
    FailureRegime, FailureRegimeClassifier, FailureRegimeContext, FailureRegimeEvidence,
    FailureRegimeOccurrence, FailureRegimeSubject, NoFailureRegimes,
};
pub use report::{ShadowEvaluation, ShadowEvaluator, ShadowRoleReport};
