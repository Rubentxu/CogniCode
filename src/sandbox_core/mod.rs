//! Sandbox Core Library
//!
//! Shared types, schemas, and utilities for the sandbox-orchestrator binary.
//! Covers: failure taxonomy, artifact models, manifest schemas, MCP lifecycle helpers,
//! ground truth matching, and quality scoring.

pub mod artifacts;
pub mod failure;
pub mod ground_truth;
pub mod history;
pub mod manifest;
pub mod mcp_core;
pub mod resource;
pub mod scoring;
