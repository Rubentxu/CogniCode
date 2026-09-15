//! Findings & evidence classes (M6.2).
//!
//! A [`Finding`] is an **evidence-backed conclusion** — never a bare
//! assertion. It carries the evidence that supports it, the detector and
//! version that produced it, and a causal chain explaining *why* the
//! conclusion holds. Gates decide whether a finding may block; a
//! hypothesis-grade finding cannot satisfy a strong gate.
//!
//! Pure domain: no I/O, no `sqlx`, no `tokio`.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::detector_ir::{DetectorId, FindingKind};

// ============================================================================
// Evidence class
// ============================================================================

/// Strength class of the evidence backing a finding.
///
/// `A` is the strongest (reproduced/verified), `D` the weakest (hypothesis
/// only). Declared order is strength order, so `A < B < C < D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceClass {
    /// Reproduced / mechanically verified.
    A,
    /// Strong static evidence (e.g. full dataflow path).
    B,
    /// Partial static evidence.
    C,
    /// Hypothesis only (heuristic / LLM-suggested).
    D,
}

impl EvidenceClass {
    /// Whether this class is at least as strong as `minimum`.
    ///
    /// `satisfies(A)` is true for every class; `satisfies(B)` only for
    /// `A`/`B`; a `D` hypothesis never satisfies a `B` requirement.
    pub fn satisfies(self, minimum: EvidenceClass) -> bool {
        self <= minimum
    }

    /// Stable single-letter name.
    pub fn name(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
        }
    }
}

impl fmt::Display for EvidenceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ============================================================================
// Severity and risk
// ============================================================================

/// Display severity hint (`Critical > Warning > Info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Critical.
    Critical,
}

impl FindingSeverity {
    /// Ascending rank (`Info = 0` … `Critical = 2`).
    pub fn rank(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for FindingSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Critical => "Critical",
        })
    }
}

/// Evaluated risk of acting on the finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Critical.
    Critical,
}

impl RiskLevel {
    /// Ascending rank (`Low = 0` … `Critical = 3`).
    pub fn rank(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        })
    }
}

// ============================================================================
// Status
// ============================================================================

/// Lifecycle status of a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    /// Raised, not yet triaged.
    Open,
    /// Triaged and accepted as real.
    Accepted,
    /// Fixed by a change.
    Fixed,
    /// Determined to be a false positive.
    FalsePositive,
}

impl FindingStatus {
    /// Whether the finding still participates in gates.
    pub fn is_active(self) -> bool {
        matches!(self, Self::Open | Self::Accepted)
    }
}

impl fmt::Display for FindingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Open => "Open",
            Self::Accepted => "Accepted",
            Self::Fixed => "Fixed",
            Self::FalsePositive => "FalsePositive",
        })
    }
}

// ============================================================================
// Identity + references
// ============================================================================

/// Stable finding identifier (non-empty).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FindingId(String);

impl FindingId {
    /// Construct a non-empty finding id.
    pub fn new(value: impl Into<String>) -> Result<Self, FindingError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(FindingError::EmptyId);
        }
        Ok(Self(value))
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FindingId {
    type Error = FindingError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FindingId> for String {
    fn from(value: FindingId) -> Self {
        value.0
    }
}

impl fmt::Display for FindingId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Opaque handle to one piece of supporting evidence.
///
/// DELIBERATE-DEFERRAL (M6): the kernel `EvidenceId` lives behind the
/// `evidence-kernel` Cargo feature (off by default). Findings are ungated,
/// so they reference evidence through this small opaque id; the wiring
/// cycle that connects findings to the kernel store will map
/// `EvidenceId → EvidenceRef` in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvidenceRef(pub u64);

/// The detector (and version) that produced a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorRef {
    /// Detector identity.
    pub id: DetectorId,
    /// Detector version at production time.
    pub version: String,
}

impl DetectorRef {
    /// Construct a detector reference with a non-empty version.
    pub fn new(id: DetectorId, version: impl Into<String>) -> Result<Self, FindingError> {
        let version = version.into();
        if version.trim().is_empty() {
            return Err(FindingError::MissingDetectorVersion);
        }
        Ok(Self { id, version })
    }
}

/// One step of a finding's causal explanation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalStep {
    /// Short label (e.g. `source`, `flow`, `sink`).
    pub label: String,
    /// Human-readable detail.
    pub detail: String,
}

impl CausalStep {
    /// Construct a causal step with non-empty label and detail.
    pub fn new(label: impl Into<String>, detail: impl Into<String>) -> Result<Self, FindingError> {
        let label = label.into();
        let detail = detail.into();
        if label.trim().is_empty() {
            return Err(FindingError::EmptyCausalStep);
        }
        if detail.trim().is_empty() {
            return Err(FindingError::EmptyCausalStep);
        }
        Ok(Self { label, detail })
    }
}

// ============================================================================
// Finding
// ============================================================================

