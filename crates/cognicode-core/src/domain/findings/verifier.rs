//! Finding verifier — referential truth before a finding may block
//! (M6, cycle e57, umbrella U42).
//!
//! [`Finding::validate`](super::Finding::validate) checks **shape**.
//! [`FindingVerifier::verify_for_gate`] checks **referential truth**: every
//! evidence id the finding claims must resolve in the store, and every causal
//! step's evidence must be part of the finding's own evidence set. Only then
//! may the gate be evaluated.
//!
//! ```text
//! Finding::validate()                 shape  (cheap, no store)
//! FindingVerifier::verify_for_gate()  truth  (needs an EvidenceLookup)
//! FindingVerifier::can_block()        truth + gate + authority
//! ```
//!
//! Pure domain: the store is reached only through the [`EvidenceLookup`] port.

use std::fmt;

use super::finding::{Finding, FindingError, FindingGate};
use super::ports::EvidenceLookup;
use crate::domain::kernel_ids::EvidenceId;

/// Verifies findings against an evidence store.
pub struct FindingVerifier<'a> {
    evidence: &'a dyn EvidenceLookup,
}

impl<'a> FindingVerifier<'a> {
    /// Construct a verifier over an evidence lookup.
    pub fn new(evidence: &'a dyn EvidenceLookup) -> Self {
        Self { evidence }
    }

    /// Verify that a finding is well-formed, explainable, has a complete
    /// execution reference, and that all its evidence is referentially real.
    pub fn verify_for_gate(&self, finding: &Finding) -> Result<(), VerificationError> {
        finding.validate().map_err(VerificationError::Malformed)?;

        if !finding.is_explainable() {
            return Err(VerificationError::NotExplainable);
        }

        if !finding.detector.is_well_formed() {
            return Err(VerificationError::IncompleteExecutionRef);
        }

        // Every claimed evidence id must resolve.
        for id in &finding.evidence {
            if !self.evidence.contains(*id) {
                return Err(VerificationError::UnresolvedEvidence(*id));
            }
        }

        // Every causal step's evidence must resolve and belong to the finding.
        for (step, cs) in finding.causal_chain.iter().enumerate() {
            if let Some(id) = cs.evidence {
                if !finding.evidence.contains(&id) {
                    return Err(VerificationError::CausalEvidenceNotInFinding {
                        step,
                        evidence: id,
                    });
                }
                if !self.evidence.contains(id) {
                    return Err(VerificationError::CausalEvidenceNotResolved {
                        step,
                        evidence: id,
                    });
                }
            }
        }

        Ok(())
    }

    /// Whether the finding may block the gate: referentially verified AND the
    /// shape-level predicate (status, authority, gate thresholds) holds.
    pub fn can_block(&self, finding: &Finding, gate: &FindingGate) -> bool {
        self.verify_for_gate(finding).is_ok() && finding.can_block(gate)
    }
}

/// Why a finding failed gate verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    /// The finding is structurally invalid.
    Malformed(FindingError),
    /// The finding is not explainable (no evidence / causal chain).
    NotExplainable,
    /// The detector execution reference is incomplete.
    IncompleteExecutionRef,
    /// A claimed evidence id does not resolve in the store.
    UnresolvedEvidence(EvidenceId),
    /// A causal step points at evidence not in the finding's evidence set.
    CausalEvidenceNotInFinding {
        /// Causal step index.
        step: usize,
        /// The offending evidence id.
        evidence: EvidenceId,
    },
    /// A causal step points at evidence that does not resolve in the store.
    CausalEvidenceNotResolved {
        /// Causal step index.
        step: usize,
        /// The offending evidence id.
        evidence: EvidenceId,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed(err) => write!(f, "finding is malformed: {err}"),
            Self::NotExplainable => f.write_str("finding is not explainable"),
            Self::IncompleteExecutionRef => {
                f.write_str("detector execution reference is incomplete")
            }
            Self::UnresolvedEvidence(id) => write!(f, "evidence {id} does not resolve"),
            Self::CausalEvidenceNotInFinding { step, evidence } => write!(
                f,
                "causal step {step} references {evidence} not in the finding's evidence"
            ),
            Self::CausalEvidenceNotResolved { step, evidence } => {
                write!(
                    f,
                    "causal step {step} references unresolved evidence {evidence}"
                )
            }
        }
    }
}

