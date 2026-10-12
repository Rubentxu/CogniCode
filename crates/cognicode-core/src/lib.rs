//! CogniCode Core - Code intelligence library for AI agents

// B3.W4 — análisis incremental fiable (BLAKE3 + CacheStore por sesión).
pub mod application;
pub mod assurance_export;
pub mod domain;
// B4.W3 — normalized quality finding representation across the 5
// quality tools (complexity, long_parameter_lists, god_functions,
// solid_audit, relationship_cycle). Pure data; no I/O.
pub mod findings;
pub mod incremental;
pub mod infrastructure;
pub mod interface;
pub mod product;
pub mod sandbox_core;
pub mod schemas;

// Re-export the main facade
pub use application::dto;
pub use application::workspace_session::{WorkspaceError, WorkspaceResult, WorkspaceSession};

// Re-export CLI types
pub use interface::cli::{Cli, CommandExecutor};

// Re-export commonly used types
pub use anyhow::Context;
pub use anyhow::Result;
