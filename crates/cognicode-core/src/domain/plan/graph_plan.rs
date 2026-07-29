//! GraphPlan — versioned, backend-neutral discriminated union for graph-selecting operations.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! `GraphPlan` is the discriminator for all graph-selecting MoldQL operations:
//! path finding, neighbor traversal, subgraph extraction, and clustering.
//! Each variant is fully self-describing with typed predicate and projection
//! payloads. No SQL, Petgraph, or tokio types appear in this enum.

use serde::{Deserialize, Serialize};
use std::fmt;

// Types from sibling modules.
use super::limits::{PlanLimits, PlanLimit};
use super::value::TypedValue;
use super::version::{PlanHash, PlanMetadata, PlanVersion};
use crate::domain::value_objects::DependencyType;

// Sealed trait — implemented by all plan types to certify backend-neutrality.
use super::neutrality::Sealed;

/// Boolean operator for combining graph sub-plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BooleanOp {
    /// Logical AND of operands.
    And,
    /// Logical OR of operands.
    Or,
    /// Logical NOT of the single operand.
    Not,
}

impl fmt::Display for BooleanOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BooleanOp::And => write!(f, "AND"),
            BooleanOp::Or => write!(f, "OR"),
            BooleanOp::Not => write!(f, "NOT"),
        }
    }
}

impl Sealed for BooleanOp {}

/// Discriminated union for all graph-selecting MoldQL operations.
///
/// `GraphPlan` is always wrapped inside `MoldPlan::Graph(_)`. It carries
/// the concrete graph operation payload with all required bounds (no
/// unbounded quantifiers allowed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphPlan {
    /// Shortest/widest path between two nodes with a bounded hop count.
    Path {
        src: String,
        dst: String,
        quantifier: PathQuantifier,
        /// Optional filter restricting traversal to edges of the listed
        /// `DependencyType`s (e.g. `[Calls]` to exclude `References`,
        /// `Imports`, etc.). When `None`, every edge type is traversed
        /// (preserves the pre-fix behavior; see `e28-2-pr5-edge-filter`).
        edge_kind_filter: Option<Vec<DependencyType>>,
        predicates: Vec<super::PathPredicate>,
        projection: PathProjection,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// Neighbors of a source node at a given depth.
    Neighbors {
        src: String,
        kind: NeighborKind,
        depth: u32,
        /// Optional filter restricting which edges contribute neighbors.
        /// `None` means "any edge kind" (pre-fix behavior).
        edge_kind_filter: Option<Vec<DependencyType>>,
        predicates: Vec<super::PathPredicate>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// Extract a subgraph from a set of nodes and optional edge filter.
    Subgraph {
        nodes: Vec<String>,
        edges: Option<Vec<super::EdgeResult>>,
        aggregations: Vec<super::TypedValue>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// Cluster nodes by a grouping key with optional aggregations.
    Cluster {
        by: Vec<String>,
        aggregations: Vec<super::TypedValue>,
        /// Optional ordering for rows (COUNT, ORDER BY, LIMIT).
        #[serde(default)]
        ordering: Option<OrderClause>,
        /// Optional result limit.
        #[serde(default)]
        limit: Option<usize>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// EXPLAIN wrapper — returns plan metadata without executing.
    Explain {
        inner: Box<GraphPlan>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// Boolean composition of sub-plans (AND, OR, NOT).
    BooleanComposition {
        /// The boolean operator combining the operands.
        op: BooleanOp,
        /// The sub-plans to combine. NOT has exactly 1 operand; AND/OR have 2+.
        operands: Vec<GraphPlan>,
        /// Limits applied to the composition result.
        limits: PlanLimits,
        /// Metadata for the composed plan.
        metadata: PlanMetadata,
    },
}

impl Sealed for GraphPlan {}

/// Quantifier for path queries — always bounded.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PathQuantifier {
    /// Maximum number of hops. Must be `Some` — unbounded paths are rejected.
    pub max_hops: Option<u32>,
    /// Minimum number of hops (default 0).
    pub min_hops: u32,
}

impl PathQuantifier {
    /// Construct a bounded quantifier. Returns `None` if `max_hops` is `None`.
    pub fn new(max_hops: Option<u32>, min_hops: u32) -> Option<Self> {
        if max_hops.is_none() {
            // Unbounded quantifier is rejected — must have a bound.
            return None;
        }
        Some(Self { max_hops, min_hops })
    }

    /// Returns `true` if the quantifier has a finite bound.
    pub fn is_bounded(&self) -> bool {
        self.max_hops.is_some()
    }
}

impl Sealed for PathQuantifier {}

/// Projection of nodes and edges for a path result.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct PathProjection {
    /// Node properties to include.
    pub nodes: Vec<String>,
    /// Edge properties to include.
    pub edges: Vec<String>,
    /// When `true`, selects the minimum-hop qualifying path (bounded shortest path).
    #[serde(default)]
    pub shortest: bool,
}

impl Sealed for PathProjection {}

/// Ordering direction for result sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderDirection {
    Asc,
    Desc,
}

/// A result-ordering clause: `ORDER BY <field> <direction>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderClause {
    pub by: String,
    pub direction: OrderDirection,
}

