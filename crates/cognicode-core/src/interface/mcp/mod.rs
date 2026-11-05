//! MCP Interface - Model Context Protocol implementation
//!
//! The server announces the capabilities it serves and no others. It
//! serves `tools`; it does not serve `prompts`, `resources` or
//! `completion`, so it does not announce them, and the modules that
//! implemented them without ever being called are gone rather than
//! waiting for a caller that never came.

pub mod cancellation;
pub mod capabilities;
pub mod cursor;
pub mod dto_mapping;
pub mod error;
pub mod file_ops_handlers;
pub mod handlers;
pub mod output_contracts;
pub mod parameter_aliases;
// CU-1.3 — presentadores puros AnalysisOutcome → BuildGraphOutput.
pub mod presenter_bridge;
pub mod rmcp_adapter;
pub mod robustness;
pub mod schemas;
pub mod security;
pub mod status;
pub mod tool_response;

pub use error::{InterfaceError, InterfaceResult};
pub use rmcp_adapter::CogniCodeHandler;

// E2E roundtrip tests for MCP protocol
#[cfg(test)]
mod mcp_roundtrip_tests;
