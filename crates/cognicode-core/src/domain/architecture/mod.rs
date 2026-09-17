//! Executable Architecture (e77, M10 first slice).
//!
//! The architecture of CogniCode's own `cognicode-core` crate lives here
//! as **typed constraints**, not as prose. A constraint can be in one of
//! two states:
//!
//! ```text
//! ConstraintCandidate  ──admit──►  ArchitectureConstraint
//! ```
//!
//! The fundamental rule (also codified in `docs/analysis/e77-architecture-ownership-map.md`):
//!
//! > An ADR by itself has ZERO execution/gating authority.
//! > Only an admitted `ArchitectureConstraint` may produce drift findings.
//!
//! This module is pure domain (no I/O, no `sqlx`, no `tokio`). The three
//! concrete rule families implemented here are:
//!
//! 1. [`LayerDependencyRule`] — a layer must not depend on a forbidden
//!    other layer (e.g. `domain` must not depend on `infrastructure`).
//! 2. [`ForbiddenDependencyRule`] — a layer must not import a given
//!    module path or crate name.
//! 3. [`NamespaceBoundaryRule`] — a namespace must not reach a forbidden
//!    other namespace (e.g. `evidence_kernel` must not reach
//!    `presentation`).
//!
//! Adding a fourth rule requires evidence (a real drift the three rules
//! above cannot express) and goes through the same admission flow.

pub mod constraint;
pub mod use_parser;
pub mod violation;

// Re-export the public surface explicitly. Callers should not reach into
// submodules.
pub use constraint::{
    Admitter, AdmitterRole, ArchitectureConstraint, ArchitectureConstraintId,
    ArchitectureConstraintKind, ArchitectureEvidence, ConstraintAdmission,
    ConstraintAdmissionDisposition, ConstraintCandidate, ConstraintError,
    ForbiddenDependencyRule, LayerDependencyRule, LayerId, NamespaceBoundaryRule,
};
pub use use_parser::{parse_use_lines, UseStatement, UseStatementError};
pub use violation::{ArchitectureViolation, ViolationId};
