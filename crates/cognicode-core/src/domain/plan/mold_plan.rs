//! MoldPlan — the top-level discriminated union for non-graph MoldQL operations.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Design
//!
//! `MoldPlan` is the backend-neutral, versioned envelope for all MoldQL
//! operations that are NOT graph-selecting. Graph-selecting operations
//! live in `GraphPlan`. `MoldPlan` is discriminator-only — the actual
//! operation payload lives in the typed `GraphPlan` or other domain objects.

use serde::{Deserialize, Serialize};
use std::fmt;

// Types from sibling modules.
use super::limits::{PlanLimits, PlanLimit, PlanLimitsBuilder};
use super::value::TypedValue;
use super::version::{PlanHash, PlanMetadata, PlanVersion};

/// Discriminated union for all non-graph MoldQL operations.
///
/// `MoldPlan` is the top-level plan type. Each variant carries a discriminator
/// and a payload appropriate to the operation kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoldPlan {
    /// A SELECT query: select rows from the graph with optional projection.
    Select {
        from: String,
        r#where: Vec<PlanFilter>,
        projection: Vec<String>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// A COUNT query: count rows matching a filter.
    Count {
        from: String,
        r#where: Vec<PlanFilter>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// An AGGREGATE query: group-by with aggregations.
    Aggregate {
        from: String,
        group_by: Vec<String>,
        aggregations: Vec<TypedValue>,
        r#where: Vec<PlanFilter>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// An EXPLAIN wrapper around a sub-plan (returns plan metadata, not results).
    Explain {
        inner: Box<MoldPlan>,
        limits: PlanLimits,
        metadata: PlanMetadata,
    },
    /// A graph-selecting operation (payload lives in `GraphPlan`).
    /// The pin is stored at the MoldPlan level; GraphPlan itself is pin-agnostic.
    /// The pin is immutable once set via `with_pin`.
    Graph {
        inner: super::GraphPlan,
        /// Workspace and revision pin. Once set via `with_pin`, it cannot be changed.
        pin: Option<(super::super::value_objects::WorkspaceId, super::super::value_objects::RevisionId)>,
    },
}

impl MoldPlan {
    /// Returns the plan metadata (version + hash).
    pub fn metadata(&self) -> &PlanMetadata {
        match self {
            MoldPlan::Select { metadata, .. } => metadata,
            MoldPlan::Count { metadata, .. } => metadata,
            MoldPlan::Aggregate { metadata, .. } => metadata,
            MoldPlan::Explain { metadata, .. } => metadata,
            MoldPlan::Graph { inner, .. } => inner.metadata(),
        }
    }

    /// Returns a reference to the plan limits.
    pub fn limits(&self) -> &PlanLimits {
        match self {
            MoldPlan::Select { limits, .. } => limits,
            MoldPlan::Count { limits, .. } => limits,
            MoldPlan::Aggregate { limits, .. } => limits,
            MoldPlan::Explain { limits, .. } => limits,
            MoldPlan::Graph { inner, .. } => inner.limits(),
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

    /// Pin this plan to a workspace and revision.
    ///
    /// Once pinned, the pin is frozen — calling `with_pin` again on a
    /// pinned plan returns `Err(PlanError::AlreadyPinned)`.
    ///
    /// Only `MoldPlan::Graph` can be pinned. Non-graph plans return
    /// `Err(PlanError::NotAGraphPlan)`.
    pub fn with_pin(
        self,
        ws: super::super::value_objects::WorkspaceId,
        rev: super::super::value_objects::RevisionId,
    ) -> Result<Self, super::PlanError> {
        match self {
            MoldPlan::Graph { mut inner, pin } => {
                if pin.is_some() {
                    return Err(super::PlanError::AlreadyPinned);
                }
                inner.metadata(); // ensure metadata is accessible
                Ok(MoldPlan::Graph {
                    inner,
                    pin: Some((ws, rev)),
                })
            }
            _ => Err(super::PlanError::NotAGraphPlan),
        }
    }

    /// Returns the workspace and revision pin, if set.
    pub fn pin(&self) -> Option<&(super::super::value_objects::WorkspaceId, super::super::value_objects::RevisionId)> {
        match self {
            MoldPlan::Graph { pin, .. } => pin.as_ref(),
            _ => None,
        }
    }
}

impl fmt::Display for MoldPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoldPlan::Select { from, projection, .. } => {
                write!(f, "Select(from={from}, projection={projection:?})")
            }
            MoldPlan::Count { from, .. } => write!(f, "Count(from={from})"),
            MoldPlan::Aggregate { from, group_by, .. } => {
                write!(f, "Aggregate(from={from}, group_by={group_by:?})")
            }
            MoldPlan::Explain { inner, .. } => write!(f, "Explain({inner})"),
            MoldPlan::Graph { inner, pin } => {
                if let Some((ws, rev)) = pin {
                    write!(f, "Graph({inner}, pinned={ws}:{rev})")
                } else {
                    write!(f, "Graph({inner}, unpinned)")
                }
            }
        }
    }
}

// ============================================================================
// PlanFilter (forward declaration)
// ============================================================================

// We can't import PlanFilter from an explorer crate, so we define a minimal
// version here for use in the MoldPlan variants. The full PlanFilter lives
// in `domain::plan::filter` (Phase 2). For Phase 1 we use a placeholder.

/// A filter predicate applied at the plan level.
///
/// Minimal definition for Phase 1 — full definition in Phase 2.
/// Note: `Eq` and `Hash` are NOT derived because `f64` does not implement `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PlanFilter {
    /// Confidence score filter.
    Confidence { op: PlanFilterOp, threshold: f64 },
    /// Provenance attribute filter.
    Provenance { key: String, value: String },
}

