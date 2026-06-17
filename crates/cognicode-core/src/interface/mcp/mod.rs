//! MCP Interface - Model Context Protocol implementation

pub mod completion;
pub mod dto_mapping;
pub mod error;
pub mod file_ops_handlers;
pub mod handlers;
pub mod prompts;
pub mod resources;
pub mod rmcp_adapter;
pub mod schemas;
pub mod security;
pub mod status;

pub use error::{InterfaceError, InterfaceResult};
pub use rmcp_adapter::CogniCodeHandler;

// E2E roundtrip tests for MCP protocol
#[cfg(test)]
mod mcp_roundtrip_tests;
