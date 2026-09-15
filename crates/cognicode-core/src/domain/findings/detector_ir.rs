//! Detector IR (M6.1) — a declarative, validated detector definition.
//!
//! A detector is expressed as a small ordered program of steps
//! (`MATCH` / `FLOW` / `EXCLUDE` / `VERIFY` / `PRODUCE`) plus the analysis
//! [`AnalysisLevel`] it needs. The IR is validated **before admission**:
//! an unsupported construct, a malformed pattern or a level that is too
//! low for the requested verification fails loud with a structured
//! [`DetectorIrError`].
//!
//! Authority is explicit: an AI-authored detector starts as
//! [`DetectorAuthority::Candidate`] and cannot block CI
//! ([`DetectorAuthority::can_block`] is the single gate predicate).
//!
//! Pure domain: no I/O, no `sqlx`, no `tokio`.

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// Analysis escalation ladder
// ============================================================================

/// Cost/expressiveness level an analysis backend provides.
///
/// Ordered from cheapest to most expensive; a detector that declares a
/// weaker level than its steps require is rejected (see
/// [`DetectorIr::validate`]). Declared order is significant: the derived
/// `Ord` matches the escalation ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisLevel {
    /// Syntactic pattern matching over an AST.
    AstPattern,
    /// Semantic query (resolved symbols, types).
    SemanticQuery,
    /// Graph query over call/flow projections.
    GraphQuery,
    /// Data-flow analysis (def-use, reaching definitions).
    Dataflow,
    /// Selective abstract interpretation.
    AbstractInterpretation,
    /// Symbolic execution / SMT feasibility checks.
    Symbolic,
    /// Runtime trace corroboration.
    RuntimeCorroboration,
    /// LLM context reasoning (least authoritative).
    LlmContext,
}

impl AnalysisLevel {
    /// Ascending cost rank (`AstPattern = 0` … `LlmContext = 7`).
    pub fn rank(self) -> u8 {
        self as u8
    }

    /// Stable UPPER_SNAKE name for diagnostics.
    pub fn name(self) -> &'static str {
        match self {
            Self::AstPattern => "AST_PATTERN",
            Self::SemanticQuery => "SEMANTIC_QUERY",
            Self::GraphQuery => "GRAPH_QUERY",
            Self::Dataflow => "DATAFLOW",
            Self::AbstractInterpretation => "ABSTRACT_INTERPRETATION",
            Self::Symbolic => "SYMBOLIC",
            Self::RuntimeCorroboration => "RUNTIME_CORROBORATION",
            Self::LlmContext => "LLM_CONTEXT",
        }
    }
}

impl fmt::Display for AnalysisLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ============================================================================
// Namespaced patterns
// ============================================================================

/// Validate the `ns.name` namespace grammar (at least one `.`, no empty
/// segments). Matches the canonical detector-IR examples such as
/// `security.user_input`.
fn parse_namespaced(value: &str) -> Result<(), &'static str> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() < 2 {
        return Err("expected at least one '.' separator (namespace.name)");
    }
    if parts.iter().any(|segment| segment.is_empty()) {
        return Err("namespace segments must not be empty");
    }
    Ok(())
}

/// A namespaced subject a detector matches or flows over
/// (e.g. `security.user_input`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SubjectPattern(String);

impl SubjectPattern {
    /// Construct a validated subject pattern.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        let value = value.into();
        parse_namespaced(&value).map_err(|reason| DetectorIrError::InvalidNamespace {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    /// Borrow the raw pattern.
    pub fn as_str(&self) -> &str {
        &self.0
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
        value.0
    }
}

impl fmt::Display for SubjectPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A namespaced finding kind a detector produces
/// (e.g. `security.sql_injection`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FindingKind(String);

impl FindingKind {
    /// Construct a validated finding kind.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        let value = value.into();
        parse_namespaced(&value).map_err(|reason| DetectorIrError::InvalidNamespace {
            value: value.clone(),
            reason,
        })?;
        Ok(Self(value))
    }

    /// Borrow the raw kind.
    pub fn as_str(&self) -> &str {
        &self.0
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
        value.0
    }
}

impl fmt::Display for FindingKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
}

/// Stable detector identifier (non-empty).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DetectorId(String);

