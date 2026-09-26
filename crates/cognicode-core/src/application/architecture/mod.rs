//! Application-layer facade for Executable Architecture (e77, M10 first slice).
//!
//! Exposes the admission flow (candidate → admitted constraint) and the
//! evaluator entry point. Pure orchestration: takes domain types,
//! returns domain types, and uses no I/O. The actual evaluation
//! against source files happens in WU3 (architecture::evaluator).
//!
//! See [`crate::domain::architecture`] for the model and
//! `docs/analysis/e77-architecture-ownership-map.md` for the ownership
//! map.

pub mod admission;
pub mod canonical_constraints;
pub mod control_query;
pub mod cr06_allowlist;
pub mod evaluator;
pub mod grounding;
pub mod registry;

pub use admission::{
    AdmissionError, AdmissionOutcome, ArchitectureAdmissionService, ArchitectureClock,
    SystemArchitectureClock,
};
pub use canonical_constraints::{canonical_constraints, canonical_promoted_admitter};
pub use control_query::{
    ArchitectureReadModel, ConstraintRef, ControlQueryService, EvaluationStatus, ViolationRef,
};
pub use evaluator::{
    ArchitectureEvaluator, ArchitectureEvaluatorError, ArchitectureSource, EvaluationReport,
    SourceFile,
};
pub use grounding::ArchitectureGroundingBridge;
pub use registry::{ArchitectureRegistry, ArchitectureRegistryError};
