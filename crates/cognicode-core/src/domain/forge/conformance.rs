//! The reusable conformance suite of the forge ports.
//!
//! [`run_forge_conformance`] drives one forge through the contract of
//! [`ReviewReader`] and [`CheckPublisher`] over a fixed pull-request
//! [`ForgeFixture`]. An [`ForgeHarness`] builds the forge in a known
//! state and gives the suite a [`ForgeScenario`]: the events a real
//! forge has on its own (the author pushes, the merge preview gets
//! computed, a conflict appears) and that the suite must be able to
//! provoke without knowing how the forge stores them.
//!
//! The suite reads the forge's declared [`ForgeCapabilities`] and
//! demands an exact answer for a declared reading and
//! `CapabilityUnsupported` for an undeclared one; it never assumes a
//! particular platform. What the review USE CASE promises over these
//! ports (nothing approved on a head the evidence is not about, results
//! published at the evaluated revision) is checked by the contract in
//! `application::pull_request_review_contract`, over the same harness.
//!
//! Compiled for unit tests and under the `test-support` feature.

use std::sync::Arc;

use crate::domain::conformance::ConformanceReport;
use crate::domain::scm::{RevisionHandle, SourceRef};

use super::error::ForgeError;
use super::identity::{HeadOrigin, MergeBase, MergePreview, PullRequestId, PullRequestState};
use super::ports::{CheckPublisher, ForgeCapabilities, ReviewReader};
use super::report::{CheckConclusion, CheckReport, PublishedCheck};
use super::target::{EvaluationMode, ForgeDiagnostic, target_for_pr_diff};

/// Identifiers of the clauses the suite checks. A negative control
/// names the clause it expects the suite to report.
pub mod clause {
    pub const PR_FACTS: &str = "forge.pull-request.reports-current-facts";
    pub const PR_ORIGIN: &str = "forge.pull-request.origin-is-reported-as-it-is";
    pub const PR_HEAD_MOVES: &str = "forge.pull-request.follows-head-movement";
    pub const PR_UNKNOWN: &str = "forge.pull-request.unknown-is-not-found";
    pub const BASE_UNDECLARED: &str = "forge.merge-base.undeclared-is-unsupported";
    pub const BASE_FOUND: &str = "forge.merge-base.found";
    pub const BASE_ABSENT: &str = "forge.merge-base.absent-is-not-an-error";
    pub const CANDIDATE_UNDECLARED: &str = "forge.merge-candidate.undeclared-is-unsupported";
    pub const CANDIDATE_PENDING: &str = "forge.merge-candidate.uncomputed-is-pending";
    pub const CANDIDATE_READY: &str = "forge.merge-candidate.ready";
    pub const CANDIDATE_CONFLICT: &str = "forge.merge-candidate.conflict";
    pub const CANDIDATE_STALE: &str = "forge.merge-candidate.stale-is-pending";
    pub const PUBLISH_AT_HEAD: &str = "forge.publish.receipt-names-the-attached-revision";
    pub const PUBLISH_REFERENCE: &str = "forge.publish.receipt-carries-a-reference";
}

/// The pull request every forge is tested over, as pure data.
#[derive(Debug, Clone)]
pub struct ForgeFixture {
    /// The pull request as the forge starts out reporting it.
    pub pull_request: PullRequestState,
    /// The merge-base of the base tip and the head.
    pub merge_base: RevisionHandle,
    /// The merge of the head into the base tip, once the forge has
    /// computed it.
    pub candidate: RevisionHandle,
    /// A head the author pushes later.
    pub moved_head: RevisionHandle,
    /// A pull request the forge has never heard of.
    pub unknown: PullRequestId,
}

/// A revision handle shaped like a full SHA-1, which every platform
/// that names revisions by hash accepts.
fn full_hash(seed: u8) -> RevisionHandle {
    RevisionHandle::new(&format!("{seed:02x}").repeat(20)).expect("a hash is a handle")
}

impl ForgeFixture {
    /// The fixture for a pull request #7 of `source` whose head lives
    /// where `origin` says.
    pub fn standard(source: SourceRef, origin: HeadOrigin) -> Self {
        let id = PullRequestId::new(source.clone(), 7).expect("a positive number");
        Self {
            pull_request: PullRequestState {
                id,
                head: full_hash(0x11),
                base_ref: "main".to_string(),
                base_tip: full_hash(0xbb),
                origin,
            },
            merge_base: full_hash(0xaa),
            candidate: full_hash(0xcc),
            moved_head: full_hash(0x22),
            unknown: PullRequestId::new(source, 404).expect("a positive number"),
        }
    }
}

