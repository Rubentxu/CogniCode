//! The pure core of a pull-request review: WHICH revisions an
//! evaluation is about, and whether evidence gathered earlier still
//! describes the pull request. No I/O: the use case reads the forge and
//! hands the facts to these functions.

use crate::domain::scm::RevisionHandle;

use super::identity::{MergeBase, MergePreview, PullRequestId, PullRequestState};

/// What a review evaluates. The two answers differ and are never
/// mixed: a clean diff can still merge into a broken result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationMode {
    /// The change the author proposes: merge-base to head.
    PrDiff,
    /// What the base branch would become: base tip to the forge's merge
    /// candidate.
    MergedResult,
}

impl EvaluationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PrDiff => "pr-diff",
            Self::MergedResult => "merged-result",
        }
    }
}

/// The exact revisions one evaluation is about. Evidence produced for a
/// target says nothing about any other pair of revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluationTarget {
    /// `merge_base..head`: the pull request's own change.
    PrDiff {
        pull_request: PullRequestId,
        base_tip: RevisionHandle,
        head: RevisionHandle,
        merge_base: RevisionHandle,
    },
    /// `base_tip..candidate`: the result of merging `head` into
    /// `base_tip`, which is `candidate`.
    MergedResult {
        pull_request: PullRequestId,
        base_tip: RevisionHandle,
        head: RevisionHandle,
        candidate: RevisionHandle,
    },
}

impl EvaluationTarget {
    pub fn mode(&self) -> EvaluationMode {
        match self {
            Self::PrDiff { .. } => EvaluationMode::PrDiff,
            Self::MergedResult { .. } => EvaluationMode::MergedResult,
        }
    }

    pub fn pull_request(&self) -> &PullRequestId {
        match self {
            Self::PrDiff { pull_request, .. } | Self::MergedResult { pull_request, .. } => {
                pull_request
            }
        }
    }

    /// The pull-request head the evaluation started from. A check is
    /// always attached here: it is the revision reviewers see.
    pub fn head(&self) -> &RevisionHandle {
        match self {
            Self::PrDiff { head, .. } | Self::MergedResult { head, .. } => head,
        }
    }

    pub fn base_tip(&self) -> &RevisionHandle {
        match self {
            Self::PrDiff { base_tip, .. } | Self::MergedResult { base_tip, .. } => base_tip,
        }
    }

    /// The revision the change is measured FROM.
    pub fn compare_from(&self) -> &RevisionHandle {
        match self {
            Self::PrDiff { merge_base, .. } => merge_base,
            Self::MergedResult { base_tip, .. } => base_tip,
        }
    }

    /// The revision whose content is evaluated: the head for a diff,
    /// the merge candidate for a merged result.
    pub fn subject(&self) -> &RevisionHandle {
        match self {
            Self::PrDiff { head, .. } => head,
            Self::MergedResult { candidate, .. } => candidate,
        }
    }
}

/// Why no evaluation target exists. Each is a stated diagnosis, never a
/// silent fallback to the other mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeDiagnostic {
    /// Base and head share no history, so there is no honest diff.
    MissingMergeBase {
        base_tip: RevisionHandle,
        head: RevisionHandle,
    },
    /// Merging head into base conflicts: there is no merged result.
    MergeConflict {
        base_tip: RevisionHandle,
        head: RevisionHandle,
    },
    /// The forge has not computed a trustworthy merge candidate yet.
    MergeabilityPending { head: RevisionHandle },
    /// The forge declares it cannot answer what this mode needs.
    CapabilityUnsupported { capability: &'static str },
}

impl ForgeDiagnostic {
    /// A one-line statement for the check summary.
    pub fn summary(&self) -> String {
        match self {
            Self::MissingMergeBase { base_tip, head } => format!(
                "no merge-base between base {} and head {}: the histories are unrelated",
                base_tip.as_str(),
                head.as_str()
            ),
            Self::MergeConflict { base_tip, head } => format!(
                "head {} conflicts with base {}: there is no merged result to evaluate",
                head.as_str(),
                base_tip.as_str()
            ),
            Self::MergeabilityPending { head } => format!(
                "the forge has not computed a merge candidate for head {}; retry later",
                head.as_str()
            ),
            Self::CapabilityUnsupported { capability } => {
                format!("the forge does not support {capability}")
            }
        }
    }
}

/// `merge_base..head`, or the reason it does not exist.
pub fn target_for_pr_diff(
    state: &PullRequestState,
    merge_base: MergeBase,
) -> Result<EvaluationTarget, ForgeDiagnostic> {
    match merge_base {
        MergeBase::Found(merge_base) => Ok(EvaluationTarget::PrDiff {
            pull_request: state.id.clone(),
            base_tip: state.base_tip.clone(),
            head: state.head.clone(),
            merge_base,
        }),
        MergeBase::Absent => Err(ForgeDiagnostic::MissingMergeBase {
            base_tip: state.base_tip.clone(),
            head: state.head.clone(),
        }),
    }
}

/// `base_tip..candidate`, or the reason it does not exist.
pub fn target_for_merged_result(
    state: &PullRequestState,
    candidate: MergePreview,
) -> Result<EvaluationTarget, ForgeDiagnostic> {
    match candidate {
        MergePreview::Ready(candidate) => Ok(EvaluationTarget::MergedResult {
            pull_request: state.id.clone(),
            base_tip: state.base_tip.clone(),
            head: state.head.clone(),
            candidate,
        }),
        MergePreview::Conflicting => Err(ForgeDiagnostic::MergeConflict {
            base_tip: state.base_tip.clone(),
            head: state.head.clone(),
        }),
        MergePreview::Pending => Err(ForgeDiagnostic::MergeabilityPending {
            head: state.head.clone(),
        }),
    }
}

/// Whether evidence for a target still describes the pull request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceFreshness {
    Current,
    /// The pull request moved while the evaluation ran. The evidence is
    /// about `evaluated_head`/`evaluated_base`, no longer the PR.
    Superseded {
        evaluated_head: RevisionHandle,
        current_head: RevisionHandle,
        evaluated_base: RevisionHandle,
        current_base: RevisionHandle,
    },
}

/// Compares the target an evaluation used with the pull request as it
/// is NOW.
///
/// A moved head always supersedes. A moved base tip supersedes only a
/// merged-result evaluation: the merge candidate is a function of both
/// tips. A PR-diff evaluation is anchored by the recorded merge-base and
/// head; base movement can only shrink that diff, so evidence over the
/// recorded range stays a safe over-approximation of the change.
pub fn reconcile(target: &EvaluationTarget, current: &PullRequestState) -> EvidenceFreshness {
    let head_moved = target.head() != &current.head;
    let base_moved =
        target.mode() == EvaluationMode::MergedResult && target.base_tip() != &current.base_tip;
    if !head_moved && !base_moved {
        return EvidenceFreshness::Current;
    }
    EvidenceFreshness::Superseded {
        evaluated_head: target.head().clone(),
        current_head: current.head.clone(),
        evaluated_base: target.base_tip().clone(),
        current_base: current.base_tip.clone(),
    }
}