impl Default for OrderClause {
    fn default() -> Self {
        Self {
            by: String::new(),
            direction: OrderDirection::Desc,
        }
    }
}

/// Kind of neighbor traversal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NeighborKind {
    /// Both incoming and outgoing edges.
    Both,
    /// Only outgoing edges.
    Outgoing,
    /// Only incoming edges.
    Incoming,
}

impl Default for NeighborKind {
    fn default() -> Self {
        Self::Both
    }
}

impl Sealed for NeighborKind {}

impl GraphPlan {
    /// Returns a reference to the plan metadata.
    pub fn metadata(&self) -> &PlanMetadata {
        match self {
            GraphPlan::Path { metadata, .. }
            | GraphPlan::Neighbors { metadata, .. }
            | GraphPlan::Subgraph { metadata, .. }
            | GraphPlan::Cluster { metadata, .. }
            | GraphPlan::Explain { metadata, .. }
            | GraphPlan::BooleanComposition { metadata, .. } => metadata,
        }
    }

    /// Returns a reference to the plan limits.
    pub fn limits(&self) -> &PlanLimits {
        match self {
            GraphPlan::Path { limits, .. }
            | GraphPlan::Neighbors { limits, .. }
            | GraphPlan::Subgraph { limits, .. }
            | GraphPlan::Cluster { limits, .. }
            | GraphPlan::Explain { limits, .. }
            | GraphPlan::BooleanComposition { limits, .. } => limits,
        }
    }

    /// Returns the plan version string.
    pub fn version(&self) -> &str {
        self.metadata().version_str()
    }

    /// Returns the plan hash hex string.
    pub fn hash(&self) -> &str {
        self.metadata().hash_str()
    }
}

impl fmt::Display for GraphPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphPlan::Path { src, dst, quantifier, .. } => {
                write!(f, "Path({src} → {dst}, max_hops={:?})", quantifier.max_hops)
            }
            GraphPlan::Neighbors { src, depth, kind, .. } => {
                write!(f, "Neighbors({src}, {kind:?}, depth={depth})")
            }
            GraphPlan::Subgraph { nodes, .. } => {
                write!(f, "Subgraph(nodes={:?})", nodes)
            }
            GraphPlan::Cluster { by, .. } => {
                write!(f, "Cluster(by={by:?})")
            }
            GraphPlan::Explain { inner, .. } => {
                write!(f, "Explain({inner})")
            }
            GraphPlan::BooleanComposition { op, operands, .. } => {
                write!(f, "BooleanComposition({op}, {} operands)", operands.len())
            }
        }
    }
}

// PathPredicate lives here for now (moved from filter.rs placeholder)
/// A predicate applied to edges or nodes during graph traversal.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PathPredicate {
    pub label: String,
    pub value: super::TypedValue,
}

impl Sealed for PathPredicate {}

