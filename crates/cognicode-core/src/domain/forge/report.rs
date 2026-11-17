//! What a review publishes: a conclusion attached to ONE revision.

use crate::domain::scm::RevisionHandle;

use super::identity::{PullRequestId, PullRequestState};
use super::target::{EvaluationMode, EvaluationTarget, ForgeDiagnostic};

/// Longest summary a report carries, in characters. Forges cap check
/// text; an over-long summary is cut at a character boundary and says so.
pub const SUMMARY_LIMIT_CHARS: usize = 60_000;

const TRUNCATION_MARK: &str = "\n[summary truncated]";

/// The verdict a check carries.
///
/// There is deliberately no neutral or skipped variant: forges count
/// those as passing for required checks, and a review that could not
/// reach a verdict must not look like approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckConclusion {
    /// The evaluation approved the evaluated revisions.
    Success,
    /// The evaluation found a violation.
    Failure,
    /// Nothing was approved or rejected; the author has to act (a
    /// conflict, unrelated histories).
    ActionRequired,
    /// No verdict exists for the evaluated revisions: the evidence went
    /// stale, the evaluation failed, or the forge was not ready.
    Cancelled,
}

impl CheckConclusion {
    pub fn is_approval(self) -> bool {
        self == Self::Success
    }
}

/// One publishable result. The revision it attaches to is fixed at
/// construction from the evaluated head: there is no way to build a
/// report for a different revision than the one the evidence is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckReport {
    pull_request: PullRequestId,
    attached_to: RevisionHandle,
    mode: EvaluationMode,
    evaluated: Option<RevisionHandle>,
    name: String,
    conclusion: CheckConclusion,
    summary: String,
}

fn bounded(summary: &str) -> String {
    if summary.chars().count() <= SUMMARY_LIMIT_CHARS {
        return summary.to_string();
    }
    let mut cut: String = summary.chars().take(SUMMARY_LIMIT_CHARS).collect();
    cut.push_str(TRUNCATION_MARK);
    cut
}

impl CheckReport {
    /// A verdict over an evaluated target, attached to its head.
    pub fn for_target(
        target: &EvaluationTarget,
        name: &str,
        conclusion: CheckConclusion,
        summary: &str,
    ) -> Self {
        Self {
            pull_request: target.pull_request().clone(),
            attached_to: target.head().clone(),
            mode: target.mode(),
            evaluated: Some(target.subject().clone()),
            name: name.to_string(),
            conclusion,
            summary: bounded(summary),
        }
    }

    /// A diagnosis that stopped planning, attached to the head that was
    /// read. Conflicts and missing ancestry need the author
    /// ([`CheckConclusion::ActionRequired`]); the rest have no verdict yet.
    pub fn for_diagnosis(
        state: &PullRequestState,
        mode: EvaluationMode,
        name: &str,
        diagnostic: &ForgeDiagnostic,
    ) -> Self {
        let conclusion = match diagnostic {
            ForgeDiagnostic::MissingMergeBase { .. } | ForgeDiagnostic::MergeConflict { .. } => {
                CheckConclusion::ActionRequired
            }
            ForgeDiagnostic::MergeabilityPending { .. }
            | ForgeDiagnostic::CapabilityUnsupported { .. } => CheckConclusion::Cancelled,
        };
        Self {
            pull_request: state.id.clone(),
            attached_to: state.head.clone(),
            mode,
            evaluated: None,
            name: name.to_string(),
            conclusion,
            summary: bounded(&diagnostic.summary()),
        }
    }

    pub fn pull_request(&self) -> &PullRequestId {
        &self.pull_request
    }

    /// The revision the check is published on.
    pub fn attached_to(&self) -> &RevisionHandle {
        &self.attached_to
    }

    pub fn mode(&self) -> EvaluationMode {
        self.mode
    }

    /// The revision whose content was evaluated, when an evaluation ran.
    pub fn evaluated(&self) -> Option<&RevisionHandle> {
        self.evaluated.as_ref()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn conclusion(&self) -> CheckConclusion {
        self.conclusion
    }

    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// The forge's receipt for a published check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedCheck {
    /// The revision the forge recorded the check on.
    pub attached_to: RevisionHandle,
    /// Forge-side locator (a URL or an id), for humans and logs.
    pub reference: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::forge::{HeadOrigin, MergeBase};
    use crate::domain::scm::SourceRef;

    fn state() -> PullRequestState {
        PullRequestState {
            id: PullRequestId::new(SourceRef::new("fake", "a/b").unwrap(), 1).unwrap(),
            head: RevisionHandle::new("head").unwrap(),
            base_ref: "main".to_string(),
            base_tip: RevisionHandle::new("base").unwrap(),
            origin: HeadOrigin::SameRepository,
        }
    }

    #[test]
    fn an_over_long_summary_is_cut_on_a_character_boundary_and_says_so() {
        let target = crate::domain::forge::target_for_pr_diff(
            &state(),
            MergeBase::Found(RevisionHandle::new("base").unwrap()),
        )
        .unwrap();
        let long = "é".repeat(SUMMARY_LIMIT_CHARS + 10);

        let report = CheckReport::for_target(&target, "check", CheckConclusion::Failure, &long);

        assert!(report.summary().ends_with("[summary truncated]"));
        assert_eq!(
            report.summary().chars().count(),
            SUMMARY_LIMIT_CHARS + TRUNCATION_MARK.chars().count()
        );
    }

    #[test]
    fn only_success_is_an_approval() {
        assert!(CheckConclusion::Success.is_approval());
        for other in [
            CheckConclusion::Failure,
            CheckConclusion::ActionRequired,
            CheckConclusion::Cancelled,
        ] {
            assert!(!other.is_approval());
        }
    }
}
