//! Detector admission — the trust boundary between a raw definition and the
//! executor (M6, cycle e57).
//!
//! A [`DetectorIr`] carries a public, deserializable `authority` field, so a
//! caller could submit `{ generated_by_llm, authority: Gated }`. The executor
//! must therefore **never** run a raw definition whose authority it trusts
//! from the caller. Instead:
//!
//! ```text
//! Raw DetectorIr ──► DetectorAdmission::admit ──► AdmittedDetector (Candidate)
//!                                                        │
//!                          DetectorAdmission::promote ───┘  (trusted transition)
//!                                                        ▼
//!                                              AdmittedDetector (Gated)
//! ```
//!
//! [`DetectorAdmission::admit`] always normalises the authority to
//! [`DetectorAuthority::Candidate`] regardless of what the raw definition
//! claims; only [`DetectorAdmission::promote`] — which requires an explicit
//! approval token — can produce a `Gated` detector. This prepares M9/M10/M11
//! (ChangeProposal authority, packs, shadow mode, promotion policy).
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::detector_ir::{DetectorAuthority, DetectorExecutionRef, DetectorIr, DetectorIrError};
use crate::domain::kernel_ids::ExecutionId;

/// A non-empty detector version.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DetectorVersion(String);

impl DetectorVersion {
    /// Construct a non-empty version.
    pub fn new(value: impl Into<String>) -> Result<Self, AdmissionError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(AdmissionError::EmptyVersion);
        }
        Ok(Self(value))
    }

    /// Borrow the raw version.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DetectorVersion {
    type Error = AdmissionError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DetectorVersion> for String {
    fn from(value: DetectorVersion) -> Self {
        value.0
    }
}

impl fmt::Display for DetectorVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a detector definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionSource {
    /// Shipped with the platform.
    Builtin,
    /// Curated and reviewed by a human.
    HumanCurated,
    /// Proposed by an AI agent.
    AiGenerated,
    /// Imported from an external source (e.g. a pack).
    Imported,
}

impl AdmissionSource {
    /// Whether this source is trusted to *gate*. Even when trusted, admission
    /// still starts a detector as `Candidate`; promotion is a separate,
    /// explicit transition.
    pub fn is_trusted_to_gate(self) -> bool {
        matches!(self, Self::Builtin | Self::HumanCurated)
    }
}

impl fmt::Display for AdmissionSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Builtin => "Builtin",
            Self::HumanCurated => "HumanCurated",
            Self::AiGenerated => "AiGenerated",
            Self::Imported => "Imported",
        })
    }
}

/// Provenance recorded at admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionRef {
    /// Where the definition came from.
    pub source: AdmissionSource,
    /// The approver of a promotion, if the detector was promoted.
    pub approval: Option<String>,
}

/// An explicit approval token for promoting a detector to `Gated`.
///
/// In production this is issued by the governance/ChangeProposal path
/// (M9/M13); here it is a plain value so the transition is explicit and
/// testable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionApproval {
    /// Identifier of the approving authority (non-empty).
    pub approver: String,
}

impl PromotionApproval {
    /// Construct an approval with a non-empty approver.
    pub fn new(approver: impl Into<String>) -> Result<Self, AdmissionError> {
        let approver = approver.into();
        if approver.trim().is_empty() {
            return Err(AdmissionError::EmptyApprover);
        }
        Ok(Self { approver })
    }
}

/// A detector that has crossed the admission boundary.
///
/// `definition.authority` is kept equal to [`AdmittedDetector::authority`] so
/// there is exactly one authoritative field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedDetector {
    /// The admitted definition (authority normalised).
    pub definition: DetectorIr,
    /// Version at admission.
    pub version: DetectorVersion,
    /// The authority this detector holds now.
    pub authority: DetectorAuthority,
    /// Admission provenance.
    pub admission: AdmissionRef,
}

impl AdmittedDetector {
    /// Detector id.
    pub fn id(&self) -> &super::DetectorId {
        &self.definition.id
    }

    /// Whether the admitted detector may block CI.
    pub fn can_block(&self) -> bool {
        self.authority.can_block() && self.definition.validate().is_ok()
    }

    /// Build the [`DetectorExecutionRef`] for a run of this detector.
    ///
    /// This is the safe constructor backends/executors use: the authority is
    /// sourced from the admission record, never from a caller-supplied
    /// definition.
    pub fn execution_ref(
        &self,
        execution_id: Option<ExecutionId>,
    ) -> Result<DetectorExecutionRef, DetectorIrError> {
        DetectorExecutionRef::new(
            self.definition.id.clone(),
            self.version.as_str(),
            self.authority,
            self.definition.semantic_digest(),
            self.definition.instance_digest(),
            execution_id,
        )
    }
}

/// The admission guard.
#[derive(Debug, Clone, Copy, Default)]
pub struct DetectorAdmission;

