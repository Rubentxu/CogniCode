//! A conforming in-memory forge and a scripted evaluator: the same
//! review use case runs over them with no network and no GitHub.
//!
//! The fake follows the port contracts exactly (unknown pull request,
//! declared capabilities, receipts naming the recorded revision), so a
//! passing review over it says something about the use case and not
//! about the fake. It is compiled for unit tests and under the
//! `test-support` feature.

use std::sync::Mutex;

use crate::domain::scm::RevisionHandle;

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
    merge_base: MergeBase,
    merge_candidate: MergePreview,
    published: Mutex<Vec<CheckReport>>,
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
            state: Mutex::new(state),
            merge_base,
            merge_candidate: MergePreview::Pending,
            published: Mutex::new(Vec::new()),
        }
    }

    pub fn with_capabilities(mut self, capabilities: ForgeCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_merge_base(mut self, merge_base: MergeBase) -> Self {
        self.merge_base = merge_base;
        self
    }

    pub fn with_merge_candidate(mut self, candidate: MergePreview) -> Self {
        self.merge_candidate = candidate;
        self
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
        Ok(self.merge_base.clone())
    }

    fn merge_candidate(&self, _state: &PullRequestState) -> Result<MergePreview, ForgeError> {
        if !self.capabilities.merge_candidate {
            return Err(ForgeError::CapabilityUnsupported {
                capability: "merge_candidate",
            });
        }
        Ok(self.merge_candidate.clone())
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
