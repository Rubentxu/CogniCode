//! Plan value objects — versioned, backend-neutral plan algebra types.
//!
//! Part of e28-1-moldplan-graphplan-contracts: PR1 Foundation Phase 1.
//!
//! ## Module structure
//!
//! - `version.rs` — `PlanVersion`, `PlanHash`, `PlanMetadata`
//! - `limits.rs`  — `PlanLimits`, `PlanLimit`
//! - `value.rs`   — `TypedValue`, `ValueError`
//! - `result.rs`  — `ResultSet`, `TruncationMarker`, `SemanticsViolation`, `Path`, `assert_equivalent`, `assert_approx_equal`
//! - `error.rs`   — `PlanError`, `ExecutorError`, `UnsupportedConstruct`, `ConstructId`, `SourceLocation`, `CancellationToken`, `ProvenanceSource`
//! - `neutrality.rs` — static backend-neutrality assertions
//! - `mold_plan.rs`  — `MoldPlan` enum
//! - `graph_plan.rs` — `GraphPlan` enum

pub mod version;
pub mod limits;
pub mod value;
pub mod result;
pub mod error;
pub mod neutrality;
pub mod mold_plan;
pub mod graph_plan;

// Re-exports for ergonomic use at the crate root.
pub use version::{PlanVersion, PlanHash, PlanMetadata, ParsePlanVersionError};
pub use limits::{PlanLimits, PlanLimit, PlanLimitsBuilder};
pub use value::{TypedValue, ValueError};
pub use result::{ResultSet, TruncationMarker, SemanticsViolation, Path, Row, NodeResult, EdgeResult, assert_equivalent, assert_approx_equal, PathHop};
pub use error::{PlanError, ExecutorError, UnsupportedConstruct, ConstructId, SourceLocation, CancellationToken, ProvenanceSource};
pub use mold_plan::{MoldPlan, PlanFilter, PlanFilterOp, PlanFilter as MoldPlanFilter};
pub use graph_plan::{GraphPlan, PathPredicate, PathQuantifier, NeighborKind, PathProjection, BooleanOp};
