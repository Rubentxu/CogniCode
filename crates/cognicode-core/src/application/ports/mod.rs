//! Application ports for the file-operations capability (ST-01).
//!
//! These traits are the seams `FileOperationsService` programs against.
//! The concrete adapters live outside `application/`:
//!
//! * [`PathPolicy`] — implemented by `interface::mcp::security`'s
//!   `InputValidator` (the MCP layer keeps owning the security rules; the
//!   application only owns the contract and its error).
//! * [`SyntaxAnalysis`] — implemented by `infrastructure::parser`'s
//!   `TreeSitterSyntaxAnalysis` (tree-sitter specifics, including the
//!   `Language` enum, stay in infrastructure).
//!
//! ST-01 exit gate: `application/services/file_operations.rs` imports
//! neither `crate::infrastructure` nor `crate::interface`. Enforced by
//! `architecture_self_host_e2e` through the canonical constraints.

use crate::application::error::{AppError, AppResult};
use crate::domain::aggregates::symbol::Symbol;
use std::path::Path;

/// Application-owned error for path-policy rejections.
///
/// Mirrors the security layer's failure taxonomy without naming it: the
/// adapter maps its concrete error onto this one, so the application never
/// imports the interface layer (ST-01).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathPolicyError {
    PathTraversalAttempt { path: String },
    PathNotAccessible { path: String },
    PathOutsideWorkspace,
    PathTooDeep { depth: usize, max: usize },
    InvalidPathCharacters { path: String },
    SymlinkDetected { path: String },
}

/// Port for workspace path-safety decisions.
///
/// Deep capability: one call answers "may this service touch this path".
/// Implementations own traversal detection, character policy, symlink and
/// depth rules.
pub trait PathPolicy: Send + Sync {
    fn validate_path(&self, path: &Path) -> Result<(), PathPolicyError>;
}

/// Result of a parse-cleanliness probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxReport {
    /// The source parses into a tree without error nodes.
    pub clean: bool,
}

/// Port for syntax analysis over source text, keyed by language name.
///
/// Deep capability, deliberately wider than a single parse: the
/// application asks for outcomes (symbols, cleanliness) and the adapter
/// owns which parser technology and language identifiers answer them.
/// The language key is the lower-case language name the service already
/// derives from file extensions (e.g. `"rust"`, `"typescript"`).
pub trait SyntaxAnalysis: Send + Sync {
    /// Extract every symbol of every kind found in `source`.
    fn find_all_symbols(&self, language: &str, source: &str) -> AppResult<Vec<Symbol>>;

    /// Probe whether `source` parses cleanly in `language`.
    fn parses_cleanly(&self, language: &str, source: &str) -> AppResult<SyntaxReport>;
}

/// Bridge helper so adapters can surface infrastructure failures with the
/// application's error vocabulary.
pub fn parser_failure(context: &str, cause: impl std::fmt::Display) -> AppError {
    AppError::InvalidParameter(format!("{context}: {cause}"))
}

/// Port for measuring the complexity of source text.
///
/// Deep capability, and deliberately *not* a parser factory. The obvious
/// cheaper shape was to hand application a `dyn Parser` and let
/// `get_complexity` walk the tree itself; that moves the import without moving
/// the coupling, because every signature on the walk still said
/// `tree_sitter::Node`. What the application actually wants is the four
/// numbers, so that is what crosses the boundary.
///
/// Keyed by lower-case language name, like [`SyntaxAnalysis`]: the application
/// classifies the file by extension and states the answer, and the adapter is
/// spared a second opinion about which extension means what.
pub trait ComplexityAnalysis: Send + Sync {
    /// Cyclomatic, cognitive and structural complexity of `source`.
    ///
    /// `function` narrows the measurement to one function; `None` measures the
    /// first one found, which is what the existing behaviour did.
    fn measure(
        &self,
        language: &str,
        source: &str,
        function: Option<&str>,
    ) -> AppResult<crate::application::dto::ComplexityResult>;
}
