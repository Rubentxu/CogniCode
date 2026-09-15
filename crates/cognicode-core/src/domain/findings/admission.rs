//! Detector admission — a real capability boundary (M6, cycles e57 / e58.1).
//!
//! A [`DetectorIr`] carries a public, deserializable `authority` field, so a
//! caller could submit `{ generated_by_llm, authority: Gated }`. e57 stopped
//! *normalising* the authority on the official path; e58.1 makes authority
//! **unforgeable by construction**:
//!
//! - [`AdmittedDetector`] is plain data — it is **not** serializable and
//!   **not** directly executable.
//! - [`ExecutionPermit`] has private fields and a private
//!   [`AdmissionSeal`], so it cannot be constructed or deserialized outside
//!   this module. The executor accepts only a permit.
//! - Persistence goes through [`AdmittedDetectorRecord`] (serializable), and
//!   recovery must re-enter through [`DetectorAdmission::restore`], which
//!   re-validates the definition and re-applies the admission policy.
//!
//! ```text
//! Raw DetectorIr ─► admit ──► ExecutionPermit (Candidate)
//!                                 ▲   │
//!                promote(approval)┘   │
//!                                     ▼
//!                        DetectorExecutor::execute(&ExecutionPermit, …)
//!
//! AdmittedDetectorRecord ─► restore (re-validate + policy) ─► ExecutionPermit
//! ```
//!
//! Not cryptographic: the approval's authenticity is a governance concern for
//! M9/M13. The type system prevents *construction*; `restore` prevents a
//! hand-written JSON record from silently minting a `Gated` permit.
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
    /// starts a detector as `Candidate`; promotion is a separate transition.
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
/// (M9/M13); here it is a plain value so the transition is explicit.
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

/// The admitted detector as plain data.
///
/// **Not** serializable and **not** executable by itself: execution requires
/// an [`ExecutionPermit`] minted by [`DetectorAdmission`].
#[derive(Debug, Clone, PartialEq, Eq)]
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
}

/// The serializable persistence DTO for an admitted detector.
///
/// Recovering from a record **must** go through
/// [`DetectorAdmission::restore`], which re-validates the definition and
/// re-applies the admission policy — a JSON record cannot simply be turned
/// into an executable permit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedDetectorRecord {
    /// The definition.
    pub definition: DetectorIr,
    /// Version.
    pub version: DetectorVersion,
    /// Authority claimed by the record.
    pub authority: DetectorAuthority,
    /// Admission provenance.
    pub admission: AdmissionRef,
}

impl AdmittedDetectorRecord {
    /// Capture a record from an admitted detector.
    pub fn from_admitted(admitted: &AdmittedDetector) -> Self {
        Self {
            definition: admitted.definition.clone(),
            version: admitted.version.clone(),
            authority: admitted.authority,
            admission: admitted.admission.clone(),
        }
    }
}

impl From<&AdmittedDetector> for AdmittedDetectorRecord {
    fn from(admitted: &AdmittedDetector) -> Self {
        Self::from_admitted(admitted)
    }
}

/// Private seal: only this module can construct an [`ExecutionPermit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AdmissionSeal(());

/// The capability token that authorises execution.
///
/// Private fields + a private seal mean the only ways to obtain one are
/// [`DetectorAdmission::admit`], [`DetectorAdmission::promote`] and
/// [`DetectorAdmission::restore`]. It is deliberately **not**
/// `Serialize`/`Deserialize`, so JSON cannot mint one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPermit {
    admitted: AdmittedDetector,
    _seal: AdmissionSeal,
}

impl ExecutionPermit {
    /// The admitted detector this permit authorises.
    pub fn admitted(&self) -> &AdmittedDetector {
        &self.admitted
    }

    /// Detector id.
    pub fn id(&self) -> &super::DetectorId {
        self.admitted.id()
    }

    /// The authority this permit carries.
    pub fn authority(&self) -> DetectorAuthority {
        self.admitted.authority
    }

    /// Whether the permitted detector may block CI.
    pub fn can_block(&self) -> bool {
        self.admitted.can_block()
    }

    /// Build the execution reference for a run under this permit.
    pub fn execution_ref(
        &self,
        execution_id: Option<ExecutionId>,
    ) -> Result<DetectorExecutionRef, DetectorIrError> {
        DetectorExecutionRef::new(
            self.admitted.definition.id.clone(),
            self.admitted.version.as_str(),
            self.admitted.authority,
            self.admitted.definition.semantic_digest(),
            self.admitted.definition.instance_digest(),
            execution_id,
        )
    }

    /// Capture a persistence record.
    pub fn record(&self) -> AdmittedDetectorRecord {
        AdmittedDetectorRecord::from_admitted(&self.admitted)
    }
}

/// The admission guard.
#[derive(Debug, Clone, Copy, Default)]
pub struct DetectorAdmission;

impl DetectorAdmission {
    /// Admit a raw definition, returning an executable permit.
    ///
    /// The permit is **always** [`DetectorAuthority::Candidate`], regardless
    /// of the authority claimed by `definition`.
    pub fn admit(
        mut definition: DetectorIr,
        version: impl Into<String>,
        source: AdmissionSource,
    ) -> Result<ExecutionPermit, AdmissionError> {
        let version = DetectorVersion::new(version)?;
        definition
            .validate()
            .map_err(AdmissionError::InvalidDefinition)?;
        // Discard any authority claimed by the untrusted definition.
        definition.authority = DetectorAuthority::Candidate;

        Ok(ExecutionPermit {
            admitted: AdmittedDetector {
                definition,
                version,
                authority: DetectorAuthority::Candidate,
                admission: AdmissionRef {
                    source,
                    approval: None,
                },
            },
            _seal: AdmissionSeal(()),
        })
    }

