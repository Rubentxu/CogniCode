//! Plan value objects — versioned, backend-neutral plan algebra types.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1 + PR2 Plan Algebra.
//!
//! ## Module structure
//!
//! - `version.rs` — `PlanVersion`, `PlanHash`, `PlanMetadata`
//! - `limits.rs`  — `PlanLimits`, `PlanLimit`
//! - `value.rs`   — `TypedValue`, `ValueError`
//! - `result.rs`  — `ResultSet`, `TruncationMarker`, `SemanticsViolation`, `Path`, `assert_equivalent`, `assert_approx_equal`
//! - `error.rs`   — `PlanError`, `ExecutorError`, `UnsupportedConstruct`, `ConstructId`, `SourceLocation`, `CancellationToken`, `ProvenanceSource`
//! - `neutrality.rs` — static backend-neutrality assertions
//! - `filter.rs`  — `PlanFilter`, `PlanFilterOp`
//! - `mold_plan.rs`  — `MoldPlan` enum
//! - `graph_plan.rs` — `GraphPlan` enum
//! - `lower.rs`   — `AstLowerer` trait + `NoOpLowerer` (port for AST→Plan lowering; adapter in `cognicode-explorer`)

pub mod error;
pub mod executor;
pub mod filter;
pub mod graph_plan;
pub mod limits;
pub mod lower;
pub mod mold_plan;
pub mod neutrality;
pub mod result;
pub mod value;
pub mod version;

// Re-exports for ergonomic use at the crate root.
pub use error::{
    CancellationToken, ConstructId, ExecutorError, PlanError, ProvenanceSource, SourceLocation,
    UnsupportedConstruct,
};
pub use executor::{GraphExecutor, ProvenanceEnvelope, StubExecutor};
pub use filter::{PlanFilter, PlanFilterOp};
pub use graph_plan::{
    BooleanOp, GraphPlan, NeighborKind, OrderClause, OrderDirection, PathPredicate, PathProjection,
    PathQuantifier,
};
pub use limits::{PLAN_LIMIT_KINDS, PlanLimit, PlanLimitKind, PlanLimits, PlanLimitsBuilder};
pub use mold_plan::MoldPlan;
pub use result::{
    EdgeResult, NodeResult, Path, PathHop, ResultSet, Row, SemanticsViolation, TruncationMarker,
    assert_approx_equal, assert_equivalent,
};
pub use value::{TypedValue, ValueError};
pub use version::{ParsePlanVersionError, PlanHash, PlanMetadata, PlanVersion};
