//! Interface Layer - External interfaces (CLI, MCP, LSP)

pub mod cli;
pub mod lsp;
pub mod mcp;

#[cfg(feature = "rig")]
pub mod rig;