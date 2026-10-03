//! Asking a question about symbols.
//!
//! `SearchQuery` and `SearchSymbolKind` lived in
//! `infrastructure::semantic`, which meant the application layer imported a
//! query type from the adapter that happens to execute it. They are not
//! adapter concerns: "find me functions and structs whose name looks like
//! this" is the question, not how the index that answers it is stored.
//!
//! The kind is now `domain::value_objects::SymbolKind`, which already existed
//! and already meant this. `SearchSymbolKind` was nine of its twenty-four
//! variants with a `to_symbol_kind()` that mapped each to itself — a second
//! vocabulary for one idea, kept in step by hand. It is gone.
//!
//! Deliberately **not** named `SearchQuery`. `domain::traits::search_provider`
//! already defines a `SearchQuery` for something else entirely: pattern and
//! regex search over text, with scopes and replacements. Two types with one
//! name and different meanings is a trap for whoever reaches for the wrong
//! one, so this one says what it searches over.
//!
//! One thing moved with the type: the label table. `"function" => Function`
//! appeared in `WorkspaceSession::map_kind_string` and again, verbatim, in the
//! MCP handler. It lives here now, once.

use crate::domain::value_objects::SymbolKind;

/// A request for symbols whose names match a query.
#[derive(Debug, Clone)]
pub struct SymbolSearchQuery {
    /// The search query string.
    pub query: String,
    /// Restricts results to these kinds. Empty means "any kind".
    pub kinds: Vec<SymbolKind>,
    /// Maximum number of results to return.
    pub max_results: usize,
}

impl Default for SymbolSearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            kinds: Vec::new(),
            max_results: 50,
        }
    }
}

impl SymbolKind {
    /// The symbol kinds a search filter accepts, by label.
    ///
    /// Narrower than [`std::str::FromStr::from_str`] on purpose, and the
    /// difference is behaviour, not taste. `FromStr` knows twenty-four kinds;
    /// the search tool documents nine. An unrecognised filter has always been
    /// dropped silently rather than treated as an error, so widening this to
    /// match `FromStr` would quietly start honouring filters nobody promised.
    /// Narrow it here rather than by accident in two call sites.
    pub fn from_search_label(label: &str) -> Option<Self> {
        match label.to_lowercase().as_str() {
            "function" => Some(SymbolKind::Function),
            "class" => Some(SymbolKind::Class),
            "method" => Some(SymbolKind::Method),
            "variable" => Some(SymbolKind::Variable),
            "trait" => Some(SymbolKind::Trait),
            "struct" => Some(SymbolKind::Struct),
            "enum" => Some(SymbolKind::Enum),
            "module" => Some(SymbolKind::Module),
            "constant" => Some(SymbolKind::Constant),
            _ => None,
        }
    }
}
