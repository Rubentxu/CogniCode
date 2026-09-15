//! Findings & Detector IR — M6 domain foundation.
//!
//! Umbrella task 7.2: *Define Detector IR schema/parser/validator.*
//!
//! This module is additive pure domain (no I/O, no `sqlx`/`tokio`),
//! mirroring the M5 `domain::analytics::program_analysis` precedent. It
//! defines the declarative Detector IR that later M6 cycles compile into
//! AST/semantic/graph/dataflow/symbolic backends.
//!
//! See `openspec/changes/e52-lsi-detector-ir/design.md` for the contract.
//!
//! ## Module map
//!
//! - [`detector_ir`] — [`AnalysisLevel`], [`DetectorStep`], [`DetectorIr`],
//!   [`DetectorAuthority`] and the fail-loud validator.
//! - [`finding`] — [`Finding`], [`EvidenceClass`], [`RiskLevel`],
//!   [`FindingGate`] and the evidence-backed conclusion model.
//! - [`quality_projection`] — lift legacy `QualityIssue` rows into
//!   [`Finding`] (conservative, non-blocking).

pub mod detector_ir;
pub mod finding;
pub mod quality_projection;

pub use detector_ir::{
    AnalysisLevel, DetectorAuthority, DetectorId, DetectorIr, DetectorIrError, DetectorStep,
    FindingKind, SubjectPattern,
};
pub use finding::{
    CausalStep, DetectorRef, EvidenceClass, EvidenceRef, Finding, FindingError, FindingGate,
    FindingId, FindingSeverity, FindingStatus, RiskLevel,
};
pub use quality_projection::project_quality_issue;
