//! A conforming in-memory forge and a scripted evaluator: the same
//! review use case runs over them with no network and no GitHub.
//!
//! The fake follows the port contracts exactly (unknown pull request,
//! declared capabilities, receipts naming the recorded revision), so a
//! passing review over it says something about the use case and not
//! about the fake. [`FakeForgeHarness`] lets the conformance suites of
//! [`super::conformance`] drive it. It is compiled for unit tests and
//! under the `test-support` feature.

use std::sync::{Arc, Mutex};

use crate::domain::scm::{RevisionHandle, SourceRef};

use super::conformance::{ForgeFixture, ForgeHarness, ForgeScenario, ProvisionedForge, Shared};
use super::error::ForgeError;
use super::grant::ExecutionGrant;
use super::identity::{MergeBase, MergePreview, PullRequestId, PullRequestState};
use super::ports::{
    CheckPublisher, Evaluation, EvaluationError, ForgeCapabilities, PullRequestEvaluator,
    ReviewReader,
};
use super::report::{CheckReport, PublishedCheck};
use super::target::EvaluationTarget;

/// One pull request in memory. Its state can be replaced while a review
/// runs, which is how a head that moves during analysis is reproduced.
pub struct FakeForge {
    capabilities: ForgeCapabilities,
    state: Mutex<PullRequestState>,
    merge_base: Mutex<MergeBase>,
    merge_candidate: Mutex<ComputedCandidate>,
    published: Mutex<Vec<CheckReport>>,
}

/// A merge preview and the two tips it was computed for: a candidate is
/// a function of both, so it answers for no other pair.
struct ComputedCandidate {
    preview: MergePreview,
    head: RevisionHandle,
    base_tip: RevisionHandle,
}

impl ComputedCandidate {
    fn for_state(preview: MergePreview, state: &PullRequestState) -> Self {
        Self {
            preview,
            head: state.head.clone(),
            base_tip: state.base_tip.clone(),
        }
    }
}

impl FakeForge {
    /// A forge with both optional readings declared, no common merge
    /// problems: the merge base is the base tip and the candidate is
    /// pending until configured.
    pub fn new(state: PullRequestState) -> Self {
        let merge_base = MergeBase::Found(state.base_tip.clone());
        Self {
            capabilities: ForgeCapabilities {
                merge_base: true,
                merge_candidate: true,
            },
            merge_candidate: Mutex::new(ComputedCandidate::for_state(
                MergePreview::Pending,
                &state,
            )),
            state: Mutex::new(state),
            merge_base: Mutex::new(merge_base),
            published: Mutex::new(Vec::new()),
        }
    }

    pub fn with_capabilities(mut self, capabilities: ForgeCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_merge_base(self, merge_base: MergeBase) -> Self {
        self.set_merge_base(merge_base);
        self
    }

    /// The merge preview of the pull request as it is now.
    pub fn with_merge_candidate(self, candidate: MergePreview) -> Self {
        self.set_merge_candidate(candidate);
        self
    }

    /// Replaces the merge-base the forge reports.
    pub fn set_merge_base(&self, merge_base: MergeBase) {
        *self.merge_base.lock().expect("fake forge merge-base lock") = merge_base;
    }

    /// Records `candidate` as the merge preview of the pull request as
    /// it is NOW. If either tip moves afterwards the forge answers
    /// `Pending` until a new candidate is recorded.
    pub fn set_merge_candidate(&self, candidate: MergePreview) {
        let computed = ComputedCandidate::for_state(candidate, &self.current());
        *self
            .merge_candidate
            .lock()
            .expect("fake forge candidate lock") = computed;
    }

    /// Replaces the pull request the forge reports from now on.
    pub fn set_state(&self, state: PullRequestState) {
        *self.state.lock().expect("fake forge state lock") = state;
    }

    /// Everything published so far, in order.
    pub fn published(&self) -> Vec<CheckReport> {
        self.published.lock().expect("fake forge log lock").clone()
    }

    fn current(&self) -> PullRequestState {
        self.state.lock().expect("fake forge state lock").clone()
    }
}

impl ReviewReader for FakeForge {
    fn capabilities(&self) -> ForgeCapabilities {
        self.capabilities
    }

    fn pull_request(&self, id: &PullRequestId) -> Result<PullRequestState, ForgeError> {
        let state = self.current();
        if &state.id != id {
            return Err(ForgeError::PullRequestNotFound {
                pull_request: id.to_string(),
            });
        }
        Ok(state)
    }

    fn merge_base(&self, _state: &PullRequestState) -> Result<MergeBase, ForgeError> {
        if !self.capabilities.merge_base {
            return Err(ForgeError::CapabilityUnsupported {
                capability: "merge_base",
            });
        }
        Ok(self
            .merge_base
            .lock()
            .expect("fake forge merge-base lock")
            .clone())
    }

