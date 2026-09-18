//! M11 (AI Foundation, cycle e79) — read-only AI support.
//!
//! This is the **application-side** complement to `domain::ai`. It
//! provides:
//!
//! - `FakeLlmPort` — the deterministic in-memory adapter.
//! - `SemanticMiner` — the read-only orchestrator that turns an
//!   `InvestigationFrame` into `Vec<MinerOutput>`.
//! - `FindingCritic` — the read-only orchestrator that critiques
//!   existing findings.
//!
//! ## Authority boundary
//!
//! No code in this module:
//! - mutates canonical state (no `FactStore::commit`, no
//!   `EvidenceStore::append_batch`).
//! - mints authority (no `ArchitectureConstraint`, no admitted
//!   detector, no `PromotionPermit`, no `PromotionAuthorization`).
//! - applies patches, writes source, or runs Git operations.
//!
//! e80b adds [`fix_agent`]: the `FixAgent` orchestrator turns a bounded frame
//! into a validated patch artifact and a `ChangeProposal` whose author is
//! stamped `RequestedBy::LlmAgent` from configuration. Producing a proposal is
//! the terminal effect — no trial, no evaluation, no authorization, no apply.
//! [`patch_sink`] provides the deterministic in-memory `PatchArtifactSink`.
//!
//! The `FakeLlmPort` is the only `LlmPort` implementation in e79/e80b.
//! Real provider adapters (OpenAI, Anthropic, Ollama, …) are future
//! work gated by DEBT-SDDK-003.

#[cfg(test)]
pub mod boundary_tests;
pub mod critic;
pub mod fake;
pub mod patch_sink;
pub mod semantic_miner;

#[cfg(feature = "evidence-kernel")]
pub mod fix_agent;

#[cfg(all(test, feature = "evidence-kernel"))]
pub mod fix_agent_test_support;
#[cfg(all(test, feature = "evidence-kernel"))]
#[path = "fix_agent_tests.rs"]
mod fix_agent_tests;

pub use critic::{CritiqueError, FindingCritic, FindingCriticError};
pub use fake::{FakeLlmPort, ScriptKey};
#[cfg(feature = "evidence-kernel")]
pub use fix_agent::{
    FIX_AGENT_CALLER, FixAgent, FixAgentError, FixLineage, FixOutcome, build_fix_request,
};
pub use patch_sink::InMemoryPatchArtifactSink;
pub use semantic_miner::{
    MinerError, MinerOutput, SemanticMiner, build_request, candidate_for_layer_dependency,
    convert_response,
};
