//! Detector IR (M6.1) — a declarative, validated detector definition.
//!
//! A detector is expressed as a small ordered program of steps
//! (`MATCH` / `FLOW` / `EXCLUDE` / `VERIFY` / `PRODUCE`) plus the set of
//! analysis **capabilities** it requires. The IR is validated **before
//! admission**: an unsupported construct, a malformed identifier, or a
//! missing capability fails loud with a structured [`DetectorIrError`].
//!
//! ## Three distinct concepts (M6 contract hardening)
//!
//! - [`AnalysisCapability`] — *what* the detector needs (a set, not a
//!   linear level: an LLM is not a super-set of symbolic execution).
//! - [`EscalationTier`] — *how expensive* satisfying that capability is
//!   (planning/cost only).
//! - [`DetectorAuthority`] — *whether* the detector may block CI.
//!
//! Authority travels into the produced finding as
//! [`DetectorExecutionRef::authority_at_execution`], so a detector that was
//! a `Candidate` at run time can never contribute a blocking finding —
//! even if it is promoted to `Gated` later.
//!
//! Pure domain: no I/O, no `sqlx`, no `tokio`.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

use super::namespaced::NamespacedName;
use crate::domain::kernel_ids::ExecutionId;

// ============================================================================
// Analysis capability (what the detector needs)
// ============================================================================

/// A capability an analysis backend can provide.
///
/// Declared order is cost order (`AstPattern` cheapest … `LlmReasoning`
/// most expensive) but the type is **set-based**, not a linear ladder:
/// `LlmReasoning` does **not** satisfy `SymbolicFeasibility`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisCapability {
    /// Syntactic pattern matching over an AST.
    AstPattern,
    /// Semantic resolution (symbols, types).
    SemanticResolution,
    /// Graph queries over call/flow projections.
    GraphQuery,
    /// Data-flow analysis (def-use, reaching definitions).
    Dataflow,
    /// Selective abstract interpretation.
    AbstractInterpretation,
    /// Symbolic execution / SMT feasibility checks.
    SymbolicFeasibility,
    /// Runtime trace corroboration.
    RuntimeEvidence,
    /// LLM context reasoning (least authoritative).
    LlmReasoning,
}

impl AnalysisCapability {
    /// Stable UPPER_SNAKE name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::AstPattern => "AST_PATTERN",
            Self::SemanticResolution => "SEMANTIC_RESOLUTION",
            Self::GraphQuery => "GRAPH_QUERY",
            Self::Dataflow => "DATAFLOW",
            Self::AbstractInterpretation => "ABSTRACT_INTERPRETATION",
            Self::SymbolicFeasibility => "SYMBOLIC_FEASIBILITY",
            Self::RuntimeEvidence => "RUNTIME_EVIDENCE",
            Self::LlmReasoning => "LLM_REASONING",
        }
    }

    /// The escalation tier that provides this capability (cost only).
    pub fn tier(self) -> EscalationTier {
        match self {
            Self::AstPattern => EscalationTier::T0Ast,
            Self::SemanticResolution => EscalationTier::T1Semantic,
            Self::GraphQuery => EscalationTier::T2Graph,
            Self::Dataflow => EscalationTier::T3Dataflow,
            Self::AbstractInterpretation | Self::SymbolicFeasibility => EscalationTier::T4Formal,
            Self::RuntimeEvidence | Self::LlmReasoning => EscalationTier::T5Contextual,
        }
    }
}

impl fmt::Display for AnalysisCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Cost band for planning, derived from a capability set. Not authority,
/// not correctness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTier {
    /// AST / syntactic.
    T0Ast,
    /// Semantic resolution.
    T1Semantic,
    /// Graph queries.
    T2Graph,
    /// Data flow.
    T3Dataflow,
    /// Formal (abstract interpretation, symbolic).
    T4Formal,
    /// Contextual (runtime evidence, LLM).
    T5Contextual,
}

impl EscalationTier {
    /// Ascending cost rank (`T0Ast = 0` … `T5Contextual = 5`).
    pub fn rank(self) -> u8 {
        self as u8
    }
}

