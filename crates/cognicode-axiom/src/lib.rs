//! cognicode-axiom — Quality analysis engine for CogniCode
//!
//! Native Rust code quality analysis: 18 built-in rules, quality gates,
//! SQALE technical debt, A-E ratings, BLAKE3 duplication detection,
//! and CallGraph-powered dead code analysis.
//!
//! # Architecture
//!
//! - **rules**: 18 rules + RuleRegistry with inventory auto-discovery + CallGraph helpers
//! - **quality**: SOLID analysis, connascence metrics, LCOM, quality deltas, boundaries
//! - **linters**: External tool wrappers (clippy, eslint, semgrep)
//! - **mcp**: MCP tool definitions for integration with cognicode-quality
//!
//! # Quick Start
//!
//! ```ignore
//! use cognicode_axiom::rules::RuleRegistry;
//!
//! let registry = RuleRegistry::discover();
//! println!("{} rules registered", registry.all().len());
//! ```

pub mod error;
pub mod rules;
pub mod quality;
pub mod linters;
pub mod mcp;
pub mod smells; // Code smells (re-exports from rules)

// Re-export core types
pub use error::{AxiomError, AxiomResult, ValidationDiagnostic, DiagnosticSeverity};

// Re-export rule engine types
pub use rules::{
    RuleRegistry, Severity, Category, Issue, Remediation, RuleEntry,
    Rule, RuleContext, FileMetrics,
};