/// An evidence-backed conclusion produced by a detector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// Stable id.
    pub id: FindingId,
    /// What was found (namespaced kind).
    pub kind: FindingKind,
    /// Display severity.
    pub severity: FindingSeverity,
    /// Evaluated risk.
    pub risk: RiskLevel,
    /// Strength of the supporting evidence.
    pub evidence_class: EvidenceClass,
    /// Supporting evidence handles.
    pub evidence: Vec<EvidenceRef>,
    /// Detector + version that produced it.
    pub detector: DetectorRef,
    /// Lifecycle status.
    pub status: FindingStatus,
    /// Human-readable message.
    pub message: String,
    /// Causal explanation (source/sink/path, in order).
    pub causal_chain: Vec<CausalStep>,
}

impl Finding {
    /// Validate structural invariants.
    pub fn validate(&self) -> Result<(), FindingError> {
        if self.id.as_str().trim().is_empty() {
            return Err(FindingError::EmptyId);
        }
        if self.message.trim().is_empty() {
            return Err(FindingError::EmptyMessage);
        }
        if self.detector.version.trim().is_empty() {
            return Err(FindingError::MissingDetectorVersion);
        }
        Ok(())
    }

    /// Whether the finding is fully explainable.
    ///
    /// A finding is explainable when it links at least one piece of
    /// evidence, carries a non-empty causal chain, and names the detector
    /// version that produced it (umbrella REQ "Blocking finding is
    /// explainable").
    pub fn is_explainable(&self) -> bool {
        !self.evidence.is_empty()
            && !self.causal_chain.is_empty()
            && !self.detector.version.trim().is_empty()
            && self
                .causal_chain
                .iter()
                .all(|step| !step.label.trim().is_empty() && !step.detail.trim().is_empty())
    }

    /// Whether this finding may block the given gate.
    ///
    /// Requires: the finding is [`validate`](Self::validate)d, active,
    /// fully explainable, and admitted by the gate (risk at least
    /// `min_risk` **and** evidence class at least as strong as
    /// `min_evidence_class`).
    pub fn can_block(&self, gate: &FindingGate) -> bool {
        self.validate().is_ok()
            && self.status.is_active()
            && self.is_explainable()
            && gate.admits(self)
    }
}

// ============================================================================
// Gate
// ============================================================================

/// A policy gate a finding may be required to satisfy to block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingGate {
    /// Minimum evidence class accepted.
    pub min_evidence_class: EvidenceClass,
    /// Minimum risk the finding must carry to be relevant.
    pub min_risk: RiskLevel,
}

impl FindingGate {
    /// Construct a gate.
    pub fn new(min_evidence_class: EvidenceClass, min_risk: RiskLevel) -> Self {
        Self {
            min_evidence_class,
            min_risk,
        }
    }

    /// Whether the finding clears this gate's class and risk thresholds
    /// (ignoring status and explainability).
    pub fn admits(&self, finding: &Finding) -> bool {
        finding.risk >= self.min_risk && finding.evidence_class.satisfies(self.min_evidence_class)
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Structural failure when building or validating a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FindingError {
    /// The finding id is empty.
    EmptyId,
    /// The message is empty.
    EmptyMessage,
    /// The detector reference has no version.
    MissingDetectorVersion,
    /// A causal step has an empty label or detail.
    EmptyCausalStep,
    /// A referenced detector identifier or finding kind is malformed.
    InvalidIdentifier(super::DetectorIrError),
}

impl From<super::DetectorIrError> for FindingError {
    fn from(value: super::DetectorIrError) -> Self {
        Self::InvalidIdentifier(value)
    }
}

impl fmt::Display for FindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("finding id must not be empty"),
            Self::EmptyMessage => f.write_str("finding message must not be empty"),
            Self::MissingDetectorVersion => {
                f.write_str("detector reference must carry a non-empty version")
            }
            Self::EmptyCausalStep => f.write_str("causal step label and detail must not be empty"),
            Self::InvalidIdentifier(err) => write!(f, "invalid finding identifier: {err}"),
        }
    }
}

