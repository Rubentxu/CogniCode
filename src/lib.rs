//! CogniCode - Premium LSP server for AI agents with code analysis and refactoring.

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interface;
pub mod sandbox_core;

// Re-export CLI types
pub use interface::cli::{Cli, CommandExecutor};

// Re-export commonly used types
pub use anyhow::Result;
pub use anyhow::Context;