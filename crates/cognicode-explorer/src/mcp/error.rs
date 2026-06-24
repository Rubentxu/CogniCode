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
    FeatureDisabled {
        tool: &'static str,
        feature: &'static str,
    },
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

impl ToolError {
    /// Return the canonical error code string for this variant.
    ///
    /// Each variant maps to a lowercase `snake_case` static string.
    /// This is the light activation that enables C9's full
    /// `Result<CallToolResult, ToolError>` return-type migration.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound { .. } => "not_found",
            Self::InvalidInput { .. } => "invalid_input",
            Self::FeatureDisabled { .. } => "feature_disabled",
            Self::Conflict { .. } => "conflict",
            Self::Storage { .. } => "storage_error",
            Self::UnknownTool(_) => "unknown_tool",
            Self::Internal { .. } => "internal_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_error_code_not_found() {
        let err = ToolError::NotFound {
            tool: "foo",
            what: "bar".into(),
        };
        assert_eq!(err.code(), "not_found");
    }

    #[test]
    fn tool_error_code_invalid_input() {
        let err = ToolError::InvalidInput {
            tool: "foo",
            field: "x".into(),
        };
        assert_eq!(err.code(), "invalid_input");
    }

    #[test]
    fn tool_error_code_feature_disabled() {
        let err = ToolError::FeatureDisabled {
            tool: "foo",
            feature: "multimodal",
        };
        assert_eq!(err.code(), "feature_disabled");
    }

    #[test]
    fn tool_error_code_conflict() {
        let err = ToolError::Conflict {
            tool: "foo",
            what: "dup".into(),
        };
        assert_eq!(err.code(), "conflict");
    }

    #[test]
    fn tool_error_code_storage() {
        let err = ToolError::Storage {
            tool: "foo",
            source: "io".into(),
        };
        assert_eq!(err.code(), "storage_error");
    }

    #[test]
    fn tool_error_code_unknown_tool() {
        let err = ToolError::UnknownTool("my_tool");
        assert_eq!(err.code(), "unknown_tool");
    }

    #[test]
    fn tool_error_code_internal() {
        let err = ToolError::Internal {
            tool: "foo",
            message: "boom".into(),
        };
        assert_eq!(err.code(), "internal_error");
    }

    #[test]
    fn tool_error_code_all_variants() {
        let variants = [
            ToolError::NotFound {
                tool: "t",
                what: "w".into(),
            },
            ToolError::InvalidInput {
                tool: "t",
                field: "f".into(),
            },
            ToolError::FeatureDisabled {
                tool: "t",
                feature: "x",
            },
            ToolError::Conflict {
                tool: "t",
                what: "w".into(),
            },
            ToolError::Storage {
                tool: "t",
                source: "s".into(),
            },
            ToolError::UnknownTool("t"),
            ToolError::Internal {
                tool: "t",
                message: "m".into(),
            },
        ];
        let expected = [
            "not_found",
            "invalid_input",
            "feature_disabled",
            "conflict",
            "storage_error",
            "unknown_tool",
            "internal_error",
        ];
        for (err, exp) in variants.iter().zip(expected.iter()) {
            assert_eq!(err.code(), *exp);
        }
    }
}
