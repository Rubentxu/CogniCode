//! PlanLimits and PlanLimit — resource governance for plan execution.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Architecture
//!
//! `PlanLimitKind` is the **single source of truth** for limit dimension metadata.
//! The `PLAN_LIMIT_KINDS` const array maps each kind to its display name and
//! field accessor. Adding a new limit requires editing only `PlanLimitKind` and
//! `PLAN_LIMIT_KINDS` — the compiler enforces completeness.

use serde::{Deserialize, Serialize};
use std::fmt;

// Sealed trait — implemented by all plan types to certify backend-neutrality.
use super::neutrality::Sealed;

// ============================================================================
// PlanLimitKind — single source of truth
// ============================================================================

/// The specific limit dimension that was exceeded.
///
/// **This enum is the single source of truth.** All limit metadata
/// (display name, field accessor) is derived from `PLAN_LIMIT_KINDS`.
/// Adding a new variant requires only updating `PlanLimitKind` and `PLAN_LIMIT_KINDS`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanLimitKind {
    TimeMs,
    Cancellation,
    MaxDepth,
    MaxHops,
    MaxVisitedNodes,
    MaxVisitedEdges,
    MaxResultRows,
    MaxPathCount,
    MemoryBytes,
}

impl fmt::Display for PlanLimitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl PlanLimitKind {
    /// Returns the kebab-case display name for this limit kind.
    pub const fn display_name(&self) -> &'static str {
        match self {
            PlanLimitKind::TimeMs => "time_ms",
            PlanLimitKind::Cancellation => "cancellation",
            PlanLimitKind::MaxDepth => "max_depth",
            PlanLimitKind::MaxHops => "max_hops",
            PlanLimitKind::MaxVisitedNodes => "max_visited_nodes",
            PlanLimitKind::MaxVisitedEdges => "max_visited_edges",
            PlanLimitKind::MaxResultRows => "max_result_rows",
            PlanLimitKind::MaxPathCount => "max_path_count",
            PlanLimitKind::MemoryBytes => "memory_bytes",
        }
    }

    /// Returns the value of this limit from `PlanLimits`, or `None` if not set.
    pub fn get(&self, limits: &PlanLimits) -> Option<u64> {
        match self {
            PlanLimitKind::TimeMs => limits.time_ms.map(|v| v as u64),
            PlanLimitKind::Cancellation => None, // Cancellation is not a u64 limit
            PlanLimitKind::MaxDepth => limits.max_depth.map(|v| v as u64),
            PlanLimitKind::MaxHops => limits.max_hops.map(|v| v as u64),
            PlanLimitKind::MaxVisitedNodes => limits.max_visited_nodes,
            PlanLimitKind::MaxVisitedEdges => limits.max_visited_edges,
            PlanLimitKind::MaxResultRows => limits.max_result_rows,
            PlanLimitKind::MaxPathCount => limits.max_path_count,
            PlanLimitKind::MemoryBytes => limits.max_memory_bytes,
        }
    }
}

impl Sealed for PlanLimitKind {}

/// Alias for backward compatibility — `PlanLimit` is now `PlanLimitKind`.
#[deprecated(since = "0.65.0", note = "use PlanLimitKind instead")]
pub type PlanLimit = PlanLimitKind;

// ============================================================================
// PlanLimits
// ============================================================================

// ============================================================================
// PlanLimits
// ============================================================================

/// Resource governance limits for a plan execution.
///
/// All fields are optional. A plan with all `None` fields is valid but
/// represents an unbounded execution — the executor may reject it or apply
/// internal defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Maximum wall-clock time in milliseconds.
    pub time_ms: Option<u64>,
    /// Shared cancellation token. When set, the executor polls `is_cancelled()`.
    ///
    /// **Note**: Not serialized — cancellation tokens are process-local and cannot
    /// be meaningfully restored across process boundaries.
    #[serde(skip)]
    pub cancellation: Option<super::CancellationToken>,
    /// Maximum traversal depth (for subgraph/recursive queries).
    pub max_depth: Option<u32>,
    /// Maximum hop count (for shortest-path queries).
    pub max_hops: Option<u32>,
    /// Maximum number of visited nodes before aborting.
    pub max_visited_nodes: Option<u64>,
    /// Maximum number of visited edges before aborting.
    pub max_visited_edges: Option<u64>,
    /// Maximum result rows returned (truncation boundary).
    pub max_result_rows: Option<u64>,
    /// Maximum number of paths returned (truncation boundary).
    pub max_path_count: Option<u64>,
    /// Maximum memory usage in bytes (soft limit — executor may estimate).
    pub max_memory_bytes: Option<u64>,
}

