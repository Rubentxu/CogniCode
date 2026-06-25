//! ESLint linter wrapper
//!
//! Runs `npx eslint --format=json` as a subprocess and normalizes its JSON output
//! into the standard `LinterReport` format.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::error::{AxiomError, AxiomResult};
use crate::linters::{Linter, LinterIssue, LinterReport, LinterSummary, Severity};

/// Runner for the ESLint linter
#[derive(Debug)]
pub struct EslintRunner {
    npx_path: PathBuf,
}

impl EslintRunner {
    /// Create a new EslintRunner with default settings
    pub fn new() -> Self {
        Self {
            npx_path: PathBuf::from("npx"),
        }
    }

    /// Set a custom path to npx
    pub fn with_npx_path(mut self, path: PathBuf) -> Self {
        self.npx_path = path;
        self
    }

    fn map_severity(level: u64) -> Severity {
        match level {
            2 => Severity::Error,
            1 => Severity::Warning,
            _ => Severity::Info,
        }
    }
}

impl Default for EslintRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter for EslintRunner {
    fn run(&self, project_path: &Path) -> AxiomResult<LinterReport> {
        let start = Instant::now();

        // Check if the path exists (eslint can lint specific files or directories)
        if !project_path.exists() {
            return Err(AxiomError::FileNotFound {
                path: project_path.to_path_buf(),
            });
        }

        // Build the eslint command
        let mut cmd = Command::new(&self.npx_path);
        cmd.arg("eslint")
            .arg("--format=json")
            .arg(project_path);

        let output = cmd
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AxiomError::LinterNotFound {
                        linter: "eslint".to_string(),
                    }
                } else {
                    AxiomError::LinterExecution {
                        linter: "eslint".to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // ESLint outputs a JSON array, one element per file
        let results: Vec<serde_json::Value> = serde_json::from_str(&stdout).map_err(|e| {
            // If eslint is not found or not installed, we get text error
            if stderr.contains("eslint") && (stderr.contains("not found") || stderr.contains("command not found")) {
                AxiomError::LinterNotFound {
                    linter: "eslint".to_string(),
                }
            } else {
                AxiomError::LinterParse {
                    message: format!("Failed to parse eslint JSON output: {}", e),
                }
            }
        })?;

        let mut issues = Vec::new();

        // Process each file result
        for file_result in results {
            let file_path = file_result
                .get("filePath")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("unknown"));

            let messages = file_result.get("messages").and_then(|v| v.as_array());

            if let Some(messages) = messages {
                for msg in messages {
                    let line = msg.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
                    let column = msg.get("column").and_then(|v| v.as_u64()).map(|v| v as usize);
                    let severity = msg
                        .get("severity")
                        .and_then(|v| v.as_u64())
                        .map(Self::map_severity)
                        .unwrap_or(Severity::Info);
                    let message = msg
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Unknown error")
                        .to_string();
                    let rule_id = msg
                        .get("ruleId")
                        .and_then(|v| v.as_str())
                        .map(String::from);

                    // ESLint's "suggestion" is in the related information, not directly accessible
                    let suggestion = msg
                        .get("fix")
                        .and_then(|f| f.as_object())
                        .map(|fix| {
                            let range = fix.get("range").map(|r| format!("{:?}", r)).unwrap_or_else(|| "unknown".to_string());
                            format!("Replace characters {} with suggested fix", range)
                        });

                    issues.push(LinterIssue {
                        file: file_path.clone(),
                        line,
                        column,
                        severity,
                        code: rule_id,
                        message,
                        suggestion,
                    });
                }
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

        let summary = LinterSummary {
            total: issues.len(),
            errors,
            warnings,
            info,
        };

        Ok(LinterReport {
            linter_name: "eslint".to_string(),
            execution_time_ms,
            issues,
            summary,
        })
    }

    fn name(&self) -> &str {
        "eslint"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_mapping() {
        assert_eq!(EslintRunner::map_severity(2), Severity::Error);
        assert_eq!(EslintRunner::map_severity(1), Severity::Warning);
        assert_eq!(EslintRunner::map_severity(0), Severity::Info);
    }

    #[test]
    fn test_eslint_runner_construction() {
        let runner = EslintRunner::new();
        assert_eq!(runner.name(), "eslint");

        let runner = EslintRunner::new().with_npx_path(PathBuf::from("/usr/local/bin/npx"));
        assert_eq!(runner.name(), "eslint");
    }

    #[test]
    fn test_parse_eslint_json() {
        // Test parsing a simple ESLint JSON output
        let json_output = r#"[
  {
    "filePath": "/project/src/index.js",
    "messages": [
      {
        "line": 10,
        "column": 15,
        "severity": 2,
        "message": "Unexpected var, use let or const.",
        "ruleId": "no-var"
      },
      {
        "line": 20,
        "column": 1,
        "severity": 1,
        "message": "Missing semicolon.",
        "ruleId": "semi"
      }
    ],
    "errorCount": 1,
    "warningCount": 1,
    "fixableErrorCount": 0,
    "fixableWarningCount": 0
  }
]"#;

        let results: Vec<serde_json::Value> = serde_json::from_str(json_output).unwrap();
        assert_eq!(results.len(), 1);

        let messages = results[0].get("messages").and_then(|v| v.as_array()).unwrap();
        assert_eq!(messages.len(), 2);

        // Check first message
        let msg1 = &messages[0];
        assert_eq!(msg1.get("line").and_then(|v| v.as_u64()), Some(10));
        assert_eq!(msg1.get("severity").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(msg1.get("message").and_then(|v| v.as_str()), Some("Unexpected var, use let or const."));
        assert_eq!(msg1.get("ruleId").and_then(|v| v.as_str()), Some("no-var"));
    }

    #[test]
    fn test_linter_issue_fields() {
        let issue = LinterIssue {
            file: PathBuf::from("src/test.js"),
            line: 5,
            column: Some(10),
            severity: Severity::Warning,
            code: Some("no-unused-vars".to_string()),
            message: "Unused variable 'x'".to_string(),
            suggestion: Some("Remove the unused variable".to_string()),
        };

        assert_eq!(issue.file, PathBuf::from("src/test.js"));
        assert_eq!(issue.line, 5);
        assert_eq!(issue.column, Some(10));
        assert_eq!(issue.severity, Severity::Warning);
        assert_eq!(issue.code.as_deref(), Some("no-unused-vars"));
        assert_eq!(issue.message, "Unused variable 'x'");
        assert!(issue.suggestion.is_some());
    }
}