impl std::error::Error for FindingError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn detector_ref() -> DetectorRef {
        DetectorRef::new(DetectorId::new("det.sql_injection").unwrap(), "1.0.0").unwrap()
    }

    fn finding(class: EvidenceClass, risk: RiskLevel, status: FindingStatus) -> Finding {
        Finding {
            id: FindingId::new("f-1").unwrap(),
            kind: FindingKind::new("security.sql_injection").unwrap(),
            severity: FindingSeverity::Critical,
            risk,
            evidence_class: class,
            evidence: vec![EvidenceRef(1), EvidenceRef(2)],
            detector: detector_ref(),
            status,
            message: "tainted input reaches SQL execution".to_string(),
            causal_chain: vec![
                CausalStep::new("source", "request.query").unwrap(),
                CausalStep::new("sink", "db.execute").unwrap(),
            ],
        }
    }

    #[test]
    fn evidence_class_ordering_and_satisfies() {
        assert!(EvidenceClass::A < EvidenceClass::B);
        assert!(EvidenceClass::B < EvidenceClass::C);
        assert!(EvidenceClass::C < EvidenceClass::D);

        for class in [
            EvidenceClass::A,
            EvidenceClass::B,
            EvidenceClass::C,
            EvidenceClass::D,
        ] {
            assert!(class.satisfies(EvidenceClass::D), "{class} satisfies D");
            assert!(class.satisfies(class), "{class} satisfies itself");
        }
        assert!(EvidenceClass::A.satisfies(EvidenceClass::B));
        assert!(!EvidenceClass::C.satisfies(EvidenceClass::B));
        assert!(!EvidenceClass::D.satisfies(EvidenceClass::B));
    }

    #[test]
    fn evidence_class_display() {
        assert_eq!(EvidenceClass::A.to_string(), "A");
        assert_eq!(EvidenceClass::D.to_string(), "D");
    }

    #[test]
    fn severity_and_risk_ranks() {
        assert!(FindingSeverity::Info.rank() < FindingSeverity::Warning.rank());
        assert!(FindingSeverity::Warning.rank() < FindingSeverity::Critical.rank());
        assert!(RiskLevel::Low.rank() < RiskLevel::Medium.rank());
        assert!(RiskLevel::Medium.rank() < RiskLevel::High.rank());
        assert!(RiskLevel::High.rank() < RiskLevel::Critical.rank());
    }

    #[test]
    fn status_active_only_for_open_and_accepted() {
        assert!(FindingStatus::Open.is_active());
        assert!(FindingStatus::Accepted.is_active());
        assert!(!FindingStatus::Fixed.is_active());
        assert!(!FindingStatus::FalsePositive.is_active());
    }

    #[test]
    fn explainable_requires_evidence_and_causal_chain() {
        let f = finding(EvidenceClass::A, RiskLevel::High, FindingStatus::Open);
        assert!(f.is_explainable());

        let mut no_evidence = f.clone();
        no_evidence.evidence.clear();
        assert!(!no_evidence.is_explainable());

        let mut no_chain = f.clone();
        no_chain.causal_chain.clear();
        assert!(!no_chain.is_explainable());
    }

    #[test]
    fn hypothesis_cannot_satisfy_strong_gate() {
        // Umbrella REQ "Hypothesis cannot satisfy strong gate".
        let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Medium);

        let hypothesis = finding(EvidenceClass::D, RiskLevel::High, FindingStatus::Open);
        assert!(
            !hypothesis.can_block(&gate),
            "grade D must not clear a grade-B gate"
        );
        assert!(!gate.admits(&hypothesis));

        let verified = finding(EvidenceClass::A, RiskLevel::High, FindingStatus::Open);
        assert!(verified.can_block(&gate), "grade A clears a grade-B gate");
    }

    #[test]
    fn blocking_finding_is_explainable() {
        // Umbrella REQ "Blocking finding is explainable".
        let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Medium);
        let f = finding(EvidenceClass::A, RiskLevel::Critical, FindingStatus::Open);
        assert!(f.can_block(&gate));
        assert!(!f.evidence.is_empty(), "blocking finding links evidence");
        assert!(
            !f.causal_chain.is_empty(),
            "blocking finding has causal lineage"
        );
    }

    #[test]
    fn inactive_or_unvalidated_findings_cannot_block() {
        let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Low);

        let fixed = finding(EvidenceClass::A, RiskLevel::High, FindingStatus::Fixed);
        assert!(!fixed.can_block(&gate));

        let mut invalid = finding(EvidenceClass::A, RiskLevel::High, FindingStatus::Open);
        invalid.message = "  ".to_string();
        assert!(!invalid.can_block(&gate));
        assert_eq!(invalid.validate().unwrap_err(), FindingError::EmptyMessage);
    }

    #[test]
    fn gate_rejects_low_risk() {
        let gate = FindingGate::new(EvidenceClass::C, RiskLevel::High);
        let low_risk = finding(EvidenceClass::A, RiskLevel::Low, FindingStatus::Open);
        assert!(!gate.admits(&low_risk));
    }

    #[test]
    fn constructors_reject_empty_parts() {
        assert_eq!(FindingId::new("").unwrap_err(), FindingError::EmptyId);
        assert_eq!(
            DetectorRef::new(DetectorId::new("d").unwrap(), "").unwrap_err(),
            FindingError::MissingDetectorVersion
        );
        assert_eq!(
            CausalStep::new("", "x").unwrap_err(),
            FindingError::EmptyCausalStep
        );
        assert_eq!(
            CausalStep::new("x", " ").unwrap_err(),
            FindingError::EmptyCausalStep
        );
    }

    #[test]
    fn finding_round_trip() {
        let f = finding(EvidenceClass::B, RiskLevel::High, FindingStatus::Accepted);
        let json = serde_json::to_string(&f).expect("serialize");
        let parsed: Finding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, f);
    }
}