impl DetectorId {
    /// Construct a non-empty detector id.
    pub fn new(value: impl Into<String>) -> Result<Self, DetectorIrError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DetectorIrError::EmptyId);
        }
        Ok(Self(value))
    }

    /// Borrow the raw id.
    pub fn as_str(&self) -> &str {
        &self.0
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
        value.0
    }
}

impl fmt::Display for DetectorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Whether a detector may block CI.
///
/// AI-authored detectors start as [`Candidate`](Self::Candidate) and have
/// no GATE authority; only a promoted [`Gated`](Self::Gated) detector may
/// block.
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
    /// Stable detector id.
    pub id: DetectorId,
    /// Human-readable name.
    pub name: String,
    /// The analysis level the detector declares it needs.
    pub required_level: AnalysisLevel,
    /// GATE authority (candidate vs gated).
    pub authority: DetectorAuthority,
    /// Ordered program steps.
    pub steps: Vec<DetectorStep>,
}

impl DetectorIr {
    /// Validate the detector before admission.
    ///
    /// Fails loud with a structured [`DetectorIrError`] when the definition
    /// is malformed or demands a construct the declared
    /// [`AnalysisLevel`] cannot support.
    pub fn validate(&self) -> Result<(), DetectorIrError> {
        // V1/V2 — identity
        if self.id.as_str().trim().is_empty() {
            return Err(DetectorIrError::EmptyId);
        }
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

        // V5/V6/V8 — ordering + minimum escalation level
        let mut seen_match = false;
        let mut seen_flow = false;
        let mut minimum = AnalysisLevel::AstPattern;
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
                    minimum = minimum.max(AnalysisLevel::GraphQuery);
                }
                DetectorStep::Exclude { .. } => {
                    if !seen_flow {
                        return Err(DetectorIrError::ExcludeBeforeFlow { index });
                    }
                }
                DetectorStep::Verify { feasible_path } => {
                    if !seen_flow {
                        return Err(DetectorIrError::VerifyBeforeFlow { index });
                    }
                    if *feasible_path {
                        minimum = minimum.max(AnalysisLevel::Symbolic);
                    }
                }
                DetectorStep::Produce { .. } => {}
            }
        }

        // V7 — declared level must be at least what the steps demand
        if self.required_level < minimum {
            return Err(DetectorIrError::LevelTooLow {
                required: self.required_level,
                minimum,
            });
        }

        Ok(())
    }

    /// Whether this detector may block CI (delegates to authority AND
    /// requires the definition to be valid).
    pub fn can_block(&self) -> bool {
        self.authority.can_block() && self.validate().is_ok()
    }
}

// ============================================================================
// Errors
// ============================================================================

/// Structured validation failure for a detector definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectorIrError {
    /// The detector id is empty.
    EmptyId,
    /// The detector name is empty.
    EmptyName,
    /// A namespaced pattern is malformed.
    InvalidNamespace {
        /// The offending value.
        value: String,
        /// Why it was rejected.
        reason: &'static str,
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
    /// The declared `required_level` is weaker than the steps demand.
    LevelTooLow {
        /// Declared level.
        required: AnalysisLevel,
        /// Minimum level implied by the steps.
        minimum: AnalysisLevel,
    },
}

