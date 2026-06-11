//! CogniCode Core - Code intelligence library for AI agents

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;
pub mod sandbox_core;

// Re-export the main facade
pub use application::dto;
pub use application::workspace_session::{WorkspaceError, WorkspaceResult, WorkspaceSession};

// Re-export CLI types
pub use interface::cli::{Cli, CommandExecutor};

// Re-export commonly used types
pub use anyhow::Context;
pub use anyhow::Result;
