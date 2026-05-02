//! Error types for cognicode-axiom
//!
//! Unified error hierarchy covering all axiom domains:
//! rule management, quality analysis, and linters.

use std::path::PathBuf;

/// Unified error type for all axiom operations
#[derive(Debug, thiserror::Error)]
pub enum AxiomError {
    // ── Rule Management Errors ────────────────────────────────
    #[error("Rule not found: {rule_id}")]
    RuleNotFound { rule_id: String },

    #[error("Rule already exists: {rule_id}")]
    RuleAlreadyExists { rule_id: String },

    #[error("Rule validation failed: {message}")]
    RuleValidation {
        message: String,
        diagnostics: Vec<ValidationDiagnostic>,
    },

    // ── Quality Analysis Errors ───────────────────────────────
    #[error("Quality analysis error: {message}")]
    Quality { message: String },

    #[error("Call graph not available — build_graph must be called first")]
    CallGraphNotAvailable,

    #[error("Symbol not found: {symbol_name}")]
    SymbolNotFound { symbol_name: String },

    // ── Linter Errors ─────────────────────────────────────────
    #[error("Linter '{linter}' execution failed: {message}")]
    LinterExecution { linter: String, message: String },

    #[error("Linter '{linter}' not found in PATH")]
    LinterNotFound { linter: String },

    #[error("Linter output parse error: {message}")]
    LinterParse { message: String },

    // ── I/O Errors ────────────────────────────────────────────
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("IO error for {context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    // ── Serialization Errors ──────────────────────────────────
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    // ── General ───────────────────────────────────────────────
    #[error("{0}")]
    Other(String),
}

/// A diagnostic message from validation (syntax, semantic, or schema checks)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ValidationDiagnostic {
    /// Severity level
    pub severity: DiagnosticSeverity,
    /// Human-readable message
    pub message: String,
    /// Optional line number (1-indexed)
    pub line: Option<usize>,
    /// Optional column number (1-indexed)
    pub column: Option<usize>,
    /// Error code if available
    pub code: Option<String>,
}

/// Severity of a validation diagnostic
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// Result type alias for axiom operations
pub type AxiomResult<T> = Result<T, AxiomError>;

impl AxiomError {
    /// Create a quality analysis error
    pub fn quality(msg: impl Into<String>) -> Self {
        Self::Quality {
            message: msg.into(),
        }
    }
}
