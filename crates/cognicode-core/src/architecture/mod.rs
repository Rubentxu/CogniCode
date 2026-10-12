//! B4.W2 — Architecture as diagnostic (read-only surface).

pub mod diagnostic;

pub use diagnostic::{
    build_diagnostic, validate_observation, ArchitectureDiagnostic, ArchitectureEvidence,
    ArchitectureObservation, ObservationValidationError, Severity,
};