// Re-export path_predicate at module level for MoldPlan references
pub use PathPredicate as GraphPathPredicate;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.8a RED — GraphPlan enum (Path/Neighbors/Subgraph/Cluster/Explain)
    // Scenario: `moldplan-graphplan::GraphPlan Bounded Traversal` (both) +
    //           GraphPlan variants serialize round-trip
    // Assert: `ShortestPath::new(..., max_hops: None)` → `Err(MissingBound)`;
    //         BooleanComposition wraps sub-plans preserving operands
    // -------------------------------------------------------------------------

    /// `PathQuantifier::new(Some(3), 0)` returns a bounded quantifier.
    #[test]
    fn path_quantifier_bounded() {
        let q = PathQuantifier::new(Some(3), 0).unwrap();
        assert!(q.is_bounded());
        assert_eq!(q.max_hops, Some(3));
        assert_eq!(q.min_hops, 0);
    }

    /// `PathQuantifier::new(None, 0)` returns `None` (unbounded rejected).
    #[test]
    fn path_quantifier_unbounded_rejected() {
        let q = PathQuantifier::new(None, 0);
        assert!(q.is_none(), "unbounded quantifier must be rejected");
    }

    /// `NeighborKind::default()` is `Both`.
    #[test]
    fn neighbor_kind_default() {
        assert_eq!(NeighborKind::default(), NeighborKind::Both);
    }

    /// `GraphPlan::Path` round-trips through serde.
    #[test]
    fn graph_plan_path_roundtrip() {
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
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::Neighbors` round-trips through serde.
    #[test]
    fn graph_plan_neighbors_roundtrip() {
        let plan = GraphPlan::Neighbors {
            src: "node1".into(),
            kind: NeighborKind::Outgoing,
            depth: 2,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::builder().max_depth(2).build(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::Subgraph` round-trips through serde.
    #[test]
    fn graph_plan_subgraph_roundtrip() {
        let plan = GraphPlan::Subgraph {
            nodes: vec!["n1".into(), "n2".into()],
            edges: None,
            aggregations: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::Cluster` round-trips through serde.
    #[test]
    fn graph_plan_cluster_roundtrip() {
        let plan = GraphPlan::Cluster {
            by: vec!["kind".into()],
            aggregations: vec![super::super::TypedValue::Int(1)],
            ordering: Some(OrderClause {
                by: "count".into(),
                direction: OrderDirection::Desc,
            }),
            limit: Some(5),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::Explain` wraps an inner plan.
    #[test]
    fn graph_plan_explain_roundtrip() {
        let inner = GraphPlan::Neighbors {
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
        let plan = GraphPlan::Explain {
            inner: Box::new(inner),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::metadata()` returns the plan metadata.
    #[test]
    fn graph_plan_metadata() {
        let version = PlanVersion::new("1.0.0").unwrap();
        let hash = PlanHash::compute(&0u32);
        let metadata = PlanMetadata::new(version.clone(), hash.clone());
        let plan = GraphPlan::Neighbors {
            src: "A".into(),
            kind: NeighborKind::Both,
            depth: 1,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: metadata.clone(),
        };
        assert_eq!(plan.metadata().version_str(), "1.0.0");
        assert_eq!(plan.metadata().hash_str(), hash.as_str());
    }

    /// `GraphPlan::Display` includes variant name and key fields.
    #[test]
    fn graph_plan_display() {
        let plan = GraphPlan::Path {
            src: "X".into(),
            dst: "Y".into(),
            quantifier: PathQuantifier { max_hops: Some(5), min_hops: 0 },
            edge_kind_filter: None,
            predicates: vec![],
            projection: PathProjection::default(),
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let display = plan.to_string();
        assert!(display.contains("Path"));
        assert!(display.contains("X"));
        assert!(display.contains("Y"));
        assert!(display.contains("max_hops"));
    }

    /// `PathProjection` is `Default` with `shortest: false`.
    #[test]
    fn path_projection_default() {
        let proj = PathProjection::default();
        assert!(proj.nodes.is_empty());
        assert!(proj.edges.is_empty());
        assert!(!proj.shortest);
    }

    /// `PathProjection` round-trips with `shortest: true`.
    #[test]
    fn path_projection_shortest_roundtrip() {
        let proj = PathProjection {
            nodes: vec!["a".into(), "b".into()],
            edges: vec!["c".into()],
            shortest: true,
        };
        let json = serde_json::to_string(&proj).expect("serialize");
        let parsed: PathProjection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, proj);
        assert!(parsed.shortest);
    }

    /// `GraphPlan` is `Send + Sync + 'static`.
    #[test]
    fn graph_plan_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<GraphPlan>();
        assert_sync::<GraphPlan>();
        assert_static::<GraphPlan>();
    }

    /// `PathPredicate` serde round-trip.
    #[test]
    fn path_predicate_roundtrip() {
        let pred = PathPredicate {
            label: "kind".into(),
            value: super::super::TypedValue::String("function".into()),
        };
        let json = serde_json::to_string(&pred).expect("serialize");
        let parsed: PathPredicate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, pred);
    }

    // -------------------------------------------------------------------------
    // Task 2.1a RED — GraphPlan BooleanComposition variant
    // Scenario: `moldplan-graphplan::GraphPlan Bounded Traversal`
    // Assert: BooleanComposition wraps sub-plans preserving operands;
    //         serde round-trip preserves variant + payload
    // -------------------------------------------------------------------------

    /// `GraphPlan::BooleanComposition` wraps sub-plans with a boolean operator.
    #[test]
    fn graph_plan_boolean_composition_roundtrip() {
        let sub_a = GraphPlan::Neighbors {
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
        let sub_b = GraphPlan::Neighbors {
            src: "B".into(),
            kind: NeighborKind::Outgoing,
            depth: 2,
            edge_kind_filter: None,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::And,
            operands: vec![sub_a.clone(), sub_b.clone()],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
        // Verify operands are preserved
        if let GraphPlan::BooleanComposition { operands: ops, .. } = parsed {
            assert_eq!(ops.len(), 2);
        } else {
            panic!("expected BooleanComposition variant");
        }
    }

    /// `GraphPlan::BooleanComposition` with NOT operator (single operand).
    #[test]
    fn graph_plan_boolean_composition_not() {
        let sub = GraphPlan::Neighbors {
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
        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::Not,
            operands: vec![sub],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: GraphPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `GraphPlan::BooleanComposition::And` Display includes operand count.
    #[test]
    fn graph_plan_boolean_composition_display() {
        let plan = GraphPlan::BooleanComposition {
            op: BooleanOp::Or,
            operands: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let display = plan.to_string();
        assert!(display.contains("BooleanComposition"));
        assert!(display.contains("OR"));
    }

    /// `BooleanOp` is `Send + Sync + 'static`.
    #[test]
    fn boolean_op_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<BooleanOp>();
        assert_sync::<BooleanOp>();
        assert_static::<BooleanOp>();
    }
}
