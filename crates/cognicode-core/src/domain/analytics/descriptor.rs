//! AlgorithmDescriptor trait and supporting types for analytics registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR1 Foundation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::domain::plan::limits::PlanLimits;

// ============================================================================
// AlgorithmId
// ============================================================================

/// A typed algorithm identifier.
///
/// Use `AlgorithmId::from_static("pagerank")` to construct. The typed
/// wrapper prevents stringly-typed ID errors at compile time.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AlgorithmId(String);

/// Error returned when parsing an `AlgorithmId` from a string fails.
#[derive(Debug, thiserror::Error)]
#[error("invalid algorithm id: {0}")]
pub struct AlgorithmIdParseError(pub String);

impl AlgorithmId {
    /// Construct an `AlgorithmId` from a static string.
    ///
    /// # Panics
    ///
    /// Panics if `s` is empty.
    #[track_caller]
    pub fn from_static(s: &'static str) -> Self {
        assert!(!s.is_empty(), "AlgorithmId must not be empty");
        Self(s.into())
    }

    /// Construct an `AlgorithmId` from an owned string.
    ///
    /// Used when reconstructing from database storage.
    pub fn from_string(s: impl Into<String>) -> Self {
        let s = s.into();
        assert!(!s.is_empty(), "AlgorithmId must not be empty");
        Self(s)
    }

    /// Returns the raw string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AlgorithmId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for AlgorithmId {
    type Err = AlgorithmIdParseError;

    /// Parse an `AlgorithmId` from a string.
    ///
    /// Accepts either:
    /// - Just the name: `"pagerank"` → `Ok(AlgorithmId("pagerank"))`
    /// - Name@version format: `"pagerank@v1.0.0"` → `Ok(AlgorithmId("pagerank"))`
    ///
    /// The version is accepted but ignored (it's validated at admission time against
    /// the descriptor's `AlgorithmVersion`). This allows callers to pass the full
    /// "name@version" string without splitting first.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(AlgorithmIdParseError("cannot be empty".into()));
        }

        // Handle "name@version" format by extracting just the name
        let name = if let Some(at_pos) = s.find('@') {
            &s[..at_pos]
        } else {
            s
        };

        if name.is_empty() {
            return Err(AlgorithmIdParseError(format!(
                "empty name in algorithm id `{}`",
                s
            )));
        }

        Ok(AlgorithmId(name.to_string()))
    }
}

// ============================================================================
// Maturity
// ============================================================================

/// Algorithm maturity level — reflects stability and support surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maturity {
    /// Fully supported, stable API, recommended for production.
    Stable,
    /// Available but API may change; not recommended for production.
    Experimental,
    /// Scheduled for removal; do not use in new code.
    Deprecated,
}

impl fmt::Display for Maturity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Maturity::Stable => write!(f, "stable"),
            Maturity::Experimental => write!(f, "experimental"),
            Maturity::Deprecated => write!(f, "deprecated"),
        }
    }
}

// ============================================================================
// AlgorithmVersion
// ============================================================================

/// A semver version string for algorithm descriptors.
///
/// Cohort-1 algorithms are always v1.0.0.
/// Uses the same validation approach as PlanVersion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlgorithmVersion(String);

impl AlgorithmVersion {
    /// Create AlgorithmVersion 1.0.0 for cohort-1 algorithms.
    pub fn v1() -> Self {
        Self("1.0.0".into())
    }

    /// Returns the version string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AlgorithmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "v{}", self.0)
    }
}

// ============================================================================
// AlgorithmIdentity
// ============================================================================

/// Identity bundle for an algorithm: id, version, maturity, cohort.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlgorithmIdentity {
    pub id: AlgorithmId,
    pub version: AlgorithmVersion,
    pub maturity: Maturity,
    /// Cohort number (1-4). Cohort 1 = PageRank, SCC, WCC, bounded shortest paths.
    pub cohort: u16,
}

// ============================================================================
// DeterminismKind
// ============================================================================

/// Describes the determinism characteristics of an algorithm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeterminismKind {
    /// Produces identical results for identical inputs with no seed required.
    Deterministic,
    /// Results depend on a seed value.
    /// - `required: true` — seed MUST be provided at runtime
    /// - `required: false` — seed is optional, `default` is used when omitted
    Seeded {
        required: bool,
        default: Option<u64>,
    },
    /// Results may differ even with the same inputs (e.g., randomized algorithms).
    None,
}

// ============================================================================
// AnalyticsMode
// ============================================================================

