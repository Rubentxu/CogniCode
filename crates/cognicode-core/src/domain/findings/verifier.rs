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
use super::ports::{
    EvidenceDescriptor, EvidenceLookup, EvidenceResolution, FactDescriptor, FactSlot,
};
use super::scope::AnalysisScope;
use crate::domain::kernel_ids::{EvidenceGrade, EvidenceId, FactId, SnapshotId};

/// The id of a descriptor (kept in one place so error payloads never disagree).
fn id_of(descriptor: &EvidenceDescriptor) -> EvidenceId {
    descriptor.id
}

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
        // Scope first, before resolving any id: `FactId`/`EvidenceId` are
        // canonical per snapshot, so a finding produced in snapshot A must
        // never be verified against a read model hydrated from B — even when
        // every numeric id happens to coincide.
        let finding_scope = finding.detector.scope();
        let lookup_scope = self.evidence.scope();
        if finding_scope != lookup_scope {
            return Err(VerificationError::ScopeMismatch {
                finding: finding_scope.cloned(),
                lookup: lookup_scope.cloned(),
            });
        }

        finding.validate().map_err(VerificationError::Malformed)?;

        if !finding.is_explainable() {
            return Err(VerificationError::NotExplainable);
        }

        if !finding.detector.is_well_formed() {
            return Err(VerificationError::IncompleteExecutionRef);
        }

        let scope_snapshot = finding_scope.map(|s| s.snapshot);

        // Every claimed evidence id must resolve to *canonical truth*: not just
        // an id that exists, but an evidence atom whose grade and fact agree
        // with the claim it is being used to support.
        for id in &finding.evidence {
            let descriptor = match self.evidence.resolve(*id) {
                EvidenceResolution::Known(descriptor) => descriptor,
                EvidenceResolution::Unknown => {
                    return Err(VerificationError::UnresolvedEvidence(*id));
                }
            };
            Self::check_coherence(*id, descriptor, scope_snapshot, None)?;
        }

        // Every causal step must be grounded and coherent: it needs both an
        // evidence atom and a fact, the atom must belong to this finding, and
        // the fact the step names must be the fact the evidence grades.
        for (step, cs) in finding.causal_chain.iter().enumerate() {
            let (Some(id), Some(step_fact)) = (cs.evidence, cs.fact) else {
                return Err(VerificationError::UngroundedCausalStep { step });
            };
            if !finding.evidence.contains(&id) {
                return Err(VerificationError::CausalEvidenceNotInFinding { step, evidence: id });
            }
            let descriptor = match self.evidence.resolve(id) {
                EvidenceResolution::Known(descriptor) => descriptor,
                EvidenceResolution::Unknown => {
                    return Err(VerificationError::CausalEvidenceNotResolved {
                        step,
                        evidence: id,
                    });
                }
            };
            let fact = Self::check_coherence(id, descriptor, scope_snapshot, Some(step))?;

            if fact.id != step_fact {
                return Err(VerificationError::FactMismatch {
                    step,
                    step_fact,
                    evidence_fact: fact.id,
                });
            }
            // A step that names a subject makes a checkable claim about the
            // fact: the fact must agree, and a fact that cannot attest its
            // subject cannot back the claim.
            if let Some(subject) = cs.subject {
                if fact.subject != Some(subject) {
                    return Err(VerificationError::SubjectMismatch {
                        step,
                        step_subject: subject,
                        fact_subject: fact.subject,
                    });
                }
            }
        }

        Ok(())
    }

    /// Verify that an evidence atom may back a claim, returning its fact.
    ///
    /// `step` is the causal step index when the atom is being checked in a
    /// causal position, and `None` when it is checked as claimed evidence.
    fn check_coherence<'d>(
        id: EvidenceId,
        descriptor: &'d EvidenceDescriptor,
        scope_snapshot: Option<SnapshotId>,
        step: Option<usize>,
    ) -> Result<&'d FactDescriptor, VerificationError> {
        if !descriptor.grade.supports_a_claim() {
            return Err(VerificationError::RefutingEvidence {
                evidence: id,
                grade: descriptor.grade,
                step,
            });
        }
        let fact = match &descriptor.fact {
            FactSlot::Resolved(fact) => fact,
            FactSlot::Dangling { id } => {
                return Err(VerificationError::DanglingFact {
                    evidence: id_of(descriptor),
                    fact: *id,
                });
            }
        };
        if let Some(snapshot) = scope_snapshot {
            if fact.snapshot != snapshot {
                return Err(VerificationError::SnapshotMismatch {
                    evidence: id_of(descriptor),
                    fact: fact.id,
                    fact_snapshot: fact.snapshot,
                    scope_snapshot: snapshot,
                });
            }
        }
        Ok(fact)
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
    /// The finding's analysis scope does not match the lookup's.
    ScopeMismatch {
        /// Scope recorded on the finding's execution.
        finding: Option<AnalysisScope>,
        /// Scope the lookup was hydrated for.
        lookup: Option<AnalysisScope>,
    },
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
    /// A causal step is not grounded: it carries no evidence, or no fact.
    ///
    /// An ungrounded route is still explainable; it just cannot gate.
    UngroundedCausalStep {
        /// Causal step index.
        step: usize,
    },
    /// The evidence refutes its fact, so it cannot license a claim.
    RefutingEvidence {
        /// The evidence id.
        evidence: EvidenceId,
        /// Its grade.
        grade: EvidenceGrade,
        /// Causal step index, when checked in a causal position.
        step: Option<usize>,
    },
    /// The evidence points at a fact that does not exist in this scope.
    DanglingFact {
        /// The evidence id.
        evidence: EvidenceId,
        /// The fact it points at.
        fact: FactId,
    },
    /// The fact belongs to a different snapshot than the execution's scope.
    SnapshotMismatch {
        /// The evidence id.
        evidence: EvidenceId,
        /// The fact id.
        fact: FactId,
        /// The snapshot the fact belongs to.
        fact_snapshot: SnapshotId,
        /// The snapshot the execution was pinned to.
        scope_snapshot: SnapshotId,
    },
    /// A causal step names a fact that its evidence does not grade.
    FactMismatch {
        /// Causal step index.
        step: usize,
        /// The fact the step claims.
        step_fact: FactId,
        /// The fact the evidence actually grades.
        evidence_fact: FactId,
    },
    /// A causal step names a subject that its fact does not concern.
    SubjectMismatch {
        /// Causal step index.
        step: usize,
        /// The subject the step claims.
        step_subject: crate::domain::kernel_ids::EntityId,
        /// The subject the fact actually carries.
        fact_subject: Option<crate::domain::kernel_ids::EntityId>,
    },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScopeMismatch { finding, lookup } => write!(
                f,
                "scope mismatch: the finding was produced in {} but the lookup was hydrated for {}",
                finding
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<unscoped>".into()),
                lookup
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<unscoped>".into())
            ),
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
            Self::UngroundedCausalStep { step } => write!(
                f,
                "causal step {step} is not grounded (no evidence or no canonical fact)"
            ),
            Self::RefutingEvidence {
                evidence,
                grade,
                step,
            } => match step {
                Some(step) => write!(
                    f,
                    "causal step {step} is backed by {evidence}, which {grade}s its fact"
                ),
                None => write!(
                    f,
                    "claimed evidence {evidence} {grade}s its fact and cannot back the finding"
                ),
            },
            Self::DanglingFact { evidence, fact } => write!(
                f,
                "evidence {evidence} grades fact {fact}, which does not exist in this scope"
            ),
            Self::SnapshotMismatch {
                evidence,
                fact,
                fact_snapshot,
                scope_snapshot,
            } => write!(
                f,
                "evidence {evidence} grades fact {fact} from snapshot {fact_snapshot}, but the execution was pinned to {scope_snapshot}"
            ),
            Self::FactMismatch {
                step,
                step_fact,
                evidence_fact,
            } => write!(
                f,
                "causal step {step} claims fact {step_fact} but its evidence grades {evidence_fact}"
            ),
            Self::SubjectMismatch {
                step,
                step_subject,
                fact_subject,
            } => write!(
                f,
                "causal step {step} claims subject {step_subject} but its fact concerns {}",
                fact_subject
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ),
        }
    }
}

