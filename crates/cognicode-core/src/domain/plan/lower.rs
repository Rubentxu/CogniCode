//! AST → Plan lowering — compile MoldQL AST to GraphPlan.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR2 Plan Algebra.
//!
//! ## Architecture
//!
//! This module defines the **port** (trait) for AST→Plan lowering.
//! The **adapter** implementation lives in `cognicode-explorer` (Phase 3 Bridge),
//! where the full `MoldQLQuery` AST is available.
//!
//! This separation enforces the hexagonal architecture invariant:
//! `cognicode-core` (domain) must NOT depend on `cognicode-explorer` (infrastructure).

use super::{GraphPlan, PlanError, PlanLimits, PlanMetadata, PlanVersion, PlanHash, PathQuantifier, PathProjection, NeighborKind};

/// Default maximum depth for Subgraph queries.
pub const DEFAULT_MAX_DEPTH: u32 = 5;

/// Default maximum hops for Path queries.
pub const DEFAULT_MAX_HOPS: u32 = 6;

/// Describes the shape of a query to determine which limits are applicable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryShape {
    /// A subgraph extraction query.
    Subgraph { depth: u32 },
    /// A shortest/widest path query.
    Path { max_hops: Option<u32> },
    /// A neighbor traversal query.
    Neighbors,
    /// A clustering query.
    Cluster,
    /// An EXPLAIN query.
    Explain,
    /// A boolean composition query.
    Boolean,
}

impl QueryShape {
    /// Returns the applicable default limits for this query shape.
    fn default_limits(&self) -> PlanLimits {
        match self {
            QueryShape::Subgraph { .. } => {
                PlanLimits::builder().max_depth(DEFAULT_MAX_DEPTH).build()
            }
            QueryShape::Path { max_hops: None } => {
                PlanLimits::builder().max_hops(DEFAULT_MAX_HOPS).build()
            }
            // Path with explicit max_hops — no default needed
            QueryShape::Path { max_hops: Some(_) } => PlanLimits::default(),
            // Other shapes have no graph-traversal defaults
            QueryShape::Neighbors
            | QueryShape::Cluster
            | QueryShape::Explain
            | QueryShape::Boolean => PlanLimits::default(),
        }
    }
}

/// Populate PlanLimits defaults based on query shape.
///
/// - `Subgraph { depth: 0 }` → `max_depth = Some(5)`
/// - `Path { max_hops: None }` → `max_hops = Some(6)`
/// - Other graph variants → default per shape
pub fn populate_defaults(plan: &GraphPlan, query_shape: &QueryShape) -> PlanLimits {
    let shape_defaults = query_shape.default_limits();
    let mut result = plan.limits().clone();

    // Apply shape defaults only where the plan doesn't already have explicit limits
    if result.max_depth.is_none() {
        result.max_depth = shape_defaults.max_depth;
    }
    if result.max_hops.is_none() {
        result.max_hops = shape_defaults.max_hops;
    }
    result
}

