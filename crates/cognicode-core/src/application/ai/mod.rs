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
//!   detector, no `PromotionPermit`).
//! - applies patches (no `SourcePatch`, no `ChangeProposal`).
//!
//! The `FakeLlmPort` is the only `LlmPort` implementation in e79.
//! Real provider adapters (OpenAI, Anthropic, Ollama, …) are future
//! work gated by DEBT-SDDK-003.

pub mod boundary_tests;
pub mod critic;
pub mod fake;
pub mod semantic_miner;

pub use critic::{CritiqueError, FindingCritic, FindingCriticError};
pub use fake::{FakeLlmPort, ScriptKey};
pub use semantic_miner::{
    build_request, candidate_for_layer_dependency, convert_response, MinerError, MinerOutput,
    SemanticMiner,
};
