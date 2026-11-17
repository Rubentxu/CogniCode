//! The pull-request review use case: read the pull request, decide
//! which revisions to evaluate, have the evaluator judge them, make sure
//! the evidence still describes the pull request, and publish the
//! result on the revision that was evaluated.
//!
//! Everything forge-specific is behind the ports of
//! [`crate::domain::forge`]; the same function runs over GitHub and over
//! the conforming fake.
//!
//! ## Guarantees
//!
//! * **No approval of a head the evidence is not about.** The result is
//!   always published on the head that was read at the start. After the
//!   evaluation the pull request is read again; if the head moved (or,
//!   for a merged-result evaluation, the base tip moved) nothing is
//!   approved: a `Cancelled` check naming the superseding head is
//!   published on the OLD head and the new head gets a fresh review.
//! * **Evidence must be about the target.** An evaluation whose subject
//!   is not the target's subject is rejected, not published.
//! * **Untrusted code stays untrusted.** The evaluator receives a
//!   [`ExecutionGrant`] derived from the head origin; the publishing
//!   port is never handed to it.
//! * **Diagnoses are results.** Unrelated histories, conflicts, a merge
//!   the forge has not computed and unsupported capabilities are
//!   published as non-approving checks with the reason.
//!
//! The publication itself is a separate forge call, so a head can still
//! move between the freshness check and the publication; that is safe
//! because the check is attached to the evaluated revision, and forges
//! judge required checks per revision.

use crate::domain::forge::{
    CheckConclusion, CheckPublisher, CheckReport, EvaluationMode, EvaluationTarget,
    EvidenceFreshness, ExecutionGrant, ForgeCapabilities, ForgeDiagnostic, ForgeError,
    PublishedCheck, PullRequestEvaluator, PullRequestId, PullRequestState, ReviewReader, reconcile,
    target_for_merged_result, target_for_pr_diff,
};
use crate::domain::scm::RevisionHandle;

/// What to review and how to name the check.
#[derive(Debug, Clone)]
pub struct ReviewRequest {
    pub pull_request: PullRequestId,
    pub mode: EvaluationMode,
    /// The check name shown by the forge. Must not be blank.
    pub check_name: String,
}

/// How the review ended. Every variant except `Evaluated` is a
/// non-approving result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewVerdict {
    /// The evaluator reached `conclusion` over `target` and the pull
    /// request was still current.
    Evaluated {
        target: EvaluationTarget,
        conclusion: CheckConclusion,
    },
    /// No target exists for the requested mode.
    Diagnosed(ForgeDiagnostic),
    /// The pull request moved during the evaluation.
    Superseded {
        evaluated_head: RevisionHandle,
        current_head: RevisionHandle,
    },
    /// The evaluator reached no verdict.
    EvaluationFailed { detail: String },
    /// The evidence was about another revision than the target.
    EvidenceRejected {
        expected: RevisionHandle,
        reported: RevisionHandle,
    },
}

/// The review's verdict, the report that was published and the forge's
/// receipt for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOutcome {
    pub verdict: ReviewVerdict,
    pub report: CheckReport,
    pub published: PublishedCheck,
}

