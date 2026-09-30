//! ST-01 adapter: [`SyntaxAnalysis`] over the tree-sitter stack.
//!
//! Owns everything `application/services/file_operations.rs` must not
//! know about: the `Language` enum, per-call `TreeSitterParser`
//! construction, and the parse-tree error-node probe. The application
//! asks by language name (the lower-case string its own
//! extension-detection already produces) and receives outcomes.
//!
//! ## This is not a second `Parser`
//!
//! [`domain::traits::Parser`] already exists and already declares a
//! `find_all_symbols`. The overlap is deliberate and the difference is not
//! cosmetic:
//!
//! * `Parser` is **per-language**: one instance, `language()` reports which,
//!   and the caller already knows the language before it calls.
//! * `SyntaxAnalysis` is a **language-keyed facade**: the service handles a
//!   workspace of mixed files, resolves the key itself on every call, and
//!   never holds a `Language`.
//!
//! Every method here delegates to a `TreeSitterParser` (which is the one
//! [`Parser`] implementation) rather than re-deriving symbols or error
//! nodes. So the parsing semantics have exactly one home; this type only
//! chooses the language and translates the error vocabulary. If a future
//! caller needs the AST itself, reach for `Parser` and widen
//! `SyntaxAnalysis` — do not fork the symbol extraction here.

use crate::application::error::{AppError, AppResult};
use crate::application::ports::{SyntaxAnalysis, SyntaxReport, parser_failure};
use crate::domain::aggregates::symbol::Symbol;
use crate::domain::traits::Parser;
use crate::infrastructure::parser::tree_sitter_parser::TreeSitterParser;

use crate::infrastructure::parser::Language;

/// Tree-sitter backed [`SyntaxAnalysis`] adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeSitterSyntaxAnalysis;

impl TreeSitterSyntaxAnalysis {
    pub fn new() -> Self {
        Self
    }

    /// Maps the service's language key onto the concrete language enum.
    fn language_for(key: &str) -> Option<Language> {
        match key {
            "rust" => Some(Language::Rust),
            "python" => Some(Language::Python),
            "javascript" => Some(Language::JavaScript),
            "typescript" => Some(Language::TypeScript),
            "go" => Some(Language::Go),
            "java" => Some(Language::Java),
            _ => None,
        }
    }
}

impl SyntaxAnalysis for TreeSitterSyntaxAnalysis {
    fn find_all_symbols(&self, language: &str, source: &str) -> AppResult<Vec<Symbol>> {
        let language = Self::language_for(language).ok_or_else(|| {
            AppError::InvalidParameter(format!("unsupported language: {language}"))
        })?;
        let parser =
            TreeSitterParser::new(language).map_err(|e| parser_failure("Parser error", e))?;
        parser
            .find_all_symbols(source)
            .map_err(|e| parser_failure("Symbol extraction error", e))
    }

    fn parses_cleanly(&self, language: &str, source: &str) -> AppResult<SyntaxReport> {
        let language = Self::language_for(language).ok_or_else(|| {
            AppError::InvalidParameter(format!("unsupported language: {language}"))
        })?;
        let parser =
            TreeSitterParser::new(language).map_err(|e| parser_failure("Parser error", e))?;
        let tree = parser
            .parse_tree(source)
            .map_err(|e| parser_failure("Parse error", e))?;
        Ok(SyntaxReport {
            clean: !TreeSitterParser::has_error_nodes(&tree),
        })
    }
}
