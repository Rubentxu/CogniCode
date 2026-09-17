//! Held-out promotion policy and gate (e83 — M13 task 13.5).
//!
//! ## The load-bearing invariant
//!
//! ```text
//! OPTIMIZE improvement  +  CONFIRM unacceptable regression  =  NO governed promotion
//! ```
//!
//! The gate consumes **CONFIRM metrics only**. OPTIMIZE metrics must never
//! participate in the decision: there is no averaging, no compensation, and no
//! blended score.
//!
//! ## Identity errors are not policy blocks
//!
//! A CONFIRM report that does not belong to the frozen candidate is a
//! **programming/plumbing error** (an `Err`), not a `Block`: the evidence is not
//! about the candidate at all, so no policy decision can be made from it.
//!
//! ## Evidence that cannot be read is `InsufficientEvidence`, never `Pass`
//!
//! Incomplete CONFIRM cases and shared platform divergence mean the comparison
//! cannot be trusted. Missing evidence is never zero regression.
//!
//! ## Policy is explicit, not hidden
//!
//! Thresholds live in an immutable [`HeldOutPolicySpec`] with a digest. Nothing
//! is hard-coded in the evaluator, and `require_complete` is the default.

use crate::application::governed_improvement::binding::CandidateFreeze;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::policy_gate::PolicyOutcome;
use crate::application::portable_execution::content_digest;
use crate::application::shadow_evaluation::analyzer::AnalyzerDescriptor;
use crate::application::shadow_evaluation::regime::{FailureRegime, FailureRegimeSubject};
use crate::application::shadow_evaluation::report::ShadowRoleReport;

/// An immutable held-out policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldOutPolicySpec {
    max_false_negative_increase: i64,
    max_false_positive_increase: i64,
    max_unknown_increase: i64,
    require_complete: bool,
    blocked_candidate_regimes: Vec<FailureRegime>,
    digest: String,
}

impl HeldOutPolicySpec {
    /// Construct a policy. `blocked_candidate_regimes` is canonicalised
    /// (sorted, deduplicated) so the digest is order-invariant.
    pub fn new(
        max_false_negative_increase: i64,
        max_false_positive_increase: i64,
        max_unknown_increase: i64,
        require_complete: bool,
        mut blocked_candidate_regimes: Vec<FailureRegime>,
    ) -> Self {
        blocked_candidate_regimes.sort();
        blocked_candidate_regimes.dedup();
        let digest = policy_digest(
            max_false_negative_increase,
            max_false_positive_increase,
            max_unknown_increase,
            require_complete,
            &blocked_candidate_regimes,
        );
        Self {
            max_false_negative_increase,
            max_false_positive_increase,
            max_unknown_increase,
            require_complete,
            blocked_candidate_regimes,
            digest,
        }
    }

    /// The conservative default: no regression of any kind is tolerated, and the
    /// evidence must be complete.
    pub fn strict_no_regression() -> Self {
        Self::new(0, 0, 0, true, Vec::new())
    }

    /// Maximum tolerated increase in false negatives.
    pub fn max_false_negative_increase(&self) -> i64 {
        self.max_false_negative_increase
    }

    /// Maximum tolerated increase in false positives.
    pub fn max_false_positive_increase(&self) -> i64 {
        self.max_false_positive_increase
    }

    /// Maximum tolerated increase in unknowns.
    pub fn max_unknown_increase(&self) -> i64 {
        self.max_unknown_increase
    }

    /// Whether complete evidence is required.
    pub fn require_complete(&self) -> bool {
        self.require_complete
    }

    /// Regimes that block when attributed to the candidate.
    pub fn blocked_candidate_regimes(&self) -> &[FailureRegime] {
        &self.blocked_candidate_regimes
    }

    /// The stable policy digest.
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

fn policy_digest(
    max_fn: i64,
    max_fp: i64,
    max_unknown: i64,
    require_complete: bool,
    blocked: &[FailureRegime],
) -> String {
    let mut canonical = String::new();
    canonical.push_str("heldout-policy.v1\n");
    canonical.push_str(&max_fn.to_string());
    canonical.push('\n');
    canonical.push_str(&max_fp.to_string());
    canonical.push('\n');
    canonical.push_str(&max_unknown.to_string());
    canonical.push('\n');
    canonical.push_str(if require_complete { "require_complete" } else { "allow_incomplete" });
    canonical.push('\n');
    for regime in blocked {
        canonical.push_str(regime.name());
        canonical.push('\n');
    }
    content_digest(canonical.as_bytes()).as_str().to_string()
}

/// Why a held-out decision came out the way it did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldOutReason {
    /// The candidate increased false negatives beyond the policy.
    FalseNegativeRegression {
        /// Observed increase.
        observed: i64,
        /// Allowed increase.
        allowed: i64,
    },
    /// The candidate increased false positives beyond the policy.
    FalsePositiveRegression {
        /// Observed increase.
        observed: i64,
        /// Allowed increase.
        allowed: i64,
    },
    /// The candidate increased unknowns beyond the policy.
    UnknownRegression {
        /// Observed increase.
        observed: i64,
        /// Allowed increase.
        allowed: i64,
    },
    /// CONFIRM evidence is incomplete, so no honest comparison exists.
    IncompleteEvidence {
        /// Incomplete cases on the current side.
        current_incomplete: usize,
        /// Incomplete cases on the candidate side.
        candidate_incomplete: usize,
    },
    /// Shared platform divergence makes the held-out evidence unreliable.
    PlatformDivergenceUnreliable {
        /// How many divergent observations were found.
        divergent_observations: usize,
    },
    /// A configured blocking regime was attributed to the candidate.
    BlockedCandidateRegime {
        /// The regime.
        regime: FailureRegime,
        /// How many occurrences.
        occurrences: usize,
    },
    /// No configured limit was exceeded.
    NoRegressionWithinPolicy,
}