impl DetectorAdmission {
    /// Admit a raw definition.
    ///
    /// The resulting detector is **always** [`DetectorAuthority::Candidate`],
    /// regardless of the authority claimed by `definition`. Use
    /// [`promote`](Self::promote) to grant `Gated`.
    pub fn admit(
        mut definition: DetectorIr,
        version: impl Into<String>,
        source: AdmissionSource,
    ) -> Result<AdmittedDetector, AdmissionError> {
        let version = DetectorVersion::new(version)?;
        definition
            .validate()
            .map_err(AdmissionError::InvalidDefinition)?;

        // Discard any authority claimed by the untrusted definition.
        definition.authority = DetectorAuthority::Candidate;

        Ok(AdmittedDetector {
            definition,
            version,
            authority: DetectorAuthority::Candidate,
            admission: AdmissionRef {
                source,
                approval: None,
            },
        })
    }

    /// Promote an admitted detector to `Gated` through the trusted transition.
    pub fn promote(
        admitted: &AdmittedDetector,
        approval: PromotionApproval,
    ) -> Result<AdmittedDetector, AdmissionError> {
        let mut promoted = admitted.clone();
        promoted.authority = DetectorAuthority::Gated;
        promoted.definition.authority = DetectorAuthority::Gated;
        promoted.admission.approval = Some(approval.approver);
        Ok(promoted)
    }
}

/// Failure at the admission boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdmissionError {
    /// The version is empty.
    EmptyVersion,
    /// The definition failed validation.
    InvalidDefinition(DetectorIrError),
    /// A promotion approval has no approver.
    EmptyApprover,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVersion => f.write_str("detector version must not be empty"),
            Self::InvalidDefinition(err) => write!(f, "detector definition rejected: {err}"),
            Self::EmptyApprover => f.write_str("promotion approval must name a non-empty approver"),
        }
    }
}

impl std::error::Error for AdmissionError {}

#[cfg(test)]
mod tests {
    use super::super::detector_ir::{DetectorId, DetectorStep, FindingKind, SubjectPattern};
    use super::*;
    use std::collections::BTreeSet;

    fn definition(authority: DetectorAuthority) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            requires: BTreeSet::new(),
            authority,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("security.weak_hash").unwrap(),
                },
            ],
        }
    }

    #[test]
    fn ai_definition_claiming_gated_is_forced_to_candidate() {
        // The whole point of the boundary: a raw definition that claims
        // `Gated` must not arrive with gate authority.
        let claimed = definition(DetectorAuthority::Gated);
        let admitted =
            DetectorAdmission::admit(claimed, "1.0.0", AdmissionSource::AiGenerated).unwrap();

        assert_eq!(admitted.authority, DetectorAuthority::Candidate);
        assert_eq!(admitted.definition.authority, DetectorAuthority::Candidate);
        assert!(!admitted.can_block());
        assert!(admitted.admission.approval.is_none());
    }

    #[test]
    fn trusted_source_still_starts_as_candidate() {
        let admitted = DetectorAdmission::admit(
            definition(DetectorAuthority::Gated),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();
        assert_eq!(admitted.authority, DetectorAuthority::Candidate);
        assert!(!admitted.can_block());
    }

    #[test]
    fn promote_is_the_only_path_to_gated() {
        let admitted = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();

        let approval = PromotionApproval::new("security-team").unwrap();
        let gated = DetectorAdmission::promote(&admitted, approval).unwrap();

        assert_eq!(gated.authority, DetectorAuthority::Gated);
        assert!(gated.can_block());
        assert_eq!(gated.admission.approval.as_deref(), Some("security-team"));

        // The original admission is untouched.
        assert_eq!(admitted.authority, DetectorAuthority::Candidate);
    }

    #[test]
    fn execution_ref_sources_authority_from_admission() {
        let admitted = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "2.3.4",
            AdmissionSource::AiGenerated,
        )
        .unwrap();
        let candidate_run = admitted.execution_ref(Some(ExecutionId(1))).unwrap();
        assert_eq!(
            candidate_run.authority_at_execution,
            DetectorAuthority::Candidate
        );
        assert!(!candidate_run.can_block());

        let gated =
            DetectorAdmission::promote(&admitted, PromotionApproval::new("team").unwrap()).unwrap();
        let gated_run = gated.execution_ref(Some(ExecutionId(2))).unwrap();
        assert!(gated_run.can_block());

        // Same semantics, different authority: the semantic digest is stable.
        assert_eq!(candidate_run.semantic_digest, gated_run.semantic_digest);
        assert_ne!(candidate_run.instance_digest, gated_run.instance_digest);
    }

    #[test]
    fn admit_rejects_empty_version_and_invalid_definition() {
        assert_eq!(
            DetectorAdmission::admit(
                definition(DetectorAuthority::Candidate),
                "  ",
                AdmissionSource::Builtin,
            )
            .unwrap_err(),
            AdmissionError::EmptyVersion
        );

        let mut invalid = definition(DetectorAuthority::Candidate);
        invalid.steps.clear();
        assert!(matches!(
            DetectorAdmission::admit(invalid, "1.0.0", AdmissionSource::Builtin).unwrap_err(),
            AdmissionError::InvalidDefinition(_)
        ));
    }

    #[test]
    fn promotion_requires_an_approver() {
        assert_eq!(
            PromotionApproval::new("").unwrap_err(),
            AdmissionError::EmptyApprover
        );
    }

    #[test]
    fn admitted_detector_round_trips() {
        let admitted = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::Imported,
        )
        .unwrap();
        let json = serde_json::to_string(&admitted).unwrap();
        let parsed: AdmittedDetector = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, admitted);
    }
}
