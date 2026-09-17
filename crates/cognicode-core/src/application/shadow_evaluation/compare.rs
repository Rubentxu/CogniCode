//! Per-case pairing and descriptive deltas (e82 — M13 task 13.4).
//!
//! ## Pair by case id, never by index
//!
//! Results are paired on `(dataset role, historical case id)`. `zip(index)`
//! would silently mis-pair if either side were reordered, so ordering is never
//! trusted. Missing or duplicated results fail loud even though the shared
//! replay plan should make those states unreachable.
//!
//! ## Descriptive arithmetic only
//!
//! [`ScoreDelta`] is signed `candidate - current` per count. There is no
//! `Improved`/`Regressed`/`Better`/`Worse` encoding: e82 reports
//! `false_negative: +2`, not "the candidate regressed". Interpretation is policy
//! and belongs to a later cycle.
//!
//! ## Incomplete stays explicit
//!
//! A case that is incomplete on either side has **no** `ScoreDelta`. It is never
//! projected to zeros, because zeros would read as a perfect measurement.

use std::collections::BTreeMap;

use crate::application::historical_replay::case::HistoricalCaseId;
use crate::application::historical_replay::replay::ReplayCaseOutcome;
use crate::application::historical_replay::replay::ReplayCaseResult;
use crate::application::historical_replay::report::ReplayReport;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::self_hosting::prediction::ScoreMatrix;
use crate::application::shadow_evaluation::analyzer::AnalyzerSide;

/// Signed descriptive count delta: `candidate - current`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScoreDelta {
    /// Change in true positives.
    pub true_positive: i64,
    /// Change in false positives.
    pub false_positive: i64,
    /// Change in false negatives.
    pub false_negative: i64,
    /// Change in true negatives.
    pub true_negative: i64,
    /// Change in unknowns.
    pub unknown: i64,
}

impl ScoreDelta {
    /// The additive identity.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Signed difference between two score matrices.
    pub fn between_matrices(current: &ScoreMatrix, candidate: &ScoreMatrix) -> Self {
        Self {
            true_positive: candidate.true_positive as i64 - current.true_positive as i64,
            false_positive: candidate.false_positive as i64 - current.false_positive as i64,
            false_negative: candidate.false_negative as i64 - current.false_negative as i64,
            true_negative: candidate.true_negative as i64 - current.true_negative as i64,
            unknown: candidate.unknown as i64 - current.unknown as i64,
        }
    }

    /// Whether every component is zero.
    pub fn is_zero(&self) -> bool {
        self.true_positive == 0
            && self.false_positive == 0
            && self.false_negative == 0
            && self.true_negative == 0
            && self.unknown == 0
    }

    /// Accumulate another delta into this one.
    fn accumulate(&mut self, other: &Self) {
        self.true_positive += other.true_positive;
        self.false_positive += other.false_positive;
        self.false_negative += other.false_negative;
        self.true_negative += other.true_negative;
        self.unknown += other.unknown;
    }
}

/// One case compared across both sides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowCaseComparison {
    /// Which case.
    pub case_id: HistoricalCaseId,
    /// Which dataset side it belonged to.
    pub role: DatasetRole,
    /// The current-side outcome.
    pub current: ReplayCaseOutcome,
    /// The candidate-side outcome.
    pub candidate: ReplayCaseOutcome,
    /// The signed score delta, present only when **both** sides scored the case.
    pub score_delta: Option<ScoreDelta>,
}

impl ShadowCaseComparison {
    /// Whether the case was scoreable on both sides.
    pub fn is_fully_scored(&self) -> bool {
        self.score_delta.is_some()
    }
}

/// Aggregate delta over a role's comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShadowAggregateDelta {
    /// Summed score delta over cases scored on both sides.
    pub score: ScoreDelta,
    /// Change in the number of scored cases.
    pub scored_cases: i64,
    /// Change in the number of incomplete cases.
    pub incomplete_cases: i64,
}