/// Runs one review of `request.pull_request`.
///
/// Errors are port failures (the forge could not be read or published
/// to, a blank check name); an evaluation that cannot proceed is an
/// outcome, not an error, because it is itself published.
pub fn review_pull_request(
    reader: &dyn ReviewReader,
    publisher: &dyn CheckPublisher,
    evaluator: &dyn PullRequestEvaluator,
    request: &ReviewRequest,
) -> Result<ReviewOutcome, ForgeError> {
    if request.check_name.trim().is_empty() {
        return Err(ForgeError::MalformedIdentity {
            field: "check name",
            value: request.check_name.clone(),
        });
    }
    let state = reader.pull_request(&request.pull_request)?;

    let target = match plan_target(reader, &state, request.mode)? {
        Ok(target) => target,
        Err(diagnostic) => {
            let report =
                CheckReport::for_diagnosis(&state, request.mode, &request.check_name, &diagnostic);
            return publish(publisher, ReviewVerdict::Diagnosed(diagnostic), report);
        }
    };

    let grant = ExecutionGrant::for_origin(state.origin);
    let (verdict, report) = match evaluator.evaluate(&target, &grant) {
        Err(failure) => withhold(
            &target,
            &request.check_name,
            &failure.detail,
            ReviewVerdict::EvaluationFailed {
                detail: failure.detail.clone(),
            },
        ),
        Ok(evaluation) if &evaluation.subject != target.subject() => withhold(
            &target,
            &request.check_name,
            "the evidence is about another revision than the evaluated one",
            ReviewVerdict::EvidenceRejected {
                expected: target.subject().clone(),
                reported: evaluation.subject,
            },
        ),
        Ok(evaluation) => {
            let current = reader.pull_request(&request.pull_request)?;
            match reconcile(&target, &current) {
                EvidenceFreshness::Current => (
                    ReviewVerdict::Evaluated {
                        target: target.clone(),
                        conclusion: evaluation.conclusion,
                    },
                    CheckReport::for_target(
                        &target,
                        &request.check_name,
                        evaluation.conclusion,
                        &evaluation.summary,
                    ),
                ),
                EvidenceFreshness::Superseded {
                    evaluated_head,
                    current_head,
                    ..
                } => withhold(
                    &target,
                    &request.check_name,
                    &format!(
                        "superseded: the pull request moved from {} to {} during the evaluation",
                        evaluated_head.as_str(),
                        current_head.as_str()
                    ),
                    ReviewVerdict::Superseded {
                        evaluated_head,
                        current_head,
                    },
                ),
            }
        }
    };
    publish(publisher, verdict, report)
}

/// The target for `mode`, or the diagnosis that explains its absence.
/// A capability the forge did not declare is a diagnosis; a failing
/// read is an error.
fn plan_target(
    reader: &dyn ReviewReader,
    state: &PullRequestState,
    mode: EvaluationMode,
) -> Result<Result<EvaluationTarget, ForgeDiagnostic>, ForgeError> {
    let ForgeCapabilities {
        merge_base,
        merge_candidate,
    } = reader.capabilities();
    match mode {
        EvaluationMode::PrDiff if !merge_base => Ok(unsupported("merge_base")),
        EvaluationMode::MergedResult if !merge_candidate => Ok(unsupported("merge_candidate")),
        EvaluationMode::PrDiff => Ok(target_for_pr_diff(state, reader.merge_base(state)?)),
        EvaluationMode::MergedResult => Ok(target_for_merged_result(
            state,
            reader.merge_candidate(state)?,
        )),
    }
}

fn unsupported(capability: &'static str) -> Result<EvaluationTarget, ForgeDiagnostic> {
    Err(ForgeDiagnostic::CapabilityUnsupported { capability })
}

/// A non-approving result for a target whose evaluation cannot stand.
fn withhold(
    target: &EvaluationTarget,
    name: &str,
    reason: &str,
    verdict: ReviewVerdict,
) -> (ReviewVerdict, CheckReport) {
    let report = CheckReport::for_target(target, name, CheckConclusion::Cancelled, reason);
    (verdict, report)
}

fn publish(
    publisher: &dyn CheckPublisher,
    verdict: ReviewVerdict,
    report: CheckReport,
) -> Result<ReviewOutcome, ForgeError> {
    let published = publisher.publish(&report)?;
    if &published.attached_to != report.attached_to() {
        return Err(ForgeError::ProviderFailure {
            detail: format!(
                "the forge recorded the check on {} instead of {}",
                published.attached_to.as_str(),
                report.attached_to().as_str()
            ),
        });
    }
    Ok(ReviewOutcome {
        verdict,
        report,
        published,
    })
}

#[cfg(test)]
#[path = "pull_request_review_tests.rs"]
mod tests;
