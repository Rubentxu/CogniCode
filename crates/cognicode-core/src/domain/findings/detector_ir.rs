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

use super::digest::DetectorDigest;
use super::finding::{FindingSeverity, RiskLevel};
use super::namespaced::NamespacedName;
use super::scope::AnalysisScope;
use crate::domain::execution::{ActorRef, CorrelationId, ExecutionContext};
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

    /// Capabilities this step cannot run without (its hard floor).
    ///
    /// `FLOW`/`EXCLUDE` are **reachability** constructs: they are satisfied by
    /// either `GraphQuery` or `Dataflow`, so they are checked by the dedicated
    /// reachability rule rather than listed here (a dataflow detector must not
    /// be forced to claim `GraphQuery`).
    pub fn required_capabilities(&self) -> BTreeSet<AnalysisCapability> {
        let mut set = BTreeSet::new();
        match self {
            Self::Match { .. } | Self::Flow { .. } | Self::Exclude { .. } => {}
            Self::Verify { feasible_path } => {
                if *feasible_path {
                    set.insert(AnalysisCapability::SymbolicFeasibility);
                }
            }
            Self::Produce { .. } => {}
        }
        set
    }

    /// Whether this step is a reachability construct (FLOW / EXCLUDE).
    pub fn is_reachability(&self) -> bool {
        matches!(self, Self::Flow { .. } | Self::Exclude { .. })
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

/// Detector-owned policy for a produced finding's severity and risk.
///
/// Backends observe matches; they do **not** decide how severe or risky the
/// result is. The policy lives in the detector definition (and therefore in
/// the instance digest), so a backend cannot inflate a finding's risk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorFindingPolicy {
    /// Severity assigned to every finding this detector produces.
    pub default_severity: FindingSeverity,
    /// Risk assigned to every finding this detector produces.
    pub default_risk: RiskLevel,
}

impl DetectorFindingPolicy {
    /// Construct a policy.
    pub fn new(default_severity: FindingSeverity, default_risk: RiskLevel) -> Self {
        Self {
            default_severity,
            default_risk,
        }
    }
}

impl Default for DetectorFindingPolicy {
    fn default() -> Self {
        Self {
            default_severity: FindingSeverity::Warning,
            default_risk: RiskLevel::Medium,
        }
    }
}