impl Sealed for PlanLimits {}

/// All known limit kinds — the **single source of truth** for limit metadata.
///
/// Adding a new limit requires only adding a new entry here and to `PlanLimitKind`.
/// The compiler enforces that every variant is covered in the lookup table.
pub const PLAN_LIMIT_KINDS: &[PlanLimitKind] = &[
    PlanLimitKind::TimeMs,
    PlanLimitKind::Cancellation,
    PlanLimitKind::MaxDepth,
    PlanLimitKind::MaxHops,
    PlanLimitKind::MaxVisitedNodes,
    PlanLimitKind::MaxVisitedEdges,
    PlanLimitKind::MaxResultRows,
    PlanLimitKind::MaxPathCount,
    PlanLimitKind::MemoryBytes,
];

/// PlanLimits derives PartialEq and Eq. Cancellation equality is pointer-based
/// (Arc::ptr_eq), which is process-local only.
///
/// **Warning**: Two PlanLimits with logically equivalent but distinct
/// cancellation tokens (different Arc allocations) are NOT equal.
impl PartialEq for PlanLimits {
    fn eq(&self, other: &Self) -> bool {
        self.time_ms == other.time_ms
            && self.cancellation == other.cancellation
            && self.max_depth == other.max_depth
            && self.max_hops == other.max_hops
            && self.max_visited_nodes == other.max_visited_nodes
            && self.max_visited_edges == other.max_visited_edges
            && self.max_result_rows == other.max_result_rows
            && self.max_path_count == other.max_path_count
            && self.max_memory_bytes == other.max_memory_bytes
    }
}

impl Eq for PlanLimits {}

impl Default for PlanLimits {
    fn default() -> Self {
        Self {
            time_ms: None,
            cancellation: None,
            max_depth: None,
            max_hops: None,
            max_visited_nodes: None,
            max_visited_edges: None,
            max_result_rows: None,
            max_path_count: None,
            max_memory_bytes: None,
        }
    }
}

impl PlanLimits {
    /// Returns a builder for `PlanLimits`.
    pub fn builder() -> PlanLimitsBuilder {
        PlanLimitsBuilder(PlanLimits::default())
    }

    /// Returns `true` if all limit fields are `None`.
    pub fn is_unbounded(&self) -> bool {
        self.time_ms.is_none()
            && self.cancellation.is_none()
            && self.max_depth.is_none()
            && self.max_hops.is_none()
            && self.max_visited_nodes.is_none()
            && self.max_visited_edges.is_none()
            && self.max_result_rows.is_none()
            && self.max_path_count.is_none()
            && self.max_memory_bytes.is_none()
    }

    /// Validate that this `PlanLimits` has all required limits for the given `GraphPlan`.
    ///
    /// Returns `Ok(())` if the limits are sufficient for the plan variant,
    /// or `Err(PlanError::MissingLimit)` if a required limit is absent.
    ///
    /// Validation rules:
    /// - `Subgraph` requires `max_depth`
    /// - `Path` requires `max_hops`
    pub fn validate(&self, plan: &super::GraphPlan) -> Result<(), super::PlanError> {
        use super::GraphPlan;
        match plan {
            GraphPlan::Subgraph { limits, .. } => {
                if limits.max_depth.is_none() {
                    return Err(super::PlanError::MissingLimit(PlanLimit::MaxDepth));
                }
            }
            GraphPlan::Path { quantifier, .. } => {
                if quantifier.max_hops.is_none() {
                    return Err(super::PlanError::MissingLimit(PlanLimit::MaxHops));
                }
            }
            // Other variants (Neighbors, Cluster, Explain, BooleanComposition) have no additional bounds
            GraphPlan::Neighbors { .. }
            | GraphPlan::Cluster { .. }
            | GraphPlan::Explain { .. }
            | GraphPlan::BooleanComposition { .. } => {}
        }
        Ok(())
    }
}

