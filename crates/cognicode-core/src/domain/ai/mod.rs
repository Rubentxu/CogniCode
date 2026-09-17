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

pub mod frame;
pub mod hypothesis;
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
pub use port::{LlmPort, LlmPortError};
pub use request::{InvestigationRequest, RequestProvenance, ToolCall};
pub use response::{LlmResponse, ResponseProvenance};
