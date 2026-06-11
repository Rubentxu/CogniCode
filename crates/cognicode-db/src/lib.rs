//! Shared persistence layer for CogniCode MCP servers.
//!
//! ## Status
//!
//! The `sqlite` feature and its `rusqlite` dependency were removed in
//! the Graph Intelligence v2 cleanup. The module surface that was
//! previously gated behind `#[cfg(feature = "sqlite")]` is therefore
//! empty in the current build.
//!
//! This crate is kept in the workspace as a placeholder for the
//! upcoming PostgreSQL-backed reimplementation; all of the source
//! modules (`schema`, `quality`, `files`, `types`, `graph`,
//! `avc_contracts`, `fts5_index`, `agent_interactions`,
//! `agent_outputs`, `agent_tasks`, `drift_events`, `tool_names`)
//! remain in the source tree and will be reintroduced behind a
//! `postgres` feature in the follow-up slice.
