//! Granular detector-result reuse (CAA-07): what one detector run
//! depended on, what it produced, and when the product may be reused.
//!
//! ## Why the identity has layers
//!
//! A detector run consumes four kinds of input, and each invalidates
//! on its own schedule:
//!
//! - **rule** — the detector definition (`DetectorDigests`), its
//!   version, and the authority it executed under;
//! - **capability** — what the run could actually use: the detector's
//!   `requires` intersected with the backends the environment offered
//!   (the *effective* capabilities, which the execution ref does not
//!   capture on its own);
//! - **parser** — the digest of the parse/extraction layer feeding
//!   the run;
//! - **sanitizer** — the digest of any sanitizer/normalization layer;
//! - **semantic dependency** — the facts the run read, as a
//!   [`ReadSet`].
//!
//! Reuse requires ALL layers to survive: [`reuse_decision`] reports
//! exactly the layers that died and nothing else, so one rule edit
//! never re-runs a detector whose parser, sanitizer and facts are
//! untouched.
//!
//! ## Output location is not semantic identity
//!
//! A finding's [`OutputLocation`]s (file, line, column) are rendering
//! data; a finding's [`semantic_fingerprint`] is a digest over the
//! detector's semantic digest, the grounded subject, and the finding
//! kind. Moving a line or renaming a file keeps the fingerprint;
//! re-grounding onto a different subject changes it. Reuse decisions
//! never read locations.
//!
//! ## No promotion, ever
//!
//! [`CachedObservation`] mirrors [`GuardObservation`] variant for
//! variant — there is no mapping whose codomain can produce
//! `Verifiable` from an `Advisory`, `Inconclusive`, `Unsupported`,
//! `Ungrounded` or `Failed` input, and rebinding onto a candidate
//! snapshot only rewrites the snapshot field of the same variant.
//! A reused finding references the candidate snapshot AND names the
//! snapshot that produced its evidence, so causality is traceable,
//! never fabricated.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::domain::evidence_kernel::ids::{FactId, SnapshotId};
use crate::domain::evidence_kernel::semantic_diff::FactDelta;
use crate::domain::findings::guard_transfer::GuardObservation;
use crate::domain::findings::{
    AnalysisCapability, DetectorAuthority, DetectorDigest, DetectorDigests, DetectorId,
};

/// The serializable mirror of [`GuardObservation`] (CAA-07 AC #4).
///
/// `Gap` and `UnknownReason` payloads are carried as their `Debug`
/// rendering: the cache is for reuse decisions and surfacing, and the
/// reuse contract never inspects a gap's internals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CachedObservation {
    Verifiable {
        snapshot_id: u64,
        evidence: Vec<TypedEvidenceRecord>,
        causes: Vec<CachedCause>,
    },
    Advisory {
        snapshot_id: u64,
        evidence: Vec<TypedEvidenceRecord>,
        message: String,
    },
    Inconclusive {
        snapshot_id: u64,
        gaps: Vec<String>,
    },
    Unsupported {
        reason: String,
    },
    Ungrounded {
        gaps: Vec<String>,
    },
    Failed {
        message: String,
    },
}

/// One `TypedEvidenceHandle` reduced to what survives persistence:
/// the evidence id and the snapshot it was observed under — exactly
/// the pair the handle is made of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedEvidenceRecord {
    pub evidence: u64,
    pub snapshot: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedCause {
    pub grounded: bool,
    pub subject: Option<u64>,
    pub fact: Option<u64>,
    pub evidence: Option<u64>,
    pub detail: String,
}

