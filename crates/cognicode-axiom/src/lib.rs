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

pub mod context;
pub mod error;
pub mod issue;
pub mod registry;
pub mod rules;
pub mod types;
pub mod catalog;

// Submodules
pub mod quality;
pub mod linters;
pub mod mcp;
pub mod smells; // Code smells (re-exports from rules)

// Re-export core types
pub use context::RuleContext;
pub use error::{AxiomError, AxiomResult, ValidationDiagnostic, DiagnosticSeverity};
pub use issue::{Category, Issue, Severity};
pub use registry::RuleRegistry;
pub use types::{RuleId, SrcLanguage, Rule};
pub use rules::types::{CleanCodeAttribute, EntityType, FileMetrics, ParseCache, Remediation, RuleEntry, Scope, SoftwareQuality, SoftwareQualityImpact};
