//! Behavior of the review over the conforming fake forge: the same use
//! case that runs over GitHub, with no network.

use std::sync::Arc;

use super::*;
use crate::domain::forge::fake::{FakeForge, ScriptedEvaluator};
use crate::domain::forge::{Evaluation, EvaluationError, HeadOrigin, MergeBase, MergePreview};
use crate::domain::scm::SourceRef;

fn rev(value: &str) -> RevisionHandle {
    RevisionHandle::new(value).unwrap()
}

fn id() -> PullRequestId {
    PullRequestId::new(SourceRef::new("fake", "acme/widgets").unwrap(), 7).unwrap()
}

fn state(head: &str, base: &str, origin: HeadOrigin) -> PullRequestState {
    PullRequestState {
        id: id(),
        head: rev(head),
        base_ref: "main".to_string(),
        base_tip: rev(base),
        origin,
    }
}

fn request(mode: EvaluationMode) -> ReviewRequest {
    ReviewRequest {
        pull_request: id(),
        mode,
        check_name: "cognicode".to_string(),
    }
}

fn approves() -> Result<Evaluation, EvaluationError> {
    Ok(Evaluation {
        subject: rev("placeholder"),
        conclusion: CheckConclusion::Success,
        summary: "no violations".to_string(),
    })
}

fn review(forge: &FakeForge, evaluator: &ScriptedEvaluator, mode: EvaluationMode) -> ReviewOutcome {
    review_pull_request(forge, forge, evaluator, &request(mode)).unwrap()
}

#[test]
fn a_pr_diff_review_publishes_the_verdict_on_the_head() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(approves());

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert!(matches!(
        outcome.verdict,
        ReviewVerdict::Evaluated {
            conclusion: CheckConclusion::Success,
            ..
        }
    ));
    let published = forge.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].attached_to(), &rev("head-1"));
    assert_eq!(published[0].conclusion(), CheckConclusion::Success);
    assert_eq!(published[0].mode(), EvaluationMode::PrDiff);
    let (target, _) = &evaluator.seen()[0];
    assert_eq!(target.compare_from(), &rev("base-1"));
    assert_eq!(target.subject(), &rev("head-1"));
}

#[test]
fn a_merged_result_review_evaluates_the_candidate_and_keeps_the_head_visible() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository))
        .with_merge_candidate(MergePreview::Ready(rev("merge-1")));
    let evaluator = ScriptedEvaluator::answering(approves());

    let outcome = review(&forge, &evaluator, EvaluationMode::MergedResult);

    let (target, _) = &evaluator.seen()[0];
    assert_eq!(target.mode(), EvaluationMode::MergedResult);
    assert_eq!(target.subject(), &rev("merge-1"));
    assert_eq!(target.compare_from(), &rev("base-1"));
    assert_eq!(outcome.report.attached_to(), &rev("head-1"));
    assert_eq!(outcome.report.evaluated(), Some(&rev("merge-1")));
    assert_eq!(outcome.report.mode(), EvaluationMode::MergedResult);
}

#[test]
fn a_head_that_moves_during_analysis_is_never_approved() {
    let forge = Arc::new(FakeForge::new(state(
        "head-1",
        "base-1",
        HeadOrigin::SameRepository,
    )));
    let mover = Arc::clone(&forge);
    let evaluator = ScriptedEvaluator::answering(approves()).during(move || {
        mover.set_state(state("head-2", "base-1", HeadOrigin::SameRepository));
    });

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert_eq!(
        outcome.verdict,
        ReviewVerdict::Superseded {
            evaluated_head: rev("head-1"),
            current_head: rev("head-2"),
        }
    );
    let published = forge.published();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].attached_to(), &rev("head-1"));
    assert_eq!(published[0].conclusion(), CheckConclusion::Cancelled);
    assert!(published[0].summary().contains("head-2"));
    assert!(
        published.iter().all(|r| r.attached_to() != &rev("head-2")),
        "nothing may be published on the new head from stale evidence"
    );
}

#[test]
fn a_base_that_moves_supersedes_a_merged_result_but_not_a_pr_diff() {
    for (mode, superseded) in [
        (EvaluationMode::MergedResult, true),
        (EvaluationMode::PrDiff, false),
    ] {
        let forge = Arc::new(
            FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository))
                .with_merge_candidate(MergePreview::Ready(rev("merge-1"))),
        );
        let mover = Arc::clone(&forge);
        let evaluator = ScriptedEvaluator::answering(approves()).during(move || {
            mover.set_state(state("head-1", "base-2", HeadOrigin::SameRepository));
        });

        let outcome = review(&forge, &evaluator, mode);

        assert_eq!(
            matches!(outcome.verdict, ReviewVerdict::Superseded { .. }),
            superseded,
            "{mode:?}"
        );
        let expected = if superseded {
            CheckConclusion::Cancelled
        } else {
            CheckConclusion::Success
        };
        assert_eq!(outcome.report.conclusion(), expected, "{mode:?}");
    }
}

#[test]
fn evidence_about_another_revision_is_rejected() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(approves()).about(rev("old-head"));

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert_eq!(
        outcome.verdict,
        ReviewVerdict::EvidenceRejected {
            expected: rev("head-1"),
            reported: rev("old-head"),
        }
    );
    assert_eq!(
        forge.published()[0].conclusion(),
        CheckConclusion::Cancelled
    );
}

