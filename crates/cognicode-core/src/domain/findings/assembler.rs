//! Finding assembler — the single place that turns backend output into
//! [`Finding`]s (M6, cycle e57).
//!
//! Backends emit raw [`DetectorOutcome`]s; only the assembler assigns the
//! evidence class, the detector execution reference, evidence ids, the
//! origin, the causal chain and the finding id. This is what keeps every
//! backend (AST now; graph/dataflow later) producing findings the same way,
//! so no backend can invent its own `Finding` shape or inflate its class.
//!
//! Pure domain: no I/O.

use std::fmt;

use super::admission::AdmittedDetector;
use super::binding::EvidenceBindings;
use super::detector_ir::DetectorExecutionRef;
use super::finding::{CausalStep, EvidenceClass, Finding, FindingError, FindingId, FindingOrigin};
use super::outcome::{DetectorMatch, DetectorOutcome};
use crate::domain::kernel_ids::EvidenceId;

/// Assigns evidence bindings, class, execution ref and causal chain.
#[derive(Debug, Clone, Copy, Default)]
pub struct FindingAssembler;

impl FindingAssembler {
    /// Assemble findings from a backend outcome and its evidence bindings.
    ///
    /// `bindings` must be index-aligned with
    /// [`DetectorOutcome::produced_evidence`].
    ///
    /// Two rules make the result auditable:
    ///
    /// - a finding claims **only grounded** evidence, so its `evidence` set is
    ///   exactly what carries canonical truth;
    /// - a causal step's fact is taken from the *binding*, never from the
    ///   backend's own observation, so a step cannot state a fact its evidence
    ///   does not carry.
    pub fn assemble(
        admitted: &AdmittedDetector,
        execution: &DetectorExecutionRef,
        outcome: &DetectorOutcome,
        bindings: &EvidenceBindings,
    ) -> Result<Vec<Finding>, AssemblyError> {
        if bindings.len() != outcome.produced_evidence.len() {
            return Err(AssemblyError::EvidenceArity {
                ids: bindings.len(),
                produced: outcome.produced_evidence.len(),
            });
        }

        let mut findings = Vec::with_capacity(outcome.matches.len());
        for (match_index, m) in outcome.matches.iter().enumerate() {
            findings.push(Self::assemble_one(
                admitted,
                execution,
                outcome,
                bindings,
                match_index,
                m,
            )?);
        }
        Ok(findings)
    }

    fn assemble_one(
        admitted: &AdmittedDetector,
        execution: &DetectorExecutionRef,
        outcome: &DetectorOutcome,
        bindings: &EvidenceBindings,
        match_index: usize,
        m: &DetectorMatch,
    ) -> Result<Finding, AssemblyError> {
        // The finding claims the grounded subset of the match's evidence: an
        // ungrounded item is still produced and still explainable, but it
        // carries no canonical truth and must not appear as claimed evidence.
        let mut evidence: Vec<EvidenceId> = Vec::with_capacity(m.evidence.len());
        for &index in &m.evidence {
            let binding = bindings
                .get(index)
                .ok_or(AssemblyError::EvidenceIndexOutOfRange { match_index, index })?;
            if let Some(id) = binding.id() {
                if !evidence.contains(&id) {
                    evidence.push(id);
                }
            }
        }

        // The assembler owns the class: the strongest class its evidence
        // supports, or D when there is no evidence.
        let evidence_class = m
            .evidence
            .iter()
            .filter_map(|i| outcome.produced_evidence.get(*i))
            .map(|pe| pe.kind.class())
            .min()
            .unwrap_or(EvidenceClass::D);

        // Build the causal chain, resolving evidence indices and requiring
        // every causal evidence id to belong to this finding's evidence set.
        let mut causal_chain = Vec::with_capacity(m.causal.len());
        for (step, obs) in m.causal.iter().enumerate() {
            // A step is grounded when the evidence it points at is grounded;
            // its fact *is* that evidence's canonical fact.
            let grounded = match obs.evidence {
                None => None,
                Some(index) => {
                    let binding =
                        bindings
                            .get(index)
                            .ok_or(AssemblyError::CausalEvidenceOutOfRange {
                                match_index,
                                step,
                                index,
                            })?;
                    match binding {
                        super::binding::EvidenceBinding::Grounded { id, fact } => {
                            if !evidence.contains(id) {
                                return Err(AssemblyError::CausalEvidenceNotInFinding {
                                    match_index,
                                    step,
                                    evidence: *id,
                                });
                            }
                            Some((*id, *fact))
                        }
                        super::binding::EvidenceBinding::Ungrounded { .. } => None,
                    }
                }
            };

            let mut causal =
                CausalStep::new(obs.kind, obs.detail.clone()).map_err(AssemblyError::Invalid)?;
            causal.subject = obs.subject;
            match grounded {
                Some((id, fact)) => {
                    causal.evidence = Some(id);
                    causal.fact = Some(fact);
                }
                None => {
                    causal.evidence = None;
                    causal.fact = None;
                }
            }
            causal_chain.push(causal);
        }

        let execution_key = execution.execution_id.map(|e| e.0).unwrap_or(0);
        let id = FindingId::new(format!(
            "{}:exec{}:m{}",
            admitted.definition.id, execution_key, match_index
        ))
        .map_err(AssemblyError::Invalid)?;

        let policy = admitted.definition.policy;
        let message = if m.message.trim().is_empty() {
            format!("detector {} matched", admitted.definition.id)
        } else {
            m.message.clone()
        };

        let finding = Finding {
            id,
            kind: m.kind.clone(),
            origin: FindingOrigin::Detector,
            severity: policy.default_severity,
            risk: policy.default_risk,
            evidence_class,
            evidence,
            detector: execution.clone(),
            status: super::finding::FindingStatus::Open,
            message,
            causal_chain,
        };

        finding.validate().map_err(AssemblyError::Invalid)?;
        Ok(finding)
    }
}