/// Execution mode for an analytics algorithm.
///
/// Each mode has distinct output semantics and failure handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyticsMode {
    /// Stream results as a sequence of typed rows.
    Stream,
    /// Return a single aggregate summary.
    Stats,
    /// Annotate nodes/edges with ephemeral overlay (canonical unchanged).
    Annotate,
    /// Persist a derived-analysis record (idempotent, no canonical mutation).
    Persist,
}

impl fmt::Display for AnalyticsMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyticsMode::Stream => write!(f, "stream"),
            AnalyticsMode::Stats => write!(f, "stats"),
            AnalyticsMode::Annotate => write!(f, "annotate"),
            AnalyticsMode::Persist => write!(f, "persist"),
        }
    }
}

// ============================================================================
// ComplexityClass
// ============================================================================

/// Algorithmic complexity characterization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexityClass {
    /// Time complexity expression (e.g., "O(V + E)").
    pub time: &'static str,
    /// Space complexity expression (e.g., "O(V)").
    pub space: &'static str,
    /// Additional notes about complexity.
    pub notes: &'static str,
}

// ============================================================================
// ProjectionAssumption
// ============================================================================

/// Describes what graph projection assumptions an algorithm makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectionAssumption {
    /// Algorithm works on the incoming call graph (rank accumulates on callees).
    CallGraphIncoming,
    /// Algorithm works on the outgoing call graph.
    CallGraphOutgoing,
    /// Algorithm works on undirected projection.
    Undirected,
    /// Algorithm has no specific projection requirements.
    Any,
}

// ============================================================================
// AlgorithmParams (trait)
// ============================================================================

/// Dynamic parameter schema for an algorithm.
///
/// Each descriptor returns its own concrete `AlgorithmParams` implementation.
pub trait AlgorithmParams: Send + Sync + 'static {
    /// Returns the parameter names and their JSON schema types.
    fn param_names(&self) -> Vec<&'static str>;

    /// Validate a params JSON against this schema.
    fn validate(&self, params: &serde_json::Value) -> Result<(), String>;
}

// ============================================================================
// OutputSchema
// ============================================================================

/// Describes the output schema of an algorithm result.
#[derive(Debug, Clone)]
pub struct OutputSchema {
    /// Field names in order.
    pub fields: Vec<OutputField>,
}

/// A single output field.
#[derive(Debug, Clone)]
pub struct OutputField {
    pub name: &'static str,
    pub type_: OutputType,
}

/// Type of an output field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputType {
    /// Floating-point score.
    Score,
    /// Integer count.
    Count,
    /// Set membership indicator.
    Membership,
    /// Node identifier.
    NodeId,
    /// Edge identifier pair.
    EdgePair,
    /// Cost value (floating-point).
    Cost,
    /// JSON blob for complex results.
    Json,
}

// ============================================================================
// Fixture
// ============================================================================

/// Conformance test fixture for cross-backend verification.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub name: &'static str,
    /// Graph description (nodes and edges).
    pub graph: FixtureGraph,
    /// Expected output (algorithm-specific).
    pub expected: serde_json::Value,
}

/// A simple graph fixture for conformance testing.
#[derive(Debug, Clone)]
pub struct FixtureGraph {
    pub nodes: Vec<&'static str>,
    pub edges: Vec<(&'static str, &'static str)>,
}

// ============================================================================
// AlgorithmDescriptor (trait)
// ============================================================================

/// The core trait that all analytics algorithms must implement.
///
/// 12-method trait: identity, params, output_schema, supported_modes,
/// complexity, limits, conformance_fixtures, determinism, directed, weighted,
/// heterogeneous, projection_assumption.
///
/// Every method is mandatory — the registry validates completeness at admission
/// time. No default implementations (except where explicitly noted).
pub trait AlgorithmDescriptor: Send + Sync + 'static {
    /// Returns the algorithm's identity bundle.
    fn identity(&self) -> &AlgorithmIdentity;

    /// Returns the parameter schema.
    fn params(&self) -> &dyn AlgorithmParams;

    /// Returns the output schema.
    fn output_schema(&self) -> &OutputSchema;

    /// Returns the supported execution modes.
    fn supported_modes(&self) -> &[AnalyticsMode];

    /// Returns the algorithmic complexity class.
    fn complexity(&self) -> &ComplexityClass;

    /// Returns the default and maximum resource limits.
    fn limits(&self) -> &PlanLimits;

    /// Returns conformance test fixtures for cross-backend verification.
    fn conformance_fixtures(&self) -> &[Fixture];

    /// Returns the determinism characteristics.
    fn determinism(&self) -> DeterminismKind;

    /// Returns `true` if the algorithm requires a directed graph.
    fn directed(&self) -> bool;

    /// Returns `true` if the algorithm uses edge weights.
    fn weighted(&self) -> bool;

    /// Returns `true` if the algorithm supports heterogeneous node/edge types.
    fn heterogeneous(&self) -> bool;

    /// Returns the projection assumption.
    fn projection_assumption(&self) -> &ProjectionAssumption;
}

