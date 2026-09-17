//! Validated replay plan: bind a split to a corpus, freeze, then run (e81 — M13).
//!
//! ## Validation happens before anything can execute
//!
//! [`HistoricalReplayPlan::prepare`] is the only way to obtain a runnable plan.
//! It validates that every split id exists in the corpus, that the split is
//! internally valid, and that the corpus is valid. Because an overlapping or
//! unknown-id configuration cannot produce a plan, it cannot produce any
//! predictor invocation either: "configuration error => zero executions" holds
//! by construction.
//!
//! ## The plan is frozen
//!
//! The plan owns its corpus and its id vectors. Mutating the caller's original
//! vectors after preparation cannot change the plan, which is what makes role
//! assignment immutable for the run.

use crate::application::historical_replay::case::HistoricalCaseId;
use crate::application::historical_replay::corpus::HistoricalCorpus;
use crate::application::historical_replay::replay::HistoricalPredictor;
use crate::application::historical_replay::replay::ReplayCaseOutcome;
use crate::application::historical_replay::replay::ReplayCaseResult;
use crate::application::historical_replay::replay::ReplayIncomplete;
use crate::application::historical_replay::report::ReplayReport;
use crate::application::historical_replay::report::ReplayReports;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::historical_replay::split::DatasetSplit;
use crate::application::self_hosting::platform_equivalence::PlatformObservation;
use crate::application::self_hosting::prediction::Observation;
use crate::application::self_hosting::prediction::score;

/// A validated, frozen replay plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalReplayPlan {
    corpus: HistoricalCorpus,
    corpus_digest: String,
    split_digest: String,
    optimize: Vec<HistoricalCaseId>,
    confirm: Vec<HistoricalCaseId>,
}

impl HistoricalReplayPlan {
    /// Bind a split to a corpus after validating both.
    ///
    /// This is the boundary a caller must cross before any predictor runs.
    pub fn prepare(corpus: &HistoricalCorpus, split: &DatasetSplit) -> Result<Self, PlanError> {
        // Corpus invariant.
        if corpus.is_empty() {
            return Err(PlanError::EmptyCorpus);
        }

        // Split invariants, re-checked here as defence in depth: a
        // `DatasetSplit` is already constructed valid, but the plan must not
        // rely on an upstream guarantee to honour the disjointness rule.
        for id in split.optimize() {
            if split.confirm().binary_search(id).is_ok() {
                return Err(PlanError::Overlap(id.clone()));
            }
        }

        // Membership: every split id must name a real case.
        for id in split.optimize().iter().chain(split.confirm().iter()) {
            if !corpus.contains(id) {
                return Err(PlanError::UnknownCaseId(id.clone()));
            }
        }

        Ok(Self {
            corpus: corpus.clone(),
            corpus_digest: corpus.digest().to_string(),
            split_digest: split.digest().to_string(),
            optimize: split.optimize().to_vec(),
            confirm: split.confirm().to_vec(),
        })
    }

    /// The frozen corpus digest.
    pub fn corpus_digest(&self) -> &str {
        &self.corpus_digest
    }

    /// The frozen split digest.
    pub fn split_digest(&self) -> &str {
        &self.split_digest
    }

    /// The frozen OPTIMIZE ids.
    pub fn optimize_ids(&self) -> &[HistoricalCaseId] {
        &self.optimize
    }

    /// The frozen CONFIRM ids.
    pub fn confirm_ids(&self) -> &[HistoricalCaseId] {
        &self.confirm
    }

    /// The frozen corpus.
    pub fn corpus(&self) -> &HistoricalCorpus {
        &self.corpus
    }

    /// Run both roles deterministically.
    ///
    /// The same plan plus the same predictor always produces the same reports.
    /// Incomplete cases are reported as incomplete; they are never scored as if
    /// they were observed.
    ///
    /// Compatibility sugar over [`Self::run_role`].
    pub fn run(&self, predictor: &dyn HistoricalPredictor) -> ReplayReports {
        ReplayReports::new(
            self.run_role(DatasetRole::Optimize, predictor),
            self.run_role(DatasetRole::Confirm, predictor),
        )
    }

    /// Run exactly ONE role.
    ///
    /// Additive seam (e82): shadow evaluation compares a single role, and e83
    /// can later sequence `OPTIMIZE -> freeze candidate -> CONFIRM` without
    /// reworking e81. Identical semantics to the corresponding half of
    /// [`Self::run`].
    pub fn run_role(&self, role: DatasetRole, predictor: &dyn HistoricalPredictor) -> ReplayReport {
        let ids = match role {
            DatasetRole::Optimize => &self.optimize,
            DatasetRole::Confirm => &self.confirm,
        };
        self.replay_role(role, ids, predictor)
    }

    fn replay_role(
        &self,
        role: DatasetRole,
        ids: &[HistoricalCaseId],
        predictor: &dyn HistoricalPredictor,
    ) -> ReplayReport {
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            let case = self
                .corpus
                .get(id)
                .expect("prepare validated corpus membership");
            let input = case.prediction_input();
            let prediction = predictor.predict(&input);

            let observations = case.replay().observations.as_slice();
            let outcome = if observations.is_empty() {
                ReplayCaseOutcome::Incomplete(ReplayIncomplete::NoRecordedObservation)
            } else {
                let scoring: Vec<Observation> = observations
                    .iter()
                    .map(observation_from_platform)
                    .collect();
                ReplayCaseOutcome::Scored(score(&prediction, &scoring))
            };

            results.push(ReplayCaseResult {
                case_id: id.clone(),
                role,
                prediction_seal: prediction.seal_digest.clone(),
                outcome,
            });
        }
        ReplayReport::from_cases(role, results)
    }
}

/// Adapt an e76 `PlatformObservation` to the scoring `Observation`.
///
/// Mirrors `HistoricalReplay::score`'s existing convention: an observation
/// whose raw payload is empty did not fire.
fn observation_from_platform(platform: &PlatformObservation) -> Observation {
    Observation {
        id: platform.id.clone(),
        present: !platform.raw.is_empty(),
    }
}

/// Why a plan could not be prepared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// The corpus had no cases.
    EmptyCorpus,
    /// An id appeared on both sides.
    Overlap(HistoricalCaseId),
    /// A split id does not name a case in the corpus.
    UnknownCaseId(HistoricalCaseId),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCorpus => f.write_str("cannot prepare a replay plan from an empty corpus"),
            Self::Overlap(id) => write!(
                f,
                "case id {id} appears in both OPTIMIZE and CONFIRM (configuration error)"
            ),
            Self::UnknownCaseId(id) => {
                write!(f, "split references unknown historical case: {id}")
            }
        }
    }
}

impl std::error::Error for PlanError {}