/// The result of evaluating held-out policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldOutPolicyDecision {
    /// Reuses the canonical policy vocabulary.
    pub outcome: PolicyOutcome,
    /// Why.
    pub reasons: Vec<HeldOutReason>,
    /// The evaluation digest the decision was made against.
    pub evaluation_digest: String,
    /// The policy digest.
    pub policy_digest: String,
}

/// Why the gate could not make a policy decision at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldOutGateError {
    /// The report was not a CONFIRM report.
    WrongDatasetRole {
        /// Required role.
        expected: DatasetRole,
        /// Supplied role.
        actual: DatasetRole,
    },
    /// The report belongs to a different evaluation than the freeze.
    EvaluationDigestMismatch {
        /// The freeze's digest.
        expected: String,
        /// The report's digest.
        actual: String,
    },
    /// The report's candidate analyzer differs from the frozen one.
    CandidateAnalyzerMismatch,
    /// The report's current analyzer differs from the frozen one.
    CurrentAnalyzerMismatch,
}

impl std::fmt::Display for HeldOutGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongDatasetRole { expected, actual } => write!(
                f,
                "expected a {} report, got {}",
                expected.name(),
                actual.name()
            ),
            Self::EvaluationDigestMismatch { expected, actual } => write!(
                f,
                "CONFIRM report evaluation digest {actual} does not match the frozen {expected}"
            ),
            Self::CandidateAnalyzerMismatch => {
                f.write_str("CONFIRM candidate analyzer does not match the frozen candidate")
            }
            Self::CurrentAnalyzerMismatch => {
                f.write_str("CONFIRM current analyzer does not match the frozen current")
            }
        }
    }
}

impl std::error::Error for HeldOutGateError {}

/// The pure held-out policy gate.
pub struct HeldOutPromotionGate;

impl HeldOutPromotionGate {
    /// Evaluate held-out policy over a CONFIRM report for the frozen candidate.
    ///
    /// Identity mismatches return `Err` (plumbing error); everything else is a
    /// policy outcome.
    pub fn evaluate(
        freeze: &CandidateFreeze,
        confirm: &ShadowRoleReport,
        policy: &HeldOutPolicySpec,
    ) -> Result<HeldOutPolicyDecision, HeldOutGateError> {
        // Identity first: the evidence must be about the frozen candidate.
        if confirm.role() != DatasetRole::Confirm {
            return Err(HeldOutGateError::WrongDatasetRole {
                expected: DatasetRole::Confirm,
                actual: confirm.role(),
            });
        }
        if confirm.evaluation_digest() != freeze.evaluation_digest() {
            return Err(HeldOutGateError::EvaluationDigestMismatch {
                expected: freeze.evaluation_digest().to_string(),
                actual: confirm.evaluation_digest().to_string(),
            });
        }
        if confirm.current_descriptor() != freeze.current_descriptor() {
            return Err(HeldOutGateError::CurrentAnalyzerMismatch);
        }
        if confirm.candidate_descriptor() != freeze.candidate_descriptor() {
            return Err(HeldOutGateError::CandidateAnalyzerMismatch);
        }

        let mut reasons = Vec::new();

        // 1. Evidence that cannot be read honestly.
        let current_incomplete = confirm.current_report().incomplete_cases();
        let candidate_incomplete = confirm.candidate_report().incomplete_cases();
        let incomplete_signal = current_incomplete > 0
            || candidate_incomplete > 0
            || !confirm.regimes_of(FailureRegime::EvidenceIncomplete).is_empty();
        let divergent = confirm
            .regimes_of(FailureRegime::PlatformDivergence)
            .iter()
            .filter(|occurrence| occurrence.subject == FailureRegimeSubject::SharedEvaluation)
            .count();
        if policy.require_complete && incomplete_signal {
            reasons.push(HeldOutReason::IncompleteEvidence {
                current_incomplete,
                candidate_incomplete,
            });
        }
        if divergent > 0 {
            reasons.push(HeldOutReason::PlatformDivergenceUnreliable {
                divergent_observations: divergent,
            });
        }
        if !reasons.is_empty() {
            return Ok(decision(
                PolicyOutcome::InsufficientEvidence,
                reasons,
                freeze,
                policy,
            ));
        }

        // 2. Configured blocking regimes attributed to the candidate.
        for regime in policy.blocked_candidate_regimes() {
            let occurrences = confirm
                .failure_regimes()
                .iter()
                .filter(|occurrence| {
                    occurrence.regime == *regime
                        && occurrence.subject == FailureRegimeSubject::Candidate
                })
                .count();
            if occurrences > 0 {
                reasons.push(HeldOutReason::BlockedCandidateRegime {
                    regime: *regime,
                    occurrences,
                });
            }
        }
        if !reasons.is_empty() {
            return Ok(decision(PolicyOutcome::Block, reasons, freeze, policy));
        }

        // 3. Regression limits. CONFIRM only: OPTIMIZE never participates.
        let score = confirm.aggregate_delta().score;
        if score.false_negative > policy.max_false_negative_increase {
            reasons.push(HeldOutReason::FalseNegativeRegression {
                observed: score.false_negative,
                allowed: policy.max_false_negative_increase,
            });
        }
        if score.false_positive > policy.max_false_positive_increase {
            reasons.push(HeldOutReason::FalsePositiveRegression {
                observed: score.false_positive,
                allowed: policy.max_false_positive_increase,
            });
        }
        if score.unknown > policy.max_unknown_increase {
            reasons.push(HeldOutReason::UnknownRegression {
                observed: score.unknown,
                allowed: policy.max_unknown_increase,
            });
        }
        if !reasons.is_empty() {
            return Ok(decision(PolicyOutcome::Block, reasons, freeze, policy));
        }

        // 4. Clean.
        Ok(decision(
            PolicyOutcome::Pass,
            vec![HeldOutReason::NoRegressionWithinPolicy],
            freeze,
            policy,
        ))
    }