/// A validated detector definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorIr {
    /// Stable, namespaced detector id.
    pub id: DetectorId,
    /// Human-readable name.
    pub name: String,
    /// Severity/risk policy for findings this detector produces.
    #[serde(default)]
    pub policy: DetectorFindingPolicy,
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

        // V9 — every detector must declare at least one capability. An empty
        // `requires` would be a subset of every backend's capability set, so
        // the planner could hand it to an arbitrary backend. Aggregation will
        // get an explicit capability (`Aggregation`) when it is implemented.
        if self.requires.is_empty() {
            return Err(DetectorIrError::NoDeclaredCapability);
        }

        // V7 — declared capabilities must cover every step's floor
        for (index, step) in self.steps.iter().enumerate() {
            for capability in step.required_capabilities() {
                if !self.requires.contains(&capability) {
                    return Err(DetectorIrError::MissingCapability { capability, index });
                }
            }
        }

        // V11 — `FLOW.source` is the authoritative reachability source, so it
        // must be declared by a MATCH. Extra MATCH subjects are informational
        // observations and never seed a traversal.
        if let Some(index) = self
            .steps
            .iter()
            .position(|s| matches!(s, DetectorStep::Flow { .. }))
        {
            let declared: BTreeSet<&SubjectPattern> = self
                .steps
                .iter()
                .filter_map(|s| match s {
                    DetectorStep::Match { subject } => Some(subject),
                    _ => None,
                })
                .collect();
            for (i, step) in self.steps.iter().enumerate() {
                if let DetectorStep::Flow { source, .. } = step {
                    if !declared.contains(source) {
                        return Err(DetectorIrError::UndeclaredFlowSource { index: i });
                    }
                }
            }
            let _ = index;
        }

        // V10 — reachability constructs need a reachability capability.
        // `FLOW`/`EXCLUDE` can be served by a graph engine (GraphQuery) or a
        // dataflow engine (Dataflow); declaring neither is unsupported.
        if let Some(index) = self.steps.iter().position(|s| s.is_reachability()) {
            let reachable = self.requires.contains(&AnalysisCapability::GraphQuery)
                || self.requires.contains(&AnalysisCapability::Dataflow);
            if !reachable {
                return Err(DetectorIrError::MissingReachabilityCapability { index });
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

    /// Digest of the detector **logic** — `(id, requires, steps)`.
    ///
    /// Excludes `name`, `authority` and `policy`, so renaming or promoting a
    /// detector does not change its logic identity.
    pub fn logic_digest(&self) -> DetectorDigest {
        let canonical = serde_json::to_string(&(&self.id, &self.requires, &self.steps))
            .unwrap_or_else(|_| format!("{}{:?}{:?}", self.id, self.requires, self.steps));
        DetectorDigest::from_content(&canonical)
    }

    /// Digest of the detector's finding **policy** (severity + risk).
    pub fn policy_digest(&self) -> DetectorDigest {
        let canonical =
            serde_json::to_string(&self.policy).unwrap_or_else(|_| format!("{:?}", self.policy));
        DetectorDigest::from_content(&canonical)
    }

    /// Digest of the detector **semantics** — `logic_digest + policy_digest`.
    ///
    /// The policy governs the finding severity/risk that reach the gate, so a
    /// policy change (e.g. `Medium` -> `Critical`) MUST change the semantic
    /// digest even though the logic digest stays the same.
    pub fn semantic_digest(&self) -> DetectorDigest {
        DetectorDigest::from_content(&format!(
            "{}:{}",
            self.logic_digest().as_str(),
            self.policy_digest().as_str()
        ))
    }

    /// Digest of the **exact instance** — the whole definition, including
    /// `name`, `authority` and `policy`.
    pub fn instance_digest(&self) -> DetectorDigest {
        let canonical = serde_json::to_string(self).unwrap_or_else(|_| format!("{self:?}"));
        DetectorDigest::from_content(&canonical)
    }

    /// All four digests, captured together for an execution record.
    pub fn digests(&self) -> DetectorDigests {
        DetectorDigests {
            logic: self.logic_digest(),
            policy: self.policy_digest(),
            semantic: self.semantic_digest(),
            instance: self.instance_digest(),
        }
    }

    /// The single kind this detector produces (validation guarantees one).
    pub fn produced_kind(&self) -> Option<&FindingKind> {
        self.steps.iter().find_map(|s| match s {
            DetectorStep::Produce { kind } => Some(kind),
            _ => None,
        })
    }
}

// ============================================================================
// Execution reference (authority captured at run time)
// ============================================================================

/// The four digests of a detector definition, captured together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectorDigests {
    /// `(id, requires, steps)`.
    pub logic: DetectorDigest,
    /// The finding policy.
    pub policy: DetectorDigest,
    /// `logic + policy`.
    pub semantic: DetectorDigest,
    /// The whole definition.
    pub instance: DetectorDigest,
}

impl DetectorDigests {
    /// Compute all digests of a definition.
    pub fn of(definition: &DetectorIr) -> Self {
        definition.digests()
    }
}

/// The detector **as executed**: id, version, the authority it held *at
/// execution time*, its digests, and an optional execution id.
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
    /// The four digests of the executed definition.
    pub digests: DetectorDigests,
    /// The concrete execution this finding came from.
    ///
    /// `None` only for findings that never came from a real run (the legacy
    /// QualityIssue projection); every `DetectorExecutor` run sets it. A scoped
    /// verification rejects a finding without one.
    ///
    /// The execution id and the scope used to be separate fields; they are one
    /// [`ExecutionContext`] now because they are two facets of the same thing —
    /// and because M7 needs the actor and the correlation beside them. Authority
    /// stays *outside*: `authority_at_execution` says what the execution was
    /// allowed to do, and it comes from admission, never from the context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<ExecutionContext>,
}

impl DetectorExecutionRef {
    /// Construct an execution reference explicitly.
    pub fn new(
        id: DetectorId,
        version: impl Into<String>,
        authority_at_execution: DetectorAuthority,
        digests: DetectorDigests,
        context: Option<ExecutionContext>,
    ) -> Result<Self, DetectorIrError> {
        let version = version.into();
        if version.trim().is_empty() {
            return Err(DetectorIrError::EmptyVersion);
        }
        Ok(Self {
            id,
            version,
            authority_at_execution,
            digests,
            context,
        })
    }

