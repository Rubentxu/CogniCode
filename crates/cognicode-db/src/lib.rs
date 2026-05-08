//! Shared SQLite persistence layer for CogniCode MCP servers.
//!
//! Provides a single `.cognicode/cognicode.db` file shared between
//! cognicode-mcp (code intelligence) and cognicode-quality (quality analysis).
//!
//! Uses WAL mode for concurrent read/write access.

pub mod schema;
pub mod quality;
pub mod files;
pub mod types;
pub mod graph;
pub mod avc_contracts;
pub mod fts5_index;
pub mod agent_interactions;
pub mod agent_outputs;
pub mod agent_tasks;
pub mod drift_events;
pub mod tool_names;

pub use quality::{QualityStore, IssueKey, IssueRow, IssueStatus};
pub use files::FileStore;
pub use graph::SqliteGraphStore;
pub use avc_contracts::{AvcContractStore, ContractRow};
pub use fts5_index::Fts5Index;
pub use agent_interactions::{AgentInteractionStore, AgentInteraction, ToolStats, classify_result_status, ResultStatus};
pub use agent_outputs::{AgentOutputsStore, AgentOutput};
pub use agent_tasks::{AgentTasksStore, AgentTask};
pub use drift_events::{DriftEventStore, DriftEvent};
pub use tool_names::*;
pub use types::*;