pub struct PlanLimitsBuilder(PlanLimits);

impl PlanLimitsBuilder {
    pub fn time_ms(mut self, v: u64) -> Self {
        self.0.time_ms = Some(v);
        self
    }

    pub fn max_depth(mut self, v: u32) -> Self {
        self.0.max_depth = Some(v);
        self
    }

    pub fn max_hops(mut self, v: u32) -> Self {
        self.0.max_hops = Some(v);
        self
    }

    pub fn max_visited_nodes(mut self, v: u64) -> Self {
        self.0.max_visited_nodes = Some(v);
        self
    }

    pub fn max_visited_edges(mut self, v: u64) -> Self {
        self.0.max_visited_edges = Some(v);
        self
    }

    pub fn max_result_rows(mut self, v: u64) -> Self {
        self.0.max_result_rows = Some(v);
        self
    }

    pub fn max_path_count(mut self, v: u64) -> Self {
        self.0.max_path_count = Some(v);
        self
    }

    pub fn max_memory_bytes(mut self, v: u64) -> Self {
        self.0.max_memory_bytes = Some(v);
        self
    }

    pub fn build(self) -> PlanLimits {
        self.0
    }
}

impl Sealed for PlanLimitsBuilder {}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.3a RED — PlanLimits Default=all None + JSON round-trip
    // Scenario: `plan-limits::PlanLimits Value Object` (both)
    // Assert: `PlanLimits::default()` → every field `None`; custom limits
    //         round-trip JSON-preserves Nones
    // Task 1.4a RED — PlanLimit enum exhaustiveness + 1-to-1 with PlanLimits
    // Scenario: `plan-limits::PlanLimit Enum` (both)
    // Assert: 9 variants present; LimitExceeded identifies dimension
    // -------------------------------------------------------------------------

    /// `PlanLimits::default()` must have every field as `None`.
    #[test]
    fn plan_limits_default_all_none() {
        let limits = PlanLimits::default();
        assert!(limits.time_ms.is_none());
        assert!(limits.cancellation.is_none());
        assert!(limits.max_depth.is_none());
        assert!(limits.max_hops.is_none());
        assert!(limits.max_visited_nodes.is_none());
        assert!(limits.max_visited_edges.is_none());
        assert!(limits.max_result_rows.is_none());
        assert!(limits.max_path_count.is_none());
        assert!(limits.max_memory_bytes.is_none());
    }

    /// `PlanLimits::is_unbounded()` returns `true` when all fields are `None`.
    #[test]
    fn plan_limits_is_unbounded_true() {
        assert!(PlanLimits::default().is_unbounded());
    }

    /// `PlanLimits::is_unbounded()` returns `false` when any field is `Some`.
    #[test]
    fn plan_limits_is_unbounded_false() {
        let limits = PlanLimits::builder().max_depth(5).build();
        assert!(!limits.is_unbounded());
    }

    /// `PlanLimits` JSON round-trip preserves all `None` fields.
    #[test]
    fn plan_limits_json_roundtrip_none_fields() {
        let original = PlanLimits::default();
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: PlanLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, original);
    }

    /// `PlanLimits` JSON round-trip preserves custom field values.
    #[test]
    fn plan_limits_json_roundtrip_custom() {
        let original = PlanLimits::builder()
            .time_ms(1000)
            .max_depth(5)
            .max_hops(6)
            .max_result_rows(1000)
            .build();
        let json = serde_json::to_string(&original).expect("serialize");
        let parsed: PlanLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.time_ms, Some(1000));
        assert_eq!(parsed.max_depth, Some(5));
        assert_eq!(parsed.max_hops, Some(6));
        assert_eq!(parsed.max_result_rows, Some(1000));
    }

    /// `PlanLimitKind` has exactly 9 variants matching all `PlanLimits` fields.
    #[test]
    fn plan_limit_kind_has_nine_variants() {
        use PlanLimitKind::*;
        let variants = [
            TimeMs,
            Cancellation,
            MaxDepth,
            MaxHops,
            MaxVisitedNodes,
            MaxVisitedEdges,
            MaxResultRows,
            MaxPathCount,
            MemoryBytes,
        ];
        assert_eq!(variants.len(), 9, "PlanLimitKind must have 9 variants");
        assert_eq!(PLAN_LIMIT_KINDS.len(), 9);
    }

    /// `PlanLimitKind::Display` returns a kebab-case name matching the variant.
    #[test]
    fn plan_limit_kind_display_names() {
        assert_eq!(PlanLimitKind::TimeMs.to_string(), "time_ms");
        assert_eq!(PlanLimitKind::MaxDepth.to_string(), "max_depth");
        assert_eq!(PlanLimitKind::MaxResultRows.to_string(), "max_result_rows");
    }

    /// `PlanLimitKind::get` returns the correct value for each limit kind.
    #[test]
    fn plan_limit_kind_get() {
        let limits = PlanLimits::builder()
            .time_ms(1000)
            .max_depth(5)
            .max_hops(6)
            .max_result_rows(100)
            .build();

        assert_eq!(PlanLimitKind::TimeMs.get(&limits), Some(1000));
        assert_eq!(PlanLimitKind::MaxDepth.get(&limits), Some(5));
        assert_eq!(PlanLimitKind::MaxHops.get(&limits), Some(6));
        assert_eq!(PlanLimitKind::MaxResultRows.get(&limits), Some(100));
        assert_eq!(PlanLimitKind::Cancellation.get(&limits), None); // not a u64 limit
    }

    /// `PlanLimit` must implement `PartialEq`, `Eq`, `Hash`, `Clone`, `Copy`.
    #[test]
    fn plan_limit_derives() {
        fn assert_derives<T: PartialEq + Eq + std::hash::Hash + Clone + Copy>() {}
        assert_derives::<PlanLimit>();
    }

    /// `PlanLimits` must derive `PartialEq`, `Default`, `Clone`, `Eq`.
    /// Note: `cancellation` equality is pointer-based (Arc::ptr_eq) — process-local only.
    #[test]
    fn plan_limits_derives() {
        fn assert_derives<T: PartialEq + Eq + Default + Clone>() {}
        assert_derives::<PlanLimits>();
    }

    // -------------------------------------------------------------------------
    // Task W7 — PlanLimits::PartialEq for cancellation field
    // Scenario: CancellationToken equality is pointer-based (process-local)
    // Assert: same token (same Arc) → equal; different tokens → not equal
    // -------------------------------------------------------------------------

    /// Two PlanLimits with the same cancellation token (same Arc) are equal.
    #[test]
    fn plan_limits_cancellation_same_token_equal() {
        use super::super::CancellationToken;

        let token = CancellationToken::new();
        let limits_a = PlanLimits {
            time_ms: Some(1000),
            cancellation: Some(token.clone()),
            max_depth: Some(5),
            max_hops: None,
            max_visited_nodes: None,
            max_visited_edges: None,
            max_result_rows: None,
            max_path_count: None,
            max_memory_bytes: None,
        };
        let limits_b = PlanLimits {
            time_ms: Some(1000),
            cancellation: Some(token.clone()),
            max_depth: Some(5),
            max_hops: None,
            max_visited_nodes: None,
            max_visited_edges: None,
            max_result_rows: None,
            max_path_count: None,
            max_memory_bytes: None,
        };
        assert_eq!(limits_a, limits_b);
    }

    /// Two PlanLimits with different cancellation tokens (different Arc) are NOT equal.
    #[test]
    fn plan_limits_cancellation_different_tokens_not_equal() {
        use super::super::CancellationToken;

        let token_a = CancellationToken::new();
        let token_b = CancellationToken::new();
        let limits_a = PlanLimits {
            time_ms: Some(1000),
            cancellation: Some(token_a),
            max_depth: Some(5),
            max_hops: None,
            max_visited_nodes: None,
            max_visited_edges: None,
            max_result_rows: None,
            max_path_count: None,
            max_memory_bytes: None,
        };
        let limits_b = PlanLimits {
            time_ms: Some(1000),
            cancellation: Some(token_b),
            max_depth: Some(5),
            max_hops: None,
            max_visited_nodes: None,
            max_visited_edges: None,
            max_result_rows: None,
            max_path_count: None,
            max_memory_bytes: None,
        };
        assert_ne!(limits_a, limits_b);
    }

    /// PlanLimits with no cancellation (None) and no other differences are equal.
    #[test]
    fn plan_limits_both_none_cancellation_equal() {
        let limits_a = PlanLimits::default();
        let limits_b = PlanLimits::default();
        assert_eq!(limits_a, limits_b);
    }

    /// Builder pattern must allow setting multiple fields.
    #[test]
    fn plan_limits_builder_multiple() {
        let limits = PlanLimits::builder()
            .time_ms(5000)
            .max_depth(10)
            .max_visited_nodes(100_000)
            .max_result_rows(500)
            .build();
        assert_eq!(limits.time_ms, Some(5000));
        assert_eq!(limits.max_depth, Some(10));
        assert_eq!(limits.max_visited_nodes, Some(100_000));
        assert_eq!(limits.max_result_rows, Some(500));
    }

    // -------------------------------------------------------------------------
    // Task 2.5a RED — PlanLimits::validate
    // Scenario: `plan-limits::Every Plan Declares Applicable Limits` (both)
    // Assert: Subgraph { depth: 0, max_depth: None } → Err(MissingLimit(MaxDepth));
    //         Path w/o max_hops → Err(MissingLimit(MaxHops))
    // -------------------------------------------------------------------------

    /// `Subgraph` without `max_depth` is rejected.
    #[test]
    fn validate_subgraph_requires_max_depth() {
        use super::super::{GraphPlan, PathQuantifier, PlanMetadata, PlanVersion, PlanHash, NeighborKind};

        // Subgraph with no max_depth limit → Err
        let limits = PlanLimits::default(); // max_depth is None
        let plan = GraphPlan::Subgraph {
            nodes: vec!["A".into()],
            edges: None,
            aggregations: vec![],
            limits: limits.clone(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let result = limits.validate(&plan);
        assert!(result.is_err(), "Subgraph without max_depth must be rejected");
        assert!(matches!(result.unwrap_err(), super::super::PlanError::MissingLimit(super::PlanLimit::MaxDepth)));
    }

    /// `Subgraph` with `max_depth` set is accepted.
    #[test]
    fn validate_subgraph_with_max_depth_ok() {
        use super::super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash};

        let limits = PlanLimits::builder().max_depth(5).build();
        let plan = GraphPlan::Subgraph {
            nodes: vec!["A".into()],
            edges: None,
            aggregations: vec![],
            limits: limits.clone(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let result = limits.validate(&plan);
        assert!(result.is_ok(), "Subgraph with max_depth must be accepted");
    }

    /// `Path` without `max_hops` is rejected.
    #[test]
    fn validate_path_requires_max_hops() {
        use super::super::{GraphPlan, PathQuantifier, PlanMetadata, PlanVersion, PlanHash, PathProjection};

        let limits = PlanLimits::default(); // max_hops is None
        let plan = GraphPlan::Path {
            src: "A".into(),
            dst: "B".into(),
            quantifier: PathQuantifier { max_hops: None, min_hops: 0 },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: limits.clone(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let result = limits.validate(&plan);
        assert!(result.is_err(), "Path without max_hops must be rejected");
        assert!(matches!(result.unwrap_err(), super::super::PlanError::MissingLimit(super::PlanLimit::MaxHops)));
    }

    /// `Path` with `max_hops` set is accepted.
    #[test]
    fn validate_path_with_max_hops_ok() {
        use super::super::{GraphPlan, PathQuantifier, PlanMetadata, PlanVersion, PlanHash, PathProjection};

        let limits = PlanLimits::builder().max_hops(6).build();
        let plan = GraphPlan::Path {
            src: "A".into(),
            dst: "B".into(),
            quantifier: PathQuantifier { max_hops: Some(6), min_hops: 0 },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: limits.clone(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let result = limits.validate(&plan);
        assert!(result.is_ok(), "Path with max_hops must be accepted");
    }
}