impl fmt::Display for EscalationTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::T0Ast => "T0_AST",
            Self::T1Semantic => "T1_SEMANTIC",
            Self::T2Graph => "T2_GRAPH",
            Self::T3Dataflow => "T3_DATAFLOW",
            Self::T4Formal => "T4_FORMAL",
            Self::T5Contextual => "T5_CONTEXTUAL",
        })
    }
}

// ============================================================================
// Namespaced identifiers
// ============================================================================

/// A namespaced subject a detector matches or flows over
/// (e.g. `security.user_input`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubjectPattern(NamespacedName);

impl SubjectPattern {
    /// Construct a validated subject pattern.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        NamespacedName::new(value)
            .map(Self)
            .map_err(|err| DetectorIrError::InvalidIdentifier {
                value: format!("{err}"),
            })
    }

    /// Borrow the raw pattern.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for SubjectPattern {
    type Error = DetectorIrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SubjectPattern> for String {
    fn from(value: SubjectPattern) -> Self {
        value.0.into()
    }
}

impl fmt::Display for SubjectPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A namespaced finding kind a detector produces
/// (e.g. `security.sql_injection`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FindingKind(NamespacedName);

impl FindingKind {
    /// Construct a validated finding kind.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        NamespacedName::new(value)
            .map(Self)
            .map_err(|err| DetectorIrError::InvalidIdentifier {
                value: format!("{err}"),
            })
    }

    /// Borrow the raw kind.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for FindingKind {
    type Error = DetectorIrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<FindingKind> for String {
    fn from(value: FindingKind) -> Self {
        value.0.into()
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Stable detector identifier (namespaced, e.g. `security.sql_injection`).
///
/// Namespacing is required so packs and federated detectors cannot collide.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DetectorId(NamespacedName);

impl DetectorId {
    /// Construct a validated, namespaced detector id.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        NamespacedName::new(value)
            .map(Self)
            .map_err(|err| DetectorIrError::InvalidIdentifier {
                value: format!("{err}"),
            })
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl TryFrom<String> for DetectorId {
    type Error = DetectorIrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DetectorId> for String {
    fn from(value: DetectorId) -> Self {
        value.0.into()
    }
}

impl fmt::Display for DetectorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ============================================================================
// IR nodes
// ============================================================================

/// One step of a detector program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum DetectorStep {
    /// Declare a subject the detector observes.
    Match { subject: SubjectPattern },
    /// Require a flow from `source` to `sink` (optionally bounded).
    Flow {
        source: SubjectPattern,
        sink: SubjectPattern,
        max_hops: Option<u32>,
    },
    /// Exclude paths whose subject matches `path_contains`.
    Exclude { path_contains: SubjectPattern },
    /// Require feasibility of the declared flow.
    Verify { feasible_path: bool },
    /// Produce a finding of the given kind.
    Produce { kind: FindingKind },
}

impl DetectorStep {
    /// Short stable name used in diagnostics.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Match { .. } => "MATCH",
            Self::Flow { .. } => "FLOW",
            Self::Exclude { .. } => "EXCLUDE",
            Self::Verify { .. } => "VERIFY",
            Self::Produce { .. } => "PRODUCE",
        }
    }

    /// Capabilities this step cannot run without (its floor).
    pub fn required_capabilities(&self) -> BTreeSet<AnalysisCapability> {
        let mut set = BTreeSet::new();
        match self {
            Self::Match { .. } => {}
            Self::Flow { .. } | Self::Exclude { .. } => {
                set.insert(AnalysisCapability::GraphQuery);
            }
            Self::Verify { feasible_path } => {
                if *feasible_path {
                    set.insert(AnalysisCapability::SymbolicFeasibility);
                }
            }
            Self::Produce { .. } => {}
        }
        set
    }
}

/// Whether a detector may block CI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetectorAuthority {
    /// Experiment/shadow only — cannot block.
    Candidate,
    /// Human-gated — may block CI.
    Gated,
}

impl DetectorAuthority {
    /// Single gate predicate consumers should use.
    pub fn can_block(self) -> bool {
        matches!(self, Self::Gated)
    }
}

impl fmt::Display for DetectorAuthority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Candidate => "Candidate",
            Self::Gated => "Gated",
        })
    }
}

// ============================================================================
// DetectorIr
// ============================================================================

