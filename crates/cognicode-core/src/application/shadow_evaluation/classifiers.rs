//! Built-in failure-regime classifiers (e82 — M13 task 13.3).
//!
//! ## What is implemented for real
//!
//! ```text
//! EvidenceIncomplete   DIRECT: e81's ReplayCaseOutcome::Incomplete
//! PlatformDivergence   DIRECT: e76 compare_platforms, only when a case actually
//!                      carries two or more distinct platforms
//! ```
//!
//! ## What is a contract plus a seam, deliberately
//!
//! ```text
//! GroundingFailure     no typed per-case grounding signal reaches the replay
//! ImpactMiss           the replay scorer does not know the prediction domain
//! ArchitectureMiss     ArchitectureViolation exists, but nothing maps a
//!                      historical case's observations onto typed violations
//! ProviderDegradation  no typed provider-health signal reaches the replay
//! ```
//!
//! These four are NOT guessed. `false_negative > 0` is not an impact miss, and
//! an observation id containing `"architecture"` is not an architecture miss.
//! A caller that has real typed evidence supplies a [`FailureRegimeClassifier`]
//! adapter (see `shadow_evaluation_tests.rs` for a deterministic example), which
//! is the documented integration seam.

use std::collections::BTreeSet;

use crate::application::self_hosting::platform_equivalence::{
    Equivalence, PlatformKind, PlatformNormaliser, compare_platforms,
};
use crate::application::shadow_evaluation::regime::{
    FailureRegime, FailureRegimeClassifier, FailureRegimeContext, FailureRegimeEvidence,
    FailureRegimeOccurrence, FailureRegimeSubject,
};

/// Classifies the regimes that existing typed signals can support honestly.
pub struct BuiltinFailureRegimeClassifier<'a, N: PlatformNormaliser> {
    normaliser: &'a N,
}

impl<'a, N: PlatformNormaliser> BuiltinFailureRegimeClassifier<'a, N> {
    /// Construct over a platform normaliser.
    pub fn new(normaliser: &'a N) -> Self {
        Self { normaliser }
    }
}

impl<N: PlatformNormaliser> FailureRegimeClassifier for BuiltinFailureRegimeClassifier<'_, N> {
    fn classify(&self, context: &FailureRegimeContext<'_>) -> Vec<FailureRegimeOccurrence> {
        let mut out = Vec::new();

        // EvidenceIncomplete — directly supported by the e81 replay outcome.
        for (subject, outcome) in [
            (FailureRegimeSubject::Current, context.current),
            (FailureRegimeSubject::Candidate, context.candidate),
        ] {
            if let crate::application::historical_replay::replay::ReplayCaseOutcome::Incomplete(_) =
                outcome
            {
                out.push(FailureRegimeOccurrence {
                    case_id: context.case.id().clone(),
                    dataset_role: context.role,
                    subject,
                    regime: FailureRegime::EvidenceIncomplete,
                    evidence: FailureRegimeEvidence::IncompleteObservation,
                });
            }
        }

        // PlatformDivergence — only when the case genuinely carries two or more
        // distinct platforms. A single-platform case is never divergence.
        let observations = context.case.replay().observations.as_slice();
        let platforms: BTreeSet<PlatformKind> = observations.iter().map(|o| o.platform).collect();
        if platforms.len() >= 2 {
            for (observation_id, equivalence) in compare_platforms(self.normaliser, observations) {
                if let Equivalence::Divergent { platforms } = equivalence {
                    out.push(FailureRegimeOccurrence {
                        case_id: context.case.id().clone(),
                        dataset_role: context.role,
                        subject: FailureRegimeSubject::SharedEvaluation,
                        regime: FailureRegime::PlatformDivergence,
                        evidence: FailureRegimeEvidence::PlatformDivergence {
                            observation_id,
                            platforms,
                        },
                    });
                }
            }
        }

        out
    }
}
