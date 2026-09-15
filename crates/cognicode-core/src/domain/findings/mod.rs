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

pub mod admission;
pub mod assembler;
pub mod ast_backend;
pub mod dataflow_input;
pub mod detector_ir;
pub mod digest;
pub mod execution;
pub mod finding;
pub mod graph_backend;
pub mod namespaced;
pub mod outcome;
pub mod ports;
pub mod quality_projection;
pub mod verifier;

pub use crate::domain::kernel_ids::{EntityId, EvidenceId, FactId};
pub use admission::{
    AdmissionError, AdmissionRef, AdmissionSource, AdmittedDetector, AdmittedDetectorRecord,
    ApprovalVerifier, DetectorAdmission, DetectorVersion, EligibleSourceVerifier, ExecutionPermit,
    PromotionAuthority, PromotionRequest, PromotionTarget, RejectAllApprovals, VerifiedPromotion,
};
pub use assembler::{AssemblyError, FindingAssembler};
pub use ast_backend::{AstBackend, AstConstruct, AstInput, AstUnit};
pub use dataflow_input::{DataflowFunction, DataflowInput, DataflowLocation, DataflowStatement};
pub use detector_ir::{
    AnalysisCapability, DetectorAuthority, DetectorDigests, DetectorExecutionRef,
    DetectorFindingPolicy, DetectorId, DetectorIr, DetectorIrError, DetectorStep, EscalationTier,
    FindingKind, SubjectPattern,
};
pub use digest::{DetectorDigest, DigestError, sha256_hex};
pub use execution::{
    AnalysisInput, BackendContractViolation, BackendError, BackendRegistry, DetectorBackend,
    DetectorExecutor, ExecutionError, ExecutionRecord, PlanError,
};
pub use finding::{
    CausalStep, CausalStepKind, EvidenceClass, Finding, FindingError, FindingGate, FindingId,
    FindingOrigin, FindingSeverity, FindingStatus, RiskLevel,
};
pub use graph_backend::{GraphBackend, GraphEdge, GraphInput, GraphNode};
pub use namespaced::{NamespacedError, NamespacedName};
pub use outcome::{
    CausalObservation, DetectorDiagnostic, DetectorMatch, DetectorOutcome, EvidenceKind,
    ProducedEvidence,
};
pub use ports::{EvidenceError, EvidenceLookup, EvidenceSink};
pub use quality_projection::{
    LEGACY_DETECTOR_ID, LEGACY_DETECTOR_VERSION, QUALITY_NAMESPACE, project_quality_issue,
};
pub use verifier::{FindingVerifier, VerificationError};