/// A validated detector definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorIr {
    /// Stable, namespaced detector id.
    pub id: DetectorId,
    /// Human-readable name.
    pub name: String,
    /// Analysis capabilities the detector requires.
    pub requires: BTreeSet<AnalysisCapability>,
    /// GATE authority (candidate vs gated).
    pub authority: DetectorAuthority,
    /// Ordered program steps.
    pub steps: Vec<DetectorStep>,
}

impl DetectorIr {
    /// Validate the detector before admission.
    ///
    /// Fails loud with a structured [`DetectorIrError`] when the definition
    /// is malformed or omits a capability its steps cannot run without.
    pub fn validate(&self) -> Result<(), DetectorIrError> {
        // V1/V2 — identity
        if self.name.trim().is_empty() {
            return Err(DetectorIrError::EmptyName);
        }

        // V3/V4 — exactly one PRODUCE, and it must be last
        let produce_indices: Vec<usize> = self
            .steps
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s, DetectorStep::Produce { .. }))
            .map(|(i, _)| i)
            .collect();
        match produce_indices.as_slice() {
            [] => return Err(DetectorIrError::NoProduce),
            [first, rest @ ..] => {
                if let Some(&second) = rest.first() {
                    return Err(DetectorIrError::DuplicateProduce { index: second });
                }
                if *first != self.steps.len() - 1 {
                    return Err(DetectorIrError::ProduceNotLast { index: *first });
                }
            }
        }

        // V5/V6/V8 — ordering
        let mut seen_match = false;
        let mut seen_flow = false;
        for (index, step) in self.steps.iter().enumerate() {
            match step {
                DetectorStep::Match { .. } => {
                    if seen_flow {
                        return Err(DetectorIrError::MatchAfterFlow { index });
                    }
                    seen_match = true;
                }
                DetectorStep::Flow { .. } => {
                    if !seen_match {
                        return Err(DetectorIrError::FlowWithoutMatch { index });
                    }
                    seen_flow = true;
                }
                DetectorStep::Exclude { .. } => {
                    if !seen_flow {
                        return Err(DetectorIrError::ExcludeBeforeFlow { index });
                    }
                }
                DetectorStep::Verify { .. } => {
                    if !seen_flow {
                        return Err(DetectorIrError::VerifyBeforeFlow { index });
                    }
                }
                DetectorStep::Produce { .. } => {}
            }
        }

        // V7 — declared capabilities must cover every step's floor
        for (index, step) in self.steps.iter().enumerate() {
            for capability in step.required_capabilities() {
                if !self.requires.contains(&capability) {
                    return Err(DetectorIrError::MissingCapability { capability, index });
                }
            }
        }

        Ok(())
    }

    /// Cost tier implied by the declared capability set (`T0Ast` if empty).
    pub fn escalation_tier(&self) -> EscalationTier {
        self.requires
            .iter()
            .map(|capability| capability.tier())
            .max()
            .unwrap_or(EscalationTier::T0Ast)
    }

    /// The capabilities the steps cannot run without (the floor).
    pub fn required_capabilities(&self) -> BTreeSet<AnalysisCapability> {
        self.steps
            .iter()
            .flat_map(|step| step.required_capabilities())
            .collect()
    }

    /// Stable content digest of this definition.
    ///
    /// Captured in [`DetectorExecutionRef`] so a finding can be tied to the
    /// exact detector *content* that produced it (not just its id/version).
    pub fn digest(&self) -> DetectorDigest {
        let canonical = serde_json::to_string(self).unwrap_or_else(|_| format!("{self:?}"));
        DetectorDigest::from_content(&canonical)
    }

    /// Whether this detector may block CI (authority AND valid definition).
    pub fn can_block(&self) -> bool {
        self.authority.can_block() && self.validate().is_ok()
    }
}

// ============================================================================
// Execution reference (authority captured at run time)
// ============================================================================

/// Stable content digest of a detector definition (`fnv1a64:<hex>`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DetectorDigest(String);

impl DetectorDigest {
    /// Construct from an already-formatted digest string.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        let value = value.into();
        if !value.starts_with("fnv1a64:") || value.len() <= "fnv1a64:".len() {
            return Err(DetectorIrError::InvalidDigest { value });
        }
        Ok(Self(value))
    }

    /// Compute the canonical digest of arbitrary content.
    pub fn from_content(content: &str) -> Self {
        Self(fnv1a64(content))
    }

    /// Borrow the raw digest.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DetectorDigest {
    type Error = DetectorIrError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<DetectorDigest> for String {
    fn from(value: DetectorDigest) -> Self {
        value.0
    }
}

