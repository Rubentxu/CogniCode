//! Shadow role reports and the evaluator (e82 — M13 task 13.4).
//!
//! ## Measurement plus diagnosis, never selection
//!
//! [`ShadowRoleReport`] carries deltas and classified regimes. It deliberately
//! has no `promotion_status`, `candidate_selected`, `candidate_accepted`, or
//! `recommendation` field. The load-bearing invariant is:
//!
//! ```text
//! candidate improves something  !=  candidate may be promoted
//! ```
//!
//! ## Role isolation
//!
//! [`ShadowEvaluator::evaluate_role`] runs exactly one dataset role, so e83 can
//! later sequence `OPTIMIZE -> freeze candidate -> CONFIRM`. [`ShadowEvaluation`]
//! keeps the two role reports as separate first-class values; there is no
//! blended report.
//!
//! ## Execution isolation
//!
//! Each side is run by its own `run_role` call, so both predictors receive
//! independently constructed base-side input. Neither side is ever fed the
//! other's prediction, score, or the recorded outcome.

use crate::application::historical_replay::plan::HistoricalReplayPlan;
use crate::application::historical_replay::report::ReplayReport;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::shadow_evaluation::analyzer::AnalyzerDescriptor;
use crate::application::shadow_evaluation::analyzer::AnalyzerUnderTest;
use crate::application::shadow_evaluation::compare::ShadowAggregateDelta;
use crate::application::shadow_evaluation::compare::ShadowCaseComparison;
use crate::application::shadow_evaluation::compare::ShadowError;
use crate::application::shadow_evaluation::compare::aggregate_delta;
use crate::application::shadow_evaluation::compare::pair_case_results;
use crate::application::shadow_evaluation::plan::ShadowEvaluationPlan;
use crate::application::shadow_evaluation::regime::FailureRegime;
use crate::application::shadow_evaluation::regime::FailureRegimeClassifier;
use crate::application::shadow_evaluation::regime::FailureRegimeContext;
use crate::application::shadow_evaluation::regime::FailureRegimeOccurrence;

/// One dataset role's shadow evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowRoleReport {
    role: DatasetRole,
    current_descriptor: AnalyzerDescriptor,
    candidate_descriptor: AnalyzerDescriptor,
    current_report: ReplayReport,
    candidate_report: ReplayReport,
    case_comparisons: Vec<ShadowCaseComparison>,
    aggregate_delta: ShadowAggregateDelta,
    failure_regimes: Vec<FailureRegimeOccurrence>,
    evaluation_digest: String,
}

impl ShadowRoleReport {
    /// The dataset role measured.
    pub fn role(&self) -> DatasetRole {
        self.role
    }

    /// The current-side analyzer identity.
    pub fn current_descriptor(&self) -> &AnalyzerDescriptor {
        &self.current_descriptor
    }

    /// The candidate-side analyzer identity.
    pub fn candidate_descriptor(&self) -> &AnalyzerDescriptor {
        &self.candidate_descriptor
    }

    /// The current-side replay report.
    pub fn current_report(&self) -> &ReplayReport {
        &self.current_report
    }

    /// The candidate-side replay report.
    pub fn candidate_report(&self) -> &ReplayReport {
        &self.candidate_report
    }

    /// The per-case comparisons, in canonical case-id order.
    pub fn case_comparisons(&self) -> &[ShadowCaseComparison] {
        &self.case_comparisons
    }

    /// The aggregate descriptive delta.
    pub fn aggregate_delta(&self) -> &ShadowAggregateDelta {
        &self.aggregate_delta
    }

    /// The classified failure regimes.
    pub fn failure_regimes(&self) -> &[FailureRegimeOccurrence] {
        &self.failure_regimes
    }

    /// Occurrences of one regime.
    pub fn regimes_of(&self, regime: FailureRegime) -> Vec<&FailureRegimeOccurrence> {
        self.failure_regimes
            .iter()
            .filter(|occurrence| occurrence.regime == regime)
            .collect()
    }

    /// The evaluation digest binding corpus + split + both revisions.
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }
}

/// Both roles' shadow reports, kept distinct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowEvaluation {
    optimize: ShadowRoleReport,
    confirm: ShadowRoleReport,
}

impl ShadowEvaluation {
    /// Build the pair.
    pub(crate) fn new(optimize: ShadowRoleReport, confirm: ShadowRoleReport) -> Self {
        Self { optimize, confirm }
    }

    /// The OPTIMIZE report.
    pub fn optimize(&self) -> &ShadowRoleReport {
        &self.optimize
    }

    /// The CONFIRM report.
    pub fn confirm(&self) -> &ShadowRoleReport {
        &self.confirm
    }

    /// The report for a role.
    pub fn report_for(&self, role: DatasetRole) -> &ShadowRoleReport {
        match role {
            DatasetRole::Optimize => &self.optimize,
            DatasetRole::Confirm => &self.confirm,
        }
    }
}

/// Runs shadow evaluations over a frozen plan.
pub struct ShadowEvaluator;

impl ShadowEvaluator {
    /// Evaluate exactly one dataset role.
    pub fn evaluate_role(
        plan: &ShadowEvaluationPlan,
        role: DatasetRole,
        current: AnalyzerUnderTest<'_>,
        candidate: AnalyzerUnderTest<'_>,
        classifier: &dyn FailureRegimeClassifier,
    ) -> Result<ShadowRoleReport, ShadowError> {
        let replay: &HistoricalReplayPlan = plan.replay_plan();

        // Each side runs independently, so both predictors receive freshly
        // constructed base-side input and never see the other's output.
        let current_report = replay.run_role(role, current.predictor());
        let candidate_report = replay.run_role(role, candidate.predictor());

        let comparisons = pair_case_results(&current_report, &candidate_report, role)?;
        let aggregate = aggregate_delta(&comparisons, &current_report, &candidate_report);

        let mut failure_regimes = Vec::new();
        for comparison in &comparisons {
            let case = replay
                .corpus()
                .get(&comparison.case_id)
                .expect("replay plan validated corpus membership");
            let context = FailureRegimeContext {
                case,
                role,
                current: &comparison.current,
                candidate: &comparison.candidate,
            };
            failure_regimes.extend(classifier.classify(&context));
        }

        Ok(ShadowRoleReport {
            role,
            current_descriptor: current.descriptor().clone(),
            candidate_descriptor: candidate.descriptor().clone(),
            current_report,
            candidate_report,
            case_comparisons: comparisons,
            aggregate_delta: aggregate,
            failure_regimes,
            evaluation_digest: plan.evaluation_digest().to_string(),
        })
    }

    /// Evaluate both roles, keeping the reports separate.
    ///
    /// This does NOT guarantee that nobody inspected CONFIRM while developing a
    /// candidate. e83 establishes candidate freeze plus held-out promotion
    /// discipline; e82 only provides the measurement.
    pub fn evaluate_all(
        plan: &ShadowEvaluationPlan,
        current: AnalyzerUnderTest<'_>,
        candidate: AnalyzerUnderTest<'_>,
        classifier: &dyn FailureRegimeClassifier,
    ) -> Result<ShadowEvaluation, ShadowError> {
        let optimize =
            Self::evaluate_role(plan, DatasetRole::Optimize, current, candidate, classifier)?;
        let confirm =
            Self::evaluate_role(plan, DatasetRole::Confirm, current, candidate, classifier)?;
        Ok(ShadowEvaluation::new(optimize, confirm))
    }
}