/// Project an observation into its cached mirror. Pure, and each arm
/// lands in its own variant: a non-verified state can never arrive at
/// `Verifiable` through this function.
pub fn cache_observation(observation: &GuardObservation) -> CachedObservation {
    match observation {
        GuardObservation::Verifiable {
            snapshot_id,
            evidence,
            causes,
            ..
        } => CachedObservation::Verifiable {
            snapshot_id: snapshot_id.0,
            evidence: evidence
                .iter()
                .map(|e| TypedEvidenceRecord {
                    evidence: e.id.0,
                    snapshot: e.snapshot.0,
                })
                .collect(),
            causes: causes
                .iter()
                .map(|c| match c {
                    crate::domain::findings::projection::CauseProjection::Grounded {
                        subject,
                        fact,
                        evidence,
                        detail,
                        ..
                    } => CachedCause {
                        grounded: true,
                        subject: subject.map(|s| s.0),
                        fact: fact.map(|f| f.0),
                        evidence: evidence.map(|e| e.0),
                        detail: detail.clone(),
                    },
                    crate::domain::findings::projection::CauseProjection::Ungrounded {
                        subject,
                        detail,
                        ..
                    } => CachedCause {
                        grounded: false,
                        subject: subject.map(|s| s.0),
                        fact: None,
                        evidence: None,
                        detail: detail.clone(),
                    },
                })
                .collect(),
        },
        GuardObservation::Advisory {
            snapshot_id,
            evidence,
            message,
            ..
        } => CachedObservation::Advisory {
            snapshot_id: snapshot_id.0,
            evidence: evidence
                .iter()
                .map(|e| TypedEvidenceRecord {
                    evidence: e.id.0,
                    snapshot: e.snapshot.0,
                })
                .collect(),
            message: message.clone(),
        },
        GuardObservation::Inconclusive {
            snapshot_id, gaps, ..
        } => CachedObservation::Inconclusive {
            snapshot_id: snapshot_id.0,
            gaps: gaps.iter().map(|g| format!("{g:?}")).collect(),
        },
        GuardObservation::Unsupported { reason } => CachedObservation::Unsupported {
            reason: format!("{reason:?}"),
        },
        GuardObservation::Ungrounded { gaps } => CachedObservation::Ungrounded {
            gaps: gaps.iter().map(|g| format!("{g:?}")).collect(),
        },
        GuardObservation::Failed { message } => CachedObservation::Failed {
            message: message.clone(),
        },
    }
}

/// The rendering coordinates of one finding. Display data only: no
/// reuse decision reads it, and no fingerprint contains it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OutputLocation {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// One finding as the cache holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedFinding {
    /// Digest over (detector semantic digest, grounding subject, kind).
    pub semantic_fingerprint: DetectorDigest,
    pub output_locations: Vec<OutputLocation>,
    pub observation: CachedObservation,
}

/// The semantic fingerprint of one finding: stable across line moves
/// and file renames, different when the grounding subject is.
pub fn semantic_fingerprint(
    detector_semantic: &DetectorDigest,
    grounding_subject: &str,
    finding_kind: &str,
) -> DetectorDigest {
    DetectorDigest::from_content(&format!(
        "{detector_semantic}|{grounding_subject}|{finding_kind}"
    ))
}

/// Everything that decides whether a run may be reused: the layered
/// identity of its inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorRunIdentity {
    pub detector_id: DetectorId,
    pub version: String,
    pub authority: DetectorAuthority,
    /// Rule layer: the four digests of the executed definition.
    pub digests: DetectorDigests,
    /// Capability layer: `requires` ∩ backends offered, resolved.
    pub effective_capabilities: BTreeSet<AnalysisCapability>,
    /// Parser layer.
    pub parser: DetectorDigest,
    /// Sanitizer layer.
    pub sanitizer: DetectorDigest,
}

/// What one run depended on and produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorRunRecord {
    pub identity: DetectorRunIdentity,
    /// The facts the run read (its semantic dependencies).
    pub observed_facts: BTreeSet<FactId>,
    /// Lineage completeness: a truncated read-set proves nothing
    /// about what else the run may have touched.
    pub observed_truncated: bool,
    /// The snapshot whose evidence backs the findings.
    pub produced_snapshot: SnapshotId,
    pub findings: Vec<CachedFinding>,
}

/// One input layer of [`DetectorRunIdentity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationLevel {
    Rule,
    Capability,
    Parser,
    Sanitizer,
    SemanticDependency,
}

/// The verdict for re-running a detector against new inputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReuseDecision {
    Reusable,
    Invalidated { levels: Vec<InvalidationLevel> },
}

impl ReuseDecision {
    pub fn is_reusable(&self) -> bool {
        matches!(self, ReuseDecision::Reusable)
    }
}

/// Decide reuse layer by layer: `current` is the identity the rerun
/// would execute under, `delta` the semantic change since the record
/// was produced (`None` when no snapshot transition is involved).
///
/// The semantic layer follows the planner's rule of e68: a REMOVED or
/// CHANGED fact invalidates only if the run read it; a fact whose
/// `(subject, predicate)` never existed invalidates every reader,
/// because an aggregate read cannot be proven unaffected.
#[must_use]
pub fn reuse_decision(
    record: &DetectorRunRecord,
    current: &DetectorRunIdentity,
    delta: Option<&FactDelta>,
) -> ReuseDecision {
    let mut levels: Vec<InvalidationLevel> = Vec::new();
    if record.identity.digests != current.digests
        || record.identity.version != current.version
        || record.identity.authority != current.authority
    {
        levels.push(InvalidationLevel::Rule);
    }
    if record.identity.effective_capabilities != current.effective_capabilities {
        levels.push(InvalidationLevel::Capability);
    }
    if record.identity.parser != current.parser {
        levels.push(InvalidationLevel::Parser);
    }
    if record.identity.sanitizer != current.sanitizer {
        levels.push(InvalidationLevel::Sanitizer);
    }
    let stale_facts = delta.is_some_and(|delta| {
        record.observed_truncated
            || delta
                .removed
                .iter()
                .any(|r| record.observed_facts.contains(&r.fact_id))
            || purely_new_facts(delta).next().is_some()
    });
    if stale_facts {
        levels.push(InvalidationLevel::SemanticDependency);
    }
    if levels.is_empty() {
        ReuseDecision::Reusable
    } else {
        ReuseDecision::Invalidated { levels }
    }
}