impl fmt::Display for DetectorDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// FNV-1a 64-bit digest, formatted `fnv1a64:<16 hex>` (house convention).
fn fnv1a64(data: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// The detector **as executed**: id, version, the authority it held *at
/// execution time*, its content digest, and an optional execution id.
///
/// Capturing authority here is what makes the AI-detector rule safe: a
/// `Candidate` run never yields a blocking finding, even if the detector is
/// promoted to `Gated` afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorExecutionRef {
    /// Detector identity.
    pub id: DetectorId,
    /// Detector version at production time.
    pub version: String,
    /// Authority the detector held when it produced the finding.
    pub authority_at_execution: DetectorAuthority,
    /// Content digest of the executed definition.
    pub detector_digest: DetectorDigest,
    /// Optional link to a concrete execution record.
    pub execution_id: Option<ExecutionId>,
}

impl DetectorExecutionRef {
    /// Construct an execution reference explicitly.
    pub fn new(
        id: DetectorId,
        version: impl Into<String>,
        authority_at_execution: DetectorAuthority,
        detector_digest: DetectorDigest,
        execution_id: Option<ExecutionId>,
    ) -> Result<Self, DetectorIrError> {
        let version = version.into();
        if version.trim().is_empty() {
            return Err(DetectorIrError::EmptyVersion);
        }
        Ok(Self {
            id,
            version,
            authority_at_execution,
            detector_digest,
            execution_id,
        })
    }

    /// Capture an execution reference **from** a detector definition.
    ///
    /// This is the constructor backends should use: it records the
    /// definition's id, its content digest and — crucially — the authority
    /// the definition holds now.
    pub fn from_definition(
        definition: &DetectorIr,
        version: impl Into<String>,
        execution_id: Option<ExecutionId>,
    ) -> Result<Self, DetectorIrError> {
        Self::new(
            definition.id.clone(),
            version,
            definition.authority,
            definition.digest(),
            execution_id,
        )
    }

    /// Whether the executed detector held blocking authority.
    pub fn can_block(&self) -> bool {
        self.authority_at_execution.can_block()
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Structured validation failure for a detector definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorIrError {
    /// The detector name is empty.
    EmptyName,
    /// The detector version is empty.
    EmptyVersion,
    /// A namespaced identifier is malformed.
    InvalidIdentifier {
        /// Why it was rejected.
        value: String,
    },
    /// A digest string is malformed.
    InvalidDigest {
        /// The offending value.
        value: String,
    },
    /// No `PRODUCE` step was declared.
    NoProduce,
    /// A second `PRODUCE` step was declared.
    DuplicateProduce {
        /// Index of the duplicate.
        index: usize,
    },
    /// `PRODUCE` was not the last step.
    ProduceNotLast {
        /// Index of the misplaced `PRODUCE`.
        index: usize,
    },
    /// A `FLOW` appeared before any `MATCH`.
    FlowWithoutMatch {
        /// Index of the offending step.
        index: usize,
    },
    /// An `EXCLUDE` appeared before any `FLOW`.
    ExcludeBeforeFlow {
        /// Index of the offending step.
        index: usize,
    },
    /// A `VERIFY` appeared before any `FLOW`.
    VerifyBeforeFlow {
        /// Index of the offending step.
        index: usize,
    },
    /// A `MATCH` appeared after a `FLOW`.
    MatchAfterFlow {
        /// Index of the offending step.
        index: usize,
    },
    /// A step needs a capability the detector did not declare.
    MissingCapability {
        /// The capability that is required.
        capability: AnalysisCapability,
        /// Index of the step that requires it.
        index: usize,
    },
}

impl fmt::Display for DetectorIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => f.write_str("detector name must not be empty"),
            Self::EmptyVersion => f.write_str("detector version must not be empty"),
            Self::InvalidIdentifier { value } => {
                write!(f, "invalid namespaced identifier: {value}")
            }
            Self::InvalidDigest { value } => {
                write!(f, "invalid digest `{value}` (expected `fnv1a64:<hex>`)")
            }
            Self::NoProduce => f.write_str("detector must declare at least one PRODUCE step"),
            Self::DuplicateProduce { index } => {
                write!(
                    f,
                    "detector declares more than one PRODUCE (second at {index})"
                )
            }
            Self::ProduceNotLast { index } => {
                write!(f, "PRODUCE at index {index} must be the last step")
            }
            Self::FlowWithoutMatch { index } => {
                write!(f, "FLOW at index {index} has no preceding MATCH")
            }
            Self::ExcludeBeforeFlow { index } => {
                write!(f, "EXCLUDE at index {index} has no preceding FLOW")
            }
            Self::VerifyBeforeFlow { index } => {
                write!(f, "VERIFY at index {index} has no preceding FLOW")
            }
            Self::MatchAfterFlow { index } => {
                write!(f, "MATCH at index {index} appears after a FLOW")
            }
            Self::MissingCapability { capability, index } => write!(
                f,
                "step at index {index} requires capability {capability}, which the detector does not declare"
            ),
        }
    }
}

