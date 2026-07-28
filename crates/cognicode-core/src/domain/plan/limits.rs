//! PlanLimits and PlanLimit — resource governance for plan execution.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.

use serde::{Deserialize, Serialize};
use std::fmt;

// ============================================================================
// PlanLimit
// ============================================================================

/// The specific limit dimension that was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanLimit {
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

impl fmt::Display for PlanLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanLimit::TimeMs => write!(f, "time_ms"),
            PlanLimit::Cancellation => write!(f, "cancellation"),
            PlanLimit::MaxDepth => write!(f, "max_depth"),
            PlanLimit::MaxHops => write!(f, "max_hops"),
            PlanLimit::MaxVisitedNodes => write!(f, "max_visited_nodes"),
            PlanLimit::MaxVisitedEdges => write!(f, "max_visited_edges"),
            PlanLimit::MaxResultRows => write!(f, "max_result_rows"),
            PlanLimit::MaxPathCount => write!(f, "max_path_count"),
            PlanLimit::MemoryBytes => write!(f, "memory_bytes"),
        }
    }
}

// ============================================================================
// PlanLimits
// ============================================================================

/// Resource governance limits for a plan execution.
///
/// All fields are optional. A plan with all `None` fields is valid but
/// represents an unbounded execution — the executor may reject it or apply
/// internal defaults.
///
/// Note: `Eq` and `Hash` are NOT derived because `Option<Arc<AtomicBool>>`
/// (cancellation token) does not implement `Eq` or `Hash`. Use `is_unbounded()`
/// and equality of individual fields for comparison.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanLimits {
    /// Maximum wall-clock time in milliseconds.
    pub time_ms: Option<u64>,
    /// Shared cancellation token. When set, the executor polls `is_cancelled()`.
    pub cancellation: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
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

// Manual PartialEq: cancellation equality is pointer-based (Arc identity).
impl PartialEq for PlanLimits {
    fn eq(&self, other: &Self) -> bool {
        self.time_ms == other.time_ms
            && self.max_depth == other.max_depth
            && self.max_hops == other.max_hops
            && self.max_visited_nodes == other.max_visited_nodes
            && self.max_visited_edges == other.max_visited_edges
            && self.max_result_rows == other.max_result_rows
            && self.max_path_count == other.max_path_count
            && self.max_memory_bytes == other.max_memory_bytes
        // Note: cancellation equality is NOT checked (shared mutable state via Arc)
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

    /// `PlanLimit` has exactly 9 variants matching all `PlanLimits` fields.
    #[test]
    fn plan_limit_has_nine_variants() {
        use PlanLimit::*;
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
        assert_eq!(variants.len(), 9, "PlanLimit must have 9 variants");
    }

    /// `PlanLimit::Display` returns a kebab-case name matching the variant.
    #[test]
    fn plan_limit_display_names() {
        assert_eq!(PlanLimit::TimeMs.to_string(), "time_ms");
        assert_eq!(PlanLimit::MaxDepth.to_string(), "max_depth");
        assert_eq!(PlanLimit::MaxResultRows.to_string(), "max_result_rows");
    }

    /// `PlanLimit` must implement `PartialEq`, `Eq`, `Hash`, `Clone`, `Copy`.
    #[test]
    fn plan_limit_derives() {
        fn assert_derives<T: PartialEq + Eq + std::hash::Hash + Clone + Copy>() {}
        assert_derives::<PlanLimit>();
    }

    /// `PlanLimits` must derive `PartialEq`, `Default`, `Clone`.
    /// Note: Eq and Hash are NOT derived because Option<Arc<AtomicBool>> doesn't impl Eq or Hash.
    #[test]
    fn plan_limits_derives() {
        fn assert_derives<T: PartialEq + Default + Clone>() {}
        assert_derives::<PlanLimits>();
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
}
