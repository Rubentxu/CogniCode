//! MCP Interface - Model Context Protocol implementation

pub mod completion;
pub mod file_ops_handlers;
pub mod handlers;
pub mod prompts;
pub mod resources;
pub mod rmcp_adapter;
pub mod schemas;
pub mod security;

pub use rmcp_adapter::CogniCodeHandler;
