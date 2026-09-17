//! Shadow evaluation plan (e82 — M13 task 13.4).
//!
//! Binds a frozen e81 replay plan to two analyzer revisions.
//!
//! ## The evaluation digest is not authority
//!
//! [`ShadowEvaluationPlan::evaluation_digest`] binds: corpus digest, split
//! digest, current revision digest, and candidate revision digest. e82 does
//! **not** consume it as authority. It exists so e83 can later prove:
//!
//! ```text
//! the candidate that passed CONFIRM
//! is exactly the candidate we are trying to promote
//! ```
//!
//! Binding a versioned prefix keeps the digest domain-separated from the e81
//! corpus/split digests.

use crate::application::historical_replay::plan::HistoricalReplayPlan;
use crate::application::portable_execution::content_digest;
use crate::application::shadow_evaluation::analyzer::AnalyzerDescriptor;

/// A frozen replay plan bound to two analyzer revisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowEvaluationPlan {
    replay: HistoricalReplayPlan,
    current: AnalyzerDescriptor,
    candidate: AnalyzerDescriptor,
    evaluation_digest: String,
}

impl ShadowEvaluationPlan {
    /// Bind a replay plan to two analyzer revisions.
    ///
    /// Infallible: the replay plan and both descriptors are already validated.
    /// Identical descriptors are **not** rejected: comparing an analyzer with
    /// itself is a useful control that proves the harness yields zero deltas
    /// (WU17).
    pub fn prepare(
        replay: HistoricalReplayPlan,
        current: AnalyzerDescriptor,
        candidate: AnalyzerDescriptor,
    ) -> Self {
        let evaluation_digest = compute_evaluation_digest(&replay, &current, &candidate);
        Self {
            replay,
            current,
            candidate,
            evaluation_digest,
        }
    }

    /// The frozen e81 replay plan.
    pub fn replay_plan(&self) -> &HistoricalReplayPlan {
        &self.replay
    }

    /// The current-side analyzer identity.
    pub fn current(&self) -> &AnalyzerDescriptor {
        &self.current
    }

    /// The candidate-side analyzer identity.
    pub fn candidate(&self) -> &AnalyzerDescriptor {
        &self.candidate
    }

    /// The evaluation digest binding corpus + split + both revisions.
    pub fn evaluation_digest(&self) -> &str {
        &self.evaluation_digest
    }
}

/// Domain-separated digest over corpus, split, and both analyzer revisions.
fn compute_evaluation_digest(
    replay: &HistoricalReplayPlan,
    current: &AnalyzerDescriptor,
    candidate: &AnalyzerDescriptor,
) -> String {
    let mut canonical = String::new();
    canonical.push_str("shadow-evaluation.v1\n");
    canonical.push_str("corpus\n");
    canonical.push_str(replay.corpus_digest());
    canonical.push('\n');
    canonical.push_str("split\n");
    canonical.push_str(replay.split_digest());
    canonical.push('\n');
    canonical.push_str("current\n");
    canonical.push_str(current.analyzer_id());
    canonical.push('\n');
    canonical.push_str(current.revision_digest());
    canonical.push('\n');
    canonical.push_str("candidate\n");
    canonical.push_str(candidate.analyzer_id());
    canonical.push('\n');
    canonical.push_str(candidate.revision_digest());
    canonical.push('\n');
    content_digest(canonical.as_bytes()).as_str().to_string()
}