/// Handles lowering of a MoldQL AST to a [`GraphPlan`].
///
/// Implementors must be provided by the infrastructure layer (e.g., `cognicode-explorer`).
pub trait AstLowerer: Send + Sync {
    /// Lower a query AST node to a [`GraphPlan`].
    ///
    /// Returns `Err(PlanError)` if the AST node cannot be lowered
    /// (e.g., unbounded quantifier, unsupported construct).
    fn lower(&self, ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError>;
}

/// Default no-op lowerer used when no adapter is wired.
pub struct NoOpLowerer;

impl AstLowerer for NoOpLowerer {
    fn lower(&self, _ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError> {
        Err(PlanError::UnsupportedConstruct(
            super::UnsupportedConstruct::new(
                super::ConstructId::Other("no lowerer wired".into()),
                "no AstLowerer adapter is wired in this build",
            )
            .with_alternative("wire an AstLowerer implementation from the infrastructure layer"),
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// `NoOpLowerer` always returns an error indicating no lowerer is wired.
    #[test]
    fn noop_lowerer_returns_error() {
        let lowerer = NoOpLowerer;
        // Use an empty Any reference — the NoOpLowerer ignores it anyway
        struct DummyQuery;
        let dummy = &DummyQuery as &dyn std::any::Any;
        let result = lowerer.lower(dummy);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, PlanError::UnsupportedConstruct { .. }));
    }

    /// `NoOpLowerer` is `Send + Sync + 'static`.
    #[test]
    fn noop_lowerer_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<NoOpLowerer>();
        assert_sync::<NoOpLowerer>();
        assert_static::<NoOpLowerer>();
    }

    // -------------------------------------------------------------------------
    // Task 2.7a RED — populate_defaults
    // Scenario: `explorerql-compilation::compile_to_plan populates PlanLimits` (both)
    // Assert: SubgraphQuery { depth: 0 } → PlanLimits { max_depth: Some(5) };
    //         PathQuery { max_hops: None } → PlanLimits { max_hops: Some(6) };
    //         no PlanError::MissingLimit raised
    // -------------------------------------------------------------------------

    /// `Subgraph` with `depth: 0` gets `max_depth = Some(5)` from defaults.
    #[test]
    fn populate_defaults_subgraph_depth_zero() {
        use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, QueryShape, DEFAULT_MAX_DEPTH};

        let plan = GraphPlan::Subgraph {
            nodes: vec!["A".into()],
            edges: None,
            aggregations: vec![],
            limits: PlanLimits::default(), // max_depth is None
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let shape = QueryShape::Subgraph { depth: 0 };
        let limits = super::populate_defaults(&plan, &shape);

        assert_eq!(limits.max_depth, Some(DEFAULT_MAX_DEPTH));
        // max_hops should remain None (not applicable to Subgraph)
        assert!(limits.max_hops.is_none());
    }

    /// `Path` with `max_hops: None` gets `max_hops = Some(6)` from defaults.
    #[test]
    fn populate_defaults_path_no_max_hops() {
        use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, PathQuantifier, PathProjection, QueryShape, DEFAULT_MAX_HOPS};

        let plan = GraphPlan::Path {
            src: "A".into(),
            dst: "B".into(),
            quantifier: PathQuantifier { max_hops: None, min_hops: 0 },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::default(), // max_hops is None
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let shape = QueryShape::Path { max_hops: None };
        let limits = super::populate_defaults(&plan, &shape);

        assert_eq!(limits.max_hops, Some(DEFAULT_MAX_HOPS));
    }

    /// `Path` with explicit `max_hops: Some(3)` keeps the explicit value.
    #[test]
    fn populate_defaults_path_explicit_max_hops_preserved() {
        use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, PathQuantifier, PathProjection, QueryShape};

        let plan = GraphPlan::Path {
            src: "A".into(),
            dst: "B".into(),
            quantifier: PathQuantifier { max_hops: Some(3), min_hops: 0 },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::builder().max_hops(3).build(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let shape = QueryShape::Path { max_hops: Some(3) };
        let limits = super::populate_defaults(&plan, &shape);

        assert_eq!(limits.max_hops, Some(3));
    }

    /// `QueryShape::Neighbors` has no graph-traversal defaults.
    #[test]
    fn populate_defaults_neighbors_no_defaults() {
        use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, NeighborKind, QueryShape};

        let plan = GraphPlan::Neighbors {
            src: "A".into(),
            kind: NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let shape = QueryShape::Neighbors;
        let limits = super::populate_defaults(&plan, &shape);

        assert!(limits.max_depth.is_none());
        assert!(limits.max_hops.is_none());
    }

    /// `populate_defaults` does not raise `PlanError::MissingLimit`.
    #[test]
    fn populate_defaults_no_missing_limit_error() {
        use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, QueryShape};

        let plan = GraphPlan::Subgraph {
            nodes: vec!["A".into()],
            edges: None,
            aggregations: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let shape = QueryShape::Subgraph { depth: 0 };
        // This should NOT return Err — it should populate defaults
        let limits = super::populate_defaults(&plan, &shape);
        assert!(limits.max_depth.is_some());
    }

    // -------------------------------------------------------------------------
    // Task 2.8a REFACTOR — wire PlanLimits::validate into lower
    // Scenario: `unsupported-operation-errors::Raised Before Execution`
    // Assert: lower(Subgraph { depth: 0 }) returns Err(MissingLimit(MaxDepth))
    //         because max_depth defaults to None and populate_defaults hasn't been called
    // -------------------------------------------------------------------------

    /// `lower` returns `Err(PlanError::MissingLimit(MaxDepth))` for Subgraph
    /// when `max_depth` is not set and `populate_defaults` has not been called.
    #[test]
    fn lower_subgraph_without_max_depth_rejected() {
        use std::any::Any;

        struct DummySubgraphQuery;

        let lowerer = DummyLowererForTest::new();
        let result = lowerer.lower(&DummySubgraphQuery as &dyn Any);

        // Without populate_defaults being called first, the plan has no max_depth
        // and PlanLimits::validate should reject it
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, super::super::PlanError::MissingLimit(super::super::PlanLimit::MaxDepth)));
    }