    /// Evaluate and, only on `Pass`, mint the sealed held-out gate pass.
    pub fn require_pass(
        freeze: &CandidateFreeze,
        confirm: &ShadowRoleReport,
        policy: &HeldOutPolicySpec,
    ) -> Result<HeldOutGatePass, HeldOutRejection> {
        let decision = Self::evaluate(freeze, confirm, policy).map_err(HeldOutRejection::Identity)?;
        if decision.outcome != PolicyOutcome::Pass {
            return Err(HeldOutRejection::Decision(decision));
        }
        Ok(HeldOutGatePass {
            proposal_id: freeze.proposal_id().clone(),
            candidate_world: freeze.candidate_world().clone(),
            current: freeze.current_descriptor().clone(),
            candidate: freeze.candidate_descriptor().clone(),
            evaluation_digest: freeze.evaluation_digest().to_string(),
            policy_digest: policy.digest().to_string(),
            _seal: PassSeal(()),
        })
    }
}

fn decision(
    outcome: PolicyOutcome,
    reasons: Vec<HeldOutReason>,
    freeze: &CandidateFreeze,
    policy: &HeldOutPolicySpec,
) -> HeldOutPolicyDecision {
    HeldOutPolicyDecision {
        outcome,
        reasons,
        evaluation_digest: freeze.evaluation_digest().to_string(),
        policy_digest: policy.digest().to_string(),
    }
}

/// Why a held-out pass could not be minted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeldOutRejection {
    /// The CONFIRM evidence was not about the frozen candidate.
    Identity(HeldOutGateError),
    /// The policy did not pass.
    Decision(HeldOutPolicyDecision),
}

impl std::fmt::Display for HeldOutRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(e) => write!(f, "held-out identity error: {e}"),
            Self::Decision(d) => write!(
                f,
                "held-out policy did not pass ({:?}): {:?}",
                d.outcome, d.reasons
            ),
        }
    }
}

impl std::error::Error for HeldOutRejection {}

/// Private seal for [`HeldOutGatePass`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PassSeal(());

/// A sealed held-out pass: CONFIRM did not block the frozen candidate.
///
/// Private fields, a private seal, no serde. It is **not** a
/// `PromotionAuthorization` and **not** a `PromotionPermit`: it grants no apply
/// authority, and human authority is still required separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldOutGatePass {
    proposal_id: crate::application::change_proposal::proposal::ChangeProposalId,
    candidate_world: crate::application::software_world::world::SoftwareWorldId,
    current: AnalyzerDescriptor,
    candidate: AnalyzerDescriptor,
    evaluation_digest: String,
    policy_digest: String,
    _seal: PassSeal,
}

impl HeldOutGatePass {
    /// The current analyzer revision the pass was evaluated against.
    pub fn current_descriptor(&self) -> &AnalyzerDescriptor {
        &self.current
    }
    /// The proposal the pass is about.
    pub fn proposal_id(&self) -> &crate::application::change_proposal::proposal::ChangeProposalId {
        &self.proposal_id
    }

    /// The candidate world the pass is about.
    pub fn candidate_world(
        &self,
    ) -> &crate::application::software_world::world::SoftwareWorldId {
        &self.candidate_world
    }

    /// The candidate analyzer revision.
    pub fn candidate_descriptor(&self) -> &AnalyzerDescriptor {
        &self.candidate
    }

    /// The evaluation digest this pass is bound to.
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }

    /// The policy digest this pass was granted under.
    pub fn policy_digest(&self) -> &str {
        &self.policy_digest
    }
}
