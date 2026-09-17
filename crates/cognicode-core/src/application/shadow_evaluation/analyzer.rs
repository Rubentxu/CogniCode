//! Analyzer identity and evaluation roles (e82 — M13 task 13.4).
//!
//! ## Identity is a revision, not a label
//!
//! Shadow evaluation must compare exact revisions, never names like `"old"` and
//! `"new"`. [`AnalyzerDescriptor`] carries a stable analyzer id plus a revision
//! digest that identifies the exact analyzer semantics/configuration being
//! evaluated.
//!
//! ## Role is not identity
//!
//! `Current` and `Candidate` are **evaluation roles**, not analyzer identity, so
//! they do not live inside the descriptor. [`AnalyzerSide`] expresses the role.
//! A useful consequence: current and candidate may legitimately share a
//! descriptor (a control experiment), which is how the harness itself is proven.

use crate::application::historical_replay::replay::HistoricalPredictor;
use crate::application::portable_execution::content_digest;

/// Exact identity of one analyzer revision.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnalyzerDescriptor {
    analyzer_id: String,
    revision_digest: String,
}

impl AnalyzerDescriptor {
    /// Construct from an analyzer id and a revision digest.
    ///
    /// Both must be non-empty; the revision digest must not contain whitespace
    /// (a revision is an opaque token, not a sentence).
    pub fn try_new(
        analyzer_id: impl Into<String>,
        revision_digest: impl Into<String>,
    ) -> Result<Self, AnalyzerError> {
        let analyzer_id = analyzer_id.into();
        let revision_digest = revision_digest.into();
        if analyzer_id.trim().is_empty() {
            return Err(AnalyzerError::EmptyAnalyzerId);
        }
        if revision_digest.trim().is_empty() {
            return Err(AnalyzerError::EmptyRevisionDigest);
        }
        if revision_digest.chars().any(char::is_whitespace) {
            return Err(AnalyzerError::MalformedRevisionDigest);
        }
        Ok(Self {
            analyzer_id,
            revision_digest,
        })
    }

    /// Derive the revision digest from the analyzer's configuration bytes using
    /// the canonical content digest, so two runs with the same configuration
    /// produce the same descriptor.
    pub fn from_config(
        analyzer_id: impl Into<String>,
        config: &[u8],
    ) -> Result<Self, AnalyzerError> {
        Self::try_new(analyzer_id, content_digest(config).as_str())
    }

    /// The stable analyzer id.
    pub fn analyzer_id(&self) -> &str {
        &self.analyzer_id
    }

    /// The exact revision digest.
    pub fn revision_digest(&self) -> &str {
        &self.revision_digest
    }
}

/// Which side of a shadow evaluation an analyzer plays.
///
/// A role, not an identity: the same descriptor may appear on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnalyzerSide {
    /// The analyzer currently in production.
    Current,
    /// The analyzer under evaluation.
    Candidate,
}

impl AnalyzerSide {
    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Candidate => "candidate",
        }
    }
}

/// An analyzer implementation bound to its descriptor for one evaluation side.
///
/// Both sides reuse the **same** [`HistoricalPredictor`] contract. There is no
/// `CurrentAnalyzerTrait`/`CandidateAnalyzerTrait`: the two sides perform the
/// same semantic operation and differ only by role.
///
/// Cheap to copy: it holds only shared references.
#[derive(Clone, Copy)]
pub struct AnalyzerUnderTest<'a> {
    descriptor: &'a AnalyzerDescriptor,
    predictor: &'a dyn HistoricalPredictor,
}

impl<'a> AnalyzerUnderTest<'a> {
    /// Bind a descriptor to an implementation.
    pub fn new(descriptor: &'a AnalyzerDescriptor, predictor: &'a dyn HistoricalPredictor) -> Self {
        Self {
            descriptor,
            predictor,
        }
    }

    /// The analyzer identity.
    pub fn descriptor(&self) -> &'a AnalyzerDescriptor {
        self.descriptor
    }

    /// The implementation.
    pub fn predictor(&self) -> &'a dyn HistoricalPredictor {
        self.predictor
    }
}

/// Why an analyzer descriptor could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalyzerError {
    /// The analyzer id was empty.
    EmptyAnalyzerId,
    /// The revision digest was empty.
    EmptyRevisionDigest,
    /// The revision digest contained whitespace.
    MalformedRevisionDigest,
}

impl std::fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyAnalyzerId => f.write_str("analyzer id must not be empty"),
            Self::EmptyRevisionDigest => f.write_str("analyzer revision digest must not be empty"),
            Self::MalformedRevisionDigest => {
                f.write_str("analyzer revision digest must not contain whitespace")
            }
        }
    }
}

impl std::error::Error for AnalyzerError {}
