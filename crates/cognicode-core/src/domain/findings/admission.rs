//! Detector admission — the trust boundary (M6, cycles e57 / e58.1 / e58.2).
//!
//! A [`DetectorIr`] carries a public, deserializable `authority` field, so a
//! caller could submit `{ generated_by_llm, authority: Gated }`. This module
//! ensures raw authority is never trusted:
//!
//! - [`AdmittedDetector`] is plain data: **not serializable**, **not directly
//!   executable**.
//! - [`ExecutionPermit`] has private fields and a private seal and is **not**
//!   `Serialize`/`Deserialize`, so it can only be minted here. The executor
//!   accepts only a permit.
//! - Promotion requires a [`VerifiedPromotion`] — likewise private-sealed and
//!   NOT serializable — which can only be produced by running an
//!   [`ApprovalVerifier`] over a [`PromotionRequest`].
//! - Persistence uses [`AdmittedDetectorRecord`] (serializable). Recovery is
//!   **fail-closed**: [`DetectorAdmission::restore`] always yields
//!   `Candidate`; only [`DetectorAdmission::restore_with`], given an explicit
//!   [`ApprovalVerifier`], can recover `Gated` — and then only if the
//!   verifier actually validates the recorded approval.
//!
//! ## Honest scope
//!
//! "Unforgeable by construction" holds for the **in-memory capability**: no
//! code outside this module can build an `ExecutionPermit` or a
//! `VerifiedPromotion`. It does **not** yet hold for the *authenticity of a
//! persisted approval string* — that needs a trusted approver registry, a
//! governance/ChangeProposal flow and (eventually) signatures (M9/M13). The
//! default restore path therefore never restores `Gated`, and the shipped
//! [`RejectAllApprovals`] verifier rejects everything.
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::detector_ir::{
    DetectorAuthority, DetectorExecutionRef, DetectorId, DetectorIr, DetectorIrError,
};
use super::digest::DetectorDigest;
use super::scope::AnalysisScope;
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

pub use crate::domain::trust::AdmissionSource;

/// Provenance recorded at admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionRef {
    /// Where the definition came from.
    pub source: AdmissionSource,
    /// The approver recorded for a promotion, if any.
    ///
    /// A stored string is **not** proof of authority: recovering `Gated` from
    /// persistence requires a verifier to accept it.
    pub approval: Option<String>,
}

/// The exact object a promotion approves: detector, version, semantics and
/// the source it was admitted from.
///
/// Derived from an [`ExecutionPermit`] (never supplied by the caller), so a
/// promotion cannot be "re-pointed" at a different version, a different logic
/// (semantic digest) or a different admission source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionTarget {
    /// The detector being promoted.
    pub detector_id: DetectorId,
    /// The exact version.
    pub version: DetectorVersion,
    /// The semantic digest (`logic + policy`) of the approved definition.
    pub semantic_digest: DetectorDigest,
    /// The admission source the detector was admitted from.
    pub source: AdmissionSource,
}

/// A request to promote a detector, to be vetted by an [`ApprovalVerifier`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionRequest {
    /// What is being approved — derived from the permit.
    pub target: PromotionTarget,
    /// The alleged approving authority.
    pub approver: String,
}

impl PromotionRequest {
    /// Build a request for exactly the detector a permit authorises.
    ///
    /// This is the only public constructor: the caller supplies the approval
    /// intent (`approver`), never the object being approved.
    pub fn for_permit(
        permit: &ExecutionPermit,
        approver: impl Into<String>,
    ) -> Result<Self, AdmissionError> {
        let approver = approver.into();
        if approver.trim().is_empty() {
            return Err(AdmissionError::EmptyApprover);
        }
        Ok(Self {
            target: permit.promotion_target(),
            approver,
        })
    }

    /// Rebuild the request a persisted record describes (used by
    /// [`DetectorAdmission::restore_with`]).
    fn from_record(record: &AdmittedDetectorRecord, approver: impl Into<String>) -> Self {
        Self {
            target: PromotionTarget {
                detector_id: record.definition.id.clone(),
                version: record.version.clone(),
                semantic_digest: record.definition.semantic_digest(),
                source: record.admission.source,
            },
            approver: approver.into(),
        }
    }

