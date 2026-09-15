//! Findings & Detector IR — M6 domain foundation.
//!
//! Umbrella tasks 7.1 (Finding/Risk/EvidenceClass lifecycle), 7.2 (Detector
//! IR schema/parser/validator) and 7.6 (QualityIssue projection). Hardened
//! by cycle e55 (capability/tier split, execution-authority capture,
//! namespaced-id unification, distinct status dispositions).
//!
//! This module is additive pure domain (no I/O, no `sqlx`/`tokio`),
//! mirroring the M5 `domain::analytics::program_analysis` precedent.
//!
//! ## Module map
//!
//! - [`namespaced`] — shared `namespace.name` identifier semantics.
//! - [`detector_ir`] — [`AnalysisCapability`], [`EscalationTier`],
//!   [`DetectorStep`], [`DetectorIr`], [`DetectorExecutionRef`] and the
//!   fail-loud validator.
//! - [`finding`] — [`Finding`], [`EvidenceClass`], [`RiskLevel`],
//!   [`FindingStatus`], [`FindingOrigin`], [`FindingGate`].
//! - [`quality_projection`] — lift legacy `QualityIssue` rows into
//!   [`Finding`] (conservative, non-blocking).

pub mod detector_ir;
pub mod finding;
pub mod namespaced;
pub mod quality_projection;

pub use detector_ir::{
    AnalysisCapability, DetectorAuthority, DetectorDigest, DetectorExecutionRef, DetectorId,
    DetectorIr, DetectorIrError, DetectorStep, EscalationTier, ExecutionId, FindingKind,
    SubjectPattern,
};
pub use finding::{
    CausalStep, EvidenceClass, EvidenceRef, Finding, FindingError, FindingGate, FindingId,
    FindingOrigin, FindingSeverity, FindingStatus, RiskLevel,
};
pub use namespaced::{NamespacedError, NamespacedName};
pub use quality_projection::{
    LEGACY_DETECTOR_ID, LEGACY_DETECTOR_VERSION, QUALITY_NAMESPACE, project_quality_issue,
};
