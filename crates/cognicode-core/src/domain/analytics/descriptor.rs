//! AlgorithmDescriptor trait and supporting types for analytics registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR1 Foundation.

use serde::{Deserialize, Serialize};
use std::fmt;

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
    Seeded { required: bool, default: Option<u64> },
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

    #[error("internal error: {0}")]
    Internal(String),
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