/// Pair two role reports by case id.
///
/// Fails loud on a role mismatch, a duplicate case result, or a case present on
/// only one side.
pub fn pair_case_results(
    current: &ReplayReport,
    candidate: &ReplayReport,
    role: DatasetRole,
) -> Result<Vec<ShadowCaseComparison>, ShadowError> {
    if current.role() != role {
        return Err(ShadowError::RoleMismatch {
            expected: role,
            actual: current.role(),
            side: AnalyzerSide::Current,
        });
    }
    if candidate.role() != role {
        return Err(ShadowError::RoleMismatch {
            expected: role,
            actual: candidate.role(),
            side: AnalyzerSide::Candidate,
        });
    }

    let current_index = index_by_case_id(current, role, AnalyzerSide::Current)?;
    let candidate_index = index_by_case_id(candidate, role, AnalyzerSide::Candidate)?;

    let mut comparisons = Vec::with_capacity(current.cases().len());
    for case in current.cases() {
        let other = candidate_index.get(&case.case_id).ok_or_else(|| {
            ShadowError::MissingCase {
                role,
                side: AnalyzerSide::Candidate,
                case_id: case.case_id.clone(),
            }
        })?;
        comparisons.push(build_comparison(case.case_id.clone(), role, &case.outcome, &other.outcome));
    }
    for case in candidate.cases() {
        if !current_index.contains_key(&case.case_id) {
            return Err(ShadowError::MissingCase {
                role,
                side: AnalyzerSide::Current,
                case_id: case.case_id.clone(),
            });
        }
    }
    Ok(comparisons)
}

fn build_comparison(
    case_id: HistoricalCaseId,
    role: DatasetRole,
    current: &ReplayCaseOutcome,
    candidate: &ReplayCaseOutcome,
) -> ShadowCaseComparison {
    let score_delta = match (current, candidate) {
        (ReplayCaseOutcome::Scored(c), ReplayCaseOutcome::Scored(k)) => {
            Some(ScoreDelta::between_matrices(c, k))
        }
        // An incomplete side has no measurement; never project it to zeros.
        _ => None,
    };
    ShadowCaseComparison {
        case_id,
        role,
        current: current.clone(),
        candidate: candidate.clone(),
        score_delta,
    }
}

fn index_by_case_id(
    report: &ReplayReport,
    role: DatasetRole,
    side: AnalyzerSide,
) -> Result<BTreeMap<&HistoricalCaseId, &ReplayCaseResult>, ShadowError> {
    let mut index = BTreeMap::new();
    for case in report.cases() {
        if index.insert(&case.case_id, case).is_some() {
            return Err(ShadowError::DuplicateCaseResult {
                role,
                side,
                case_id: case.case_id.clone(),
            });
        }
    }
    Ok(index)
}

/// Aggregate a role's comparisons plus the report-level case counts.
pub fn aggregate_delta(
    comparisons: &[ShadowCaseComparison],
    current: &ReplayReport,
    candidate: &ReplayReport,
) -> ShadowAggregateDelta {
    let mut aggregate = ShadowAggregateDelta {
        score: ScoreDelta::zero(),
        scored_cases: candidate.scored_cases() as i64 - current.scored_cases() as i64,
        incomplete_cases: candidate.incomplete_cases() as i64 - current.incomplete_cases() as i64,
    };
    for comparison in comparisons {
        if let Some(delta) = &comparison.score_delta {
            aggregate.score.accumulate(delta);
        }
    }
    aggregate
}

/// Why a shadow comparison could not be performed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowError {
    /// A report's role did not match the requested role.
    RoleMismatch {
        /// The requested role.
        expected: DatasetRole,
        /// The role found on the report.
        actual: DatasetRole,
        /// Which side was wrong.
        side: AnalyzerSide,
    },
    /// A case was present on one side only.
    MissingCase {
        /// The dataset role being compared.
        role: DatasetRole,
        /// The side that lacks the case.
        side: AnalyzerSide,
        /// The case.
        case_id: HistoricalCaseId,
    },
    /// The same case appeared twice in one report.
    DuplicateCaseResult {
        /// The dataset role being compared.
        role: DatasetRole,
        /// The side with the duplicate.
        side: AnalyzerSide,
        /// The duplicated case.
        case_id: HistoricalCaseId,
    },
}

impl std::fmt::Display for ShadowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RoleMismatch {
                expected,
                actual,
                side,
            } => write!(
                f,
                "{side:?} report role mismatch: expected {}, found {}",
                expected.name(),
                actual.name()
            ),
            Self::MissingCase {
                role,
                side,
                case_id,
            } => write!(
                f,
                "case {case_id} is missing on the {} side of the {} comparison",
                side.name(),
                role.name()
            ),
            Self::DuplicateCaseResult {
                role,
                side,
                case_id,
            } => write!(
                f,
                "case {case_id} appears twice on the {} side of the {} comparison",
                side.name(),
                role.name()
            ),
        }
    }
}

impl std::error::Error for ShadowError {}