    /// The approving authority.
    pub fn approver(&self) -> &str {
        &self.approver
    }

    /// The approved target.
    pub fn target(&self) -> &PromotionTarget {
        &self.target
    }
}

/// Decides whether a [`PromotionRequest`] is authorised.
///
/// The real implementation (trusted approver registry / ChangeProposal
/// governance) is supplied at the composition root in M9/M13. The shipped
/// default, [`RejectAllApprovals`], authorises nothing.
pub trait ApprovalVerifier {
    /// Whether this request is authorised.
    fn verify(&self, request: &PromotionRequest) -> bool;
}

/// The fail-closed default: authorises nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct RejectAllApprovals;

impl ApprovalVerifier for RejectAllApprovals {
    fn verify(&self, _request: &PromotionRequest) -> bool {
        false
    }
}

/// A non-cryptographic placeholder verifier: authorises a promotion when the
/// source is eligible and an approver is named.
///
/// Documented as a **placeholder** pending M9/M13: it does not prove that the
/// approver actually approved anything. Never wire it as the production
/// verifier without replacing it.
#[derive(Debug, Clone, Copy, Default)]
pub struct EligibleSourceVerifier;

impl ApprovalVerifier for EligibleSourceVerifier {
    fn verify(&self, request: &PromotionRequest) -> bool {
        request.target.source.is_trusted_to_gate() && !request.approver.trim().is_empty()
    }
}

/// Private seal for [`VerifiedPromotion`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PromotionSeal(());

/// A promotion that an [`ApprovalVerifier`] has authorised.
///
/// Private fields + private seal + no serde: it can only be produced by
/// [`PromotionAuthority::verify`]. [`DetectorAdmission::promote`] accepts
/// only this type, so a caller cannot promote by asserting authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPromotion {
    request: PromotionRequest,
    _seal: PromotionSeal,
}

impl VerifiedPromotion {
    /// The vetted request.
    pub fn request(&self) -> &PromotionRequest {
        &self.request
    }

    /// The approved target.
    pub fn target(&self) -> &PromotionTarget {
        &self.request.target
    }

    /// The approved detector id.
    pub fn detector_id(&self) -> &DetectorId {
        &self.request.target.detector_id
    }

    /// The approving authority.
    pub fn approver(&self) -> &str {
        &self.request.approver
    }
}

/// The only minter of [`VerifiedPromotion`]s.
#[derive(Debug, Clone, Copy)]
pub struct PromotionAuthority;

impl PromotionAuthority {
    /// Run a verifier over a request; mint a [`VerifiedPromotion`] only on
    /// success.
    pub fn verify(
        verifier: &dyn ApprovalVerifier,
        request: PromotionRequest,
    ) -> Result<VerifiedPromotion, AdmissionError> {
        if request.approver.trim().is_empty() {
            return Err(AdmissionError::EmptyApprover);
        }
        if !verifier.verify(&request) {
            return Err(AdmissionError::PromotionRejected {
                detector: request.target.detector_id.as_str().to_string(),
            });
        }
        Ok(VerifiedPromotion {
            request,
            _seal: PromotionSeal(()),
        })
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
    pub fn id(&self) -> &DetectorId {
        &self.definition.id
    }

    /// Whether the admitted detector may block CI.
    pub fn can_block(&self) -> bool {
        self.authority.can_block() && self.definition.validate().is_ok()
    }
}

/// The serializable persistence DTO for an admitted detector.
///
/// Recovering from a record goes through [`DetectorAdmission::restore`]
/// (fail-closed) or [`DetectorAdmission::restore_with`] (verifier-gated).
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
/// [`DetectorAdmission::admit`], [`DetectorAdmission::promote`],
/// [`DetectorAdmission::restore`] and [`DetectorAdmission::restore_with`].
/// It is deliberately **not** `Serialize`/`Deserialize`.
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
    pub fn id(&self) -> &DetectorId {
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
        scope: Option<AnalysisScope>,
    ) -> Result<DetectorExecutionRef, DetectorIrError> {
        DetectorExecutionRef::new(
            self.admitted.definition.id.clone(),
            self.admitted.version.as_str(),
            self.admitted.authority,
            self.admitted.definition.digests(),
            execution_id,
            scope,
        )
    }

