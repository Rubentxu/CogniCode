//! Forge-neutral pull-request review: the vocabulary, the pure planning
//! core and the ports a code-hosting platform must satisfy.
//!
//! Boundaries:
//! * Forge ports are separate from the SCM ports of [`crate::domain::scm`]:
//!   revisions stay [`RevisionHandle`](crate::domain::scm::RevisionHandle)s;
//!   this module adds the review around them (pull request, merge
//!   candidate, check).
//! * Nothing here performs I/O. `target` and [`grant`](crate::domain::forge::grant) are pure; the
//!   ports are satisfied by adapters (`infrastructure::github`) and by
//!   the conforming fake in `fake`.
//! * Policy is not here. The [`PullRequestEvaluator`](crate::domain::forge::PullRequestEvaluator) seam is what the
//!   policy side implements; this module only guarantees the evidence it
//!   receives and the revision a result is published on.

mod error;
pub mod grant;
mod identity;
mod ports;
mod report;
mod target;

#[cfg(any(test, feature = "test-support"))]
pub mod conformance;
#[cfg(test)]
pub(crate) mod conformance_tests;
#[cfg(any(test, feature = "test-support"))]
pub mod fake;

pub use error::ForgeError;
pub use grant::{ExecutionGrant, TrustedExecution};
pub use identity::{HeadOrigin, MergeBase, MergePreview, PullRequestId, PullRequestState};
pub use ports::{
    CheckPublisher, Evaluation, EvaluationError, ForgeCapabilities, PullRequestEvaluator,
    ReviewReader,
};
pub use report::{CheckConclusion, CheckReport, PublishedCheck, SUMMARY_LIMIT_CHARS};
pub use target::{
    EvaluationMode, EvaluationTarget, EvidenceFreshness, ForgeDiagnostic, reconcile,
    target_for_merged_result, target_for_pr_diff,
};
