//! CogniCode - Code Intelligence Library for AI Agents
//!
//! This is a thin re-export crate that provides the `cognicode` namespace
//! for integration tests and legacy compatibility.
//!
//! All types are re-exported from `cognicode-core`.

pub use cognicode_core::application;
pub use cognicode_core::domain;
pub use cognicode_core::infrastructure;
pub use cognicode_core::interface;
pub use cognicode_core::sandbox_core;

// Re-export commonly used types
pub use anyhow;
pub use cognicode_core::application::workspace_session::{WorkspaceError, WorkspaceResult, WorkspaceSession};
pub use cognicode_core::interface::cli::{Cli, CommandExecutor};
