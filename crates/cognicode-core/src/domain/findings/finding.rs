//! Findings & evidence classes (M6.2).
//!
//! A [`Finding`] is an **evidence-backed conclusion** — never a bare
//! assertion. It carries the evidence that supports it, the detector
//! **as executed** (id, version, content digest and the authority it held
//! at run time), its origin, and a causal chain explaining *why* the
//! conclusion holds. Gates decide whether a finding may block; a
//! hypothesis-grade finding cannot satisfy a strong gate.
//!
//! Pure domain: no I/O, no `sqlx`, no `tokio`.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::detector_ir::{DetectorExecutionRef, FindingKind};
use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId};

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
    /// `satisfies(D)` is true for every class (D is the weakest bar);
    /// `satisfies(B)` is true only for `A`/`B`; a `D` hypothesis never
    /// satisfies a `B` requirement.
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
///
/// `FalsePositive`, `RiskAccepted` and `Suppressed` are **distinct
/// dispositions** and must not be collapsed: a false positive was never
/// real, a risk acceptance is a real finding we chose not to fix, and a
/// suppression is a policy exemption. Governance, historical replay and
/// false-positive measurement all depend on the difference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingStatus {
    /// Raised, not yet triaged.
    Open,
    /// Triaged and accepted as real.
    Accepted,
    /// Fixed by a change.
    Fixed,
    /// Determined never to have been real.
    FalsePositive,
    /// Real, but the risk is explicitly accepted.
    RiskAccepted,
    /// Hidden by an explicit suppression/exemption policy.
    Suppressed,
}

impl FindingStatus {
    /// Whether the finding still participates in gates.
    ///
    /// Only `Open` and `Accepted` are active: fixed, false-positive,
    /// risk-accepted and suppressed findings do not block.
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
            Self::RiskAccepted => "RiskAccepted",
            Self::Suppressed => "Suppressed",
        })
    }
}

// ============================================================================
// Origin
// ============================================================================

/// Where a finding came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum FindingOrigin {
    /// Produced by executing a detector.
    Detector,
    /// Projected from a legacy quality issue (preserves the legacy link).
    LegacyQuality {
        /// The legacy `QualityIssue.id`.
        issue_id: i64,
        /// The legacy rule id.
        rule_id: String,
    },
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

/// The role a causal step plays in a finding's explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CausalStepKind {
    /// Where tainted/interesting data enters.
    Source,
    /// A flow/traversal between subjects.
    Flow,
    /// A call edge.
    Call,
    /// A sanitizer that neutralises the flow.
    Sanitizer,
    /// A guard that constrains the flow.
    Guard,
    /// Where the effect materialises.
    Sink,
    /// A runtime observation.
    RuntimeObservation,
    /// A feasibility verification.
    Verification,
    /// A bare location (used by legacy projections).
    Location,
}

impl CausalStepKind {
    /// Stable label used for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Flow => "flow",
            Self::Call => "call",
            Self::Sanitizer => "sanitizer",
            Self::Guard => "guard",
            Self::Sink => "sink",
            Self::RuntimeObservation => "runtime_observation",
            Self::Verification => "verification",
            Self::Location => "location",
        }
    }
}

impl fmt::Display for CausalStepKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One **navigable** step of a finding's causal explanation.
///
/// Beyond a human-readable `detail`, a step can point at the kernel
/// entities/facts/evidence that ground it, so a consumer (Explorer) can
/// walk finding -> subject entity -> fact -> evidence rather than render
/// opaque strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalStep {
    /// The role of this step.
    pub kind: CausalStepKind,
    /// Subject entity this step concerns, if any.
    pub subject: Option<EntityId>,
    /// Fact backing this step, if any.
    pub fact: Option<FactId>,
    /// Evidence backing this step, if any.
    pub evidence: Option<EvidenceId>,
    /// Human-readable detail (must be non-empty).
    pub detail: String,
}

impl CausalStep {
    /// Construct a causal step with a non-empty detail.
    pub fn new(kind: CausalStepKind, detail: impl Into<String>) -> Result<Self, FindingError> {
        let detail = detail.into();
        if detail.trim().is_empty() {
            return Err(FindingError::EmptyCausalStep);
        }
        Ok(Self {
            kind,
            subject: None,
            fact: None,
            evidence: None,
            detail,
        })
    }