    /// The exact target a promotion would approve.
    pub fn promotion_target(&self) -> PromotionTarget {
        PromotionTarget {
            detector_id: self.admitted.definition.id.clone(),
            version: self.admitted.version.clone(),
            semantic_digest: self.admitted.definition.semantic_digest(),
            source: self.admitted.admission.source,
        }
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

    /// Promote a permitted detector using a **verified** promotion.
    pub fn promote(
        permit: &ExecutionPermit,
        verified: VerifiedPromotion,
    ) -> Result<ExecutionPermit, AdmissionError> {
        // Bind the promotion to the exact permit: detector, version, semantics
        // and admission source must all match.
        let target = permit.promotion_target();
        if verified.target() != &target {
            return Err(AdmissionError::PromotionMismatch {
                permit: format!(
                    "{}@{} semantic={} source={}",
                    target.detector_id,
                    target.version,
                    target.semantic_digest.as_str(),
                    target.source
                ),
                promotion: format!(
                    "{}@{} semantic={} source={}",
                    verified.target().detector_id,
                    verified.target().version,
                    verified.target().semantic_digest.as_str(),
                    verified.target().source
                ),
            });
        }
        let mut admitted = permit.admitted.clone();
        admitted.authority = DetectorAuthority::Gated;
        admitted.definition.authority = DetectorAuthority::Gated;
        admitted.admission.approval = Some(verified.approver().to_string());
        Ok(ExecutionPermit {
            admitted,
            _seal: AdmissionSeal(()),
        })
    }

    /// Restore a permit from a persistence record — **fail-closed**.
    ///
    /// The definition is re-validated and the authority is forced to
    /// `Candidate`, whatever the record claims. A stored approval string is
    /// never treated as proof of authority. Use
    /// [`restore_with`](Self::restore_with) to recover `Gated` through a
    /// verifier.
    pub fn restore(record: &AdmittedDetectorRecord) -> Result<ExecutionPermit, AdmissionError> {
        let mut definition = record.definition.clone();
        definition
            .validate()
            .map_err(AdmissionError::InvalidDefinition)?;
        definition.authority = DetectorAuthority::Candidate;

        Ok(ExecutionPermit {
            admitted: AdmittedDetector {
                definition,
                version: record.version.clone(),
                authority: DetectorAuthority::Candidate,
                admission: record.admission.clone(),
            },
            _seal: AdmissionSeal(()),
        })
    }

    /// Restore a permit, recovering `Gated` only if `verifier` validates the
    /// recorded approval. Otherwise the permit is `Candidate` (fail-closed).
    pub fn restore_with(
        record: &AdmittedDetectorRecord,
        verifier: &dyn ApprovalVerifier,
    ) -> Result<ExecutionPermit, AdmissionError> {
        let mut definition = record.definition.clone();
        definition
            .validate()
            .map_err(AdmissionError::InvalidDefinition)?;

        let authority = if record.authority == DetectorAuthority::Gated {
            match &record.admission.approval {
                Some(approver) => {
                    let request = PromotionRequest::from_record(record, approver.clone());
                    if PromotionAuthority::verify(verifier, request).is_ok() {
                        DetectorAuthority::Gated
                    } else {
                        DetectorAuthority::Candidate
                    }
                }
                None => DetectorAuthority::Candidate,
            }
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
    /// A promotion request has no approver.
    EmptyApprover,
    /// The verifier rejected the promotion.
    PromotionRejected {
        /// The detector the promotion targeted.
        detector: String,
    },
    /// The promotion was for a different detector than the permit.
    PromotionMismatch {
        /// The permit's detector id.
        permit: String,
        /// The promotion's detector id.
        promotion: String,
    },
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVersion => f.write_str("detector version must not be empty"),
            Self::InvalidDefinition(err) => write!(f, "detector definition rejected: {err}"),
            Self::EmptyApprover => f.write_str("promotion request must name a non-empty approver"),
            Self::PromotionRejected { detector } => {
                write!(
                    f,
                    "promotion of `{detector}` was rejected by the approval verifier"
                )
            }
            Self::PromotionMismatch { permit, promotion } => write!(
                f,
                "promotion targets `{promotion}` but the permit is for `{permit}`"
            ),
        }
    }
}

impl std::error::Error for AdmissionError {}

#[cfg(test)]
mod tests {
    use super::super::detector_ir::{
        DetectorFindingPolicy, DetectorStep, FindingKind, SubjectPattern,
    };
    use super::*;

