//! What a forge must make true for [`review_pull_request`] to keep its
//! promises, as a suite any forge can be run through.
//!
//! The suite of [`crate::domain::forge::conformance`] checks the ports
//! one reading at a time. This one drives the whole review over a
//! forge the harness builds and checks what the review then does with
//! it: the result is published on the head that was evaluated, a head
//! that moves during the evaluation is never approved, and a missing
//! merge-base, a conflict, a merge the forge has not computed and an
//! undeclared capability are each published as a non-approving check
//! the evaluator never sees. A forge that lies about any of these
//! readings (a frozen head, a stale merge candidate) makes the review
//! approve what it should not, which is what the negative controls
//! over this suite demonstrate.
//!
//! Compiled for unit tests and under the `test-support` feature.

use std::sync::Arc;

use crate::domain::conformance::ConformanceReport;
use crate::domain::forge::conformance::{
    ForgeFixture, ForgeHarness, ForgeScenario, ORIGINS, ProvisionedForge,
};
use crate::domain::forge::fake::ScriptedEvaluator;
use crate::domain::forge::{
    CheckConclusion, Evaluation, EvaluationMode, ForgeDiagnostic, ForgeError, HeadOrigin,
};
use crate::domain::scm::RevisionHandle;

use super::pull_request_review::{
    ReviewOutcome, ReviewRequest, ReviewVerdict, review_pull_request,
};

/// Identifiers of the clauses the suite checks.
pub mod clause {
    pub const PR_DIFF_PUBLISHED_AT_HEAD: &str = "review.pr-diff.result-is-published-at-the-head";
    pub const PR_DIFF_STALE_HEAD: &str = "review.pr-diff.moved-head-is-superseded";
    pub const UNRELATED_HISTORIES: &str = "review.pr-diff.unrelated-histories-are-diagnosed";
    pub const MERGED_PUBLISHED_AT_HEAD: &str = "review.merged-result.is-evaluated-at-the-candidate";
    pub const MERGED_STALE_HEAD: &str = "review.merged-result.moved-head-is-superseded";
    pub const MERGED_CONFLICT: &str = "review.merged-result.conflict-needs-the-author";
    pub const MERGED_PENDING: &str = "review.merged-result.uncomputed-is-withheld";
    pub const UNSUPPORTED: &str = "review.undeclared-capability-is-diagnosed";
}

const CHECK_NAME: &str = "conformance";

/// The capability each mode depends on, as the forge names it.
fn capability_of(mode: EvaluationMode) -> &'static str {
    match mode {
        EvaluationMode::PrDiff => "merge_base",
        EvaluationMode::MergedResult => "merge_candidate",
    }
}

fn declares(forge: &ProvisionedForge, mode: EvaluationMode) -> bool {
    let declared = forge.reader.capabilities();
    match mode {
        EvaluationMode::PrDiff => declared.merge_base,
        EvaluationMode::MergedResult => declared.merge_candidate,
    }
}

/// An evaluator that approves whatever it is given.
fn approving() -> ScriptedEvaluator {
    ScriptedEvaluator::answering(Ok(Evaluation {
        subject: RevisionHandle::new("approved").expect("a name is a handle"),
        conclusion: CheckConclusion::Success,
        summary: "approved".to_string(),
    }))
}

/// An evaluator that, while it runs, makes the author push `to`.
fn approving_while_the_author_pushes(
    scenario: &Arc<dyn ForgeScenario>,
    to: &RevisionHandle,
) -> ScriptedEvaluator {
    let (scenario, to) = (Arc::clone(scenario), to.clone());
    approving().during(move || scenario.move_head(&to))
}

fn review(
    forge: &ProvisionedForge,
    fixture: &ForgeFixture,
    mode: EvaluationMode,
    evaluator: &ScriptedEvaluator,
) -> Result<ReviewOutcome, ForgeError> {
    review_pull_request(
        forge.reader.as_ref(),
        forge.publisher.as_ref(),
        evaluator,
        &ReviewRequest {
            pull_request: fixture.pull_request.id.clone(),
            mode,
            check_name: CHECK_NAME.to_string(),
        },
    )
}

/// The check was published on `head`, as the report and as the forge's
/// receipt.
fn published_at(outcome: &ReviewOutcome, head: &RevisionHandle) -> bool {
    outcome.report.attached_to() == head && &outcome.published.attached_to == head
}

/// The review ended without approving and without asking the
/// evaluator, with `conclusion` published on `head`.
fn withheld(
    outcome: &Result<ReviewOutcome, ForgeError>,
    evaluator: &ScriptedEvaluator,
    head: &RevisionHandle,
    conclusion: CheckConclusion,
) -> bool {
    matches!(outcome, Ok(o) if published_at(o, head) && o.report.conclusion() == conclusion)
        && evaluator.seen().is_empty()
}

/// Runs every clause of the review contract against the forges the
/// harness builds.
pub fn run_review_contract(harness: &dyn ForgeHarness) -> ConformanceReport {
    let fixture = ForgeFixture::standard(harness.source(), HeadOrigin::SameRepository);
    let mut report = ConformanceReport::new();
    check_pr_diff(harness, &fixture, &mut report);
    check_merged_result(harness, &fixture, &mut report);
    check_undeclared(harness, &fixture, &mut report);
    report
}

