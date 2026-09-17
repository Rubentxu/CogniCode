//! Hypothesis + Critique — AI-output advisory observation types.
//!
//! ## Hypothesis vs Fact vs Finding authority vs Evidence proof
//!
//! ```text
//! Hypothesis       advisory observation the reader interprets
//! Fact             canonical truth committed via FactStore::commit
//! Finding authority whether a finding is Gated / Warn / Informational
//! Evidence proof   a graded record that backs a finding's claim
//! ```
//!
//! A `Hypothesis` is **none** of the latter three. It may reference
//! canonical evidence; that reference is recorded in `supporting_refs`
//! and `contradicting_refs`. The reference does not turn the hypothesis
//! into canonical truth: the verifier still sees `EvidenceClass::D`
//! when a hypothesis backs a finding, and class `D` can never gate.
//!
//! ## Critique
//!
//! A `Critique` is an advisory disposition about an existing `Finding`.
//! It does not modify the finding, does not delete it, does not
//! promote its severity, and does not change its gate status. The
//! policy gate decides — the critic advises.

use serde::{Deserialize, Serialize};

use crate::domain::architecture::ArchitectureConstraintId;
use crate::domain::findings::FindingId;
use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId};

// ============================================================================
// HypothesisId
// ============================================================================

/// Stable id for one hypothesis. Constructed deterministically from
/// the response's request digest and the hypothesis's index in the
/// response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HypothesisId(pub u64);

impl HypothesisId {
    /// Construct from a numeric id.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// The raw value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for HypothesisId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "h:{}", self.0)
    }
}

// ============================================================================
// HypothesisConfidence
// ============================================================================

/// How confident the AI is in the hypothesis.
///
/// Bounded to `[0, 100]`. Stored as a `u8` to make the type small and
/// stable. The critic and the lineage layer treat the value as
/// **advisory**; the policy gate ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HypothesisConfidence(u8);

impl HypothesisConfidence {
    /// Construct, clamping to `[0, 100]`.
    pub fn new(value: u8) -> Self {
        Self(value.min(100))
    }

    /// The raw value in `[0, 100]`.
    pub fn value(self) -> u8 {
        self.0
    }
}

impl Default for HypothesisConfidence {
    fn default() -> Self {
        Self(50)
    }
}

// ============================================================================
// HypothesisRef
// ============================================================================

/// A reference to canonical knowledge the hypothesis depends on.
///
/// The reference is **advisory**: the hypothesis points at canonical
/// facts / evidence / entities / findings / architecture constraints
/// the agent consulted or cited. The reference does NOT turn the
/// hypothesis into a fact, finding, or evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HypothesisRef {
    /// A canonical fact id.
    Fact(FactId),
    /// A canonical evidence id.
    Evidence(EvidenceId),
    /// A canonical entity id.
    Entity(EntityId),
    /// A finding id (for cross-references between findings and hypotheses).
    Finding(FindingId),
    /// An architecture constraint id.
    ArchitectureConstraint(ArchitectureConstraintId),
}

impl HypothesisRef {
    /// A stable label for diagnostics.
    pub fn label(&self) -> String {
        match self {
            Self::Fact(f) => format!("fact:{}", f.get()),
            Self::Evidence(e) => format!("evidence:{}", e.get()),
            Self::Entity(e) => format!("entity:{}", e.get()),
            Self::Finding(f) => format!("finding:{}", f.as_str()),
            Self::ArchitectureConstraint(a) => format!("constraint:{}", a.as_str()),
        }
    }
}

// ============================================================================
// HypothesisStatement
// ============================================================================

/// The bounded textual statement of a hypothesis.
///
/// A statement is one of:
///
/// - A `Suggestion` — "I think X" / "consider Y" / "maybe Z".
/// - A `Question` — "is X true?" / "why does Y?" (an open question).
/// - A `Candidate { kind, summary }` — a typed candidate (architecture
///   constraint candidate, detector idea candidate, ownership
///   boundary candidate).
///
/// The discriminator prevents a `Candidate` from being silently
/// confused with a `Suggestion`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HypothesisStatement {
    /// A free-form suggestion.
    Suggestion(String),
    /// An open question.
    Question(String),
    /// A typed candidate definition.
    Candidate {
        /// The kind of candidate (e.g. "architecture_constraint",
        /// "detector_idea", "ownership_boundary").
        kind: String,
        /// The bounded summary.
        summary: String,
    },
}