/// Why assembly failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyError {
    /// `evidence_ids` and `produced_evidence` lengths differ.
    EvidenceArity {
        /// Number of ids provided.
        ids: usize,
        /// Number of evidence produced.
        produced: usize,
    },
    /// A match referenced an evidence index out of range.
    EvidenceIndexOutOfRange {
        /// Match index.
        match_index: usize,
        /// Referenced evidence index.
        index: usize,
    },
    /// A causal observation referenced an evidence index out of range.
    CausalEvidenceOutOfRange {
        /// Match index.
        match_index: usize,
        /// Causal step index.
        step: usize,
        /// Referenced evidence index.
        index: usize,
    },
    /// A causal step pointed at evidence not in the finding's evidence set.
    CausalEvidenceNotInFinding {
        /// Match index.
        match_index: usize,
        /// Causal step index.
        step: usize,
        /// The offending evidence id.
        evidence: EvidenceId,
    },
    /// The assembled finding failed structural validation.
    Invalid(FindingError),
}

impl fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EvidenceArity { ids, produced } => write!(
                f,
                "evidence id count ({ids}) does not match produced evidence ({produced})"
            ),
            Self::EvidenceIndexOutOfRange { match_index, index } => {
                write!(
                    f,
                    "match {match_index} references evidence index {index} out of range"
                )
            }
            Self::CausalEvidenceOutOfRange {
                match_index,
                step,
                index,
            } => write!(
                f,
                "match {match_index} causal step {step} references evidence index {index} out of range"
            ),
            Self::CausalEvidenceNotInFinding {
                match_index,
                step,
                evidence,
            } => write!(
                f,
                "match {match_index} causal step {step} references {evidence} not in the finding's evidence"
            ),
            Self::Invalid(err) => write!(f, "assembled finding is invalid: {err}"),
        }
    }
}

