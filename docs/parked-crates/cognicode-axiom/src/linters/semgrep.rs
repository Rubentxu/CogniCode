//! Semgrep linter wrapper
//!
//! Runs `semgrep --json` as a subprocess and normalizes its JSON output
//! into the standard `LinterReport` format.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::error::{AxiomError, AxiomResult};
use crate::linters::{Linter, LinterIssue, LinterReport, LinterSummary, Severity};

/// Runner for the Semgrep linter
#[derive(Debug)]
pub struct SemgrepRunner {
    semgrep_path: PathBuf,
}

impl SemgrepRunner {
    /// Create a new SemgrepRunner with default settings
    pub fn new() -> Self {
        Self {
            semgrep_path: PathBuf::from("semgrep"),
        }
    }

    /// Set a custom path to the semgrep binary
    pub fn with_semgrep_path(mut self, path: PathBuf) -> Self {
        self.semgrep_path = path;
        self
    }

    fn map_severity(severity: &str) -> Severity {
        match severity.to_lowercase().as_str() {
            "error" => Severity::Error,
            "warning" => Severity::Warning,
            _ => Severity::Info,
        }
    }
}

impl Default for SemgrepRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter for SemgrepRunner {
    fn run(&self, project_path: &Path) -> AxiomResult<LinterReport> {
        let start = Instant::now();

        // Semgrep can scan a path (file or directory)
        if !project_path.exists() {
            return Err(AxiomError::FileNotFound {
                path: project_path.to_path_buf(),
            });
        }

        // Build the semgrep command
        let mut cmd = Command::new(&self.semgrep_path);
        cmd.arg("--json")
            .arg(project_path);

        let output = cmd
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AxiomError::LinterNotFound {
                        linter: "semgrep".to_string(),
                    }
                } else {
                    AxiomError::LinterExecution {
                        linter: "semgrep".to_string(),
                        message: e.to_string(),
                    }
                }
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Semgrep outputs a JSON object with a "results" array
        let json_output: serde_json::Value = serde_json::from_str(&stdout).map_err(|e| {
            // If semgrep is not found
            if stderr.contains("not found") || stderr.contains("command not found") {
                AxiomError::LinterNotFound {
                    linter: "semgrep".to_string(),
                }
            } else {
                AxiomError::LinterParse {
                    message: format!("Failed to parse semgrep JSON output: {}", e),
                }
            }
        })?;

        let results = json_output
            .get("results")
            .and_then(|v| v.as_array())
            .map(|arr| arr.to_vec())
            .unwrap_or_default();

        let mut issues = Vec::new();

        for result in results {
            let path = result
                .get("path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("unknown"));

            let start_obj = result.get("start").and_then(|v| v.as_object());
            let line = start_obj
                .and_then(|s| s.get("line"))
                .and_then(|v| v.as_u64())
                .unwrap_or(1) as usize;
            let column = start_obj
                .and_then(|s| s.get("col"))
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);

            let extra = result.get("extra").and_then(|v| v.as_object());

            let severity_str = extra
                .and_then(|e| e.get("severity"))
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let severity = Self::map_severity(severity_str);

            let message = extra
                .and_then(|e| e.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("No message")
                .to_string();

            // Get the rule ID (check_id)
            let check_id = result
                .get("check_id")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| {
                    extra
                        .and_then(|e| e.get("metadata"))
                        .and_then(|m| m.get("category"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });

            // Get suggested fix if available
            let suggestion = extra
                .and_then(|e| e.get("fix"))
                .and_then(|f| f.as_str())
                .map(String::from);

            issues.push(LinterIssue {
                file: path,
                line,
                column,
                severity,
                code: check_id,
                message,
                suggestion,
            });
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
            linter_name: "semgrep".to_string(),
            execution_time_ms,
            issues,
            summary,
        })
    }

    fn name(&self) -> &str {
        "semgrep"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_severity_mapping() {
        assert_eq!(SemgrepRunner::map_severity("ERROR"), Severity::Error);
        assert_eq!(SemgrepRunner::map_severity("error"), Severity::Error);
        assert_eq!(SemgrepRunner::map_severity("WARNING"), Severity::Warning);
        assert_eq!(SemgrepRunner::map_severity("warning"), Severity::Warning);
        assert_eq!(SemgrepRunner::map_severity("INFO"), Severity::Info);
        assert_eq!(SemgrepRunner::map_severity("unknown"), Severity::Info);
    }

    #[test]
    fn test_semgrep_runner_construction() {
        let runner = SemgrepRunner::new();
        assert_eq!(runner.name(), "semgrep");

        let runner = SemgrepRunner::new().with_semgrep_path(PathBuf::from("/usr/local/bin/semgrep"));
        assert_eq!(runner.name(), "semgrep");
    }

    #[test]
    fn test_parse_semgrep_json() {
        // Test parsing a simple Semgrep JSON output
        let json_output = r#"{
  "results": [
    {
      "check_id": "javascript.lang.security.detect-non-literal-regex",
      "path": "src/login.js",
      "start": {
        "line": 15,
        "col": 10
      },
      "end": {
        "line": 15,
        "col": 25
      },
      "extra": {
        "severity": "ERROR",
        "message": "Regex problem",
        "metadata": {
          "category": "security"
        }
      }
    },
    {
      "check_id": "python.lang.security.audit-sql-injection",
      "path": "db.py",
      "start": {
        "line": 5,
        "col": 0
      },
      "end": {
        "line": 5,
        "col": 30
      },
      "extra": {
        "severity": "WARNING",
        "message": "SQL injection vulnerability",
        "fix": "Use parameterized queries"
      }
    }
  ],
  "errors": [],
  "version": "1.0.0"
}"#;

        let json: serde_json::Value = serde_json::from_str(json_output).unwrap();
        let results = json.get("results").and_then(|v| v.as_array()).unwrap();

        assert_eq!(results.len(), 2);

        // Check first result
        let r1 = &results[0];
        assert_eq!(r1.get("check_id").and_then(|v| v.as_str()), Some("javascript.lang.security.detect-non-literal-regex"));
        assert_eq!(r1.get("path").and_then(|v| v.as_str()), Some("src/login.js"));

        let start = r1.get("start").and_then(|v| v.as_object()).unwrap();
        assert_eq!(start.get("line").and_then(|v| v.as_u64()), Some(15));

        let extra = r1.get("extra").and_then(|v| v.as_object()).unwrap();
        assert_eq!(extra.get("severity").and_then(|v| v.as_str()), Some("ERROR"));

        // Check second result
        let r2 = &results[1];
        let extra2 = r2.get("extra").and_then(|v| v.as_object()).unwrap();
        assert_eq!(extra2.get("fix").and_then(|v| v.as_str()), Some("Use parameterized queries"));
    }

    #[test]
    fn test_linter_report_serialization() {
        let report = LinterReport {
            linter_name: "semgrep".to_string(),
            execution_time_ms: 1500,
            issues: vec![
                LinterIssue {
                    file: PathBuf::from("test.js"),
                    line: 10,
                    column: Some(5),
                    severity: Severity::Error,
                    code: Some("test-rule".to_string()),
                    message: "Test issue".to_string(),
                    suggestion: None,
                },
            ],
            summary: LinterSummary {
                total: 1,
                errors: 1,
                warnings: 0,
                info: 0,
            },
        };

        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("semgrep"));
        assert!(json.contains("test.js"));
        assert!(json.contains("Test issue"));
    }
}