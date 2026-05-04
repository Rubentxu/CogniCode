//! MCP Interface - Model Context Protocol implementation

pub mod completion;
pub mod dto_mapping;
pub mod file_ops_handlers;
pub mod handlers;
pub mod prompts;
pub mod resources;
pub mod rmcp_adapter;
pub mod schemas;
pub mod security;

pub use rmcp_adapter::CogniCodeHandler;

// E2E roundtrip tests for MCP protocol
#[cfg(test)]
mod mcp_roundtrip_tests;
