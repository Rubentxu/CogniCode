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

pub use quality::QualityStore;
pub use files::FileStore;
pub use graph::SqliteGraphStore;
pub use types::*;