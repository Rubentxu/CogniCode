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
pub mod evaluator;
pub mod registry;

pub use admission::{
    AdmissionError, AdmissionOutcome, ArchitectureAdmissionService, ArchitectureClock,
    SystemArchitectureClock,
};
pub use evaluator::{
    ArchitectureEvaluator, ArchitectureEvaluatorError, ArchitectureSource, EvaluationReport,
    SourceFile,
};
pub use registry::{ArchitectureRegistry, ArchitectureRegistryError};
