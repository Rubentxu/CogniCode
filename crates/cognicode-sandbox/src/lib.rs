//! Sandbox orchestrator library.
//!
//! `sandbox_core` holds the scenario manifests, artifact models, ground-truth
//! matching, scoring and run history that the `sandbox-orchestrator` binary
//! drives. It is compiled here, with its only consumer, and not in
//! `cognicode-core`: the analysis core knows nothing about sandbox runs.

pub mod sandbox_core;
