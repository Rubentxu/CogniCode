//! CogniCode Core - Code intelligence library for AI agents

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interface;
pub mod sandbox_core;

// Re-export the main facade
pub use application::workspace_session::{WorkspaceSession, WorkspaceError, WorkspaceResult};
pub use application::dto;

// Re-export CLI types
pub use interface::cli::{Cli, CommandExecutor};

// Re-export commonly used types
pub use anyhow::Result;
pub use anyhow::Context;
