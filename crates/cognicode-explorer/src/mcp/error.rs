//! Error types for MCP tool dispatch.
//!
//! These variants map to the structured error envelope format used across
//! all 34 tools. The dispatch layer converts these to `CallToolResult::error`
//! with the canonical wire-level JSON envelope.

use std::fmt;

/// Errors raised by [`super::handler::ToolHandler::handle`] implementations.
///
/// Each variant carries the tool name so error log messages can be
/// correlated to the correct family without additional context passing.
#[derive(Debug)]
pub enum ToolError {
    /// The requested resource was not found.
    NotFound { tool: &'static str, what: String },
    /// One or more input arguments failed validation.
    InvalidInput { tool: &'static str, field: String },
    /// The tool requires a feature that is not enabled in this build.
    FeatureDisabled { tool: &'static str, feature: &'static str },
    /// The request conflicts with current state (e.g. duplicate, unique violation).
    Conflict { tool: &'static str, what: String },
    /// A storage or I/O operation failed.
    Storage { tool: &'static str, source: String },
    /// The tool name was not recognised by the registry.
    UnknownTool(&'static str),
    /// An unexpected internal error occurred.
    Internal { tool: &'static str, message: String },
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { tool, what } => write!(f, "{tool}: {what}"),
            Self::InvalidInput { tool, field } => write!(f, "{tool}: {field}"),
            Self::FeatureDisabled { tool, feature } => {
                write!(f, "{tool}: requires feature `{feature}`")
            }
            Self::Conflict { tool, what } => write!(f, "{tool}: {what}"),
            Self::Storage { tool, source } => write!(f, "{tool}: storage error: {source}"),
            Self::UnknownTool(name) => write!(f, "Unknown tool: {name}"),
            Self::Internal { tool, message } => write!(f, "{tool}: {message}"),
        }
    }
}