fn check_pr_diff(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let mode = EvaluationMode::PrDiff;
    let head = &fixture.pull_request.head;
    let probe = harness.provision(fixture);
    if !declares(&probe, mode) {
        return;
    }

    for origin in ORIGINS {
        let fixture = ForgeFixture::standard(harness.source(), origin);
        let forge = harness.provision(&fixture);
        let evaluator = approving();
        let outcome = review(&forge, &fixture, mode, &evaluator);
        report.require(
            clause::PR_DIFF_PUBLISHED_AT_HEAD,
            matches!(&outcome, Ok(o)
                if published_at(o, head)
                    && matches!(&o.verdict, ReviewVerdict::Evaluated { conclusion: CheckConclusion::Success, .. }))
                && evaluator.seen().len() == 1,
            || format!("a pull request whose head is from {origin:?}: {outcome:?}"),
        );
    }

    let forge = harness.provision(fixture);
    let evaluator = approving_while_the_author_pushes(&forge.scenario, &fixture.moved_head);
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::PR_DIFF_STALE_HEAD,
        matches!(&outcome, Ok(o)
            if published_at(o, head)
                && !o.report.conclusion().is_approval()
                && matches!(&o.verdict, ReviewVerdict::Superseded { evaluated_head, current_head }
                    if evaluated_head == head && *current_head == fixture.moved_head)),
        || format!("the author pushed during the evaluation: {outcome:?}"),
    );

    let forge = harness.provision(fixture);
    forge.scenario.unrelate_histories();
    let evaluator = approving();
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::UNRELATED_HISTORIES,
        withheld(&outcome, &evaluator, head, CheckConclusion::ActionRequired)
            && matches!(&outcome, Ok(o)
                if matches!(o.verdict, ReviewVerdict::Diagnosed(ForgeDiagnostic::MissingMergeBase { .. }))),
        || format!("base and head share no history: {outcome:?}"),
    );
}

fn check_merged_result(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let mode = EvaluationMode::MergedResult;
    let head = &fixture.pull_request.head;
    if !declares(&harness.provision(fixture), mode) {
        return;
    }

    let forge = harness.provision(fixture);
    forge.scenario.compute_candidate();
    let evaluator = approving();
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::MERGED_PUBLISHED_AT_HEAD,
        matches!(&outcome, Ok(o)
            if published_at(o, head)
                && matches!(&o.verdict, ReviewVerdict::Evaluated { conclusion: CheckConclusion::Success, .. }))
            && evaluator
                .seen()
                .iter()
                .map(|(target, _)| target.subject())
                .eq([&fixture.candidate]),
        || format!("a merge computed for the current head: {outcome:?}"),
    );

    let forge = harness.provision(fixture);
    forge.scenario.compute_candidate();
    let evaluator = approving_while_the_author_pushes(&forge.scenario, &fixture.moved_head);
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::MERGED_STALE_HEAD,
        matches!(&outcome, Ok(o)
            if published_at(o, head)
                && !o.report.conclusion().is_approval()
                && matches!(o.verdict, ReviewVerdict::Superseded { .. })),
        || format!("the author pushed after the merge was computed: {outcome:?}"),
    );

    let forge = harness.provision(fixture);
    forge.scenario.conflict();
    let evaluator = approving();
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::MERGED_CONFLICT,
        withheld(&outcome, &evaluator, head, CheckConclusion::ActionRequired)
            && matches!(&outcome, Ok(o)
                if matches!(o.verdict, ReviewVerdict::Diagnosed(ForgeDiagnostic::MergeConflict { .. }))),
        || format!("a merge that conflicts: {outcome:?}"),
    );

    let forge = harness.provision(fixture);
    let evaluator = approving();
    let outcome = review(&forge, fixture, mode, &evaluator);
    report.require(
        clause::MERGED_PENDING,
        withheld(&outcome, &evaluator, head, CheckConclusion::Cancelled)
            && matches!(&outcome, Ok(o)
                if matches!(o.verdict, ReviewVerdict::Diagnosed(ForgeDiagnostic::MergeabilityPending { .. }))),
        || format!("a merge the forge has not computed: {outcome:?}"),
    );
}

fn check_undeclared(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let head = &fixture.pull_request.head;
    for mode in [EvaluationMode::PrDiff, EvaluationMode::MergedResult] {
        let forge = harness.provision(fixture);
        if declares(&forge, mode) {
            continue;
        }
        let evaluator = approving();
        let outcome = review(&forge, fixture, mode, &evaluator);
        report.require(
            clause::UNSUPPORTED,
            withheld(&outcome, &evaluator, head, CheckConclusion::Cancelled)
                && matches!(&outcome, Ok(o)
                    if matches!(&o.verdict, ReviewVerdict::Diagnosed(ForgeDiagnostic::CapabilityUnsupported { capability })
                        if *capability == capability_of(mode))),
            || format!("{mode:?} over a forge that does not declare {}: {outcome:?}", capability_of(mode)),
        );
    }
}

#[cfg(test)]
#[path = "pull_request_review_contract_tests.rs"]
mod tests;
