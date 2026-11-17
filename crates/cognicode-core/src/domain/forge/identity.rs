//! What a pull request is as the review pipeline sees it: identity,
//! the revisions that matter, and where the head came from. Revisions
//! reuse the SCM vocabulary ([`RevisionHandle`]) — the forge ports
//! describe the REVIEW around revisions, they do not re-model them.

use crate::domain::scm::{RevisionHandle, SourceRef};

use super::error::ForgeError;

/// One pull request of one forge project: `source` names provider and
/// project (`"github"`, `"owner/repo"`), `number` the request inside it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PullRequestId {
    source: SourceRef,
    number: u64,
}

impl PullRequestId {
    /// Refuses a zero number: forges number from one.
    pub fn new(source: SourceRef, number: u64) -> Result<Self, ForgeError> {
        if number == 0 {
            return Err(ForgeError::InvalidPullRequestNumber);
        }
        Ok(Self { source, number })
    }

    pub fn source(&self) -> &SourceRef {
        &self.source
    }

    pub fn number(&self) -> u64 {
        self.number
    }
}

impl std::fmt::Display for PullRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}#{}",
            self.source.provider().as_str(),
            self.source.project(),
            self.number
        )
    }
}

/// Where the head revision of a pull request lives. It decides how
/// much trust the evaluation of that head gets (see
/// [`ExecutionGrant`](super::ExecutionGrant)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadOrigin {
    /// The head branch is in the same repository as the base: its
    /// author already holds write access there.
    SameRepository,
    /// The head branch lives in another repository (a fork). Its code
    /// is authored by someone with no standing in the base repository.
    Fork,
    /// The forge did not say (a deleted fork, a missing field). Treated
    /// as a fork: absence of proof of trust is not trust.
    Unknown,
}

/// The facts about a pull request that anchor an evaluation. Every
/// revision is an exact handle read from the forge at one instant;
/// nothing here is a branch name that could move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullRequestState {
    pub id: PullRequestId,
    /// The head revision under review.
    pub head: RevisionHandle,
    /// The base branch name the pull request targets.
    pub base_ref: String,
    /// The tip of the base branch when the state was read.
    pub base_tip: RevisionHandle,
    pub origin: HeadOrigin,
}

/// The merge-base of a pull request's base tip and head.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeBase {
    Found(RevisionHandle),
    /// The forge established that base and head share no history.
    Absent,
}

/// The result of merging the head into the base, as the forge reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergePreview {
    /// A merge commit of exactly this head into exactly this base tip.
    Ready(RevisionHandle),
    /// Head and base conflict: no merged result exists.
    Conflicting,
    /// The forge has not (or not yet reliably) computed the merge.
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_request_number_starts_at_one() {
        let source = SourceRef::new("github", "acme/widgets").unwrap();
        assert_eq!(
            PullRequestId::new(source.clone(), 0),
            Err(ForgeError::InvalidPullRequestNumber)
        );
        assert_eq!(
            PullRequestId::new(source, 12).unwrap().to_string(),
            "github:acme/widgets#12"
        );
    }
}
