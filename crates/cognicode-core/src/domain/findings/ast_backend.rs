//! AST backend — the first (deliberately modest) detector backend
//! (M6, cycle e57).
//!
//! It matches the IR's `MATCH` subjects against a supplied AST view and emits
//! one evidence item + one match per hit. It does **not** build findings: the
//! executor persists the evidence and the
//! [`FindingAssembler`](super::FindingAssembler) owns the rest. The goal is
//! the architecture (IR → admission → plan → backend → evidence → finding →
//! gate), not detection intelligence.
//!
//! ## AST view
//!
//! The backend consumes an abstract [`AstInput`] of units and constructs.
//! Wiring a real tree-sitter extractor that produces these constructs is a
//! follow-up; the backend contract does not change when it lands.
//!
//! Pure domain: no I/O.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::admission::AdmittedDetector;
use super::detector_ir::{AnalysisCapability, DetectorStep, SubjectPattern};
use super::execution::{AnalysisInput, BackendError, DetectorBackend};
use super::finding::{CausalStepKind, EvidenceClass};
use super::outcome::{
    CausalObservation, DetectorMatch, DetectorOutcome, EvidenceKind, ProducedEvidence,
};

/// One AST construct observed in a unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstConstruct {
    /// The namespaced subject this construct constitutes.
    pub subject: SubjectPattern,
    /// 1-based line.
    pub line: u32,
    /// Human-readable detail.
    pub detail: String,
}

/// One source unit and the constructs observed in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstUnit {
    /// File path (for messages).
    pub path: String,
    /// Constructs observed in the unit.
    pub constructs: Vec<AstConstruct>,
}

/// The AST view handed to [`AstBackend`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstInput {
    /// Source units.
    pub units: Vec<AstUnit>,
}

/// The AST/construct backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct AstBackend;

impl DetectorBackend for AstBackend {
    fn name(&self) -> &'static str {
        "ast"
    }

    fn capabilities(&self) -> BTreeSet<AnalysisCapability> {
        // It matches syntactic constructs against already-classified subjects.
        // It does NOT resolve symbols or types, so it must not advertise
        // `SemanticResolution`: a detector that needs it must plan elsewhere.
        [AnalysisCapability::AstPattern].into_iter().collect()
    }

    fn evidence_ceiling(&self) -> EvidenceClass {
        // AST matching can never produce stronger than partial static
        // evidence (class C): no graph paths, no runtime traces.
        EvidenceClass::C
    }

    fn run(
        &self,
        admitted: &AdmittedDetector,
        input: &AnalysisInput,
    ) -> Result<DetectorOutcome, BackendError> {
        let ast = input
            .ast
            .as_ref()
            .ok_or(BackendError::MissingInput("ast"))?;

        // Collect the subjects the detector observes and the kind it produces.
        let mut subjects: Vec<&SubjectPattern> = Vec::new();
        let mut produce = None;
        for step in &admitted.definition.steps {
            match step {
                DetectorStep::Match { subject } => subjects.push(subject),
                DetectorStep::Produce { kind } => produce = Some(kind),
                _ => {}
            }
        }
        let produce = produce.ok_or(BackendError::Internal(
            "detector has no PRODUCE step".to_string(),
        ))?;

        let mut outcome = DetectorOutcome::empty();
        if subjects.is_empty() {
            outcome
                .diagnostics
                .push(super::outcome::DetectorDiagnostic {
                    code: "no_match_steps".to_string(),
                    message: "detector declares no MATCH subject; nothing to scan".to_string(),
                });
            return Ok(outcome);
        }

        for unit in &ast.units {
            for construct in &unit.constructs {
                if !subjects.iter().any(|s| *s == &construct.subject) {
                    continue;
                }

                let evidence_index = outcome.produced_evidence.len();
                outcome.produced_evidence.push(ProducedEvidence {
                    kind: EvidenceKind::AstMatch,
                    detail: format!(
                        "{} at {}:{} ({})",
                        construct.subject, unit.path, construct.line, construct.detail
                    ),
                    subject: None,
                    fact: None,
                });

                outcome.matches.push(DetectorMatch {
                    kind: produce.clone(),
                    message: format!("{} detected at {}:{}", produce, unit.path, construct.line),
                    evidence: vec![evidence_index],
                    causal: vec![CausalObservation {
                        kind: CausalStepKind::Source,
                        detail: format!(
                            "{} at {}:{}",
                            construct.subject, unit.path, construct.line
                        ),
                        subject: None,
                        fact: None,
                        evidence: Some(evidence_index),
                    }],
                });
            }
        }

        Ok(outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::findings::DetectorAuthority;
    use crate::domain::findings::admission::{AdmissionSource, DetectorAdmission, ExecutionPermit};
    use crate::domain::findings::detector_ir::{DetectorId, DetectorIr, FindingKind};

    fn admitted_md5() -> ExecutionPermit {
        let ir = DetectorIr {
            id: DetectorId::new("security.weak_hash").unwrap(),
            name: "weak hash".to_string(),
            policy: super::super::detector_ir::DetectorFindingPolicy::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Gated,
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

    fn input_with(subject: &str) -> AnalysisInput {
        AnalysisInput {
            graph: None,
            ast: Some(AstInput {
                units: vec![AstUnit {
                    path: "src/hash.rs".to_string(),
                    constructs: vec![AstConstruct {
                        subject: SubjectPattern::new(subject).unwrap(),
                        line: 12,
                        detail: "md5::Md5::new()".to_string(),
                    }],
                }],
            }),
        }
    }

    #[test]
    fn matches_the_declared_subject() {
        let outcome = AstBackend
            .run(admitted_md5().admitted(), &input_with("security.md5_usage"))
            .unwrap();
        assert_eq!(outcome.matches.len(), 1);
        assert_eq!(outcome.produced_evidence.len(), 1);
        assert_eq!(outcome.matches[0].kind.as_str(), "security.weak_hash");
        assert_eq!(outcome.matches[0].evidence, vec![0]);
    }

    #[test]
    fn ignores_unrelated_constructs() {
        let outcome = AstBackend
            .run(
                admitted_md5().admitted(),
                &input_with("security.sha256_usage"),
            )
            .unwrap();
        assert!(outcome.is_empty());
        assert!(outcome.diagnostics.is_empty());
    }

    #[test]
    fn missing_ast_input_fails_loud() {
        let err = AstBackend
            .run(admitted_md5().admitted(), &AnalysisInput::default())
            .unwrap_err();
        assert_eq!(err, BackendError::MissingInput("ast"));
    }

    #[test]
    fn no_match_steps_is_a_diagnostic_not_a_panic() {
        let ir = DetectorIr {
            id: DetectorId::new("security.no_match").unwrap(),
            name: "no match".to_string(),
            policy: super::super::detector_ir::DetectorFindingPolicy::default(),
            requires: [AnalysisCapability::AstPattern].into_iter().collect(),
            authority: DetectorAuthority::Candidate,
            steps: vec![DetectorStep::Produce {
                kind: FindingKind::new("security.no_match").unwrap(),
            }],
        };
        let admitted = DetectorAdmission::admit(ir, "1", AdmissionSource::Builtin).unwrap();
        let outcome = AstBackend
            .run(admitted.admitted(), &input_with("security.md5_usage"))
            .unwrap();
        assert!(outcome.is_empty());
        assert_eq!(outcome.diagnostics[0].code, "no_match_steps");
    }
}
