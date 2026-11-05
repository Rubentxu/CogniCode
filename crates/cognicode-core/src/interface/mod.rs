//! Interface Layer - External interfaces (CLI, MCP, LSP)

pub mod catalog;
pub mod cli;
pub mod composition;
pub mod coverage;
// TASK-92 (slice 1) — the daemon's local socket: how a client on this
// machine asks whether the index the daemon keeps is still fresh.
pub mod daemon;
pub mod intent;
pub mod lsp;
pub mod mcp;
pub mod symbol_identity;

#[cfg(feature = "rig")]
pub mod rig;