impl HypothesisStatement {
    /// The bounded summary (always present).
    pub fn summary(&self) -> &str {
        match self {
            Self::Suggestion(s) | Self::Question(s) | Self::Candidate { summary: s, .. } => s,
        }
    }

    /// Whether this statement names a candidate kind.
    pub fn is_candidate(&self) -> bool {
        matches!(self, Self::Candidate { .. })
    }

    /// The candidate kind, if any.
    pub fn candidate_kind(&self) -> Option<&str> {
        match self {
            Self::Candidate { kind, .. } => Some(kind),
            _ => None,
        }
    }
}

// ============================================================================
// Hypothesis
// ============================================================================

/// An advisory AI-generated observation.
///
/// Constructed via [`Hypothesis::new`] which rejects an empty
/// statement and validates that supporting/contradicting refs do not
/// contain the same canonical id twice (de-duplication is the caller's
/// responsibility, but the type guarantees the supporting and
/// contradicting sets are disjoint).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hypothesis {
    id: HypothesisId,
    statement: HypothesisStatement,
    supporting_refs: Vec<HypothesisRef>,
    contradicting_refs: Vec<HypothesisRef>,
    confidence: HypothesisConfidence,
}

impl Hypothesis {
    /// Construct a hypothesis, rejecting empty or whitespace
    /// statements.
    pub fn try_new(
        id: HypothesisId,
        statement: HypothesisStatement,
        supporting_refs: Vec<HypothesisRef>,
        contradicting_refs: Vec<HypothesisRef>,
        confidence: HypothesisConfidence,
    ) -> Result<Self, HypothesisError> {
        if statement.summary().trim().is_empty() {
            return Err(HypothesisError::EmptyStatement);
        }
        // Disjointness check: supporting and contradicting must not
        // share a canonical id (a hypothesis cannot both support and
        // contradict the same canonical reference).
        for s in &supporting_refs {
            for c in &contradicting_refs {
                if s.label() == c.label() {
                    return Err(HypothesisError::ConflictingRefs(s.label()));
                }
            }
        }
        Ok(Self {
            id,
            statement,
            supporting_refs,
            contradicting_refs,
            confidence,
        })
    }

    /// The hypothesis id.
    pub fn id(&self) -> HypothesisId {
        self.id
    }

    /// The bounded statement.
    pub fn statement(&self) -> &HypothesisStatement {
        &self.statement
    }

    /// The supporting canonical references.
    pub fn supporting_refs(&self) -> &[HypothesisRef] {
        &self.supporting_refs
    }

    /// The contradicting canonical references.
    pub fn contradicting_refs(&self) -> &[HypothesisRef] {
        &self.contradicting_refs
    }

    /// The advisory confidence.
    pub fn confidence(&self) -> HypothesisConfidence {
        self.confidence
    }

    /// Whether this hypothesis carries any canonical references.
    pub fn has_canonical_refs(&self) -> bool {
        !self.supporting_refs.is_empty() || !self.contradicting_refs.is_empty()
    }
}

/// Why a hypothesis could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HypothesisError {
    /// The statement is empty or whitespace.
    EmptyStatement,
    /// A canonical reference appears in both supporting and
    /// contradicting sets.
    ConflictingRefs(String),
}

impl std::fmt::Display for HypothesisError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyStatement => f.write_str("hypothesis statement is empty"),
            Self::ConflictingRefs(s) => {
                write!(f, "hypothesis references {s} as both supporting and contradicting")
            }
        }
    }
}

impl std::error::Error for HypothesisError {}

// ============================================================================
// CritiqueDisposition
// ============================================================================

/// The disposition the critic assigns to a finding.
///
/// The critic advises; the policy gate decides. These dispositions
/// have **no effect** on the finding's gate status, severity, or
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CritiqueDisposition {
    /// The finding is well supported by canonical evidence.
    Supported,
    /// The finding is partially supported; some claims are not
    /// grounded.
    WeaklySupported,
    /// The finding is contradicted by canonical evidence.
    Contradicted,
    /// The finding references evidence that cannot be resolved.
    MissingEvidence,
    /// The finding needs a deeper AI investigation to assess.
    NeedsInvestigation,
}

impl CritiqueDisposition {
    /// The disposition name, stable for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::WeaklySupported => "weakly_supported",
            Self::Contradicted => "contradicted",
            Self::MissingEvidence => "missing_evidence",
            Self::NeedsInvestigation => "needs_investigation",
        }
    }
}

// ============================================================================
// Critique
// ============================================================================

