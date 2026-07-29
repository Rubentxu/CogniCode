//! Weakly Connected Components (WCC) descriptor for the analytics registry.
//!
//! Part of E28.4 Analytics Registry Cohort 1 — PR2 Cohort-1 Core.

use std::sync::LazyLock;

use crate::domain::analytics::{
    AlgorithmDescriptor, AlgorithmIdentity, AlgorithmId, AlgorithmParams,
    AlgorithmVersion, AnalyticsMode, ComplexityClass, DeterminismKind,
    Fixture, FixtureGraph, Maturity, OutputField, OutputSchema, OutputType,
    ProjectionAssumption,
};
use crate::domain::plan::limits::PlanLimits;

// =============================================================================
// WCC output schema
// =============================================================================

static WCC_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "node_id", type_: OutputType::NodeId },
        OutputField { name: "component_id", type_: OutputType::Count },
        OutputField { name: "total_components", type_: OutputType::Count },
    ],
});

// =============================================================================
// WCC limits
// =============================================================================

static WCC_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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

// =============================================================================
// WCC complexity
// =============================================================================

static WCC_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "Union-find with path compression".into(),
});

// =============================================================================
// WCC conformance fixtures
// =============================================================================

static WCC_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // Single node: one component
        Fixture {
            name: "single node one component",
            graph: FixtureGraph {
                nodes: vec!["A"],
                edges: vec![],
            },
            expected: serde_json::json!({
                "type": "single_node",
                "expectation": "one_component"
            }),
        },
        // Two disconnected nodes: two components
        Fixture {
            name: "disconnected nodes two components",
            graph: FixtureGraph {
                nodes: vec!["A", "B"],
                edges: vec![],
            },
            expected: serde_json::json!({
                "type": "disconnected",
                "expectation": "two_components"
            }),
        },
        // Chain A→B→C: all in one WCC (ignores direction)
        Fixture {
            name: "chain one component",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "chain",
                "expectation": "one_component"
            }),
        },
        // Two cycles disconnected: two components
        Fixture {
            name: "two cycles two components",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("B", "A"), ("C", "D"), ("D", "C")],
            },
            expected: serde_json::json!({
                "type": "two_cycles_disconnected",
                "expectation": "two_components"
            }),
        },
    ]
});

// =============================================================================
// WCC identity
// =============================================================================

static WCC_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("wcc"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 1,
});

// =============================================================================
// WCC params (no params)
// =============================================================================

pub struct WccParams;

impl AlgorithmParams for WccParams {
    fn param_names(&self) -> Vec<&'static str> {
        vec![]
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if params.is_null() || params.as_object().map_or(false, |o| o.is_empty()) {
            Ok(())
        } else {
            Err("WCC algorithm accepts no parameters".into())
        }
    }
}

// =============================================================================
// WCC descriptor
// =============================================================================

/// Weakly Connected Components descriptor using union-find.
///
/// Wraps `cognicode_graph_algos::cluster_components::cluster_components`:
/// - Directed: NO (treats graph as undirected for connectivity)
/// - Weighted: no
/// - Heterogeneous: no
/// - Determinism: deterministic (component IDs assigned by discovery order)
pub struct WccDescriptor;

impl AlgorithmDescriptor for WccDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &WCC_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &WccParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &WCC_SCHEMA
    }

    fn supported_modes(&self) -> &[AnalyticsMode] {
        &[
            AnalyticsMode::Stream,
            AnalyticsMode::Stats,
            AnalyticsMode::Annotate,
            AnalyticsMode::Persist,
        ]
    }

    fn complexity(&self) -> &ComplexityClass {
        &WCC_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &WCC_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &WCC_FIXTURES
    }

    fn determinism(&self) -> DeterminismKind {
        // Component IDs depend on traversal order — but for same graph, same result.
        DeterminismKind::Deterministic
    }

    fn directed(&self) -> bool {
        false
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        // WCC ignores edge direction — uses both in and out neighbors
        &ProjectionAssumption::Undirected
    }
}