    fn definition(authority: DetectorAuthority) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            policy: DetectorFindingPolicy::default(),
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

    fn promote_with(
        permit: &ExecutionPermit,
        verifier: &dyn ApprovalVerifier,
        approver: &str,
    ) -> Result<ExecutionPermit, AdmissionError> {
        let request = PromotionRequest::for_permit(permit, approver)?;
        let verified = PromotionAuthority::verify(verifier, request)?;
        DetectorAdmission::promote(permit, verified)
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
        assert!(!permit.can_block());
    }

    #[test]
    fn promote_requires_a_verified_promotion() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();

        // The default verifier rejects everything: no promotion possible.
        assert!(matches!(
            promote_with(&permit, &RejectAllApprovals, "security-team").unwrap_err(),
            AdmissionError::PromotionRejected { .. }
        ));

        // Only a verifier that actually accepts yields a Gated permit.
        let gated = promote_with(&permit, &EligibleSourceVerifier, "security-team").unwrap();
        assert_eq!(gated.authority(), DetectorAuthority::Gated);
        assert!(gated.can_block());
        assert_eq!(permit.authority(), DetectorAuthority::Candidate);
    }

    #[test]
    fn promote_rejects_a_target_from_another_permit() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();

        let other = DetectorAdmission::admit(
            {
                let mut ir = definition(DetectorAuthority::Candidate);
                ir.id = DetectorId::new("other.detector").unwrap();
                ir
            },
            "1.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();

        let request = PromotionRequest::for_permit(&other, "team").unwrap();
        let verified = PromotionAuthority::verify(&EligibleSourceVerifier, request).unwrap();
        assert!(matches!(
            DetectorAdmission::promote(&permit, verified).unwrap_err(),
            AdmissionError::PromotionMismatch { .. }
        ));
    }

    #[test]
    fn promotion_is_bound_to_the_admission_source() {
        let mut ir = definition(DetectorAuthority::Candidate);
        ir.id = DetectorId::new("security.same_id").unwrap();

        let ai =
            DetectorAdmission::admit(ir.clone(), "1.0.0", AdmissionSource::AiGenerated).unwrap();
        let human = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap();

        // A promotion verified for the human-curated permit must NOT promote
        // the AI permit (whose source the requester cannot rewrite).
        let request = PromotionRequest::for_permit(&human, "team").unwrap();
        let verified = PromotionAuthority::verify(&EligibleSourceVerifier, request).unwrap();
        assert!(matches!(
            DetectorAdmission::promote(&ai, verified).unwrap_err(),
            AdmissionError::PromotionMismatch { .. }
        ));

        // The AI permit cannot even build an approvable request with the
        // eligible-source verifier.
        let ai_request = PromotionRequest::for_permit(&ai, "team").unwrap();
        assert_eq!(ai_request.target().source, AdmissionSource::AiGenerated);
        assert!(matches!(
            PromotionAuthority::verify(&EligibleSourceVerifier, ai_request).unwrap_err(),
            AdmissionError::PromotionRejected { .. }
        ));
    }

    #[test]
    fn promotion_is_bound_to_version_and_logic() {
        let mut ir = definition(DetectorAuthority::Candidate);
        ir.id = DetectorId::new("security.versioned").unwrap();

        let v1 =
            DetectorAdmission::admit(ir.clone(), "1.0.0", AdmissionSource::HumanCurated).unwrap();
        let v2 = DetectorAdmission::admit(ir, "2.0.0", AdmissionSource::HumanCurated).unwrap();

        let verified = PromotionAuthority::verify(
            &EligibleSourceVerifier,
            PromotionRequest::for_permit(&v1, "team").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DetectorAdmission::promote(&v2, verified).unwrap_err(),
            AdmissionError::PromotionMismatch { .. }
        ));

