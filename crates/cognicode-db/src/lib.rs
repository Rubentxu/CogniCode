//! Shared SQLite persistence layer for CogniCode MCP servers.
//!
//! Provides a single `.cognicode/cognicode.db` file shared between
//! cognicode-mcp (code intelligence) and cognicode-quality (quality analysis).
//!
//! Uses WAL mode for concurrent read/write access.
//!
//! ## Feature flags
//!
//! This crate is gated by the `sqlite` feature. The default build of
//! the workspace no longer pulls in `rusqlite`, so all SQLite modules
//! here are `#[cfg(feature = "sqlite")]`-gated. To use this crate,
//! enable the feature on the consuming crate (e.g.
//! `cognicode-mcp = { features = ["sqlite"] }`).

#[cfg(feature = "sqlite")]
pub mod schema;
#[cfg(feature = "sqlite")]
pub mod quality;
#[cfg(feature = "sqlite")]
pub mod files;
#[cfg(feature = "sqlite")]
pub mod types;
#[cfg(feature = "sqlite")]
pub mod graph;
#[cfg(feature = "sqlite")]
pub mod avc_contracts;
#[cfg(feature = "sqlite")]
pub mod fts5_index;
#[cfg(feature = "sqlite")]
pub mod agent_interactions;
#[cfg(feature = "sqlite")]
pub mod agent_outputs;
#[cfg(feature = "sqlite")]
pub mod agent_tasks;
#[cfg(feature = "sqlite")]
pub mod drift_events;
#[cfg(feature = "sqlite")]
pub mod tool_names;

#[cfg(feature = "sqlite")]
pub use quality::{QualityStore, IssueKey, IssueRow, IssueStatus};
#[cfg(feature = "sqlite")]
pub use files::FileStore;
#[cfg(feature = "sqlite")]
pub use graph::SqliteGraphStore;
#[cfg(feature = "sqlite")]
pub use avc_contracts::{AvcContractStore, ContractRow};
#[cfg(feature = "sqlite")]
pub use fts5_index::Fts5Index;
#[cfg(feature = "sqlite")]
pub use agent_interactions::{
    AgentInteractionStore, AgentInteraction, ToolStats, classify_result_status, ResultStatus,
};
#[cfg(feature = "sqlite")]
pub use agent_outputs::{AgentOutputsStore, AgentOutput};
#[cfg(feature = "sqlite")]
pub use agent_tasks::{AgentTasksStore, AgentTask};
#[cfg(feature = "sqlite")]
pub use drift_events::{DriftEventStore, DriftEvent};
#[cfg(feature = "sqlite")]
pub use tool_names::*;
#[cfg(feature = "sqlite")]
pub use types::*;
