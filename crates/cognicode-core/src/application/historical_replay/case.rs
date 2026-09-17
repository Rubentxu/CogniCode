//! Historical case identity + the base-only prediction input (e81 — M13).
//!
//! ## Base and outcome are distinct
//!
//! The M13 contract is "predict from the base, then compare against the
//! successor". A single ambiguous snapshot id cannot express that, so a
//! [`HistoricalCase`] carries an explicit `base_ref` and `outcome_ref`.
//!
//! ## The predictor never sees the case
//!
//! A predictor receives [`HistoricalPredictionInput`], which contains **only**
//! what existed at the historical base: the case id, the base snapshot, and the
//! base source reference. There is no field that could carry a successor
//! digest, an observation, a ground-truth outcome, a score, or a confirm
//! result. This turns e76's "predict first, observe after" from a convention
//! into an API boundary.
//!
//! The secret side (`outcome_ref` and the [`HistoricalReplay`] payload) stays on
//! the case, which is never handed to a predictor.

use crate::application::portable_execution::content_digest;
use crate::application::self_hosting::platform_equivalence::HistoricalReplay;

/// Stable identifier of one historical case.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HistoricalCaseId(String);

impl HistoricalCaseId {
    /// Construct, rejecting an empty/whitespace id.
    pub fn try_new(raw: impl Into<String>) -> Result<Self, HistoricalCaseError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(HistoricalCaseError::EmptyCaseId);
        }
        Ok(Self(raw))
    }

    /// Borrow the id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HistoricalCaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An opaque, project-neutral reference to a recorded state.
///
/// Deliberately not a path, a Git ref, or a filesystem location: the historical
/// replay contract must not require repository access, and the caller decides
/// how a reference resolves.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContentRef(String);

impl ContentRef {
    /// Construct, rejecting an empty/whitespace reference.
    pub fn try_new(raw: impl Into<String>) -> Result<Self, HistoricalCaseError> {
        let raw = raw.into();
        if raw.trim().is_empty() {
            return Err(HistoricalCaseError::EmptyContentRef);
        }
        Ok(Self(raw))
    }

    /// Borrow the reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ContentRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One historical evaluation case.
///
/// Holds the e76 [`HistoricalReplay`] payload (recorded observations and an
/// optional sealed prediction) next to e81's case identity and explicit
/// base/successor separation. `HistoricalReplay` keeps its own semantics; it
/// does not own dataset policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalCase {
    id: HistoricalCaseId,
    base_ref: ContentRef,
    base_snapshot: String,
    outcome_ref: ContentRef,
    replay: HistoricalReplay,
}

impl HistoricalCase {
    /// Construct a case.
    ///
    /// `base_snapshot` is the digest of the base state the prediction must start
    /// from; `outcome_ref` names the observed successor. They are recorded
    /// separately even when they happen to be equal (a no-op historical change
    /// is degenerate but not illegal).
    pub fn try_new(
        id: HistoricalCaseId,
        base_ref: ContentRef,
        base_snapshot: impl Into<String>,
        outcome_ref: ContentRef,
        replay: HistoricalReplay,
    ) -> Result<Self, HistoricalCaseError> {
        let base_snapshot = base_snapshot.into();
        if base_snapshot.trim().is_empty() {
            return Err(HistoricalCaseError::EmptyBaseSnapshot);
        }
        Ok(Self {
            id,
            base_ref,
            base_snapshot,
            outcome_ref,
            replay,
        })
    }

    /// The case id.
    pub fn id(&self) -> &HistoricalCaseId {
        &self.id
    }

    /// The base-side source reference. Safe for prediction.
    pub fn base_ref(&self) -> &ContentRef {
        &self.base_ref
    }

    /// The base snapshot digest. Safe for prediction.
    pub fn base_snapshot(&self) -> &str {
        &self.base_snapshot
    }

    /// The successor reference. Outcome side: never handed to a predictor.
    pub fn outcome_ref(&self) -> &ContentRef {
        &self.outcome_ref
    }

    /// The e76 replay payload (observations + optional seal). Outcome side:
    /// never handed to a predictor.
    pub fn replay(&self) -> &HistoricalReplay {
        &self.replay
    }

    /// The only view of a case a predictor may receive.
    pub fn prediction_input(&self) -> HistoricalPredictionInput {
        HistoricalPredictionInput {
            case_id: self.id.clone(),
            base_snapshot: self.base_snapshot.clone(),
            base_source_ref: self.base_ref.clone(),
        }
    }

    /// Canonical content digest of the case, used for the corpus digest.
    ///
    /// Covers identity, both refs, the base snapshot, and the replay payload
    /// (observations and, when present, the historical seal). Sensitive to
    /// observation content so a changed corpus yields a changed digest.
    pub fn content_digest(&self) -> String {
        let mut canonical = String::new();
        canonical.push_str("case\n");
        canonical.push_str(self.id.as_str());
        canonical.push('\n');
        canonical.push_str(self.base_ref.as_str());
        canonical.push('\n');
        canonical.push_str(&self.base_snapshot);
        canonical.push('\n');
        canonical.push_str(self.outcome_ref.as_str());
        canonical.push('\n');
        canonical.push_str(&self.replay.snapshot_digest);
        canonical.push('\n');
        for obs in &self.replay.observations {
            canonical.push_str("obs\n");
            canonical.push_str(&format!("{:?}", obs.platform));
            canonical.push('\n');
            canonical.push_str(&obs.id);
            canonical.push('\n');
            for b in &obs.raw {
                canonical.push_str(&format!("{b:02x}"));
            }
            canonical.push('\n');
        }
        match &self.replay.sealed_prediction {
            Some(p) => {
                canonical.push_str("sealed\n");
                canonical.push_str(&p.label);
                canonical.push('\n');
                canonical.push_str(&p.seal_digest);
                canonical.push('\n');
            }
            None => canonical.push_str("sealed:none\n"),
        }
        content_digest(canonical.as_bytes()).as_str().to_string()
    }
}

/// The base-side-only prediction input.
///
/// Structurally cannot carry the outcome: there is no successor, observation,
/// score, or ground-truth field, and no way to derive one from these three.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalPredictionInput {
    /// Which case is being predicted.
    pub case_id: HistoricalCaseId,
    /// Digest of the base state the prediction starts from.
    pub base_snapshot: String,
    /// Reference to the base source the predictor may inspect.
    pub base_source_ref: ContentRef,
}

/// Why a historical case could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalCaseError {
    /// The case id was empty.
    EmptyCaseId,
    /// A content reference was empty.
    EmptyContentRef,
    /// The base snapshot digest was empty.
    EmptyBaseSnapshot,
}

impl std::fmt::Display for HistoricalCaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyCaseId => f.write_str("historical case id must not be empty"),
            Self::EmptyContentRef => f.write_str("content reference must not be empty"),
            Self::EmptyBaseSnapshot => f.write_str("base snapshot digest must not be empty"),
        }
    }
}

impl std::error::Error for HistoricalCaseError {}
