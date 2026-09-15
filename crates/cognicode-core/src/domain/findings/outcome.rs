//! Backend outputs — raw analysis facts, never [`Finding`](super::Finding)s
//! (M6, cycle e57).
//!
//! A backend returns a [`DetectorOutcome`]: the evidence it produced, the
//! matches it observed, and diagnostics. It does **not** construct findings —
//! that is the sole job of [`FindingAssembler`](super::FindingAssembler), so
//! every backend produces findings the same way (evidence class, execution
//! reference, evidence ids, origin and causal chain are all assigned in one
//! place).
//!
//! Pure domain: no I/O.

use serde::{Deserialize, Serialize};

use super::FindingKind;
use super::finding::{CausalStepKind, EvidenceClass};
use crate::domain::kernel_ids::{EntityId, FactId};

/// What kind of analysis produced a piece of evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    /// A syntactic/AST match.
    AstMatch,
    /// A graph path (call/flow projection).
    GraphPath,
    /// A dataflow path (def-use / taint).
    DataflowPath,
    /// A runtime trace observation.
    RuntimeTrace,
    /// A hypothesis with no mechanical backing.
    Hypothesis,
}

impl EvidenceKind {
    /// The evidence class this kind alone can support.
    ///
    /// The assembler takes the strongest class across a match's evidence, so
    /// a backend cannot inflate the class beyond what its evidence supports.
    pub fn class(self) -> EvidenceClass {
        match self {
            Self::RuntimeTrace => EvidenceClass::A,
            Self::GraphPath | Self::DataflowPath => EvidenceClass::B,
            Self::AstMatch => EvidenceClass::C,
            Self::Hypothesis => EvidenceClass::D,
        }
    }

    /// Stable name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::AstMatch => "ast_match",
            Self::GraphPath => "graph_path",
            Self::DataflowPath => "dataflow_path",
            Self::RuntimeTrace => "runtime_trace",
            Self::Hypothesis => "hypothesis",
        }
    }
}

impl std::fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// A piece of evidence a backend produced, before an id is assigned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducedEvidence {
    /// What kind of analysis produced it.
    pub kind: EvidenceKind,
    /// Human-readable detail.
    pub detail: String,
    /// Subject entity, if the backend resolved one.
    pub subject: Option<EntityId>,
    /// Backing fact, if any.
    pub fact: Option<FactId>,
}

/// One observation on a matched finding's causal path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalObservation {
    /// The role of this step.
    pub kind: CausalStepKind,
    /// Human-readable detail.
    pub detail: String,
    /// Subject entity, if any.
    pub subject: Option<EntityId>,
    /// Backing fact, if any.
    pub fact: Option<FactId>,
    /// Index into [`DetectorOutcome::produced_evidence`], if any.
    pub evidence: Option<usize>,
}

/// A match a backend observed, before evidence ids are assigned.
///
/// Deliberately carries **no** severity/risk: a backend observes facts, it
/// does not decide how severe or risky the result is. Severity and risk come
/// from the detector's [`DetectorFindingPolicy`](super::DetectorFindingPolicy),
/// applied by the assembler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorMatch {
    /// What the match constitutes.
    pub kind: FindingKind,
    /// Human-readable message.
    pub message: String,
    /// Indices into [`DetectorOutcome::produced_evidence`].
    pub evidence: Vec<usize>,
    /// Causal path for this match.
    pub causal: Vec<CausalObservation>,
}

/// A non-fatal backend diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorDiagnostic {
    /// Stable code (e.g. `no_ast_input`).
    pub code: String,
    /// Human-readable message.
    pub message: String,
}

/// The raw output of a backend run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorOutcome {
    /// Evidence produced during the run.
    pub produced_evidence: Vec<ProducedEvidence>,
    /// Matches observed during the run.
    pub matches: Vec<DetectorMatch>,
    /// Non-fatal diagnostics.
    pub diagnostics: Vec<DetectorDiagnostic>,
}

impl DetectorOutcome {
    /// An empty outcome (no matches, no evidence).
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether the run produced no matches.
    pub fn is_empty(&self) -> bool {
        self.matches.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_kind_classes_are_ordered_by_strength() {
        assert_eq!(EvidenceKind::RuntimeTrace.class(), EvidenceClass::A);
        assert_eq!(EvidenceKind::GraphPath.class(), EvidenceClass::B);
        assert_eq!(EvidenceKind::DataflowPath.class(), EvidenceClass::B);
        assert_eq!(EvidenceKind::AstMatch.class(), EvidenceClass::C);
        assert_eq!(EvidenceKind::Hypothesis.class(), EvidenceClass::D);
    }

    #[test]
    fn outcome_round_trips() {
        let outcome = DetectorOutcome {
            produced_evidence: vec![ProducedEvidence {
                kind: EvidenceKind::AstMatch,
                detail: "md5 at src/hash.rs:12".to_string(),
                subject: Some(EntityId::new(1)),
                fact: None,
            }],
            matches: vec![DetectorMatch {
                kind: FindingKind::new("security.weak_hash").unwrap(),
                message: "weak hash".to_string(),
                evidence: vec![0],
                causal: vec![CausalObservation {
                    kind: CausalStepKind::Source,
                    detail: "md5 usage".to_string(),
                    subject: None,
                    fact: None,
                    evidence: Some(0),
                }],
            }],
            diagnostics: vec![DetectorDiagnostic {
                code: "ok".to_string(),
                message: "done".to_string(),
            }],
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let parsed: DetectorOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, outcome);
    }
}
