//! Replay reports (e81 — M13): measurement, not verdicts.
//!
//! OPTIMIZE and CONFIRM reports are produced separately and stay separately
//! identifiable. There is deliberately no combined summary: a single blended
//! score is exactly what held-out discipline exists to prevent, and it would be
//! too easy to mistake for authority.
//!
//! e81 emits counts and rates (`TP/FP/FN/TN`, precision, recall). It does NOT
//! emit `Promote`, `RejectCandidate`, `CandidateBetter`, or any promotion
//! verdict. Those belong to e82/e83.
//!
//! ## e81 vs e83
//!
//! e81 guarantees **structural** dataset separation: the two sides are disjoint,
//! immutable, and reported independently. e83 will enforce promotion-time
//! held-out discipline (a gate that refuses a candidate whose CONFIRM side
//! regressed). e81 does not decide what is "good enough".

use crate::application::historical_replay::replay::ReplayCaseOutcome;
use crate::application::historical_replay::replay::ReplayCaseResult;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::self_hosting::prediction::ScoreMatrix;

/// One role's replay report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayReport {
    role: DatasetRole,
    cases: Vec<ReplayCaseResult>,
    aggregate: ScoreMatrix,
    scored_cases: usize,
    incomplete_cases: usize,
}

impl ReplayReport {
    /// Build a report from case results, accumulating only scored cases.
    pub(crate) fn from_cases(role: DatasetRole, cases: Vec<ReplayCaseResult>) -> Self {
        let mut aggregate = ScoreMatrix::default();
        let mut scored_cases = 0usize;
        let mut incomplete_cases = 0usize;
        for case in &cases {
            match &case.outcome {
                ReplayCaseOutcome::Scored(matrix) => {
                    accumulate(&mut aggregate, matrix);
                    scored_cases += 1;
                }
                ReplayCaseOutcome::Incomplete(_) => {
                    incomplete_cases += 1;
                }
            }
        }
        Self {
            role,
            cases,
            aggregate,
            scored_cases,
            incomplete_cases,
        }
    }

    /// The role this report measures.
    pub fn role(&self) -> DatasetRole {
        self.role
    }

    /// The per-case results.
    pub fn cases(&self) -> &[ReplayCaseResult] {
        &self.cases
    }

    /// The summed score matrix over scored cases only.
    ///
    /// Incomplete cases are excluded from the counts and reported separately;
    /// they are never folded into the `unknown` bucket.
    pub fn aggregate(&self) -> &ScoreMatrix {
        &self.aggregate
    }

    /// How many cases produced a score.
    pub fn scored_cases(&self) -> usize {
        self.scored_cases
    }

    /// How many cases could not be scored.
    pub fn incomplete_cases(&self) -> usize {
        self.incomplete_cases
    }

    /// Precision over the scored aggregate.
    pub fn precision(&self) -> Option<f64> {
        self.aggregate.precision()
    }

    /// Recall over the scored aggregate.
    pub fn recall(&self) -> Option<f64> {
        self.aggregate.recall()
    }
}

/// Both roles' reports, kept distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayReports {
    optimize: ReplayReport,
    confirm: ReplayReport,
}

impl ReplayReports {
    /// Build the pair.
    pub(crate) fn new(optimize: ReplayReport, confirm: ReplayReport) -> Self {
        Self { optimize, confirm }
    }

    /// The OPTIMIZE report. Usable independently.
    pub fn optimize(&self) -> &ReplayReport {
        &self.optimize
    }

    /// The CONFIRM report. Usable independently.
    pub fn confirm(&self) -> &ReplayReport {
        &self.confirm
    }

    /// The report for a role.
    pub fn report_for(&self, role: DatasetRole) -> &ReplayReport {
        match role {
            DatasetRole::Optimize => &self.optimize,
            DatasetRole::Confirm => &self.confirm,
        }
    }
}

/// Sum a case matrix into the aggregate. Counts only; no thresholds.
fn accumulate(aggregate: &mut ScoreMatrix, case: &ScoreMatrix) {
    aggregate.true_positive += case.true_positive;
    aggregate.false_positive += case.false_positive;
    aggregate.false_negative += case.false_negative;
    aggregate.true_negative += case.true_negative;
    aggregate.unknown += case.unknown;
}