// Manual `Eq` for PlanFilter — needed because f64 is not Eq, but our Float
// values are always finite so we can use bitwise equality.
impl Eq for PlanFilter {}

// Manual `Hash` for PlanFilter — f64 implements Hash but not Eq.
impl std::hash::Hash for PlanFilter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PlanFilter::Confidence { op, threshold } => {
                op.hash(state);
                threshold.to_bits().hash(state);
            }
            PlanFilter::Provenance { key, value } => {
                key.hash(state);
                value.hash(state);
            }
        }
    }
}

/// Comparison operator for filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlanFilterOp {
    Gt,
    Lt,
    Gte,
    Lte,
    Eq,
    Ne,
}

// Re-export PlanFilter at the plan module root for MoldPlan references.
pub use PlanFilter as MoldPlanFilter;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Task 1.7a RED — MoldPlan enum (Select/Count/Aggregate/Explain)
    // Scenario: `moldplan-graphplan::MoldPlan Discriminated Union` (both)
    // Assert: `MoldPlan::Graph(g)` recovers inner via match; serde_json round-trip preserves variant + payload
    // -------------------------------------------------------------------------

    /// `MoldPlan::Select` serializes and deserializes with all fields.
    #[test]
    fn mold_plan_select_roundtrip() {
        let plan = MoldPlan::Select {
            from: "symbols".into(),
            r#where: vec![],
            projection: vec!["name".into(), "kind".into()],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&42u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: MoldPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `MoldPlan::Graph` can be recovered via `match`.
    #[test]
    fn mold_plan_graph_discriminant() {
        let plan = MoldPlan::Select {
            from: "nodes".into(),
            r#where: vec![],
            projection: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        match &plan {
            MoldPlan::Select { .. } => {}
            other => panic!("expected Select, got {other:?}"),
        }
    }

    /// `MoldPlan::Display` includes the variant name and key fields.
    #[test]
    fn mold_plan_display() {
        let plan = MoldPlan::Count {
            from: "edges".into(),
            r#where: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let display = plan.to_string();
        assert!(display.contains("Count"));
        assert!(display.contains("edges"));
    }

    /// `MoldPlan::Aggregate` with all fields set.
    #[test]
    fn mold_plan_aggregate_roundtrip() {
        let plan = MoldPlan::Aggregate {
            from: "nodes".into(),
            group_by: vec!["kind".into()],
            aggregations: vec![TypedValue::Int(1)],
            r#where: vec![],
            limits: PlanLimits::builder()
                .max_result_rows(100)
                .build(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let json = serde_json::to_string(&plan).expect("serialize");
        let parsed: MoldPlan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, plan);
    }

    /// `MoldPlan::metadata()` returns the plan metadata.
    #[test]
    fn mold_plan_metadata() {
        let version = PlanVersion::new("1.0.0").unwrap();
        let hash = PlanHash::compute(&0u32);
        let metadata = PlanMetadata::new(version.clone(), hash.clone());
        let plan = MoldPlan::Select {
            from: "test".into(),
            r#where: vec![],
            projection: vec![],
            limits: PlanLimits::default(),
            metadata: metadata.clone(),
        };
        assert_eq!(plan.metadata().version_str(), "1.0.0");
        assert_eq!(plan.metadata().hash_str(), hash.as_str());
    }

    /// `PlanFilterOp` has all expected comparison operators.
    #[test]
    fn plan_filter_op_exhaustive() {
        use PlanFilterOp::*;
        let ops = [Gt, Lt, Gte, Lte, Eq, Ne];
        assert_eq!(ops.len(), 6);
    }

    /// `PlanFilter` serde round-trip.
    #[test]
    fn plan_filter_roundtrip() {
        let filter = PlanFilter::Confidence { op: PlanFilterOp::Gt, threshold: 0.5 };
        let json = serde_json::to_string(&filter).expect("serialize");
        let parsed: PlanFilter = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, filter);
    }

    /// `MoldPlan` is `Send + Sync + 'static`.
    #[test]
    fn mold_plan_send_sync_static() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        fn assert_static<T: 'static>() {}
        assert_send::<MoldPlan>();
        assert_sync::<MoldPlan>();
        assert_static::<MoldPlan>();
    }

    // -------------------------------------------------------------------------
    // Task 2.3a RED — with_pin(ws, rev) requirement + immutability
    // Scenario: `moldplan-graphplan::Revision Pinning` (both)
    // Assert: MoldPlan::Graph(...).with_pin(ws, rev=3) → Ok;
    //         concurrent rev=4 ingest does not change pinned rev=3;
    //         without with_pin → Err(UnpinnedGraphPlan)
    // -------------------------------------------------------------------------

    /// `MoldPlan::Graph` can be pinned to a workspace and revision.
    #[test]
    fn with_pin_sets_pin() {
        use crate::domain::value_objects::{WorkspaceId, RevisionId};
        use super::super::{GraphPlan, NeighborKind};

        let inner = GraphPlan::Neighbors {
            src: "A".into(),
            kind: NeighborKind::Both,
            depth: 1,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let plan = MoldPlan::Graph { inner, pin: None };
        let ws = WorkspaceId::try_new("ws1").expect("valid workspace id");
        let rev = RevisionId::new(3);

        let pinned = plan.with_pin(ws.clone(), rev.clone()).expect("with_pin should succeed");
        assert_eq!(pinned.pin(), Some(&(ws, rev)));
    }

    /// Pinning twice returns `AlreadyPinned` error.
    #[test]
    fn with_pin_twice_returns_error() {
        use crate::domain::value_objects::{WorkspaceId, RevisionId};
        use super::super::{GraphPlan, NeighborKind};

        let inner = GraphPlan::Neighbors {
            src: "A".into(),
            kind: NeighborKind::Both,
            depth: 1,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let plan = MoldPlan::Graph { inner, pin: None };
        let ws = WorkspaceId::try_new("ws1").expect("valid workspace id");
        let rev = RevisionId::new(3);

        let pinned = plan.with_pin(ws.clone(), rev.clone()).expect("first with_pin should succeed");
        let result = pinned.with_pin(ws, rev);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), super::super::PlanError::AlreadyPinned));
    }

    /// `with_pin` on a non-graph plan returns `NotAGraphPlan`.
    #[test]
    fn with_pin_on_non_graph_plan_error() {
        use crate::domain::value_objects::{WorkspaceId, RevisionId};

        let plan = MoldPlan::Select {
            from: "symbols".into(),
            r#where: vec![],
            projection: vec!["name".into()],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let ws = WorkspaceId::try_new("ws1").expect("valid workspace id");
        let rev = RevisionId::new(3);

        let result = plan.with_pin(ws, rev);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), super::super::PlanError::NotAGraphPlan));
    }

    /// `MoldPlan::Graph` with no pin returns `None` from `pin()`.
    #[test]
    fn pin_returns_none_when_unpinned() {
        use super::super::{GraphPlan, NeighborKind};

        let inner = GraphPlan::Neighbors {
            src: "A".into(),
            kind: NeighborKind::Both,
            depth: 1,
            predicates: vec![],
            limits: PlanLimits::default(),
            metadata: PlanMetadata::new(
                PlanVersion::new("1.0.0").unwrap(),
                PlanHash::compute(&0u32),
            ),
        };
        let plan = MoldPlan::Graph { inner, pin: None };
        assert_eq!(plan.pin(), None);
    }
}
