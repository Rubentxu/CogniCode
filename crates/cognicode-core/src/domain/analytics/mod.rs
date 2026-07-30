//! Analytics domain types for the algorithm registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR3 Bounded Paths.
//! Part of E28.5 Structural Analytics Cohort 2 — PR2 Descriptors.

use std::sync::LazyLock;

// =============================================================================
// Shared static limits for cohort-2 algorithms (all identical)
// =============================================================================

/// Default limits for cohort-2 structural algorithms.
pub(crate) static COHORT2_LIMITS: LazyLock<crate::domain::plan::limits::PlanLimits> =
    LazyLock::new(|| crate::domain::plan::limits::PlanLimits {
        time_ms: Some(30000),
        cancellation: None,
        max_depth: None,
        max_hops: None,
        max_visited_nodes: Some(1_000_000),
        max_visited_edges: None,
        max_result_rows: Some(100_000),
        max_path_count: None,
        max_memory_bytes: Some(512 * 1024 * 1024),
    });

/// Common supported modes for cohort-2 algorithms (Stream, Stats, Annotate).
pub(crate) static COHORT2_MODES: LazyLock<Vec<AnalyticsMode>> = LazyLock::new(|| {
    vec![
        AnalyticsMode::Stream,
        AnalyticsMode::Stats,
        AnalyticsMode::Annotate,
    ]
});

// =============================================================================
// Macro: reduce AlgorithmDescriptor impl boilerplate for cohort-2 algorithms
// =============================================================================

/// Generates `impl AlgorithmDescriptor` for cohort-2 descriptors.
///
/// Reduces ~150 LOC of repetitive impl boilerplate across Dominators,
/// Bridges, ArticulationPoints, and KCore descriptors.
///
/// # Arguments
///
/// - `$name`: The descriptor struct name (e.g., `DominatorsDescriptor`)
/// - `$directed`: Boolean literal — `true` for directed, `false` for undirected
/// - `$identity`: Reference to the static `AlgorithmIdentity` (e.g., `&DOMINATORS_IDENTITY`)
/// - `$params`: Reference to the params instance (e.g., `&DominatorsParams`)
/// - `$schema`: Reference to the static `OutputSchema` (e.g., `&DOMINATORS_SCHEMA`)
/// - `$fixtures`: Reference to the static `Vec<Fixture>` (e.g., `&DOMINATORS_FIXTURES`)
/// - `$complexity`: Reference to the static `ComplexityClass` (e.g., `&DOMINATORS_COMPLEXITY`)
/// - `$projection`: `ProjectionAssumption` value (e.g., `ProjectionAssumption::CallGraphOutgoing`)
#[macro_export]
macro_rules! impl_cohort2_descriptor {
    ($name:ident, $directed:expr, $identity:expr, $params:expr, $schema:expr, $fixtures:expr, $complexity:expr, $projection:expr) => {
        impl AlgorithmDescriptor for $name {
            fn identity(&self) -> &AlgorithmIdentity {
                $identity
            }

            fn params(&self) -> &dyn AlgorithmParams {
                $params
            }

            fn output_schema(&self) -> &OutputSchema {
                $schema
            }

            fn supported_modes(&self) -> &[AnalyticsMode] {
                $crate::domain::analytics::COHORT2_MODES.as_ref()
            }

            fn complexity(&self) -> &ComplexityClass {
                $complexity
            }

            fn limits(&self) -> &PlanLimits {
                &$crate::domain::analytics::COHORT2_LIMITS
            }

            fn conformance_fixtures(&self) -> &[Fixture] {
                $fixtures
            }

            fn determinism(&self) -> DeterminismKind {
                DeterminismKind::Deterministic
            }

            fn directed(&self) -> bool {
                $directed
            }

            fn weighted(&self) -> bool {
                false
            }

            fn heterogeneous(&self) -> bool {
                false
            }

            fn projection_assumption(&self) -> &ProjectionAssumption {
                &$projection
            }
        }
    };
}

pub mod articulation_descriptor;
pub mod bounded_shortest_paths_descriptor;
pub mod bridges_descriptor;
pub mod conductance_descriptor;
pub mod descriptor;
pub mod dominators_descriptor;
pub mod kcore_descriptor;
pub mod lineage;
pub mod modularity_descriptor;
pub mod oracle;
pub mod pagerank_descriptor;
pub mod personalized_pagerank_descriptor;
pub mod scc_descriptor;
pub mod wcc_descriptor;

pub use articulation_descriptor::*;
pub use bounded_shortest_paths_descriptor::*;
pub use bridges_descriptor::*;
pub use conductance_descriptor::*;
pub use descriptor::*;
pub use dominators_descriptor::*;
pub use kcore_descriptor::*;
pub use lineage::*;
pub use modularity_descriptor::*;
pub use oracle::*;
pub use pagerank_descriptor::*;
pub use personalized_pagerank_descriptor::*;
pub use scc_descriptor::*;
pub use wcc_descriptor::*;
