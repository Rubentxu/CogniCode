//! M11 (AI Foundation, cycle e79) — read-only AI support domain types.
//!
//! This module defines the bounded, provider-neutral vocabulary for an
//! AI investigation:
//!
//! - [`InvestigationFrame`] — bounded immutable input.
//! - [`InvestigationRequest`] / [`LlmResponse`] — the port shape.
//! - [`Hypothesis`] — the AI-output observation type.
//! - [`CritiqueDisposition`] / [`Critique`] — the critic shape.
//!
//! The application layer under `application::ai` provides:
//! - `LlmPort` (the trait).
//! - `FakeLlmPort` (deterministic in-memory adapter).
//! - `SemanticMiner`, `FindingCritic` (read-only behaviour wrappers).
//!
//! ## Authority boundary
//!
//! No type in this module mints canonical facts, gate findings, or apply
//! patches. AI output becomes a `Hypothesis` (advisory) or a `Critique`
//! (advisory). The `LlmResponse` carries the inputs that the lineage
//! layer audits; the absence of any write surface in this module is the
//! load-bearing invariant.
//!
//! e80b adds [`patch`]: a typed `SourcePatchCandidate` (a suggestion), a
//! `ValidatedSourcePatch` (a structurally safe candidate), and a
//! `PatchArtifactSink` port. None of these is a `ChangeProposal`, a source
//! mutation, evidence, or authority — the patch types carry no envelope and no
//! approval surface.

pub mod frame;
pub mod hypothesis;
pub mod patch;
pub mod port;
pub mod request;
pub mod response;

pub use frame::{
    InvestigationBudget, InvestigationFrame, InvestigationFrameError, InvestigationFrameId,
    InvestigationScope,
};
pub use hypothesis::{
    Critique, CritiqueDisposition, CritiqueError, Hypothesis, HypothesisConfidence, HypothesisError,
    HypothesisId, HypothesisRef, HypothesisStatement,
};
pub use patch::{
    PatchArtifactError, PatchArtifactSink, PatchBaseScope, PatchBudget, PatchRef,
    PatchValidationError, RelativePath, SourceEdit, SourcePatchCandidate, ValidatedSourceEdit,
    ValidatedSourcePatch, validate_relative_path, validate_source_patch,
};
pub use port::{LlmPort, LlmPortError};
pub use request::{InvestigationRequest, RequestProvenance, ToolCall};
pub use response::{LlmResponse, ResponseProvenance};
