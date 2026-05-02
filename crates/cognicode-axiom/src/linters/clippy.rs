//! Clippy linter wrapper
//!
//! Runs `cargo clippy` as a subprocess and normalizes its JSON output
//! into the standard `LinterReport` format.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::error::{AxiomError, AxiomResult};
use crate::linters::{Linter, LinterIssue, LinterReport, LinterSummary, Severity};

/// Runner for the Clippy linter
#[derive(Debug)]
pub struct ClippyRunner {
    cargo_path: PathBuf,
    extra_args: Vec<String>,
}

impl ClippyRunner {
    /// Create a new ClippyRunner with default settings
    pub fn new() -> Self {
        Self {
            cargo_path: PathBuf::from("cargo"),
            extra_args: vec![],
        }
    }

    /// Set a custom path to the cargo binary
    pub fn with_cargo_path(mut self, path: PathBuf) -> Self {
        self.cargo_path = path;
        self
    }

    /// Add extra arguments to pass to cargo clippy
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.extra_args = args;
        self
    }

    fn map_severity(level: &str) -> Severity {
        match level {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            "note" => Severity::Note,
            _ => Severity::Info,
        }
    }

    fn parse_message(&self, line: &str) -> Option<LinterIssue> {
        let json: serde_json::Value = serde_json::from_str(line).ok()?;

        // Filter for compiler messages (actual lint messages)
        let reason = json.get("reason")?.as_str()?;
        if reason != "compiler-message" {
            return None;
        }

        let message = json.get("message")?;

        // Extract location info
        let spans = message.get("spans")?;
        let first_span = spans.get(0)?;

        let file = PathBuf::from(first_span.get("file_name")?.as_str()?);
        let line = first_span.get("line_start")?.as_u64()? as usize;
        let column = first_span.get("line_start").and_then(|v| v.as_u64()).map(|v| v as usize);

        let level = message.get("level")?.as_str()?;
        let message_text = message.get("message")?.as_str()?.to_string();

        // Get the lint code (e.g., "clippy::unwrap_used")
        let code = message
            .get("code")
            .and_then(|c| c.get("code"))
            .and_then(|c| c.as_str())
            .map(String::from);

        // Get rendered message (full formatted message)
        let rendered = message
            .get("rendered")
            .and_then(|r| r.as_str())
            .map(String::from);

        Some(LinterIssue {
            file,
            line,
            column,
            severity: Self::map_severity(level),
            code,
            message: rendered.unwrap_or(message_text),
            suggestion: None,
        })
    }
}

impl Default for ClippyRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter for ClippyRunner {
    fn run(&self, project_path: &Path) -> AxiomResult<LinterReport> {
        let start = Instant::now();

        let manifest_path = project_path.join("Cargo.toml");
        if !manifest_path.exists() {
            return Err(AxiomError::FileNotFound {
                path: manifest_path,
            });
        }

        // Build the cargo clippy command
        let mut cmd = Command::new(&self.cargo_path);
        cmd.arg("clippy")
            .arg("--message-format=json")
            .arg("--manifest-path")
            .arg(&manifest_path);

        // Add any extra args
        for arg in &self.extra_args {
            cmd.arg(arg);
        }

        // Set working directory for the command
        let output = cmd
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AxiomError::LinterNotFound {
                        linter: "clippy".to_string(),
                    }
                } else {
                    AxiomError::LinterExecution {
                        linter: "clippy".to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut issues = Vec::new();

        // Parse each line of stdout as JSON
        for line in stdout.lines() {
            if let Some(issue) = self.parse_message(line) {
                issues.push(issue);
            }
        }

        // If clippy returned an error but we got no issues, check stderr
        if issues.is_empty() && !output.status.success() {
            // Check if clippy is not installed
            if stderr.contains("clippy 0.0") || stderr.contains("Unable to find clippy") {
                return Err(AxiomError::LinterNotFound {
                    linter: "clippy".to_string(),
                });
            }
        }

        let execution_time_ms = start.elapsed().as_millis() as u64;

        // Count issues by severity
        let errors = issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count();
        let warnings = issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count();
        let info = issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .count();
        let notes = issues
            .iter()
            .filter(|i| i.severity == Severity::Note)
            .count();

        let summary = LinterSummary {
            total: issues.len(),
            errors,
            warnings,
            info: info + notes, // Notes are included in info count
        };

        Ok(LinterReport {
            linter_name: "clippy".to_string(),
            execution_time_ms,
            issues,
            summary,
        })
    }

    fn name(&self) -> &str {
        "clippy"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_mapping() {
        assert_eq!(ClippyRunner::map_severity("error"), Severity::Error);
        assert_eq!(ClippyRunner::map_severity("warning"), Severity::Warning);
        assert_eq!(ClippyRunner::map_severity("note"), Severity::Note);
        assert_eq!(ClippyRunner::map_severity("info"), Severity::Info);
    }

    #[test]
    fn test_parse_clippy_json() {
        let runner = ClippyRunner::new();

        // Sample cargo clippy JSON output line
        let json_line = r#"{"reason":"compiler-message","message":{"code":{"code":"clippy::unwrap_used","explanation":null},"level":"warning","message":"unnecessary use of `unwrap`","spans":[{"file_name":"src/main.rs","line_start":10,"line_end":10,"column_start":5,"column_end":11,"text":[{"text":"    let x = something.unwrap();","highlight_start":5,"highlight_end":11}]}],"children":[],"rendered":"warning: unnecessary use of `unwrap`"}}"#;

        let issue = runner.parse_message(json_line);
        assert!(issue.is_some());

        let issue = issue.unwrap();
        assert_eq!(issue.file, PathBuf::from("src/main.rs"));
        assert_eq!(issue.line, 10);
        assert_eq!(issue.severity, Severity::Warning);
        assert_eq!(issue.code.as_deref(), Some("clippy::unwrap_used"));
    }

    #[test]
    fn test_parse_non_message_line() {
        let runner = ClippyRunner::new();

        // Lines with reason != "compiler-message" should return None
        let non_message = r#"{"reason":"build-finished","message":{"success":true}}"#;
        assert!(runner.parse_message(non_message).is_none());
    }

    #[test]
    fn test_parse_invalid_json() {
        let runner = ClippyRunner::new();
        assert!(runner.parse_message("not valid json").is_none());
    }

    #[test]
    fn test_clippy_runner_construction() {
        let runner = ClippyRunner::new();
        assert_eq!(runner.name(), "clippy");

        let runner = ClippyRunner::new()
            .with_cargo_path(PathBuf::from("/usr/local/bin/cargo"))
            .with_args(vec!["--".to_string(), "-D".to_string(), "warnings".to_string()]);
        assert_eq!(runner.name(), "clippy");
    }

    #[test]
    fn test_linter_summary_counts() {
        let summary = LinterSummary {
            total: 10,
            errors: 2,
            warnings: 5,
            info: 3,
        };
        assert_eq!(summary.total, 10);
        assert_eq!(summary.errors, 2);
        assert_eq!(summary.warnings, 5);
        assert_eq!(summary.info, 3);
    }
}