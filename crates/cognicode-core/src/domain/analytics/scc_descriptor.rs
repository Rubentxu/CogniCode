//! Strongly Connected Components (SCC) descriptor for the analytics registry.
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
// SCC output schema
// =============================================================================

static SCC_SCHEMA: LazyLock<OutputSchema> = LazyLock::new(|| OutputSchema {
    fields: vec![
        OutputField { name: "node_id", type_: OutputType::NodeId },
        OutputField { name: "scc_id", type_: OutputType::Count },
        OutputField { name: "total_sccs", type_: OutputType::Count },
    ],
});

// =============================================================================
// SCC limits
// =============================================================================

static SCC_LIMITS: LazyLock<PlanLimits> = LazyLock::new(|| PlanLimits {
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
// SCC complexity
// =============================================================================

static SCC_COMPLEXITY: LazyLock<ComplexityClass> = LazyLock::new(|| ComplexityClass {
    time: "O(V + E)".into(),
    space: "O(V)".into(),
    notes: "Tarjan's algorithm, single DFS pass".into(),
});

// =============================================================================
// SCC conformance fixtures
// =============================================================================

static SCC_FIXTURES: LazyLock<Vec<Fixture>> = LazyLock::new(|| {
    vec![
        // 3-node cycle: A→B→C→A — all in one SCC
        Fixture {
            name: "3-node cycle single SCC",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C"), ("C", "A")],
            },
            expected: serde_json::json!({
                "type": "single_scc",
                "expectation": "all_nodes_same_scc"
            }),
        },
        // DAG: each node is its own SCC
        Fixture {
            name: "DAG singleton SCCs",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C"],
                edges: vec![("A", "B"), ("B", "C")],
            },
            expected: serde_json::json!({
                "type": "dag",
                "expectation": "each_node_own_scc"
            }),
        },
        // Two disconnected cycles
        Fixture {
            name: "two cycles two SCCs",
            graph: FixtureGraph {
                nodes: vec!["A", "B", "C", "D"],
                edges: vec![("A", "B"), ("B", "A"), ("C", "D"), ("D", "C")],
            },
            expected: serde_json::json!({
                "type": "two_cycles",
                "expectation": "two_sccs"
            }),
        },
        // Empty graph
        Fixture {
            name: "empty graph",
            graph: FixtureGraph {
                nodes: vec![],
                edges: vec![],
            },
            expected: serde_json::json!({
                "type": "empty",
                "expectation": "empty_result"
            }),
        },
    ]
});

// =============================================================================
// SCC identity
// =============================================================================

static SCC_IDENTITY: LazyLock<AlgorithmIdentity> = LazyLock::new(|| AlgorithmIdentity {
    id: AlgorithmId::from_static("scc"),
    version: AlgorithmVersion::v1(),
    maturity: Maturity::Stable,
    cohort: 1,
});

// =============================================================================
// SCC params (no params)
// =============================================================================

pub struct SccParams;

impl AlgorithmParams for SccParams {
    fn param_names(&self) -> Vec<&'static str> {
        vec![]
    }

    fn validate(&self, params: &serde_json::Value) -> Result<(), String> {
        if params.is_null() || params.as_object().map_or(false, |o| o.is_empty()) {
            Ok(())
        } else {
            Err("SCC algorithm accepts no parameters".into())
        }
    }
}

// =============================================================================
// SCC descriptor
// =============================================================================

/// Strongly Connected Components descriptor using Tarjan's algorithm.
///
/// Wraps `cognicode_graph_algos::condensation::condensation`:
/// - Deterministic: post-order DFS, alphabetic within each SCC
/// - Directed: yes
/// - Weighted: no
/// - Heterogeneous: no
pub struct SccDescriptor;

impl AlgorithmDescriptor for SccDescriptor {
    fn identity(&self) -> &AlgorithmIdentity {
        &SCC_IDENTITY
    }

    fn params(&self) -> &dyn AlgorithmParams {
        &SccParams
    }

    fn output_schema(&self) -> &OutputSchema {
        &SCC_SCHEMA
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
        &SCC_COMPLEXITY
    }

    fn limits(&self) -> &PlanLimits {
        &SCC_LIMITS
    }

    fn conformance_fixtures(&self) -> &[Fixture] {
        &SCC_FIXTURES
    }

    fn determinism(&self) -> DeterminismKind {
        DeterminismKind::Deterministic
    }

    fn directed(&self) -> bool {
        true
    }

    fn weighted(&self) -> bool {
        false
    }

    fn heterogeneous(&self) -> bool {
        false
    }

    fn projection_assumption(&self) -> &ProjectionAssumption {
        &ProjectionAssumption::CallGraphOutgoing
    }
}