        // Different logic (different steps) => different semantic digest.
        let mut changed = definition(DetectorAuthority::Candidate);
        changed.id = DetectorId::new("security.versioned").unwrap();
        changed.steps = vec![DetectorStep::Produce {
            kind: FindingKind::new("security.weak_hash").unwrap(),
        }];
        let new_logic =
            DetectorAdmission::admit(changed, "1.0.0", AdmissionSource::HumanCurated).unwrap();
        let verified2 = PromotionAuthority::verify(
            &EligibleSourceVerifier,
            PromotionRequest::for_permit(&v1, "team").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DetectorAdmission::promote(&new_logic, verified2).unwrap_err(),
            AdmissionError::PromotionMismatch { .. }
        ));
    }

    #[test]
    fn renaming_does_not_invalidate_an_approval() {
        let mut ir = definition(DetectorAuthority::Candidate);
        ir.id = DetectorId::new("security.renamed").unwrap();
        let a =
            DetectorAdmission::admit(ir.clone(), "1.0.0", AdmissionSource::HumanCurated).unwrap();

        let mut renamed = ir;
        renamed.name = "a totally different display name".to_string();
        let b = DetectorAdmission::admit(renamed, "1.0.0", AdmissionSource::HumanCurated).unwrap();

        assert_eq!(a.promotion_target(), b.promotion_target());
        let verified = PromotionAuthority::verify(
            &EligibleSourceVerifier,
            PromotionRequest::for_permit(&a, "team").unwrap(),
        )
        .unwrap();
        assert_eq!(
            DetectorAdmission::promote(&b, verified)
                .unwrap()
                .authority(),
            DetectorAuthority::Gated
        );
    }

    #[test]
    fn policy_change_requires_a_new_approval() {
        let mut ir = definition(DetectorAuthority::Candidate);
        ir.id = DetectorId::new("security.policy").unwrap();
        let low =
            DetectorAdmission::admit(ir.clone(), "1.0.0", AdmissionSource::HumanCurated).unwrap();

        ir.policy = DetectorFindingPolicy::new(
            super::super::finding::FindingSeverity::Critical,
            super::super::finding::RiskLevel::Critical,
        );
        let high = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap();

        assert_ne!(low.promotion_target(), high.promotion_target());
        let verified = PromotionAuthority::verify(
            &EligibleSourceVerifier,
            PromotionRequest::for_permit(&low, "team").unwrap(),
        )
        .unwrap();
        assert!(matches!(
            DetectorAdmission::promote(&high, verified).unwrap_err(),
            AdmissionError::PromotionMismatch { .. }
        ));
    }

    #[test]
    fn json_record_with_forged_approval_cannot_restore_gated() {
        // authority=Gated + approval="forged" must NOT restore as Gated.
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
            "admission": { "source": "human_curated", "approval": "forged" }
        }"#;
        let record: AdmittedDetectorRecord = serde_json::from_str(forged).unwrap();

        // Fail-closed restore: Candidate.
        let permit = DetectorAdmission::restore(&record).unwrap();
        assert_eq!(permit.authority(), DetectorAuthority::Candidate);
        assert!(!permit.can_block());

        // Even with a verifier, the shipped default rejects it.
        let permit2 = DetectorAdmission::restore_with(&record, &RejectAllApprovals).unwrap();
        assert_eq!(permit2.authority(), DetectorAuthority::Candidate);
        assert!(!permit2.can_block());
    }

    #[test]
    fn restore_with_a_verifier_recovers_gated_when_validated() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "2.0.0",
            AdmissionSource::HumanCurated,
        )
        .unwrap();
        let gated = promote_with(&permit, &EligibleSourceVerifier, "team").unwrap();
        let record = gated.record();

        let restored = DetectorAdmission::restore_with(&record, &EligibleSourceVerifier).unwrap();
        assert_eq!(restored.authority(), DetectorAuthority::Gated);

        // Fail-closed restore never recovers Gated.
        let plain = DetectorAdmission::restore(&record).unwrap();
        assert_eq!(plain.authority(), DetectorAuthority::Candidate);

        // A record whose definition does not validate is rejected.
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
    fn promotion_requires_a_non_empty_approver() {
        let permit = DetectorAdmission::admit(
            definition(DetectorAuthority::Candidate),
            "1.0.0",
            AdmissionSource::Builtin,
        )
        .unwrap();
        assert_eq!(
            PromotionRequest::for_permit(&permit, "  ").unwrap_err(),
            AdmissionError::EmptyApprover
        );
    }
}