    /// Capture an execution reference **from** a raw detector definition.
    ///
    /// Prefer [`ExecutionPermit::execution_ref`](super::ExecutionPermit::execution_ref)
    /// in production: it sources the authority from the admission permit.
    pub fn from_definition(
        definition: &DetectorIr,
        version: impl Into<String>,
        context: Option<ExecutionContext>,
    ) -> Result<Self, DetectorIrError> {
        Self::new(
            definition.id.clone(),
            version,
            definition.authority,
            definition.digests(),
            context,
        )
    }

    /// The execution id, when this finding came from a real run.
    pub fn execution_id(&self) -> Option<ExecutionId> {
        self.context.as_ref().map(|c| c.execution_id)
    }

    /// The `(workspace, snapshot)` the run was pinned to.
    pub fn scope(&self) -> Option<&AnalysisScope> {
        self.context.as_ref().map(|c| &c.scope)
    }

    /// Who ran it.
    pub fn actor(&self) -> Option<&ActorRef> {
        self.context.as_ref().map(|c| &c.actor)
    }

    /// The logical operation it belonged to.
    pub fn correlation(&self) -> Option<&CorrelationId> {
        self.context.as_ref().map(|c| &c.correlation)
    }

    /// Whether the executed detector held blocking authority.
    pub fn can_block(&self) -> bool {
        self.authority_at_execution.can_block()
    }

