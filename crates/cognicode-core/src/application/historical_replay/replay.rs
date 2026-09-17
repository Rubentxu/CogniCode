//! Deterministic replay execution (e81 — M13).
//!
//! e81 replays **one** analyzer/predictor against each case. It does NOT compare
//! `AnalyzerCurrent` vs `AnalyzerCandidate`; that belongs to e82.
//!
//! ```text
//! for each HistoricalCase
//!     derive HistoricalPredictionInput from the base side only
//!     predictor.predict(input) -> SealedPrediction
//!     compare against the case's recorded observations
//!     -> ReplayCaseResult
//! ```
//!
//! ## Measurement, never authority
//!
//! Nothing here mints a `PolicyDecision`, a `PromotionPermit`, or any
//! "candidate is better" verdict. A replay produces counts.

use crate::application::historical_replay::case::HistoricalCaseId;
use crate::application::historical_replay::case::HistoricalPredictionInput;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::self_hosting::prediction::ScoreMatrix;
use crate::application::self_hosting::prediction::SealedPrediction;

/// Port: turn base-side input into a sealed prediction.
///
/// The parameter type is the whole structural defence against leakage: it has
/// no successor, observation, outcome, or score field, so a predictor cannot
/// peek at the answer even by accident.
pub trait HistoricalPredictor {
    /// Produce the prediction. The implementation seals it (see
    /// [`SealedPrediction::seal`]).
    fn predict(&self, input: &HistoricalPredictionInput) -> SealedPrediction;
}

/// Why a case could not be scored.
///
/// e76's `score()` always yields a matrix and treats a missing observation as
/// absent. That is right for a present observation set with gaps, but it would
/// let a case with *no recorded observation at all* look like a normal
/// comparison. e81 therefore represents that case explicitly instead of faking
/// an `unknown` count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayIncomplete {
    /// The historical case holds no observation, so there is nothing to score
    /// against. This is never treated as success.
    NoRecordedObservation,
}

/// What happened to one case during replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayCaseOutcome {
    /// The case was compared against its recorded observations.
    Scored(ScoreMatrix),
    /// The case could not be scored honestly.
    Incomplete(ReplayIncomplete),
}

impl ReplayCaseOutcome {
    /// Whether this case produced a score.
    pub fn is_scored(&self) -> bool {
        matches!(self, Self::Scored(_))
    }

    /// The score, when the case was scored.
    pub fn score(&self) -> Option<&ScoreMatrix> {
        match self {
            Self::Scored(m) => Some(m),
            Self::Incomplete(_) => None,
        }
    }
}

/// The result of replaying one case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCaseResult {
    /// Which case.
    pub case_id: HistoricalCaseId,
    /// Which dataset side it belonged to.
    pub role: DatasetRole,
    /// The seal digest of the prediction that was made (audit: proves which
    /// prediction was compared).
    pub prediction_seal: String,
    /// The outcome.
    pub outcome: ReplayCaseOutcome,
}
