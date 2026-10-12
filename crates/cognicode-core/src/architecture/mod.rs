//! B4.W2 — Architecture as diagnostic (read-only surface).

pub mod diagnostic;

pub use diagnostic::{
    ArchitectureDiagnostic, ArchitectureEvidence, ArchitectureObservation,
    ObservationValidationError, Severity, build_diagnostic, validate_observation,
};
