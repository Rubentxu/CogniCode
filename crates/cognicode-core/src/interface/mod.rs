//! Interface Layer - External interfaces (CLI, MCP, LSP)

pub mod catalog;
pub mod cli;
pub mod composition;
pub mod intent;
pub mod lsp;
pub mod mcp;
pub mod symbol_identity;

#[cfg(feature = "rig")]
pub mod rig;