/// A critic's disposition about an existing `Finding`.
///
/// The critic cannot make a finding more authoritative than it
/// already is: a `Critique` with disposition `Supported` does NOT
/// promote the finding; a `Critique` with disposition `Contradicted`
/// does NOT delete or downgrade the finding. The platform's policy
/// gate remains the gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Critique {
    /// The finding id under critique.
    pub finding_id: FindingId,
    /// The disposition.
    pub disposition: CritiqueDisposition,
    /// The bounded rationale (the critic's explanation).
    pub rationale: String,
    /// Optional canonical references that ground the disposition.
    pub grounding: Vec<HypothesisRef>,
}

impl Critique {
    /// Construct a critique, rejecting empty rationale.
    pub fn try_new(
        finding_id: FindingId,
        disposition: CritiqueDisposition,
        rationale: impl Into<String>,
        grounding: Vec<HypothesisRef>,
    ) -> Result<Self, CritiqueError> {
        let rationale = rationale.into();
        if rationale.trim().is_empty() {
            return Err(CritiqueError::EmptyRationale);
        }
        Ok(Self {
            finding_id,
            disposition,
            rationale,
            grounding,
        })
    }
}

/// Why a critique could not be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CritiqueError {
    /// The rationale is empty or whitespace.
    EmptyRationale,
}

impl std::fmt::Display for CritiqueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyRationale => f.write_str("critique rationale is empty"),
        }
    }
}

impl std::error::Error for CritiqueError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hypothesis_rejects_empty_statement() {
        let stmt = HypothesisStatement::Suggestion("   ".into());
        let h = Hypothesis::try_new(HypothesisId::new(1), stmt, vec![], vec![], HypothesisConfidence::default());
        assert!(matches!(h.unwrap_err(), HypothesisError::EmptyStatement));
    }

    #[test]
    fn hypothesis_rejects_overlapping_supporting_and_contradicting_refs() {
        let stmt = HypothesisStatement::Suggestion("claim".into());
        let f = crate::domain::kernel_ids::FactId::new(42);
        let supporting = vec![HypothesisRef::Fact(f)];
        let contradicting = vec![HypothesisRef::Fact(f)];
        let h = Hypothesis::try_new(HypothesisId::new(1), stmt, supporting, contradicting, HypothesisConfidence::default());
        assert!(matches!(h.unwrap_err(), HypothesisError::ConflictingRefs(_)));
    }

    #[test]
    fn a_well_formed_hypothesis_is_accepted() {
        let stmt = HypothesisStatement::Candidate {
            kind: "architecture_constraint".into(),
            summary: "domain must not depend on infrastructure".into(),
        };
        let h = Hypothesis::try_new(
            HypothesisId::new(1),
            stmt,
            vec![],
            vec![],
            HypothesisConfidence::new(75),
        )
        .unwrap();
        assert_eq!(h.id().get(), 1);
        assert_eq!(h.confidence().value(), 75);
        assert!(h.statement().is_candidate());
        assert!(!h.has_canonical_refs());
    }

    #[test]
    fn critique_rejects_empty_rationale() {
        let fid = FindingId::new("f-1").unwrap();
        let c = Critique::try_new(fid, CritiqueDisposition::Supported, "   ", vec![]);
        assert!(matches!(c.unwrap_err(), CritiqueError::EmptyRationale));
    }

    #[test]
    fn a_well_formed_critique_is_accepted() {
        let fid = FindingId::new("f-1").unwrap();
        let c = Critique::try_new(fid, CritiqueDisposition::WeaklySupported, "two of three claims are grounded", vec![])
            .unwrap();
        assert_eq!(c.disposition, CritiqueDisposition::WeaklySupported);
        assert_eq!(c.finding_id.as_str(), "f-1");
    }

    #[test]
    fn critique_disposition_names_are_stable() {
        assert_eq!(CritiqueDisposition::Supported.name(), "supported");
        assert_eq!(CritiqueDisposition::Contradicted.name(), "contradicted");
        assert_eq!(CritiqueDisposition::MissingEvidence.name(), "missing_evidence");
        assert_eq!(CritiqueDisposition::NeedsInvestigation.name(), "needs_investigation");
        assert_eq!(CritiqueDisposition::WeaklySupported.name(), "weakly_supported");
    }

    #[test]
    fn confidence_is_clamped_to_100() {
        let c = HypothesisConfidence::new(255);
        assert_eq!(c.value(), 100);
        let c = HypothesisConfidence::new(0);
        assert_eq!(c.value(), 0);
        let c = HypothesisConfidence::new(50);
        assert_eq!(c.value(), 50);
    }
}