    /// Promote a permitted detector to `Gated` through the trusted transition.
    pub fn promote(
        permit: &ExecutionPermit,
        approval: PromotionApproval,
    ) -> Result<ExecutionPermit, AdmissionError> {
        let mut admitted = permit.admitted.clone();
        admitted.authority = DetectorAuthority::Gated;
        admitted.definition.authority = DetectorAuthority::Gated;
        admitted.admission.approval = Some(approval.approver);
        Ok(ExecutionPermit {
            admitted,
            _seal: AdmissionSeal(()),
        })
    }

    /// Restore a permit from a persistence record.
    ///
    /// Re-validates the definition and re-applies the admission policy: a
    /// record that claims `Gated` **without** a recorded approval is
    /// downgraded to `Candidate`, so a hand-written JSON record cannot mint
    /// gate authority.
    pub fn restore(record: &AdmittedDetectorRecord) -> Result<ExecutionPermit, AdmissionError> {
        let mut definition = record.definition.clone();
        definition
            .validate()
            .map_err(AdmissionError::InvalidDefinition)?;

        let claimed_gated = record.authority == DetectorAuthority::Gated;
        let approved = record.admission.approval.is_some();
        let authority = if claimed_gated && approved {
            DetectorAuthority::Gated
        } else {
            DetectorAuthority::Candidate
        };
        definition.authority = authority;

        Ok(ExecutionPermit {
            admitted: AdmittedDetector {
                definition,
                version: record.version.clone(),
                authority,
                admission: record.admission.clone(),
            },
            _seal: AdmissionSeal(()),
        })
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
            policy: super::super::detector_ir::DetectorFindingPolicy::default(),
            requires: [super::super::AnalysisCapability::AstPattern]
                .into_iter()
                .collect(),
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
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Gated),
            "1.0.0",
            AdmissionSource::AiGenerated,
        )
        .unwrap();

        assert_eq!(permit.authority(), DetectorAuthority::Candidate);
        assert_eq!(
            permit.admitted().definition.authority,
            DetectorAuthority::Candidate
        );
        assert!(!permit.can_block());
        assert!(permit.admitted().admission.approval.is_none());
    }

    #[test]
    fn trusted_source_still_starts_as_candidate() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Gated),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();
        assert_eq!(permit.authority(), DetectorAuthority::Candidate);
        assert!(!permit.can_block());
    }

    #[test]
    fn promote_is_the_only_path_to_gated_and_needs_an_approval() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();
        let approval = PromotionApproval::new("security-team").unwrap();
        let gated = DetectorAdmission::promote(&permit, approval).unwrap();

        assert_eq!(gated.authority(), DetectorAuthority::Gated);
        assert!(gated.can_block());
        assert_eq!(
            gated.admitted().admission.approval.as_deref(),
            Some("security-team")
        );
        // The original permit is untouched.
        assert_eq!(permit.authority(), DetectorAuthority::Candidate);

        assert_eq!(
            PromotionApproval::new("").unwrap_err(),
            AdmissionError::EmptyApprover
        );
    }

    #[test]
    fn json_record_claiming_gated_without_approval_restores_as_candidate() {
        // A hand-written JSON record that claims Gated but records no approval
        // must NOT mint gate authority on restore.
        let forged = r#"{
            "definition": {
                "id": "security.weak_hash",
                "name": "weak hash",
                "requires": ["ast_pattern"],
                "authority": "gated",
                "steps": [
                    {"step": "match", "subject": "security.md5_usage"},
                    {"step": "produce", "kind": "security.weak_hash"}
                ]
            },
            "version": "1.0.0",
            "authority": "gated",
            "admission": { "source": "ai_generated", "approval": null }
        }"#;
        let record: AdmittedDetectorRecord = serde_json::from_str(forged).unwrap();
        assert_eq!(record.authority, DetectorAuthority::Gated);

        let permit = DetectorAdmission::restore(&record).unwrap();
        assert_eq!(
            permit.authority(),
            DetectorAuthority::Candidate,
            "an unapproved Gated record must be downgraded"
        );
        assert!(!permit.can_block());
    }

    #[test]
    fn restore_honours_a_recorded_approval_and_revalidates() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "2.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();
        let gated =
            DetectorAdmission::promote(&permit, PromotionApproval::new("team").unwrap()).unwrap();

        let record = gated.record();
        let restored = DetectorAdmission::restore(&record).unwrap();
        assert_eq!(restored.authority(), DetectorAuthority::Gated);
        assert!(restored.can_block());

        // A record whose definition does not validate is rejected on restore.
        let mut broken = record.clone();
        broken.definition.steps.clear();
        assert!(matches!(
            DetectorAdmission::restore(&broken).unwrap_err(),
            AdmissionError::InvalidDefinition(_)
        ));
    }

    #[test]
    fn record_round_trips() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::Imported,
        )
        .unwrap();
        let record = permit.record();
        let json = serde_json::to_string(&record).unwrap();
        let parsed: AdmittedDetectorRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn execution_ref_sources_authority_from_the_permit() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "2.3.4",
            AdmissionSource::AiGenerated,
        )
        .unwrap();
        let candidate_run = permit.execution_ref(Some(ExecutionId(1))).unwrap();
        assert_eq!(
            candidate_run.authority_at_execution,
            DetectorAuthority::Candidate
        );
        assert!(!candidate_run.can_block());

        let gated =
            DetectorAdmission::promote(&permit, PromotionApproval::new("team").unwrap()).unwrap();
        let gated_run = gated.execution_ref(Some(ExecutionId(2))).unwrap();
        assert!(gated_run.can_block());

        // Same semantics, different authority.
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
}
