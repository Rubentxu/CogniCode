//! Application Layer - Use cases and application services
//!
//! This module contains the application services that orchestrate
//! domain logic and provide use case implementations.

pub mod behaviors;
// e68 WU2 — affected-work planner over WU1 FactDelta + e66 ReadSet.
#[cfg(feature = "evidence-kernel")]
pub mod change_tracking;
// M7.4 Behavior Runtime adapters — Clock port + SystemClock (application, not domain).
pub mod commands;
pub mod dto;
pub mod error;
// e69 WU1 — EvidenceBundle data types (application-layer; derived; not canonical).
// Gated by `evidence-kernel` because it consumes e66/e68 gated types.
#[cfg(feature = "evidence-kernel")]
pub mod evidence_bundle;
// e69 WU3 — PolicyGate (application-layer; the only authority that
// turns an EvidenceBundle into a structured decision).
#[cfg(feature = "evidence-kernel")]
pub mod fact_bridge;
pub mod findings;
// e69 WU3 — PolicyGate (application-layer; the only authority that
// turns an EvidenceBundle into a structured decision). Gated by
// `evidence-kernel` because it consumes gated types from
// `evidence_bundle`.
#[cfg(feature = "evidence-kernel")]
pub mod policy_gate;
// M7 — recording causal slices into the Intelligence Event Log (application
// helper; the log itself is pure domain).
pub mod ingest;
pub mod intelligence_log;
pub mod investigation_service;
// e70 WU1+WU2 — Local CI vertical orchestration (in-memory WorkExecutor,
// planner → executor → bundle → gate → LocalVerticalReport). Gated by
// `evidence-kernel` because it consumes e68/e69 gated types.
#[cfg(feature = "evidence-kernel")]
pub mod local_ci;
// e75 WU1 — Portable execution seam (spec + outcome + backend trait).
// Container-free: no process spawning here. The trait surface added
// in this module is the only host-execution seam, kept distinct from
// WorkExecutor (see `crate::application::local_ci::WorkExecutor`).
// The composition that bridges both seams lives in
// `application::portable_execution::comp` and is total, pure, no-IO.
// Gated by `evidence-kernel` because `comp` consumes e70
// (`local_ci::WorkExecutor`, `evidence_bundle::Outcome`, …) and
// `evidence_translate` consumes `evidence_bundle::*` and
// `domain::evidence_kernel::ids::*`.
#[cfg(feature = "evidence-kernel")]
pub mod portable_execution;
// e71 WU1 — SoftwareWorld foundation (M9). Lineage + isolation metadata,
// not a second truth store. Reuses canonical SnapshotId; never mirrors
// Facts. WU2 will add fork(); WU3 will compose with e68 SemanticFactDelta.
pub mod program_analysis;
pub mod services;
// e72 WU1 — ChangeProposal (M9). Intent-only, no authority. Creation !=
// authority. Author class (Human/Plugin/LlmAgent) is recorded for e73's
// adversarial gate; the proposal itself does not grant apply power.
#[cfg(feature = "evidence-kernel")]
pub mod change_proposal;
// e73 WU1 — PromotionEvaluation (M9). Three-way dry-run over
// base/candidate/current worlds. WU2 will add PromotionPermit +
// apply authority.
#[cfg(feature = "evidence-kernel")]
pub mod promotion_authority;
#[cfg(feature = "evidence-kernel")]
pub mod software_world;
pub mod workspace_session;

// Re-export error types for convenience
pub use error::{AppError, AppResult};
pub use workspace_session::{WorkspaceError, WorkspaceResult, WorkspaceSession};