/// Events on the forge that happen outside the review. The forge starts
/// with [`ForgeFixture::pull_request`], a merge-base of
/// [`ForgeFixture::merge_base`] and no merge preview computed yet.
pub trait ForgeScenario: Send + Sync {
    /// The author pushes: the pull request now has `to` as its head. Any
    /// merge preview computed before is about another head.
    fn move_head(&self, to: &RevisionHandle);
    /// The forge computes the merge of the current head into the
    /// current base tip, and it is [`ForgeFixture::candidate`].
    fn compute_candidate(&self);
    /// The forge finds that the merge conflicts.
    fn conflict(&self);
    /// The histories of the base and the head turn out to share no
    /// ancestor.
    fn unrelate_histories(&self);
}

/// A forge built in a known state, plus the means to change it.
pub struct ProvisionedForge {
    pub reader: Arc<dyn ReviewReader>,
    pub publisher: Arc<dyn CheckPublisher>,
    pub scenario: Arc<dyn ForgeScenario>,
}

/// Builds a fresh forge for a fixture. Called many times per run; each
/// call must return an independent forge.
pub trait ForgeHarness {
    /// The source (provider and project) the forge's pull requests
    /// belong to.
    fn source(&self) -> SourceRef;

    fn provision(&self, fixture: &ForgeFixture) -> ProvisionedForge;
}

