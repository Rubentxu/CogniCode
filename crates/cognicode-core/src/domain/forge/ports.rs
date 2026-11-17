//! The ports of the review pipeline. Forge ports are NOT SCM ports: a
//! forge knows pull requests, merge candidates and checks; the SCM
//! ports ([`crate::domain::scm`]) know revisions and their content.
//! A GitHub review reads PR facts through [`ReviewReader`], revisions
//! through the SCM ports, and publishes through [`CheckPublisher`].
//!
//! Like the SCM ports they are narrow and synchronous, and fail with
//! named errors instead of invented data.
//!
//! ## Modeling another forge
//!
//! A forge adapter declares what it can answer in [`ForgeCapabilities`]
//! and implements the two ports; nothing else changes. A forge without
//! a precomputed merge preview (Bitbucket-style) declares
//! `merge_candidate: false` and the review reports a
//! [`ForgeDiagnostic::CapabilityUnsupported`](super::ForgeDiagnostic)
//! for the merged-result mode while the PR-diff mode keeps working. No
//! adapter exists for such a forge until one is needed; the conforming
//! fake in [`super::fake`] is the executable form of this contract.

use crate::domain::scm::RevisionHandle;

use super::error::ForgeError;
use super::grant::ExecutionGrant;
use super::identity::{MergeBase, MergePreview, PullRequestId, PullRequestState};
use super::report::{CheckConclusion, CheckReport, PublishedCheck};
use super::target::EvaluationTarget;

/// Which optional readings a forge answers. The zero value promises
/// nothing; a consumer asking for more gets a diagnostic, not a guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForgeCapabilities {
    /// The forge can establish the merge-base of base and head.
    pub merge_base: bool,
    /// The forge computes the merge of head into base.
    pub merge_candidate: bool,
}

impl ForgeCapabilities {
    pub const NONE: Self = Self {
        merge_base: false,
        merge_candidate: false,
    };
}

/// Reads the pull-request facts of a forge.
pub trait ReviewReader: Send + Sync {
    fn capabilities(&self) -> ForgeCapabilities;

    /// The pull request as the forge reports it NOW: exact head and base
    /// revisions and the origin of the head. Unknown pull request:
    /// `PullRequestNotFound`.
    fn pull_request(&self, id: &PullRequestId) -> Result<PullRequestState, ForgeError>;

    /// The merge-base of `state.base_tip` and `state.head`. Requires the
    /// `merge_base` capability. Histories without a common ancestor are
    /// `Ok(MergeBase::Absent)`, not an error.
    fn merge_base(&self, state: &PullRequestState) -> Result<MergeBase, ForgeError>;

    /// The merge of `state.head` into `state.base_tip`. Requires the
    /// `merge_candidate` capability. A candidate is `Ready` only when it
    /// is a merge of exactly those two revisions; a stale or unverifiable
    /// one is `Pending`.
    fn merge_candidate(&self, state: &PullRequestState) -> Result<MergePreview, ForgeError>;
}

/// Publishes a review result to the forge.
pub trait CheckPublisher: Send + Sync {
    /// Publishes `report` on `report.attached_to()` and nowhere else.
    /// The receipt names the revision the forge actually recorded.
    fn publish(&self, report: &CheckReport) -> Result<PublishedCheck, ForgeError>;
}

/// The verdict an evaluator reached over one target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evaluation {
    /// The revision whose content the evidence is about. The review
    /// refuses an evaluation whose subject is not the target's.
    pub subject: RevisionHandle,
    pub conclusion: CheckConclusion,
    pub summary: String,
}

/// The evaluator could not reach a verdict.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("evaluation failed: {detail}")]
pub struct EvaluationError {
    pub detail: String,
}

/// The policy seam. CogniCode decides WHICH revisions are evaluated and
/// whether the result is still current; what counts as approval is the
/// evaluator's policy (the Guard side), never decided here.
pub trait PullRequestEvaluator: Send + Sync {
    /// Evaluates `target` within `grant`. Implementations must respect
    /// the grant: a [`ExecutionGrant::Sandboxed`] evaluation analyzes the
    /// revisions as data only.
    fn evaluate(
        &self,
        target: &EvaluationTarget,
        grant: &ExecutionGrant,
    ) -> Result<Evaluation, EvaluationError>;
}