// ============================================================================
// AlgorithmRegistry errors
// ============================================================================

/// Errors that can occur during algorithm registration.
#[derive(Debug, thiserror::Error)]
pub enum AdmissionError {
    #[error("descriptor is missing required fields: {0}")]
    Incomplete(String),

    #[error("algorithm {0} already admitted with version {1}")]
    AlreadyAdmitted(String, AlgorithmVersion),

    #[error("descriptor version conflict for {0}: same version {1} with different limits")]
    VersionConflict(String, AlgorithmVersion),
}

/// Errors that can occur during algorithm execution.
#[derive(Debug, thiserror::Error)]
pub enum AnalyticsError {
    #[error("algorithm {0} is not admitted")]
    NotAdmitted(AlgorithmId),

    #[error("projection mismatch: algorithm requires {0:?}")]
    ProjectionMismatch(ProjectionAssumption),

    #[error("missing required seed")]
    MissingSeed,

    #[error("limit policy violation: {0}")]
    LimitPolicyViolation(String),

    #[error("missing required limit: {0:?}")]
    MissingLimit(crate::domain::plan::limits::PlanLimitKind),

    #[error("limit exceeded: {0:?}")]
    LimitExceeded(crate::domain::plan::limits::PlanLimitKind),

    #[error("result truncated: {0}")]
    Truncated(String),

    #[error("canonical write violation: analytics must not mutate canonical graph")]
    CanonicalWriteViolation,

    #[error("idempotency conflict: same key with different parameters")]
    IdempotencyConflict,

    #[error("persist authorization required")]
    PersistUnauthorized,

    #[error("run not found: {0}")]
    RunNotFound(String),

    #[error("invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("internal error: {0}")]
    Internal(String),
}

// ============================================================================
// RunOutput
// ============================================================================

/// Output produced by an algorithm's [`AlgorithmDescriptor::execute()`] call.
///
/// Each variant corresponds to one algorithm family. The registry's `run()`
/// method wraps this in a [`RunResult`] with lineage tracking.
#[derive(Debug, Clone)]
pub enum RunOutput {
    /// PageRank scores: node symbol ID → score.
    PageRank(serde_json::Value),
    /// Strongly Connected Components: list of component lists.
    Scc(serde_json::Value),
    /// Weakly Connected Components: list of component lists.
    Wcc(serde_json::Value),
    /// Bounded shortest paths: list of paths.
    BoundedShortestPaths(serde_json::Value),
    /// Dominators: nodes, immediate dominators, and depths.
    Dominators {
        nodes: Vec<usize>,
        immediate_dominators: Vec<Option<usize>>,
        depths: Vec<u32>,
    },
    /// Articulation Points: nodes and their cut-vertex counts.
    ArticulationPoints {
        nodes: Vec<usize>,
        cut_vertices_counts: Vec<usize>,
    },
    /// Bridges: list of edge pairs.
    Bridges { edges: Vec<(usize, usize)> },
    /// K-Core: nodes and their core numbers.
    KCore {
        nodes: Vec<usize>,
        core_numbers: Vec<u32>,
    },
    /// Conductance: community IDs and their conductance scores.
    Conductance {
        community_ids: Vec<usize>,
        scores: Vec<f64>,
    },
    /// Modularity: score and community count.
    Modularity { score: f64, community_count: usize },
}

impl RunOutput {
    /// Returns the number of result items (rows) in this output.
    pub fn row_count(&self) -> i64 {
        match self {
            RunOutput::PageRank(v) => v.as_array().map(|a| a.len()).unwrap_or(0) as i64,
            RunOutput::Scc(v) => v.as_array().map(|a| a.len()).unwrap_or(0) as i64,
            RunOutput::Wcc(v) => v.as_array().map(|a| a.len()).unwrap_or(0) as i64,
            RunOutput::BoundedShortestPaths(v) => v.as_array().map(|a| a.len()).unwrap_or(0) as i64,
            RunOutput::Dominators { nodes, .. } => nodes.len() as i64,
            RunOutput::ArticulationPoints { nodes, .. } => nodes.len() as i64,
            RunOutput::Bridges { edges } => edges.len() as i64,
            RunOutput::KCore { nodes, .. } => nodes.len() as i64,
            RunOutput::Conductance { community_ids, .. } => community_ids.len() as i64,
            RunOutput::Modularity { .. } => 1,
        }
    }

