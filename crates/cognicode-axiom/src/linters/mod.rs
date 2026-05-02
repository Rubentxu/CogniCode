//! External linter wrappers
//!
//! Standardized interface for running external linting tools (clippy, eslint, semgrep)
//! and normalizing their output into a common `LinterReport` format.

pub mod clippy;
pub mod eslint;
pub mod semgrep;

use crate::error::AxiomResult;
use std::path::{Path, PathBuf};

/// Normalized linter report
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinterReport {
    /// Name of the linter that produced this report
    pub linter_name: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Issues found
    pub issues: Vec<LinterIssue>,
    /// Summary counts
    pub summary: LinterSummary,
}

/// A single linter issue
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinterIssue {
    /// File path relative to project root
    pub file: PathBuf,
    /// Line number (1-indexed)
    pub line: usize,
    /// Column number (1-indexed)
    pub column: Option<usize>,
    /// Issue severity
    pub severity: Severity,
    /// Linter-specific code (e.g., "clippy::unwrap_used")
    pub code: Option<String>,
    /// Human-readable message
    pub message: String,
    /// Suggested fix if available
    pub suggestion: Option<String>,
}

/// Severity levels for linter issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Note,
}

/// Summary counts of issues by severity
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LinterSummary {
    pub total: usize,
    pub errors: usize,
    pub warnings: usize,
    pub info: usize,
}

/// Trait for all linter implementations
pub trait Linter: Send + Sync {
    /// Run the linter on the given project path
    fn run(&self, project_path: &Path) -> AxiomResult<LinterReport>;
    /// Name of this linter
    fn name(&self) -> &str;
}

// Re-export linter runners
pub use clippy::ClippyRunner;
pub use eslint::EslintRunner;
pub use semgrep::SemgrepRunner;