    fn merge_candidate(&self, state: &PullRequestState) -> Result<MergePreview, ForgeError> {
        if !self.capabilities.merge_candidate {
            return Err(ForgeError::CapabilityUnsupported {
                capability: "merge_candidate",
            });
        }
        let current = self.current();
        let computed = self
            .merge_candidate
            .lock()
            .expect("fake forge candidate lock");
        let asked_is_current = state.head == current.head && state.base_tip == current.base_tip;
        let computed_is_current =
            computed.head == current.head && computed.base_tip == current.base_tip;
        if asked_is_current && computed_is_current {
            Ok(computed.preview.clone())
        } else {
            Ok(MergePreview::Pending)
        }
    }
}

impl CheckPublisher for FakeForge {
    fn publish(&self, report: &CheckReport) -> Result<PublishedCheck, ForgeError> {
        let mut log = self.published.lock().expect("fake forge log lock");
        log.push(report.clone());
        Ok(PublishedCheck {
            attached_to: report.attached_to().clone(),
            reference: format!("fake-check-{}", log.len()),
        })
    }
}

/// Builds a [`FakeForge`] for the conformance suites, declaring
/// whichever capabilities the run needs.
pub struct FakeForgeHarness {
    capabilities: ForgeCapabilities,
}

impl FakeForgeHarness {
    pub fn new(capabilities: ForgeCapabilities) -> Self {
        Self { capabilities }
    }
}

/// The events of a [`ForgeScenario`] as edits of one [`FakeForge`].
struct FakeScenario {
    forge: Arc<FakeForge>,
    candidate: RevisionHandle,
}

impl ForgeScenario for FakeScenario {
    fn move_head(&self, to: &RevisionHandle) {
        let mut state = self.forge.current();
        state.head = to.clone();
        self.forge.set_state(state);
    }

    fn compute_candidate(&self) {
        self.forge
            .set_merge_candidate(MergePreview::Ready(self.candidate.clone()));
    }

    fn conflict(&self) {
        self.forge.set_merge_candidate(MergePreview::Conflicting);
    }

    fn unrelate_histories(&self) {
        self.forge.set_merge_base(MergeBase::Absent);
    }
}

impl ForgeHarness for FakeForgeHarness {
    fn source(&self) -> SourceRef {
        SourceRef::new("fake", "acme/widgets").expect("a fixed source")
    }

    fn provision(&self, fixture: &ForgeFixture) -> ProvisionedForge {
        let forge = Arc::new(
            FakeForge::new(fixture.pull_request.clone())
                .with_capabilities(self.capabilities)
                .with_merge_base(MergeBase::Found(fixture.merge_base.clone())),
        );
        ProvisionedForge {
            reader: Arc::new(Shared(Arc::clone(&forge))),
            publisher: Arc::new(Shared(Arc::clone(&forge))),
            scenario: Arc::new(FakeScenario {
                forge,
                candidate: fixture.candidate.clone(),
            }),
        }
    }
}

type DuringEvaluation = Box<dyn Fn() + Send + Sync>;

/// An evaluator that answers what it was told and records what it was
/// given. `during` runs in the middle of the evaluation, where a test
/// moves the head of the pull request.
pub struct ScriptedEvaluator {
    answer: Result<Evaluation, EvaluationError>,
    subject_override: Option<RevisionHandle>,
    during: Option<DuringEvaluation>,
    seen: Mutex<Vec<(EvaluationTarget, ExecutionGrant)>>,
}

impl ScriptedEvaluator {
    /// Approves (or whatever `answer` says) the subject of the target it
    /// receives.
    pub fn answering(answer: Result<Evaluation, EvaluationError>) -> Self {
        Self {
            answer,
            subject_override: None,
            during: None,
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Reports evidence about `subject` instead of the target's subject:
    /// what an evaluator holding stale evidence would do.
    pub fn about(mut self, subject: RevisionHandle) -> Self {
        self.subject_override = Some(subject);
        self
    }

    pub fn during(mut self, action: impl Fn() + Send + Sync + 'static) -> Self {
        self.during = Some(Box::new(action));
        self
    }

    /// The targets and grants received, in order.
    pub fn seen(&self) -> Vec<(EvaluationTarget, ExecutionGrant)> {
        self.seen.lock().expect("evaluator log lock").clone()
    }
}

impl PullRequestEvaluator for ScriptedEvaluator {
    fn evaluate(
        &self,
        target: &EvaluationTarget,
        grant: &ExecutionGrant,
    ) -> Result<Evaluation, EvaluationError> {
        self.seen
            .lock()
            .expect("evaluator log lock")
            .push((target.clone(), *grant));
        if let Some(action) = &self.during {
            action();
        }
        let mut answer = self.answer.clone()?;
        answer.subject = self
            .subject_override
            .clone()
            .unwrap_or_else(|| target.subject().clone());
        Ok(answer)
    }
}