    /// Convert to JSON value for serialization.
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            RunOutput::PageRank(v) => v.clone(),
            RunOutput::Scc(v) => v.clone(),
            RunOutput::Wcc(v) => v.clone(),
            RunOutput::BoundedShortestPaths(v) => v.clone(),
            RunOutput::Dominators {
                nodes,
                immediate_dominators,
                depths,
            } => serde_json::json!({
                "nodes": nodes,
                "immediate_dominators": immediate_dominators,
                "depths": depths,
            }),
            RunOutput::ArticulationPoints {
                nodes,
                cut_vertices_counts,
            } => serde_json::json!({
                "nodes": nodes,
                "cut_vertices_counts": cut_vertices_counts,
            }),
            RunOutput::Bridges { edges } => serde_json::json!({
                "edges": edges,
            }),
            RunOutput::KCore {
                nodes,
                core_numbers,
            } => serde_json::json!({
                "nodes": nodes,
                "core_numbers": core_numbers,
            }),
            RunOutput::Conductance {
                community_ids,
                scores,
            } => serde_json::json!({
                "community_ids": community_ids,
                "scores": scores,
            }),
            RunOutput::Modularity { score, community_count } => serde_json::json!({
                "score": score,
                "community_count": community_count,
            }),
        }
    }
}

// ============================================================================
// AlgorithmDescriptor::execute
// ============================================================================

/// Extension trait providing the async `execute` method on [`AlgorithmDescriptor`].
///
/// This is implemented by each cohort-1 descriptor. The async signature is
/// required for dynamic dispatch via `#[async_trait]`.
#[async_trait::async_trait]
pub trait AlgorithmExecute: AlgorithmDescriptor {
    /// Execute the algorithm against the given call graph.
    ///
    /// # Arguments
    ///
    /// - `params` — validated algorithm parameters (schema-validated before this call)
    /// - `graph` — the call graph to run the algorithm on
    /// - `limits` — effective resource limits (already validated against descriptor maxima)
    ///
    /// # Returns
    ///
    /// The algorithm's typed output wrapped in [`RunOutput`]. The caller
    /// ( [`AlgorithmRegistry::run()`][super::AlgorithmRegistry::run]) handles
    /// lineage tracking and mode-specific wrapping.
    async fn execute(
        &self,
        params: &serde_json::Value,
        graph: &crate::domain::aggregates::CallGraph,
        limits: &PlanLimits,
    ) -> Result<RunOutput, AnalyticsError>;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn algorithm_id_from_static_valid() {
        let id = AlgorithmId::from_static("pagerank");
        assert_eq!(id.as_str(), "pagerank");
        assert_eq!(id.to_string(), "pagerank");
    }

    #[test]
    #[should_panic(expected = "must not be empty")]
    fn algorithm_id_from_static_empty_panics() {
        AlgorithmId::from_static("");
    }

    #[test]
    fn algorithm_id_from_str_name_only() {
        use std::str::FromStr;
        let id: AlgorithmId = "pagerank".parse().unwrap();
        assert_eq!(id.as_str(), "pagerank");
    }

    #[test]
    fn algorithm_id_from_str_name_at_version() {
        use std::str::FromStr;
        let id: AlgorithmId = "pagerank@v1.0.0".parse().unwrap();
        assert_eq!(id.as_str(), "pagerank");
    }

    #[test]
    fn algorithm_id_from_str_empty_is_error() {
        use std::str::FromStr;
        let result: Result<AlgorithmId, _> = "".parse();
        assert!(result.is_err());
    }

    #[test]
    fn algorithm_id_from_str_empty_name_is_error() {
        use std::str::FromStr;
        // "@v1.0.0" has empty name
        let result: Result<AlgorithmId, _> = "@v1.0.0".parse();
        assert!(result.is_err());
    }

    #[test]
    fn algorithm_version_v1() {
        let v = AlgorithmVersion::v1();
        assert_eq!(v.as_str(), "1.0.0");
        assert_eq!(v.to_string(), "v1.0.0");
    }

    #[test]
    fn maturity_display() {
        assert_eq!(Maturity::Stable.to_string(), "stable");
        assert_eq!(Maturity::Experimental.to_string(), "experimental");
        assert_eq!(Maturity::Deprecated.to_string(), "deprecated");
    }

    #[test]
    fn analytics_mode_display() {
        assert_eq!(AnalyticsMode::Stream.to_string(), "stream");
        assert_eq!(AnalyticsMode::Stats.to_string(), "stats");
        assert_eq!(AnalyticsMode::Annotate.to_string(), "annotate");
        assert_eq!(AnalyticsMode::Persist.to_string(), "persist");
    }
}