impl std::error::Error for VerificationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
    use crate::domain::findings::assembler::FindingAssembler;
    use crate::domain::findings::detector_ir::{
        DetectorId, DetectorIr, DetectorStep, FindingKind, SubjectPattern,
    };
    use crate::domain::findings::finding::{
        CausalStepKind, EvidenceClass, FindingSeverity, RiskLevel,
    };
    use crate::domain::findings::outcome::{
        CausalObservation, DetectorMatch, DetectorOutcome, EvidenceKind, ProducedEvidence,
    };
    use crate::domain::kernel_ids::ExecutionId;
    use std::collections::HashSet;

    struct SetLookup(HashSet<EvidenceId>);
    impl EvidenceLookup for SetLookup {
        fn contains(&self, id: EvidenceId) -> bool {
            self.0.contains(&id)
        }
    }

    fn gated_detector() -> super::super::admission::ExecutionPermit {
        let ir = DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            policy: super::super::detector_ir::DetectorFindingPolicy::default(),
            requires: [super::super::AnalysisCapability::AstPattern]
                .into_iter()
                .collect(),
            authority: super::super::DetectorAuthority::Candidate,
            steps: vec![
                DetectorStep::Match {
                    subject: SubjectPattern::new("security.md5_usage").unwrap(),
                },
                DetectorStep::Produce {
                    kind: FindingKind::new("security.weak_hash").unwrap(),
                },
            ],
        };
        let candidate = DetectorAdmission::admit(ir, "1", AdmissionSource::HumanCurated).unwrap();
        DetectorAdmission::promote(
            &candidate,
            super::super::admission::PromotionApproval::new("team").unwrap(),
        )
        .unwrap()
    }

    fn build(gated: bool) -> Finding {
        let permit = if gated {
            gated_detector()
        } else {
            let ir = DetectorIr {
                id: DetectorId::new("security.weak_hash").unwrap(),
                name: "weak hash".to_string(),
                policy: super::super::detector_ir::DetectorFindingPolicy::default(),
                requires: [super::super::AnalysisCapability::AstPattern]
                    .into_iter()
                    .collect(),
                authority: super::super::DetectorAuthority::Gated,
                steps: vec![
                    DetectorStep::Match {
                        subject: SubjectPattern::new("security.md5_usage").unwrap(),
                    },
                    DetectorStep::Produce {
                        kind: FindingKind::new("security.weak_hash").unwrap(),
                    },
                ],
            };
            DetectorAdmission::admit(ir, "1", AdmissionSource::AiGenerated).unwrap()
        };
        let execution = permit.execution_ref(Some(ExecutionId(1))).unwrap();
        let outcome = DetectorOutcome {
            produced_evidence: vec![ProducedEvidence {
                kind: EvidenceKind::AstMatch,
                detail: "md5".to_string(),
                subject: None,
                fact: None,
            }],
            matches: vec![DetectorMatch {
                kind: FindingKind::new("security.weak_hash").unwrap(),
                message: "md5".to_string(),
                evidence: vec![0],
                causal: vec![CausalObservation {
                    kind: CausalStepKind::Source,
                    detail: "md5".to_string(),
                    subject: None,
                    fact: None,
                    evidence: Some(0),
                }],
            }],
            diagnostics: vec![],
        };
        let ids = vec![EvidenceId::new(1)];
        FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &ids)
            .unwrap()
            .pop()
            .unwrap()
    }

    fn lookup_with(ids: &[u64]) -> SetLookup {
        SetLookup(ids.iter().map(|n| EvidenceId::new(*n)).collect())
    }

    #[test]
    fn verified_finding_can_block() {
        let f = build(true);
        let store = lookup_with(&[1]);
        let verifier = FindingVerifier::new(&store);
        assert!(verifier.verify_for_gate(&f).is_ok());
        let gate = FindingGate::new(EvidenceClass::C, RiskLevel::Low);
        assert!(verifier.can_block(&f, &gate));
    }

    #[test]
    fn unresolved_evidence_blocks_the_gate() {
        let f = build(true);
        let store = lookup_with(&[]); // evidence 1 not persisted
        let verifier = FindingVerifier::new(&store);
        assert_eq!(
            verifier.verify_for_gate(&f).unwrap_err(),
            VerificationError::UnresolvedEvidence(EvidenceId::new(1))
        );
        assert!(!verifier.can_block(&f, &FindingGate::new(EvidenceClass::C, RiskLevel::Low)));
    }

    #[test]
    fn candidate_detector_cannot_block_even_when_referentially_valid() {
        let f = build(false); // produced by a Candidate (forced by admission)
        let store = lookup_with(&[1]);
        let verifier = FindingVerifier::new(&store);
        assert!(
            verifier.verify_for_gate(&f).is_ok(),
            "referential truth holds"
        );
        assert!(
            !verifier.can_block(&f, &FindingGate::new(EvidenceClass::C, RiskLevel::Low)),
            "but a Candidate run must never block"
        );
    }

    #[test]
    fn causal_evidence_outside_the_finding_is_rejected() {
        let mut f = build(true);
        // Point the causal step at an evidence id not in the finding.
        f.causal_chain[0].evidence = Some(EvidenceId::new(99));
        let store = lookup_with(&[1, 99]);
        let verifier = FindingVerifier::new(&store);
        assert_eq!(
            verifier.verify_for_gate(&f).unwrap_err(),
            VerificationError::CausalEvidenceNotInFinding {
                step: 0,
                evidence: EvidenceId::new(99),
            }
        );
    }
}
