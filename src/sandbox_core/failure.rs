//! Failure Taxonomy for Sandbox Scenarios
//!
//! Every scenario outcome maps to exactly one failure class.
//! These are used for CI gating, reporting, and developer feedback.

use serde::{Deserialize, Serialize};

/// The 18-class failure taxonomy for scenario outcomes.
/// Each variant corresponds to a specific failure mode discovered during execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Scenario passed with expected outcome
    Pass,
    /// Scenario failed as expected (e.g., capability_missing probe correctly failed)
    ExpectedFail,
    /// Tool/action not implemented for this language (Java/Go probes)
    CapabilityMissing,
    /// MCP tool returned an error (file-not-found, schema mismatch, timeout, etc.)
    McpToolError {
        /// The MCP tool name that failed
        tool_name: String,
        /// Error message from the tool or MCP layer
        error_message: String,
    },
    /// MCP server emitted non-JSON-RPC on stdout (protocol contamination)
    ProtocolViolation,
    /// Tool behaviour did not match its contract (e.g., preview-only refactor)
    ToolContractMismatch,
    /// Mutation attempted path outside allowed workspace (path safety rejection)
    PathSafetyRejection,
    /// Mutation produced syntactically invalid code
    SyntaxValidationFailure,
    /// Code failed format check (rustfmt, black, prettier, etc.)
    FormatFailure,
    /// Code failed lint check (clippy, ruff, eslint, etc.)
    LintFailure,
    /// Code failed to compile or build
    BuildFailure,
    /// Tests failed after mutation
    TestFailure,
    /// Semantic regression: mutation changed behaviour, not just syntax
    SemanticRegression,
    /// Sandbox infrastructure failure (container start, podman error, etc.)
    SandboxInfraFailure,
    /// Container hit CPU/memory/pids/fd/time limits
    ResourceLimitExceeded,
    /// Scenario exceeded its declared timeout
    Timeout,
    /// Scenario produces non-deterministic results across identical runs.
    ///
    /// **Phase 3 only** — Batch C of `production-ready-sandbox-validation` does NOT
    /// wire this class. Detecting nondeterminism requires running each scenario
    /// twice with identical inputs and comparing outputs, which needs the Phase 3
    /// rerun architecture (not yet implemented). This class remains CI-blocking
    /// in the taxonomy but is never actively produced until Phase 3.
    Nondeterministic,
    /// Repo had pre-existing build/test failure before mutation
    PreexistingRepoFailure,
    /// Scenario expected to fail but passed (unexpected pass)
    UnexpectedPass,
    /// Scenario failed with no classified reason
    UnexpectedFail,
}

impl FailureClass {
    /// Human-readable description of the failure class.
    pub fn description(&self) -> &'static str {
        match self {
            FailureClass::Pass => "Scenario passed as expected",
            FailureClass::ExpectedFail => "Scenario failed as expected",
            FailureClass::CapabilityMissing => "Tool/capability not implemented for this language",
            FailureClass::McpToolError { .. } => "MCP tool returned an error",
            FailureClass::ProtocolViolation => "MCP server emitted non-JSON-RPC output",
            FailureClass::ToolContractMismatch => "Tool behaviour did not match its contract",
            FailureClass::PathSafetyRejection => {
                "Mutation attempted path outside allowed workspace"
            }
            FailureClass::SyntaxValidationFailure => "Mutation produced syntactically invalid code",
            FailureClass::FormatFailure => "Code failed format check",
            FailureClass::LintFailure => "Code failed lint check",
            FailureClass::BuildFailure => "Code failed to build",
            FailureClass::TestFailure => "Tests failed after mutation",
            FailureClass::SemanticRegression => "Mutation changed behaviour (semantic regression)",
            FailureClass::SandboxInfraFailure => "Sandbox infrastructure failure",
            FailureClass::ResourceLimitExceeded => "Container hit resource limits",
            FailureClass::Timeout => "Scenario exceeded declared timeout",
            FailureClass::Nondeterministic => "Scenario produced non-deterministic results",
            FailureClass::PreexistingRepoFailure => "Repo had pre-existing build/test failure",
            FailureClass::UnexpectedPass => "Scenario expected to fail but passed",
            FailureClass::UnexpectedFail => "Scenario failed with unclassified reason",
        }
    }

    /// Whether this failure class represents a scenario that should gate CI.
    /// Expected fails, capability-missing probes, and preexisting failures should NOT gate CI.
    pub fn is_ci_blocking(&self) -> bool {
        !matches!(
            self,
            FailureClass::Pass
                | FailureClass::ExpectedFail
                | FailureClass::CapabilityMissing
                | FailureClass::PreexistingRepoFailure
                | FailureClass::McpToolError { .. }
        )
    }
}

impl std::fmt::Display for FailureClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FailureClass::McpToolError {
                tool_name,
                error_message,
            } => {
                write!(f, "mcp_tool_error[{}: {}]", tool_name, error_message)
            }
            _ => {
                let snake = serde_json::to_string(self).unwrap_or_default();
                // Remove quotes from JSON string
                write!(f, "{}", &snake[1..snake.len() - 1])
            }
        }
    }
}

impl Default for FailureClass {
    fn default() -> Self {
        FailureClass::UnexpectedFail
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_failure_class_serde_roundtrip() {
        let classes: Vec<FailureClass> = vec![
            FailureClass::Pass,
            FailureClass::CapabilityMissing,
            FailureClass::Timeout,
            FailureClass::BuildFailure,
            FailureClass::McpToolError {
                tool_name: "read_file".into(),
                error_message: "File not found".into(),
            },
        ];
        for fc in &classes {
            let json = serde_json::to_string(fc).unwrap();
            let back: FailureClass = serde_json::from_str(&json).unwrap();
            assert_eq!(*fc, back);
        }
    }

    #[test]
    fn test_failure_class_is_ci_blocking() {
        assert!(!FailureClass::CapabilityMissing.is_ci_blocking());
        assert!(!FailureClass::ExpectedFail.is_ci_blocking());
        assert!(!FailureClass::McpToolError {
            tool_name: "read_file".into(),
            error_message: "File not found".into(),
        }
        .is_ci_blocking());
        assert!(FailureClass::BuildFailure.is_ci_blocking());
        assert!(FailureClass::TestFailure.is_ci_blocking());
    }

    #[test]
    fn test_failure_class_display() {
        assert_eq!(format!("{}", FailureClass::Timeout), "timeout");
        assert_eq!(
            format!("{}", FailureClass::CapabilityMissing),
            "capability_missing"
        );
        assert!(format!(
            "{}",
            FailureClass::McpToolError {
                tool_name: "read_file".into(),
                error_message: "File not found".into(),
            }
        )
        .contains("mcp_tool_error"));
    }
}
