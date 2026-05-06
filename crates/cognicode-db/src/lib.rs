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

pub use quality::{QualityStore, IssueKey, IssueRow, IssueStatus};
pub use files::FileStore;
pub use graph::SqliteGraphStore;
pub use avc_contracts::AvcContractStore;
pub use fts5_index::Fts5Index;
pub use types::*;