impl std::error::Error for VerificationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
    use crate::domain::findings::assembler::FindingAssembler;
    use crate::domain::findings::binding::EvidenceBindings;
    use crate::domain::findings::detector_ir::{
        DetectorId, DetectorIr, DetectorStep, FindingKind, SubjectPattern,
    };
    use crate::domain::findings::finding::{CausalStepKind, EvidenceClass, RiskLevel};
    use crate::domain::findings::outcome::{
        CausalObservation, DetectorMatch, DetectorOutcome, EvidenceKind, ProducedEvidence,
    };
    use crate::domain::findings::ports::{EvidenceDescriptor, FactDescriptor, FactSlot};
    use crate::domain::kernel_ids::{ExecutionId, FactId};

    /// A hand-built lookup: every id resolves to a supporting descriptor whose
    /// fact lives in the given scope's snapshot.
    struct SetLookup {
        descriptors: Vec<EvidenceDescriptor>,
        scope: Option<AnalysisScope>,
    }

    impl SetLookup {
        fn scoped(ids: &[u64], scope: AnalysisScope) -> Self {
            let descriptors = ids
                .iter()
                .map(|n| EvidenceDescriptor {
                    id: EvidenceId::new(*n),
                    grade: EvidenceGrade::Supports,
                    fact: FactSlot::Resolved(FactDescriptor {
                        id: FactId::new(7),
                        subject: None,
                        snapshot: scope.snapshot,
                    }),
                })
                .collect();
            Self {
                descriptors,
                scope: Some(scope),
            }
        }

        /// Replace one id's descriptor, so a test can make it refute, dangle,
        /// or name a different fact.
        fn with_descriptor(mut self, id: u64, descriptor: EvidenceDescriptor) -> Self {
            if let Some(slot) = self.descriptors.iter_mut().find(|d| d.id.get() == id) {
                *slot = descriptor;
            }
            self
        }
    }

    impl EvidenceLookup for SetLookup {
        fn resolve(&self, id: EvidenceId) -> EvidenceResolution<'_> {
            self.descriptors
                .iter()
                .find(|d| d.id == id)
                .map(EvidenceResolution::Known)
                .unwrap_or(EvidenceResolution::Unknown)
        }

        fn scope(&self) -> Option<&AnalysisScope> {
            self.scope.as_ref()
        }
    }

    /// A detector execution context for tests, in `scope(snapshot)`.
    fn test_context(id: u64) -> crate::domain::execution::ExecutionContext {
        crate::domain::execution::ExecutionContext::try_new(
            ExecutionId(id),
            scope(1),
            crate::domain::execution::ActorRef::detector("security.weak_hash"),
            crate::domain::execution::CorrelationId::new("c").unwrap(),
            None,
        )
        .unwrap()
    }

    fn scope(snapshot: u64) -> AnalysisScope {
        AnalysisScope::new(
            crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
            crate::domain::kernel_ids::SnapshotId::new(snapshot),
        )
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
        let request =
            super::super::admission::PromotionRequest::for_permit(&candidate, "team").unwrap();
        let verified = super::super::admission::PromotionAuthority::verify(
            &super::super::admission::EligibleSourceVerifier,
            request,
        )
        .unwrap();
        DetectorAdmission::promote(&candidate, verified).unwrap()
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
        let execution = permit.execution_ref(Some(test_context(1))).unwrap();
        let outcome = DetectorOutcome {
            produced_evidence: vec![ProducedEvidence {
                kind: EvidenceKind::AstMatch,
                detail: "md5".to_string(),
                subject: None,
                grounding: Some(super::super::GroundingRef::fact(FactId::new(7))),
            }],
            matches: vec![DetectorMatch {
                kind: FindingKind::new("security.weak_hash").unwrap(),
                message: "md5".to_string(),
                evidence: vec![0],
                causal: vec![CausalObservation {
                    kind: CausalStepKind::Source,
                    detail: "md5".to_string(),
                    subject: None,
                    evidence: Some(0),
                }],
            }],
            diagnostics: vec![],
        };
        let bindings = EvidenceBindings::new(vec![super::super::EvidenceBinding::grounded(
            EvidenceId::new(1),
            FactId::new(7),
        )]);
        FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings)
            .unwrap()
            .pop()
            .unwrap()
    }

    fn lookup_with(ids: &[u64]) -> SetLookup {
        SetLookup::scoped(ids, scope(1))
    }

    /// C — evidence that *refutes* its fact can never license a gate, even when
    /// the id resolves and the shape is perfect.
    #[test]
    fn refuting_evidence_never_gates() {
        let f = build(true);
        let store = lookup_with(&[1]).with_descriptor(
            1,
            EvidenceDescriptor {
                id: EvidenceId::new(1),
                grade: EvidenceGrade::Refutes,
                fact: FactSlot::Resolved(FactDescriptor {
                    id: FactId::new(7),
                    subject: None,
                    snapshot: scope(1).snapshot,
                }),
            },
        );
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::RefutingEvidence { .. })
        ));
        assert!(!verifier.can_block(&f, &FindingGate::new(EvidenceClass::C, RiskLevel::Low)));
    }

    /// B — a causal step whose declared fact is not the fact its evidence
    /// grades is incoherent, even when both facts exist and the ids resolve.
    #[test]
    fn a_fact_that_disagrees_with_its_evidence_is_rejected() {
        let mut f = build(true);
        f.causal_chain[0].fact = Some(FactId::new(8));
        let store = lookup_with(&[1]);
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::FactMismatch { .. })
        ));
    }

    /// A step with no fact (or no evidence) is ungrounded, not verified.
    #[test]
    fn an_ungrounded_causal_step_is_rejected() {
        let mut f = build(true);
        f.causal_chain[0].fact = None;
        let store = lookup_with(&[1]);
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::UngroundedCausalStep { step: 0 })
        ));
    }

    /// Evidence whose fact belongs to another snapshot cannot back a finding
    /// pinned to this one, even when the evidence id itself resolves.
    #[test]
    fn a_fact_from_another_snapshot_is_rejected() {
        let f = build(true);
        let store = lookup_with(&[1]).with_descriptor(
            1,
            EvidenceDescriptor {
                id: EvidenceId::new(1),
                grade: EvidenceGrade::Supports,
                fact: FactSlot::Resolved(FactDescriptor {
                    id: FactId::new(7),
                    subject: None,
                    snapshot: scope(2).snapshot,
                }),
            },
        );
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::SnapshotMismatch { .. })
        ));
    }

    /// A step that names a subject makes a checkable claim about its fact.
    #[test]
    fn a_step_subject_that_disagrees_with_the_fact_is_rejected() {
        let mut f = build(true);
        f.causal_chain[0].subject = Some(crate::domain::kernel_ids::EntityId::new(3));
        let store = lookup_with(&[1]);
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::SubjectMismatch { .. })
        ));
    }

    /// Evidence pointing at a fact that does not exist is dangling.
    #[test]
    fn dangling_facts_are_rejected() {
        let f = build(true);
        let store = lookup_with(&[1]).with_descriptor(
            1,
            EvidenceDescriptor {
                id: EvidenceId::new(1),
                grade: EvidenceGrade::Supports,
                fact: FactSlot::Dangling { id: FactId::new(7) },
            },
        );
        let verifier = FindingVerifier::new(&store);
        assert!(matches!(
            verifier.verify_for_gate(&f),
            Err(VerificationError::DanglingFact { .. })
        ));
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
    #[test]
    fn scope_mismatch_is_rejected_before_any_id_is_resolved() {
        // The SAME evidence id (and the same numeric fact) exists in snapshots
        // 1 and 2. A finding produced in snapshot 1 verified against a lookup
        // hydrated from snapshot 2 must be rejected even though every id
        // resolves.
        let f = build(true);
        assert_eq!(
            f.detector.scope().as_ref().map(|s| s.snapshot),
            Some(crate::domain::kernel_ids::SnapshotId::new(1))
        );

        let wrong_snapshot = SetLookup::scoped(&[1], scope(2));
        let verifier = FindingVerifier::new(&wrong_snapshot);
        assert!(matches!(
            verifier.verify_for_gate(&f).unwrap_err(),
            VerificationError::ScopeMismatch { .. }
        ));
        assert!(
            !verifier.can_block(&f, &FindingGate::new(EvidenceClass::C, RiskLevel::Low)),
            "a finding must never be verified against another snapshot"
        );

        // Sanity: the same ids against the CORRECT snapshot verify fine.
        let right_snapshot = SetLookup::scoped(&[1], scope(1));
        assert!(
            FindingVerifier::new(&right_snapshot)
                .verify_for_gate(&f)
                .is_ok()
        );
    }
}