impl fmt::Display for DetectorIrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyId => f.write_str("detector id must not be empty"),
            Self::EmptyName => f.write_str("detector name must not be empty"),
            Self::InvalidNamespace { value, reason } => {
                write!(f, "invalid namespaced pattern `{value}`: {reason}")
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
            Self::LevelTooLow { required, minimum } => write!(
                f,
                "declared level {required} is too low; steps require at least {minimum}"
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

    fn base_steps() -> Vec<DetectorStep> {
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

    fn detector(level: AnalysisLevel, steps: Vec<DetectorStep>) -> DetectorIr {
        DetectorIr {
            id: DetectorId::new("det.sql_injection").unwrap(),
            name: "SQL injection".to_string(),
            required_level: level,
            authority: DetectorAuthority::Candidate,
            steps,
        }
    }

    #[test]
    fn analysis_level_rank_is_strictly_ascending() {
        let ladder = [
            AnalysisLevel::AstPattern,
            AnalysisLevel::SemanticQuery,
            AnalysisLevel::GraphQuery,
            AnalysisLevel::Dataflow,
            AnalysisLevel::AbstractInterpretation,
            AnalysisLevel::Symbolic,
            AnalysisLevel::RuntimeCorroboration,
            AnalysisLevel::LlmContext,
        ];
        for pair in ladder.windows(2) {
            assert!(
                pair[0].rank() < pair[1].rank(),
                "{} must rank below {}",
                pair[0],
                pair[1]
            );
            assert!(pair[0] < pair[1], "Ord must match rank order");
        }
        assert_eq!(ladder[0].rank(), 0);
        assert_eq!(ladder[7].rank(), 7);
    }

    #[test]
    fn analysis_level_display() {
        assert_eq!(AnalysisLevel::AstPattern.to_string(), "AST_PATTERN");
        assert_eq!(AnalysisLevel::GraphQuery.to_string(), "GRAPH_QUERY");
        assert_eq!(AnalysisLevel::LlmContext.to_string(), "LLM_CONTEXT");
    }

    #[test]
    fn subject_pattern_accepts_namespaced() {
        assert_eq!(
            subject("security.user_input").as_str(),
            "security.user_input"
        );
        assert_eq!(
            kind("security.sql_injection").as_str(),
            "security.sql_injection"
        );
    }

    #[test]
    fn subject_pattern_rejects_malformed() {
        for bad in ["", "ns.", ".name", "a..b", "nons", "  "] {
            let err = SubjectPattern::new(bad).unwrap_err();
            assert!(
                matches!(err, DetectorIrError::InvalidNamespace { .. }),
                "`{bad}` must be rejected, got {err:?}"
            );
        }
    }

    #[test]
    fn valid_graph_flow_detector_admits() {
        let ir = detector(AnalysisLevel::GraphQuery, base_steps());
        ir.validate().expect("graph-flow detector must admit");
    }

    #[test]
    fn unsupported_construct_fails_loud() {
        // VERIFY feasible_path demands SYMBOLIC, but the detector declares
        // only AST_PATTERN → rejected before scanning.
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
        let ir = detector(AnalysisLevel::AstPattern, steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::LevelTooLow {
                required: AnalysisLevel::AstPattern,
                minimum: AnalysisLevel::Symbolic,
            }
        );
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
        let ir = detector(AnalysisLevel::GraphQuery, steps);
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
        let ir = detector(AnalysisLevel::GraphQuery, steps);
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
        let ir = detector(AnalysisLevel::AstPattern, steps);
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
        let ir = detector(AnalysisLevel::GraphQuery, steps);
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
        let ir = detector(AnalysisLevel::AstPattern, steps);
        assert_eq!(
            ir.validate().unwrap_err(),
            DetectorIrError::DuplicateProduce { index: 2 }
        );
    }

    #[test]
    fn empty_identity_rejected() {
        let mut ir = detector(AnalysisLevel::GraphQuery, base_steps());
        ir.name = "   ".to_string();
        assert_eq!(ir.validate().unwrap_err(), DetectorIrError::EmptyName);
        assert_eq!(DetectorId::new("").unwrap_err(), DetectorIrError::EmptyId);
    }

    #[test]
    fn candidate_detector_cannot_block() {
        let mut ir = detector(AnalysisLevel::GraphQuery, base_steps());
        ir.authority = DetectorAuthority::Candidate;
        assert!(!ir.can_block());
        assert!(!DetectorAuthority::Candidate.can_block());

        ir.authority = DetectorAuthority::Gated;
        assert!(ir.can_block());

        // A gated but invalid detector still cannot block.
        ir.steps.clear();
        assert!(!ir.can_block());
    }

    #[test]
    fn detector_ir_round_trip() {
        let ir = detector(AnalysisLevel::Symbolic, base_steps());
        let json = serde_json::to_string(&ir).expect("serialize");
        let parsed: DetectorIr = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, ir);
    }

    #[test]
    fn malformed_subject_fails_deserialization() {
        let bad = r#"{"step":"match","subject":"not-namespaced"}"#;
        // The `try_from` on `SubjectPattern` must reject it.
        let attempted: Result<DetectorStep, _> = serde_json::from_str(bad);
        assert!(
            attempted.is_err(),
            "non-namespaced subject must not deserialize"
        );
    }
}