/// Additions whose `(subject, predicate)` did not exist on the
/// removed side — the same notion the planner uses to keep
/// modifications out of the conservative-addition bucket.
fn purely_new_facts(
    delta: &FactDelta,
) -> impl Iterator<Item = &crate::domain::evidence_kernel::semantic_diff::FactChange> {
    let changed: BTreeSet<(u64, String)> = delta
        .removed
        .iter()
        .map(|r| {
            (
                r.semantic_key.subject.0,
                r.semantic_key.predicate.to_string(),
            )
        })
        .collect();
    delta.added.iter().filter(move |a| {
        !changed.contains(&(
            a.semantic_key.subject.0,
            a.semantic_key.predicate.to_string(),
        ))
    })
}

/// A reused finding: the candidate snapshot it is being reported in,
/// the snapshot that actually produced its evidence, and the
/// unchanged variant of its observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReboundFinding {
    pub produced_in: SnapshotId,
    pub reported_in: SnapshotId,
    pub semantic_fingerprint: DetectorDigest,
    pub output_locations: Vec<OutputLocation>,
    pub observation: CachedObservation,
}

/// Re-reference a record's findings for `candidate`. Only the
/// snapshot field of each variant moves; statuses, evidence handles
/// and cause records are carried verbatim.
pub fn rebind_candidate(record: &DetectorRunRecord, candidate: SnapshotId) -> Vec<ReboundFinding> {
    record
        .findings
        .iter()
        .map(|finding| ReboundFinding {
            produced_in: record.produced_snapshot,
            reported_in: candidate,
            semantic_fingerprint: finding.semantic_fingerprint.clone(),
            output_locations: finding.output_locations.clone(),
            observation: rebind_observation(&finding.observation, candidate),
        })
        .collect()
}

fn rebind_observation(observation: &CachedObservation, candidate: SnapshotId) -> CachedObservation {
    match observation {
        CachedObservation::Verifiable {
            evidence, causes, ..
        } => CachedObservation::Verifiable {
            snapshot_id: candidate.0,
            evidence: evidence.clone(),
            causes: causes.clone(),
        },
        CachedObservation::Advisory {
            evidence, message, ..
        } => CachedObservation::Advisory {
            snapshot_id: candidate.0,
            evidence: evidence.clone(),
            message: message.clone(),
        },
        CachedObservation::Inconclusive { gaps, .. } => CachedObservation::Inconclusive {
            snapshot_id: candidate.0,
            gaps: gaps.clone(),
        },
        // These states carry no snapshot by construction (H0.4):
        // rebinding would have to invent one, so they stay verbatim.
        other => other.clone(),
    }
}

/// The whole cache for one workspace, keyed by detector id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DetectorReuseIndex {
    pub schema: String,
    pub records: std::collections::BTreeMap<String, DetectorRunRecord>,
}

pub const DETECTOR_REUSE_SCHEMA: &str = "cognicode.detector-reuse/v1";

impl DetectorReuseIndex {
    pub fn empty() -> Self {
        Self {
            schema: DETECTOR_REUSE_SCHEMA.to_string(),
            records: Default::default(),
        }
    }

    /// Insert or replace the record of one detector.
    pub fn remember(&mut self, record: DetectorRunRecord) {
        self.records
            .insert(record.identity.detector_id.as_str().to_string(), record);
    }

    /// Parse a persisted index, refusing any other schema shape.
    ///
    /// # Errors
    ///
    /// Fails when the JSON names a different schema — a foreign shape
    /// never decodes into a half-understood record.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let index: Self = serde_json::from_str(text)?;
        if index.schema != DETECTOR_REUSE_SCHEMA {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "unknown detector-reuse schema {:?}, expected {DETECTOR_REUSE_SCHEMA:?}",
                    index.schema
                ),
            )));
        }
        Ok(index)
    }

    /// # Errors
    /// See [`Self::from_json`] and serde.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}
