//! FailureRegime taxonomy (e82 — M13 task 13.3).
//!
//! ## Description, not severity
//!
//! A [`FailureRegime`] names *what kind* of thing went wrong. It is not a
//! severity, a policy, a block decision, or a promotion decision. e82 emits
//! classifications; a later cycle decides what they mean.
//!
//! ## Evidence, never string parsing
//!
//! A regime occurrence always carries typed provenance
//! ([`FailureRegimeEvidence`]). Classification never inspects arbitrary
//! observation strings: an observation whose id merely contains
//! `"architecture"` cannot produce an [`FailureRegime::ArchitectureMiss`].
//! Where an existing subsystem does not expose enough typed information, the
//! answer is an explicit [`FailureRegimeClassifier`] adapter, not a heuristic.
//!
//! ## The generic scorer does not know domains
//!
//! `ScoreMatrix::false_negative > 0` must NOT be mapped to
//! [`FailureRegime::ImpactMiss`]: the replay scorer does not know whether the
//! prediction domain was impact, architecture, security, or anything else. Only
//! a classifier that knows the domain may emit those regimes.

use crate::application::historical_replay::case::{HistoricalCase, HistoricalCaseId};
use crate::application::historical_replay::replay::ReplayCaseOutcome;
use crate::application::historical_replay::split::DatasetRole;
use crate::application::self_hosting::platform_equivalence::PlatformKind;

/// A descriptive category for a shadow-evaluation finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureRegime {
    /// Canonical grounding/evidence needed for the analysis is invalid, missing,
    /// contradictory, or cannot support the expected conclusion.
    GroundingFailure,
    /// Independently observed affected work was not predicted.
    ImpactMiss,
    /// A declared or independently observed architecture violation was missed.
    ArchitectureMiss,
    /// Semantic/provider capability degraded, fell back, became unavailable, or
    /// violated its expected contract.
    ProviderDegradation,
    /// Equivalent logical input produced semantically divergent normalized
    /// results across platforms.
    PlatformDivergence,
    /// The evaluation cannot honestly score the case because mandatory
    /// observation/evidence is absent.
    EvidenceIncomplete,
}

impl FailureRegime {
    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::GroundingFailure => "grounding-failure",
            Self::ImpactMiss => "impact-miss",
            Self::ArchitectureMiss => "architecture-miss",
            Self::ProviderDegradation => "provider-degradation",
            Self::PlatformDivergence => "platform-divergence",
            Self::EvidenceIncomplete => "evidence-incomplete",
        }
    }
}

/// Which side a regime occurrence is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FailureRegimeSubject {
    /// The current analyzer.
    Current,
    /// The candidate analyzer.
    Candidate,
    /// The evaluation itself (e.g. the recorded historical observations).
    SharedEvaluation,
}

impl FailureRegimeSubject {
    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Candidate => "candidate",
            Self::SharedEvaluation => "shared-evaluation",
        }
    }
}

/// Typed provenance explaining why a regime was emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureRegimeEvidence {
    /// The e81 replay could not score the case because no observation was
    /// recorded.
    IncompleteObservation,
    /// e76 platform equivalence found divergence for a named observation.
    PlatformDivergence {
        /// The observation id that diverged.
        observation_id: String,
        /// The platforms involved.
        platforms: Vec<PlatformKind>,
    },
    /// Typed evidence supplied by a classifier adapter for a subsystem whose
    /// diagnostics are not yet expressible as a built-in variant.
    Adapter {
        /// Stable identifier of the adapter that emitted this occurrence.
        adapter: String,
        /// Bounded, adapter-specific explanation.
        detail: String,
    },
}

/// One classified regime occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureRegimeOccurrence {
    /// The case the occurrence is about.
    pub case_id: HistoricalCaseId,
    /// The dataset role under evaluation.
    pub dataset_role: DatasetRole,
    /// Which side it is about.
    pub subject: FailureRegimeSubject,
    /// The regime.
    pub regime: FailureRegime,
    /// Why it was emitted.
    pub evidence: FailureRegimeEvidence,
}

/// Bounded typed input to a classifier.
///
/// The classifier is downstream of scoring, so it may see both outcomes; the
/// no-leak rule applies to predictors, not to classification.
pub struct FailureRegimeContext<'a> {
    /// The historical case (its recorded observations are outcome-side).
    pub case: &'a HistoricalCase,
    /// The dataset role under evaluation.
    pub role: DatasetRole,
    /// The current-side outcome.
    pub current: &'a ReplayCaseOutcome,
    /// The candidate-side outcome.
    pub candidate: &'a ReplayCaseOutcome,
}

/// Port: classify failure regimes from typed shadow-evaluation context.
///
/// Implementations MUST be deterministic, perform no I/O, mutate nothing, and
/// grant no authority.
pub trait FailureRegimeClassifier {
    /// Classify zero or more regimes for one case.
    fn classify(&self, context: &FailureRegimeContext<'_>) -> Vec<FailureRegimeOccurrence>;
}

/// The fail-closed classifier: classifies nothing.
///
/// Useful when a caller wants raw deltas without diagnosis.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFailureRegimes;

impl FailureRegimeClassifier for NoFailureRegimes {
    fn classify(&self, _context: &FailureRegimeContext<'_>) -> Vec<FailureRegimeOccurrence> {
        Vec::new()
    }
}