/// Lets one shared forge object be handed out as both ports.
pub struct Shared<T>(pub Arc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: ReviewReader> ReviewReader for Shared<T> {
    fn capabilities(&self) -> ForgeCapabilities {
        self.0.capabilities()
    }

    fn pull_request(&self, id: &PullRequestId) -> Result<PullRequestState, ForgeError> {
        self.0.pull_request(id)
    }

    fn merge_base(&self, state: &PullRequestState) -> Result<MergeBase, ForgeError> {
        self.0.merge_base(state)
    }

    fn merge_candidate(&self, state: &PullRequestState) -> Result<MergePreview, ForgeError> {
        self.0.merge_candidate(state)
    }
}

impl<T: CheckPublisher> CheckPublisher for Shared<T> {
    fn publish(&self, report: &CheckReport) -> Result<PublishedCheck, ForgeError> {
        self.0.publish(report)
    }
}

/// The origins every forge must report faithfully.
pub const ORIGINS: [HeadOrigin; 3] = [
    HeadOrigin::SameRepository,
    HeadOrigin::Fork,
    HeadOrigin::Unknown,
];

fn fixture_for(harness: &dyn ForgeHarness, origin: HeadOrigin) -> ForgeFixture {
    ForgeFixture::standard(harness.source(), origin)
}

/// Runs every clause of the forge contract against the forges the
/// harness builds, and reports each clause they break.
pub fn run_forge_conformance(harness: &dyn ForgeHarness) -> ConformanceReport {
    let mut report = ConformanceReport::new();
    for origin in ORIGINS {
        check_pull_request_facts(harness, origin, &mut report);
    }
    let fixture = fixture_for(harness, HeadOrigin::SameRepository);
    check_pull_request_movement(harness, &fixture, &mut report);
    check_merge_base(harness, &fixture, &mut report);
    check_merge_candidate(harness, &fixture, &mut report);
    check_publication(harness, &fixture, &mut report);
    report
}

fn read_current(
    forge: &ProvisionedForge,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) -> Option<PullRequestState> {
    match forge.reader.pull_request(&fixture.pull_request.id) {
        Ok(state) => Some(state),
        Err(error) => {
            report.violated(
                clause::PR_FACTS,
                format!("a known pull request could not be read: {error:?}"),
            );
            None
        }
    }
}

fn check_pull_request_facts(
    harness: &dyn ForgeHarness,
    origin: HeadOrigin,
    report: &mut ConformanceReport,
) {
    let fixture = fixture_for(harness, origin);
    let forge = harness.provision(&fixture);
    let Some(state) = read_current(&forge, &fixture, report) else {
        return;
    };
    let expected = &fixture.pull_request;
    report.expect_eq(clause::PR_FACTS, "pull request id", &state.id, &expected.id);
    report.expect_eq(clause::PR_FACTS, "head", &state.head, &expected.head);
    report.expect_eq(
        clause::PR_FACTS,
        "base branch",
        &state.base_ref,
        &expected.base_ref,
    );
    report.expect_eq(
        clause::PR_FACTS,
        "base tip",
        &state.base_tip,
        &expected.base_tip,
    );
    report.expect_eq(
        clause::PR_ORIGIN,
        "origin of the head",
        &state.origin,
        &origin,
    );
}

fn check_pull_request_movement(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let forge = harness.provision(fixture);
    let first = read_current(&forge, fixture, report);
    forge.scenario.move_head(&fixture.moved_head);
    let after = forge.reader.pull_request(&fixture.pull_request.id);
    report.require(
        clause::PR_HEAD_MOVES,
        first.is_some() && matches!(&after, Ok(state) if state.head == fixture.moved_head),
        || {
            format!(
                "after the author pushed {}, the pull request reads {after:?}",
                fixture.moved_head.as_str()
            )
        },
    );

    let unknown = forge.reader.pull_request(&fixture.unknown);
    report.require(
        clause::PR_UNKNOWN,
        matches!(&unknown, Err(ForgeError::PullRequestNotFound { pull_request })
            if *pull_request == fixture.unknown.to_string()),
        || format!("a pull request the forge never heard of answered {unknown:?}"),
    );
}

fn unsupported(capability: &'static str) -> Result<(), ForgeError> {
    Err(ForgeError::CapabilityUnsupported { capability })
}

fn check_merge_base(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let forge = harness.provision(fixture);
    let state = &fixture.pull_request;
    let answer = forge.reader.merge_base(state);
    if !forge.reader.capabilities().merge_base {
        report.require(
            clause::BASE_UNDECLARED,
            answer.map(drop) == unsupported("merge_base"),
            || "an undeclared merge-base was answered instead of refused".to_string(),
        );
        return;
    }
    report.require(
        clause::BASE_FOUND,
        answer == Ok(MergeBase::Found(fixture.merge_base.clone())),
        || {
            format!(
                "merge-base of related histories: expected {}, got {answer:?}",
                fixture.merge_base.as_str()
            )
        },
    );
    forge.scenario.unrelate_histories();
    let unrelated = forge.reader.merge_base(state);
    report.require(
        clause::BASE_ABSENT,
        unrelated == Ok(MergeBase::Absent),
        || format!("histories with no common ancestor: expected Absent, got {unrelated:?}"),
    );
}

fn check_merge_candidate(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let state = &fixture.pull_request;
    let forge = harness.provision(fixture);
    let uncomputed = forge.reader.merge_candidate(state);
    if !forge.reader.capabilities().merge_candidate {
        report.require(
            clause::CANDIDATE_UNDECLARED,
            uncomputed.map(drop) == unsupported("merge_candidate"),
            || "an undeclared merge candidate was answered instead of refused".to_string(),
        );
        return;
    }
    report.require(
        clause::CANDIDATE_PENDING,
        uncomputed == Ok(MergePreview::Pending),
        || format!("a merge the forge has not computed: expected Pending, got {uncomputed:?}"),
    );

    forge.scenario.compute_candidate();
    let ready = forge.reader.merge_candidate(state);
    report.require(
        clause::CANDIDATE_READY,
        ready == Ok(MergePreview::Ready(fixture.candidate.clone())),
        || {
            format!(
                "a computed merge: expected Ready({}), got {ready:?}",
                fixture.candidate.as_str()
            )
        },
    );

    forge.scenario.move_head(&fixture.moved_head);
    let moved = PullRequestState {
        head: fixture.moved_head.clone(),
        ..state.clone()
    };
    for (asked, whose) in [(state, "the old head"), (&moved, "the new head")] {
        let stale = forge.reader.merge_candidate(asked);
        report.require(
            clause::CANDIDATE_STALE,
            !matches!(&stale, Ok(MergePreview::Ready(_))),
            || {
                format!(
                    "the head moved after the merge was computed; asking about {whose}: {stale:?}"
                )
            },
        );
    }

    let conflicting = harness.provision(fixture);
    conflicting.scenario.conflict();
    let answer = conflicting.reader.merge_candidate(state);
    report.require(
        clause::CANDIDATE_CONFLICT,
        answer == Ok(MergePreview::Conflicting),
        || format!("a merge that conflicts: expected Conflicting, got {answer:?}"),
    );
}

fn check_publication(
    harness: &dyn ForgeHarness,
    fixture: &ForgeFixture,
    report: &mut ConformanceReport,
) {
    let forge = harness.provision(fixture);
    let state = &fixture.pull_request;
    let target = target_for_pr_diff(state, MergeBase::Found(fixture.merge_base.clone()))
        .expect("a found merge-base is a target");
    let moved = PullRequestState {
        head: fixture.moved_head.clone(),
        ..state.clone()
    };
    let diagnosis = ForgeDiagnostic::MergeabilityPending {
        head: moved.head.clone(),
    };
    let reports = [
        CheckReport::for_target(
            &target,
            "conformance",
            CheckConclusion::Success,
            "evaluated",
        ),
        CheckReport::for_diagnosis(
            &moved,
            EvaluationMode::MergedResult,
            "conformance",
            &diagnosis,
        ),
    ];
    for check in reports {
        match forge.publisher.publish(&check) {
            Ok(receipt) => {
                report.require(
                    clause::PUBLISH_AT_HEAD,
                    &receipt.attached_to == check.attached_to(),
                    || {
                        format!(
                            "a check for {} was recorded on {}",
                            check.attached_to().as_str(),
                            receipt.attached_to.as_str()
                        )
                    },
                );
                report.require(
                    clause::PUBLISH_REFERENCE,
                    !receipt.reference.trim().is_empty(),
                    || "the receipt has no reference".to_string(),
                );
            }
            Err(error) => report.violated(
                clause::PUBLISH_AT_HEAD,
                format!(
                    "a check for {} was refused: {error:?}",
                    check.attached_to().as_str()
                ),
            ),
        }
    }
}