impl std::error::Error for DetectorIrError {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn subject(value: &str) -> SubjectPattern {
        SubjectPattern::new(value).expect("valid subject")
    }

    fn kind(value: &str) -> FindingKind {
        FindingKind::new(value).expect("valid kind")
    }

    fn caps(items: impl IntoIterator<Item = AnalysisCapability>) -> BTreeSet<AnalysisCapability> {
        items.into_iter().collect()
    }

    fn flow_steps() -> Vec<DetectorStep> {
        vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
        ]
    }

    fn detector(requires: BTreeSet<AnalysisCapability>, steps: Vec<DetectorStep>) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("security.sql_injection").unwrap(),
            name: "SQL injection".to_string(),
            requires,
            authority: DetectorAuthority::Candidate,
            steps,
        }
    }

    #[test]
    fn capability_tiers_are_ordered_but_capabilities_are_a_set() {
        assert!(EscalationTier::T0Ast.rank() < EscalationTier::T5Contextual.rank());
        assert_eq!(AnalysisCapability::AstPattern.tier(), EscalationTier::T0Ast);
        assert_eq!(
            AnalysisCapability::Dataflow.tier(),
            EscalationTier::T3Dataflow
        );
        assert_eq!(
            AnalysisCapability::SymbolicFeasibility.tier(),
            EscalationTier::T4Formal
        );
        assert_eq!(
            AnalysisCapability::LlmReasoning.tier(),
            EscalationTier::T5Contextual
        );

        // Key property: LLM reasoning does NOT imply symbolic feasibility.
        let llm_only = detector(caps([AnalysisCapability::LlmReasoning]), flow_steps());
        assert!(
            !llm_only
                .requires
                .contains(&AnalysisCapability::SymbolicFeasibility)
        );
    }

    #[test]
    fn escalation_tier_is_the_max_of_required_capabilities() {
        let ir = detector(
            caps([AnalysisCapability::GraphQuery, AnalysisCapability::Dataflow]),
            flow_steps(),
        );
        assert_eq!(ir.escalation_tier(), EscalationTier::T3Dataflow);

        let empty = detector(caps([]), vec![DetectorStep::Produce { kind: kind("a.b") }]);
        assert_eq!(empty.escalation_tier(), EscalationTier::T0Ast);
    }

    #[test]
    fn namespaced_identifiers_require_a_dot() {
        assert!(DetectorId::new("security.sql_injection").is_ok());
        assert!(DetectorId::new("nons").is_err());
        assert!(SubjectPattern::new("security.user_input").is_ok());
        assert!(SubjectPattern::new("nons").is_err());
        assert!(FindingKind::new("nons").is_err());
    }

    #[test]
    fn valid_graph_flow_detector_admits() {
        let ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        ir.validate().expect("graph-flow detector must admit");
    }

    #[test]
    fn unsupported_construct_fails_loud() {
        // VERIFY feasible_path needs SYMBOLIC_FEASIBILITY. Declare enough for
        // the FLOW (GraphQuery) but omit the symbolic capability, so the
        // unsupported construct is what fails.
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Verify {
                feasible_path: true,
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::MissingCapability {
                capability: AnalysisCapability::SymbolicFeasibility,
                index: 2,
            }
        );
    }

    #[test]
    fn declaring_the_capability_admits_the_verify_step() {
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Verify {
                feasible_path: true,
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
        ];
        let ir = detector(
            caps([
                AnalysisCapability::GraphQuery,
                AnalysisCapability::SymbolicFeasibility,
            ]),
            steps,
        );
        ir.validate()
            .expect("declaring the capability admits the detector");
        assert_eq!(ir.escalation_tier(), EscalationTier::T4Formal);
    }

    #[test]
    fn flow_without_match_rejected() {
        let steps = vec![
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::FlowWithoutMatch { index: 0 }
        );
    }

    #[test]
    fn exclude_before_flow_rejected() {
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Exclude {
                path_contains: subject("security.sql_sanitizer"),
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::ExcludeBeforeFlow { index: 1 }
        );
    }

    #[test]
    fn no_produce_rejected() {
        let steps = vec![DetectorStep::Match {
            subject: subject("security.user_input"),
        }];
        let ir = detector(caps([]), steps);
        assert_eq!(ir.validate().unwrap_err(), DetectorIrError::NoProduce);
    }

    #[test]
    fn produce_must_be_last() {
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Produce {
                kind: kind("security.sql_injection"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::ProduceNotLast { index: 1 }
        );
    }

    #[test]
    fn duplicate_produce_rejected() {
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Produce {
                kind: kind("security.a"),
            },
            DetectorStep::Produce {
                kind: kind("security.b"),
            },
        ];
        let ir = detector(caps([]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::DuplicateProduce { index: 2 }
        );
    }

    #[test]
    fn empty_name_rejected() {
        let mut ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        ir.name = "   ".to_string();
        assert_eq!(ir.validate().unwrap_err(), DetectorIrError::EmptyName);
    }

    #[test]
    fn candidate_detector_cannot_block() {
        let mut ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        ir.authority = DetectorAuthority::Candidate;
        assert!(!ir.can_block());

        ir.authority = DetectorAuthority::Gated;
        assert!(ir.can_block());

        ir.steps.clear();
        assert!(!ir.can_block(), "invalid gated detector still cannot block");
    }

    #[test]
    fn digest_is_stable_and_order_sensitive() {
        let a = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        let b = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        assert_eq!(
            a.digest(),
            b.digest(),
            "identical definitions digest equally"
        );

        let mut c = a.clone();
        c.name = "different".to_string();
        assert_ne!(
            a.digest(),
            c.digest(),
            "a content change changes the digest"
        );

        assert!(a.digest().as_str().starts_with("fnv1a64:"));
    }

    #[test]
    fn execution_ref_captures_authority_and_digest() {
        let ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        let execution =
            DetectorExecutionRef::from_definition(&ir, "1.0.0", Some(ExecutionId(9))).unwrap();

        assert_eq!(execution.id, ir.id);
        assert_eq!(execution.version, "1.0.0");
        assert_eq!(
            execution.authority_at_execution,
            DetectorAuthority::Candidate
        );
        assert_eq!(execution.detector_digest, ir.digest());
        assert_eq!(execution.execution_id, Some(ExecutionId(9)));
        assert!(!execution.can_block());
    }

    #[test]
    fn execution_ref_rejects_empty_version_and_bad_digest() {
        let id = DetectorId::new("security.sql_injection").unwrap();
        let digest = DetectorDigest::from_content("x");
        assert_eq!(
            DetectorExecutionRef::new(id, "", DetectorAuthority::Gated, digest, None).unwrap_err(),
            DetectorIrError::EmptyVersion
        );
        assert!(DetectorDigest::new("not-a-digest").is_err());
    }

    #[test]
    fn detector_ir_round_trip() {
        let ir = detector(
            caps([
                AnalysisCapability::GraphQuery,
                AnalysisCapability::SymbolicFeasibility,
            ]),
            flow_steps(),
        );
        let json = serde_json::to_string(&ir).expect("serialize");
        let parsed: DetectorIr = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ir);
    }

    #[test]
    fn malformed_subject_fails_deserialization() {
        let bad = r#"{"step":"match","subject":"not-namespaced"}"#;
        let attempted: Result<DetectorStep, _> = serde_json::from_str(bad);
        assert!(
            attempted.is_err(),
            "non-namespaced subject must not deserialize"
        );
    }
}