    /// `lower_with_defaults` applies populate_defaults first, so validation passes.
    #[test]
    fn lower_with_defaults_validates_ok() {
        use std::any::Any;

        struct DummySubgraphQuery;

        let lowerer = DummyLowererWithDefaults::new();
        let result = lowerer.lower(&DummySubgraphQuery as &dyn Any);

        // With populate_defaults called first, max_depth is set, so validate passes
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);
    }

    // -------------------------------------------------------------------------
    // Test helpers — these implement AstLowerer to test the validate wiring
    // -------------------------------------------------------------------------

    /// A test lowerer that constructs a Subgraph without calling populate_defaults.
    struct DummyLowererForTest;

    impl DummyLowererForTest {
        fn new() -> Self {
            Self
        }
    }

    impl super::AstLowerer for DummyLowererForTest {
        fn lower(&self, _ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError> {
            use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits};

            // Construct a GraphPlan with NO max_depth set
            let plan = GraphPlan::Subgraph {
                nodes: vec!["A".into()],
                edges: None,
                aggregations: vec![],
                limits: PlanLimits::default(), // max_depth is None
                metadata: PlanMetadata::new(
                    PlanVersion::new("1.0.0").unwrap(),
                    PlanHash::compute(&0u32),
                ),
            };

            // Wire validate: if limits are missing required bounds, return error
            plan.limits().validate(&plan)?;
            Ok(plan)
        }
    }

    /// A test lowerer that calls populate_defaults before validate.
    struct DummyLowererWithDefaults;

    impl DummyLowererWithDefaults {
        fn new() -> Self {
            Self
        }
    }

    impl super::AstLowerer for DummyLowererWithDefaults {
        fn lower(&self, _ast: &dyn std::any::Any) -> Result<GraphPlan, PlanError> {
            use super::{GraphPlan, PlanMetadata, PlanVersion, PlanHash, PlanLimits, populate_defaults, QueryShape};

            // Construct a GraphPlan with NO max_depth set
            let plan = GraphPlan::Subgraph {
                nodes: vec!["A".into()],
                edges: None,
                aggregations: vec![],
                limits: PlanLimits::default(),
                metadata: PlanMetadata::new(
                    PlanVersion::new("1.0.0").unwrap(),
                    PlanHash::compute(&0u32),
                ),
            };

            // Apply populate_defaults before validate
            let populated_limits = populate_defaults(&plan, &QueryShape::Subgraph { depth: 0 });

            // Create a new plan with the populated limits
            let plan_with_limits = GraphPlan::Subgraph {
                nodes: vec!["A".into()],
                edges: None,
                aggregations: vec![],
                limits: populated_limits,
                metadata: plan.metadata().clone(),
            };

            // Now validate should pass
            plan_with_limits.limits().validate(&plan_with_limits)?;
            Ok(plan_with_limits)
        }
    }
}