    /// Whether the execution reference is structurally complete.
    pub fn is_well_formed(&self) -> bool {
        !self.version.trim().is_empty()
            && !self.digests.logic.as_str().is_empty()
            && !self.digests.policy.as_str().is_empty()
            && !self.digests.semantic.as_str().is_empty()
            && !self.digests.instance.as_str().is_empty()
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
    /// The detector declares no capability.
    NoDeclaredCapability,
    /// A `FLOW.source` that no `MATCH` declares.
    UndeclaredFlowSource {
        /// Index of the offending `FLOW` step.
        index: usize,
    },
    /// A reachability step (FLOW/EXCLUDE) but neither GraphQuery nor Dataflow.
    MissingReachabilityCapability {
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
            Self::NoDeclaredCapability => {
                f.write_str("a detector must declare at least one capability")
            }
            Self::UndeclaredFlowSource { index } => write!(
                f,
                "FLOW at index {index} uses a source subject that no MATCH declares; FLOW.source is the authoritative reachability source"
            ),
            Self::MissingReachabilityCapability { index } => write!(
                f,
                "step at index {index} is a reachability construct (FLOW/EXCLUDE) but the detector declares neither GRAPH_QUERY nor DATAFLOW"
            ),
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
            policy: DetectorFindingPolicy::default(),
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
    fn raw_ir_authority_is_only_a_claim() {
        // The raw IR no longer answers "can I block?" — authority is enforced
        // at the admission boundary (see `admission.rs`). It only carries the
        // claimed authority.
        let mut ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        ir.authority = DetectorAuthority::Gated;
        assert!(ir.authority.can_block(), "the claim itself is readable");

        ir.authority = DetectorAuthority::Candidate;
        assert!(!ir.authority.can_block());
    }

    #[test]
    fn semantic_digest_is_stable_and_content_sensitive() {
        let a = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        let b = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        assert_eq!(
            a.semantic_digest(),
            b.semantic_digest(),
            "identical definitions digest equally"
        );

        let mut c = a.clone();
        c.steps.pop();
        assert_ne!(
            a.semantic_digest(),
            c.semantic_digest(),
            "a step change changes the semantic digest"
        );

        assert!(a.semantic_digest().as_str().starts_with("sha256:"));
    }

    #[test]
    fn semantic_digest_ignores_name_and_authority() {
        let mut a = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        let baseline = a.semantic_digest();

        // Renaming does not change the algorithm.
        a.name = "renamed".to_string();
        assert_eq!(a.semantic_digest(), baseline);

        // Promoting Candidate -> Gated does not change the algorithm...
        a.authority = DetectorAuthority::Gated;
        assert_eq!(a.semantic_digest(), baseline);
        // ...but it does change the instance digest.
        assert_ne!(a.instance_digest(), baseline);
    }

    /// A detector execution context for tests.
    fn test_context(id: u64) -> crate::domain::execution::ExecutionContext {
        crate::domain::execution::ExecutionContext::try_new(
            ExecutionId(id),
            crate::domain::findings::AnalysisScope::new(
                crate::domain::value_objects::WorkspaceId::try_new("ws").unwrap(),
                SnapshotId::new(1),
            ),
            crate::domain::execution::ActorRef::detector("security.sql_injection"),
            crate::domain::execution::CorrelationId::new("c").unwrap(),
            None,
        )
        .unwrap()
    }

    use crate::domain::kernel_ids::SnapshotId;

    #[test]
    fn execution_ref_captures_authority_and_digest() {
        let ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        let execution =
            DetectorExecutionRef::from_definition(&ir, "1.0.0", Some(test_context(9))).unwrap();

        assert_eq!(execution.id, ir.id);
        assert_eq!(execution.version, "1.0.0");
        assert_eq!(
            execution.authority_at_execution,
            DetectorAuthority::Candidate
        );
        assert_eq!(execution.digests.semantic, ir.semantic_digest());
        assert_eq!(execution.digests.instance, ir.instance_digest());
        assert_eq!(execution.digests.logic, ir.logic_digest());
        assert_eq!(execution.digests.policy, ir.policy_digest());
        assert_eq!(execution.execution_id(), Some(ExecutionId(9)));
        assert!(execution.is_well_formed());
        assert!(!execution.can_block());
    }

    #[test]
    fn execution_ref_rejects_empty_version() {
        let id = DetectorId::new("security.sql_injection").unwrap();
        let digest = DetectorDigest::from_content("x");
        let digests = DetectorDigests {
            logic: digest.clone(),
            policy: digest.clone(),
            semantic: digest.clone(),
            instance: digest,
        };
        assert_eq!(
            DetectorExecutionRef::new(id, "", DetectorAuthority::Gated, digests, None).unwrap_err(),
            DetectorIrError::EmptyVersion
        );
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
    #[test]
    fn policy_change_changes_semantic_digest_but_not_logic_digest() {
        let mut ir = detector(caps([AnalysisCapability::GraphQuery]), flow_steps());
        ir.policy = DetectorFindingPolicy::new(
            super::super::finding::FindingSeverity::Info,
            super::super::finding::RiskLevel::Low,
        );
        let logic = ir.logic_digest();
        let semantic_low = ir.semantic_digest();

        ir.policy = DetectorFindingPolicy::new(
            super::super::finding::FindingSeverity::Critical,
            super::super::finding::RiskLevel::Critical,
        );
        assert_eq!(ir.logic_digest(), logic, "logic is unchanged by the policy");
        assert_ne!(
            ir.semantic_digest(),
            semantic_low,
            "a policy change must change the semantic digest"
        );
        assert_ne!(ir.policy_digest(), DetectorDigest::from_content("other"));
    }

    #[test]
    fn empty_requires_is_rejected() {
        let mut ir = detector(caps([]), flow_steps());
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::NoDeclaredCapability
        );
        // Even a PRODUCE-only detector must declare a capability.
        ir.steps = vec![DetectorStep::Produce {
            kind: kind("security.weak_hash"),
        }];
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::NoDeclaredCapability
        );
    }
    #[test]
    fn flow_source_must_be_a_declared_match_subject() {
        // MATCH cookie + FLOW user_input -> sink is incoherent: the
        // authoritative reachability source is undeclared.
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.cookie"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: kind("security.incoherent"),
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::UndeclaredFlowSource { index: 1 }
        );

        // Declaring the flow source as well is coherent.
        let steps = vec![
            DetectorStep::Match {
                subject: subject("security.cookie"),
            },
            DetectorStep::Match {
                subject: subject("security.user_input"),
            },
            DetectorStep::Flow {
                source: subject("security.user_input"),
                sink: subject("security.sql_execution"),
                max_hops: None,
            },
            DetectorStep::Produce {
                kind: kind("security.coherent"),
            },
        ];
        let ir = detector(caps([AnalysisCapability::GraphQuery]), steps);
        ir.validate()
            .expect("declaring the flow source admits the detector");
    }
}