    /// Attach the subject entity this step concerns.
    pub fn with_subject(mut self, subject: EntityId) -> Self {
        self.subject = Some(subject);
        self
    }

    /// Attach the fact backing this step.
    pub fn with_fact(mut self, fact: FactId) -> Self {
        self.fact = Some(fact);
        self
    }

    /// Attach the evidence backing this step.
    pub fn with_evidence(mut self, evidence: EvidenceId) -> Self {
        self.evidence = Some(evidence);
        self
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
    /// Where it came from.
    pub origin: FindingOrigin,
    /// Display severity.
    pub severity: FindingSeverity,
    /// Evaluated risk.
    pub risk: RiskLevel,
    /// Strength of the supporting evidence.
    pub evidence_class: EvidenceClass,
    /// Supporting evidence handles (kernel evidence ids).
    pub evidence: Vec<EvidenceId>,
    /// Detector as executed (id, version, digest, authority at run time).
    pub detector: DetectorExecutionRef,
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
            return Err(FindingError::EmptyDetectorVersion);
        }
        Ok(())
    }

    /// Whether the finding is fully explainable.
    ///
    /// Requires: at least one evidence handle, a non-empty causal chain with
    /// no empty steps, and a detector execution reference carrying a version
    /// and a content digest.
    pub fn is_explainable(&self) -> bool {
        !self.evidence.is_empty()
            && !self.causal_chain.is_empty()
            && !self.detector.version.trim().is_empty()
            && !self.detector.digests.semantic.as_str().is_empty()
            && self
                .causal_chain
                .iter()
                .all(|step| !step.detail.trim().is_empty())
    }

    /// Whether this finding may block the given gate.
    ///
    /// Requires: valid, active, fully explainable, admitted by the gate
    /// (risk + evidence class), **and** the detector that produced it held
    /// blocking authority *at execution time*.
    pub fn can_block(&self, gate: &FindingGate) -> bool {
        self.validate().is_ok()
            && self.status.is_active()
            && self.is_explainable()
            && self.detector.can_block()
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
    /// (ignoring status, explainability and detector authority).
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
    /// The detector execution reference has no version.
    EmptyDetectorVersion,
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
            Self::EmptyDetectorVersion => {
                f.write_str("detector execution reference must carry a non-empty version")
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
    use crate::domain::findings::detector_ir::{DetectorAuthority, DetectorId};

    fn detector_at(authority: DetectorAuthority) -> DetectorExecutionRef {
        let d = super::super::digest::DetectorDigest::from_content("detector-content");
        DetectorExecutionRef::new(
            DetectorId::new("security.sql_injection").unwrap(),
            "1.0.0",
            authority,
            super::super::detector_ir::DetectorDigests {
                logic: d.clone(),
                policy: d.clone(),
                semantic: d.clone(),
                instance: d,
            },
            None,
        )
        .unwrap()
    }

    fn finding(
        class: EvidenceClass,
        risk: RiskLevel,
        status: FindingStatus,
        authority: DetectorAuthority,
    ) -> Finding {
        Finding {
            id: FindingId::new("f-1").unwrap(),
            kind: FindingKind::new("security.sql_injection").unwrap(),
            origin: FindingOrigin::Detector,
            severity: FindingSeverity::Critical,
            risk,
            evidence_class: class,
            evidence: vec![EvidenceId::new(1), EvidenceId::new(2)],
            detector: detector_at(authority),
            status,
            message: "tainted input reaches SQL execution".to_string(),
            causal_chain: vec![
                CausalStep::new(CausalStepKind::Source, "request.query").unwrap(),
                CausalStep::new(CausalStepKind::Sink, "db.execute").unwrap(),
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
        assert!(!FindingStatus::RiskAccepted.is_active());
        assert!(!FindingStatus::Suppressed.is_active());
    }

    #[test]
    fn explainable_requires_evidence_and_causal_chain() {
        let f = finding(
            EvidenceClass::A,
            RiskLevel::High,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
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
        let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Medium);

        let hypothesis = finding(
            EvidenceClass::D,
            RiskLevel::High,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
        assert!(
            !hypothesis.can_block(&gate),
            "grade D must not clear a grade-B gate"
        );
        assert!(!gate.admits(&hypothesis));

        let verified = finding(
            EvidenceClass::A,
            RiskLevel::High,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
        assert!(verified.can_block(&gate), "grade A clears a grade-B gate");
    }

    #[test]
    fn candidate_detector_never_blocks_even_when_promoted_later() {
        // P0: authority is captured at execution time.
        let gate = FindingGate::new(EvidenceClass::A, RiskLevel::Low);
        let candidate_run = finding(
            EvidenceClass::A,
            RiskLevel::Critical,
            FindingStatus::Open,
            DetectorAuthority::Candidate,
        );
        assert!(
            !candidate_run.can_block(&gate),
            "a Candidate detector's finding must never block, regardless of evidence class"
        );
        assert!(!candidate_run.detector.can_block());

        let gated_run = finding(
            EvidenceClass::A,
            RiskLevel::Critical,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
        assert!(gated_run.can_block(&gate));
    }

    #[test]
    fn blocking_finding_is_explainable() {
        let gate = FindingGate::new(EvidenceClass::B, RiskLevel::Medium);
        let f = finding(
            EvidenceClass::A,
            RiskLevel::Critical,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
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

        for status in [
            FindingStatus::Fixed,
            FindingStatus::FalsePositive,
            FindingStatus::RiskAccepted,
            FindingStatus::Suppressed,
        ] {
            let f = finding(
                EvidenceClass::A,
                RiskLevel::High,
                status,
                DetectorAuthority::Gated,
            );
            assert!(!f.can_block(&gate), "{status} must not block");
        }

        let mut invalid = finding(
            EvidenceClass::A,
            RiskLevel::High,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
        invalid.message = "  ".to_string();
        assert!(!invalid.can_block(&gate));
        assert_eq!(invalid.validate().unwrap_err(), FindingError::EmptyMessage);
    }

    #[test]
    fn gate_rejects_low_risk() {
        let gate = FindingGate::new(EvidenceClass::C, RiskLevel::High);
        let low_risk = finding(
            EvidenceClass::A,
            RiskLevel::Low,
            FindingStatus::Open,
            DetectorAuthority::Gated,
        );
        assert!(!gate.admits(&low_risk));
    }

    #[test]
    fn constructors_reject_empty_parts() {
        assert_eq!(FindingId::new("").unwrap_err(), FindingError::EmptyId);
        assert_eq!(
            CausalStep::new(CausalStepKind::Source, " ").unwrap_err(),
            FindingError::EmptyCausalStep
        );
    }

    #[test]
    fn origin_preserves_legacy_link() {
        let origin = FindingOrigin::LegacyQuality {
            issue_id: 7,
            rule_id: "rule-1".to_string(),
        };
        let mut f = finding(
            EvidenceClass::C,
            RiskLevel::Low,
            FindingStatus::Open,
            DetectorAuthority::Candidate,
        );
        f.origin = origin.clone();
        assert_eq!(f.origin, origin);
    }

    #[test]
    fn causal_step_is_navigable_and_round_trips() {
        let step = CausalStep::new(CausalStepKind::Sink, "db.execute")
            .unwrap()
            .with_subject(EntityId::new(42))
            .with_fact(FactId::new(7))
            .with_evidence(EvidenceId::new(3));

        assert_eq!(step.kind, CausalStepKind::Sink);
        assert_eq!(step.kind.name(), "sink");
        assert_eq!(step.subject, Some(EntityId::new(42)));
        assert_eq!(step.fact, Some(FactId::new(7)));
        assert_eq!(step.evidence, Some(EvidenceId::new(3)));

        let json = serde_json::to_string(&step).unwrap();
        let parsed: CausalStep = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, step);

        // A bare step has no navigation targets but is still valid.
        let bare = CausalStep::new(CausalStepKind::Source, "request.query").unwrap();
        assert_eq!(bare.subject, None);
        assert_eq!(bare.fact, None);
        assert_eq!(bare.evidence, None);
    }

    #[test]
    fn finding_round_trip() {
        let f = finding(
            EvidenceClass::B,
            RiskLevel::High,
            FindingStatus::Accepted,
            DetectorAuthority::Gated,
        );
        let json = serde_json::to_string(&f).expect("serialize");
        let parsed: Finding = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, f);
    }
}