impl std::error::Error for AssemblyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::GroundingRef;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission};
    use crate::domain::findings::binding::EvidenceBinding;
    use crate::domain::findings::detector_ir::{
        DetectorId, DetectorIr, DetectorStep, FindingKind, SubjectPattern,
    };
    use crate::domain::findings::finding::{CausalStepKind, FindingSeverity, RiskLevel};
    use crate::domain::findings::outcome::{CausalObservation, EvidenceKind, ProducedEvidence};
    use crate::domain::kernel_ids::{ExecutionId, FactId};

    /// Bindings for `n` grounded items, ids `1..=n`, all grading `FactId(7)`.
    fn grounded(n: usize) -> EvidenceBindings {
        EvidenceBindings::new(
            (1..=n)
                .map(|i| EvidenceBinding::grounded(EvidenceId::new(i as u64), FactId::new(7)))
                .collect(),
        )
    }

    fn grounded_evidence(kind: EvidenceKind, detail: &str) -> ProducedEvidence {
        ProducedEvidence {
            kind,
            detail: detail.to_string(),
            subject: None,
            grounding: Some(GroundingRef::fact(FactId::new(7))),
        }
    }

    fn permit() -> super::super::admission::ExecutionPermit {
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
        DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::HumanCurated).unwrap()
    }

    fn outcome_with(kind: EvidenceKind) -> DetectorOutcome {
        DetectorOutcome {
            produced_evidence: vec![grounded_evidence(kind, "md5 at src/hash.rs:12")],
            matches: vec![DetectorMatch {
                kind: FindingKind::new("security.weak_hash").unwrap(),
                message: "MD5 used".to_string(),
                evidence: vec![0],
                causal: vec![CausalObservation {
                    kind: CausalStepKind::Source,
                    detail: "md5 usage".to_string(),
                    subject: None,
                    evidence: Some(0),
                }],
            }],
            diagnostics: vec![],
        }
    }

    #[test]
    fn assembles_a_finding_with_assigned_class_and_execution() {
        let permit = permit();
        let execution = permit.execution_ref(Some(ExecutionId(5)), None).unwrap();
        let outcome = outcome_with(EvidenceKind::AstMatch);
        let bindings = EvidenceBindings::new(vec![EvidenceBinding::grounded(
            EvidenceId::new(11),
            FactId::new(7),
        )]);

        let findings =
            FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings).unwrap();
        assert_eq!(findings.len(), 1);
        let f = &findings[0];
        assert_eq!(f.id.as_str(), "security.weak_hash:exec5:m0");
        assert_eq!(f.evidence, vec![EvidenceId::new(11)]);
        assert_eq!(
            f.evidence_class,
            EvidenceClass::C,
            "AST evidence -> class C"
        );
        assert_eq!(f.detector.execution_id, Some(ExecutionId(5)));
        assert_eq!(f.origin, FindingOrigin::Detector);
        assert_eq!(f.causal_chain[0].evidence, Some(EvidenceId::new(11)));
        assert_eq!(
            f.causal_chain[0].fact,
            Some(FactId::new(7)),
            "a causal step's fact comes from the evidence binding, not the backend"
        );
        assert!(f.validate().is_ok());
    }

    #[test]
    fn class_is_the_strongest_of_the_evidence() {
        let permit = permit();
        let execution = permit.execution_ref(None, None).unwrap();
        let mut outcome = outcome_with(EvidenceKind::AstMatch);
        outcome
            .produced_evidence
            .push(grounded_evidence(EvidenceKind::RuntimeTrace, "observed"));
        outcome.matches[0].evidence = vec![0, 1];
        let bindings = grounded(2);

        let findings =
            FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings).unwrap();
        assert_eq!(findings[0].evidence_class, EvidenceClass::A);
    }

    #[test]
    fn no_evidence_yields_class_d() {
        let permit = permit();
        let execution = permit.execution_ref(None, None).unwrap();
        let mut outcome = outcome_with(EvidenceKind::Hypothesis);
        outcome.matches[0].evidence = vec![];
        outcome.matches[0].causal[0].evidence = None;
        let bindings = grounded(1);
        let findings =
            FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings).unwrap();
        assert_eq!(findings[0].evidence_class, EvidenceClass::D);
        assert!(findings[0].evidence.is_empty());
    }

    #[test]
    fn rejects_arity_mismatch() {
        let permit = permit();
        let execution = permit.execution_ref(None, None).unwrap();
        let outcome = outcome_with(EvidenceKind::AstMatch);
        let err = FindingAssembler::assemble(
            permit.admitted(),
            &execution,
            &outcome,
            &EvidenceBindings::default(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            AssemblyError::EvidenceArity {
                ids: 0,
                produced: 1
            }
        );
    }

    #[test]
    fn rejects_causal_evidence_not_in_finding() {
        let permit = permit();
        let execution = permit.execution_ref(None, None).unwrap();
        let mut outcome = outcome_with(EvidenceKind::AstMatch);
        outcome
            .produced_evidence
            .push(grounded_evidence(EvidenceKind::AstMatch, "other"));
        // Evidence set is [0]; causal step points at index 1.
        outcome.matches[0].causal[0].evidence = Some(1);
        let bindings = grounded(2);

        let err = FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings)
            .unwrap_err();
        assert_eq!(
            err,
            AssemblyError::CausalEvidenceNotInFinding {
                match_index: 0,
                step: 0,
                evidence: EvidenceId::new(2),
            }
        );
    }
    #[test]
    fn detector_policy_not_the_backend_decides_severity_and_risk() {
        // The backend's DetectorMatch carries neither severity nor risk; the
        // assembler takes both from the detector's finding policy.
        let mut ir = super::super::detector_ir::DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            policy: super::super::detector_ir::DetectorFindingPolicy::new(
                FindingSeverity::Critical,
                RiskLevel::Critical,
            ),
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
        let permit =
            DetectorAdmission::admit(ir.clone(), "1.0.0", AdmissionSource::Builtin).unwrap();
        let execution = permit.execution_ref(Some(ExecutionId(1)), None).unwrap();
        let outcome = outcome_with(EvidenceKind::AstMatch);
        let bindings = grounded(1);

        let findings =
            FindingAssembler::assemble(permit.admitted(), &execution, &outcome, &bindings).unwrap();
        assert_eq!(findings[0].severity, FindingSeverity::Critical);
        assert_eq!(findings[0].risk, RiskLevel::Critical);
        // The evidence class is still capped by the evidence itself (AST).
        assert_eq!(findings[0].evidence_class, EvidenceClass::C);

        // Changing only the policy changes severity/risk...
        ir.policy = super::super::detector_ir::DetectorFindingPolicy::new(
            FindingSeverity::Info,
            RiskLevel::Low,
        );
        let permit2 = DetectorAdmission::admit(ir, "1.0.0", AdmissionSource::Builtin).unwrap();
        let execution2 = permit2.execution_ref(Some(ExecutionId(1)), None).unwrap();
        let findings2 =
            FindingAssembler::assemble(permit2.admitted(), &execution2, &outcome, &bindings)
                .unwrap();
        assert_eq!(findings2[0].severity, FindingSeverity::Info);
        assert_eq!(findings2[0].risk, RiskLevel::Low);
    }
}