#[test]
fn an_evaluator_that_fails_leaves_a_non_approving_check() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(Err(EvaluationError {
        detail: "analysis crashed".to_string(),
    }));

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert!(matches!(
        outcome.verdict,
        ReviewVerdict::EvaluationFailed { .. }
    ));
    let published = forge.published();
    assert_eq!(published[0].conclusion(), CheckConclusion::Cancelled);
    assert!(published[0].summary().contains("analysis crashed"));
}

#[test]
fn a_failure_conclusion_is_published_as_failure() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(Ok(Evaluation {
        subject: rev("x"),
        conclusion: CheckConclusion::Failure,
        summary: "1 violation".to_string(),
    }));

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert_eq!(outcome.report.conclusion(), CheckConclusion::Failure);
    assert_eq!(outcome.report.summary(), "1 violation");
}

#[test]
fn only_a_head_from_the_base_repository_may_run_code() {
    for (origin, runs_code) in [
        (HeadOrigin::SameRepository, true),
        (HeadOrigin::Fork, false),
        (HeadOrigin::Unknown, false),
    ] {
        let forge = FakeForge::new(state("head-1", "base-1", origin));
        let evaluator = ScriptedEvaluator::answering(approves());

        review(&forge, &evaluator, EvaluationMode::PrDiff);

        let (_, grant) = &evaluator.seen()[0];
        assert_eq!(grant.permits_code_execution(), runs_code, "{origin:?}");
        assert_eq!(grant.permits_secrets(), runs_code, "{origin:?}");
    }
}

#[test]
fn unrelated_histories_are_diagnosed_and_never_evaluated() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository))
        .with_merge_base(MergeBase::Absent);
    let evaluator = ScriptedEvaluator::answering(approves());

    let outcome = review(&forge, &evaluator, EvaluationMode::PrDiff);

    assert!(matches!(
        outcome.verdict,
        ReviewVerdict::Diagnosed(ForgeDiagnostic::MissingMergeBase { .. })
    ));
    assert!(evaluator.seen().is_empty());
    assert_eq!(
        forge.published()[0].conclusion(),
        CheckConclusion::ActionRequired
    );
}

#[test]
fn a_conflict_is_diagnosed_for_the_merged_result_only() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository))
        .with_merge_candidate(MergePreview::Conflicting);

    let merged = ScriptedEvaluator::answering(approves());
    let outcome = review(&forge, &merged, EvaluationMode::MergedResult);
    assert!(matches!(
        outcome.verdict,
        ReviewVerdict::Diagnosed(ForgeDiagnostic::MergeConflict { .. })
    ));
    assert_eq!(outcome.report.conclusion(), CheckConclusion::ActionRequired);
    assert!(merged.seen().is_empty());

    // The diff of the same pull request is still reviewable.
    let diff = ScriptedEvaluator::answering(approves());
    let outcome = review(&forge, &diff, EvaluationMode::PrDiff);
    assert!(matches!(outcome.verdict, ReviewVerdict::Evaluated { .. }));
}

#[test]
fn a_merge_the_forge_has_not_computed_is_not_an_approval() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(approves());

    let outcome = review(&forge, &evaluator, EvaluationMode::MergedResult);

    assert!(matches!(
        outcome.verdict,
        ReviewVerdict::Diagnosed(ForgeDiagnostic::MergeabilityPending { .. })
    ));
    assert_eq!(outcome.report.conclusion(), CheckConclusion::Cancelled);
}

#[test]
fn a_forge_without_merge_candidates_still_serves_the_diff() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository))
        .with_capabilities(ForgeCapabilities {
            merge_base: true,
            merge_candidate: false,
        });

    let merged = ScriptedEvaluator::answering(approves());
    let outcome = review(&forge, &merged, EvaluationMode::MergedResult);
    assert_eq!(
        outcome.verdict,
        ReviewVerdict::Diagnosed(ForgeDiagnostic::CapabilityUnsupported {
            capability: "merge_candidate"
        })
    );
    assert!(merged.seen().is_empty());

    let diff = ScriptedEvaluator::answering(approves());
    let outcome = review(&forge, &diff, EvaluationMode::PrDiff);
    assert!(matches!(outcome.verdict, ReviewVerdict::Evaluated { .. }));
}

#[test]
fn a_pull_request_the_forge_does_not_know_is_an_error_not_a_check() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(approves());
    let other = PullRequestId::new(SourceRef::new("fake", "acme/widgets").unwrap(), 8).unwrap();

    let result = review_pull_request(
        &forge,
        &forge,
        &evaluator,
        &ReviewRequest {
            pull_request: other,
            mode: EvaluationMode::PrDiff,
            check_name: "cognicode".to_string(),
        },
    );

    assert!(matches!(
        result,
        Err(ForgeError::PullRequestNotFound { .. })
    ));
    assert!(forge.published().is_empty());
}

#[test]
fn a_blank_check_name_is_refused_before_touching_the_forge() {
    let forge = FakeForge::new(state("head-1", "base-1", HeadOrigin::SameRepository));
    let evaluator = ScriptedEvaluator::answering(approves());

    let result = review_pull_request(
        &forge,
        &forge,
        &evaluator,
        &ReviewRequest {
            pull_request: id(),
            mode: EvaluationMode::PrDiff,
            check_name: "  ".to_string(),
        },
    );

    assert!(matches!(result, Err(ForgeError::MalformedIdentity { .. })));
    assert!(forge.published().is_empty());
}
