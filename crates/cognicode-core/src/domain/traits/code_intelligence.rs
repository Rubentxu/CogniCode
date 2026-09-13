//! Trait for code intelligence operations
//!
//! Provides methods for extracting and analyzing code symbols and their relationships.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(dead_code)]

use crate::domain::aggregates::Symbol;
use crate::domain::value_objects::{Location, Provenance, SymbolKind};
use async_trait::async_trait;
use std::fmt;
use std::str::FromStr;

/// Provider for code intelligence operations
#[async_trait]
pub trait CodeIntelligenceProvider: Send + Sync {
    /// Gets all symbols in a file or directory
    async fn get_symbols(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<Symbol>, CodeIntelligenceError>;

    /// Finds all references to a symbol at the given location
    async fn find_references(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> Result<Vec<Reference>, CodeIntelligenceError>;

    /// Gets the type hierarchy for a symbol
    async fn get_hierarchy(
        &self,
        location: &Location,
    ) -> Result<TypeHierarchy, CodeIntelligenceError>;

    /// Gets the definition location for a reference
    async fn get_definition(
        &self,
        location: &Location,
    ) -> Result<Option<Location>, CodeIntelligenceError>;

    /// Gets document symbols for a file
    async fn get_document_symbols(
        &self,
        path: &std::path::Path,
    ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError>;

    /// Gets hover information (type + docs) for a symbol at the given location
    async fn hover(&self, location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError>;
}

/// Precision tier of a code-intelligence provider, ordered from lowest
/// precision (`S0`) to highest (`S4`).
///
/// The scale is fixed and shared by the composite tier pipeline (LSI M4):
///
/// * `S0` — tree-sitter heuristics (no cross-file resolution).
/// * `S1` — local resolver (workspace-index-backed resolution).
/// * `S2` — language server (LSP) observations.
/// * `S3` — SCIP/LSIF index. Reserved: no M4 producer exists.
/// * `S4` — compiler IR. Reserved: no M4 producer exists.
///
/// `Ord` follows declaration order, so a more precise tier compares
/// greater and "highest-first" iteration is descending order.
///
/// **Never persisted.** This type deliberately implements no serde
/// traits: a tier crosses persistence only as a `tier=<Tier>` provenance
/// detail string, never as an enum variant inside a bincode blob
/// (adding persisted variants is a breaking blob-format change).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrecisionTier {
    /// Tree-sitter heuristics.
    S0,
    /// Local resolver backed by the workspace index.
    S1,
    /// Language server (LSP) observations.
    S2,
    /// SCIP/LSIF index. Reserved for a future milestone.
    S3,
    /// Compiler IR. Reserved for a future milestone.
    S4,
}

impl PrecisionTier {
    /// The canonical string form (`"S0"` … `"S4"`).
    pub const fn as_str(self) -> &'static str {
        match self {
            PrecisionTier::S0 => "S0",
            PrecisionTier::S1 => "S1",
            PrecisionTier::S2 => "S2",
            PrecisionTier::S3 => "S3",
            PrecisionTier::S4 => "S4",
        }
    }

    /// The provenance class the fact bridge binds to this tier.
    ///
    /// * `S2` (LSP observation) → [`Provenance::Extracted`]
    /// * `S1` (local resolver) → [`Provenance::Inferred`]
    /// * `S0` (tree-sitter heuristic) → [`Provenance::Ambiguous`]
    /// * `S3`/`S4` → `Extracted` (index/compiler grade). Unreachable in M4:
    ///   no producer exists for the reserved tiers.
    pub const fn provenance_class(self) -> Provenance {
        match self {
            PrecisionTier::S0 => Provenance::Ambiguous,
            PrecisionTier::S1 => Provenance::Inferred,
            PrecisionTier::S2 => Provenance::Extracted,
            // Reserved tiers are index/compiler grade by declaration; no M4
            // producer can reach them yet.
            PrecisionTier::S3 | PrecisionTier::S4 => Provenance::Extracted,
        }
    }
}

impl fmt::Display for PrecisionTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for PrecisionTier {
    type Err = ();

    /// Parses the canonical `Display` strings (`"S0"` … `"S4"`).
    /// Returns `Err(())` for any other input.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "S0" => Ok(PrecisionTier::S0),
            "S1" => Ok(PrecisionTier::S1),
            "S2" => Ok(PrecisionTier::S2),
            "S3" => Ok(PrecisionTier::S3),
            "S4" => Ok(PrecisionTier::S4),
            _ => Err(()),
        }
    }
}

/// Why an attempted provider tier did not serve a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderOutcome {
    /// The provider (or its backing server) is not available.
    Unavailable,
    /// The provider was attempted and failed with an error.
    Error,
    /// The provider answered successfully but with a degraded (empty)
    /// result, so a lower tier was tried.
    Degraded,
}

impl fmt::Display for ProviderOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProviderOutcome::Unavailable => "unavailable",
            ProviderOutcome::Error => "error",
            ProviderOutcome::Degraded => "degraded",
        };
        f.write_str(s)
    }
}

/// Structured diagnostic describing one failed or degraded tier attempt.
///
/// Diagnostics travel attached to the result ([`Tiered::diagnostics`]) and
/// as the payload of [`TieredOutcome::Unresolved`]; they are also emitted
/// through `tracing` by the composite pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDiagnostic {
    /// Identity of the provider that was attempted (e.g. `"lsp"`).
    pub provider: String,
    /// The tier the provider was attempted for.
    pub attempted_tier: PrecisionTier,
    /// Why the attempt did not serve.
    pub outcome: ProviderOutcome,
    /// Human-readable detail (never a structured error type; consumers
    /// must not parse it).
    pub message: String,
}

impl ProviderDiagnostic {
    /// Builds a diagnostic for one attempted tier.
    pub fn new(
        provider: impl Into<String>,
        attempted_tier: PrecisionTier,
        outcome: ProviderOutcome,
        message: impl Into<String>,
    ) -> Self {
        Self {
            provider: provider.into(),
            attempted_tier,
            outcome,
            message: message.into(),
        }
    }
}

impl fmt::Display for ProviderDiagnostic {
    /// `"<provider>@<tier> <outcome>: <message>"`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{} {}: {}",
            self.provider, self.attempted_tier, self.outcome, self.message
        )
    }
}

/// A value served by a specific precision tier, together with the
/// diagnostics of the lower-priority attempts made before it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tiered<T> {
    /// The served value.
    pub value: T,
    /// The tier that produced the value.
    pub tier: PrecisionTier,
    /// Diagnostics of attempts that preceded (and fell through to) `tier`.
    pub diagnostics: Vec<ProviderDiagnostic>,
}

impl<T> Tiered<T> {
    /// A served value with no prior diagnostics.
    pub fn new(value: T, tier: PrecisionTier) -> Self {
        Self {
            value,
            tier,
            diagnostics: Vec::new(),
        }
    }

    /// A served value carrying the diagnostics of prior failed attempts.
    pub fn with_diagnostics(
        value: T,
        tier: PrecisionTier,
        diagnostics: Vec<ProviderDiagnostic>,
    ) -> Self {
        Self {
            value,
            tier,
            diagnostics,
        }
    }
}

/// Result of a tiered code-intelligence query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TieredOutcome<T> {
    /// Some tier produced a value; the tier and prior diagnostics travel
    /// with it.
    Served(Tiered<T>),
    /// No tier produced a result. The diagnostics name every attempted
    /// tier and outcome; callers must not fabricate a value.
    Unresolved(Vec<ProviderDiagnostic>),
}

impl<T> TieredOutcome<T> {
    /// A value served by `tier` with no prior diagnostics.
    pub fn served(value: T, tier: PrecisionTier) -> Self {
        TieredOutcome::Served(Tiered::new(value, tier))
    }

    /// A value served by `tier` after the diagnostics in `diagnostics`.
    pub fn served_with(
        value: T,
        tier: PrecisionTier,
        diagnostics: Vec<ProviderDiagnostic>,
    ) -> Self {
        TieredOutcome::Served(Tiered::with_diagnostics(value, tier, diagnostics))
    }

    /// No tier served; the diagnostics record the exhausted attempts.
    pub fn unresolved(diagnostics: Vec<ProviderDiagnostic>) -> Self {
        TieredOutcome::Unresolved(diagnostics)
    }

    /// The serving tier, when the outcome is served.
    pub fn tier(&self) -> Option<PrecisionTier> {
        match self {
            TieredOutcome::Served(tiered) => Some(tiered.tier),
            TieredOutcome::Unresolved(_) => None,
        }
    }

    /// The served value, when the outcome is served.
    pub fn value(&self) -> Option<&T> {
        match self {
            TieredOutcome::Served(tiered) => Some(&tiered.value),
            TieredOutcome::Unresolved(_) => None,
        }
    }

    /// True when some tier served a result.
    pub fn is_served(&self) -> bool {
        matches!(self, TieredOutcome::Served(_))
    }

    /// Diagnostics attached to the outcome (empty when served with no
    /// prior fall-through).
    pub fn diagnostics(&self) -> &[ProviderDiagnostic] {
        match self {
            TieredOutcome::Served(tiered) => &tiered.diagnostics,
            TieredOutcome::Unresolved(diagnostics) => diagnostics,
        }
    }
}

/// Additive tiered sibling of [`CodeIntelligenceProvider`].
///
/// Every operation returns a [`TieredOutcome`] declaring the serving tier
/// and carrying the structured diagnostics of failed higher-priority
/// attempts. Implementors: the composite tier pipeline and test doubles
/// that inject tiered observations.
///
/// Method names carry a `_tiered` suffix so a type may implement both this
/// trait and [`CodeIntelligenceProvider`] without method-resolution
/// ambiguity. The existing trait is intentionally untouched (no signature
/// ripple to its `dyn` consumers).
#[async_trait]
pub trait TieredCodeIntelligenceProvider: Send + Sync {
    /// Tiered variant of [`CodeIntelligenceProvider::get_symbols`].
    async fn get_symbols_tiered(&self, path: &std::path::Path) -> TieredOutcome<Vec<Symbol>>;

    /// Tiered variant of [`CodeIntelligenceProvider::find_references`].
    async fn find_references_tiered(
        &self,
        location: &Location,
        include_declaration: bool,
    ) -> TieredOutcome<Vec<Reference>>;

    /// Tiered variant of [`CodeIntelligenceProvider::get_hierarchy`].
    async fn get_hierarchy_tiered(&self, location: &Location) -> TieredOutcome<TypeHierarchy>;

    /// Tiered variant of [`CodeIntelligenceProvider::get_definition`].
    async fn get_definition_tiered(&self, location: &Location) -> TieredOutcome<Option<Location>>;

    /// Tiered variant of [`CodeIntelligenceProvider::get_document_symbols`].
    async fn get_document_symbols_tiered(
        &self,
        path: &std::path::Path,
    ) -> TieredOutcome<Vec<DocumentSymbol>>;

    /// Tiered variant of [`CodeIntelligenceProvider::hover`].
    async fn hover_tiered(&self, location: &Location) -> TieredOutcome<Option<HoverInfo>>;
}

/// Represents a reference to a symbol
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Reference {
    /// The location of the reference
    pub location: Location,
    /// The kind of reference (read, write, call, etc.)
    pub reference_kind: ReferenceKind,
    /// Optional container context (e.g., enclosing function)
    pub container: Option<String>,
}

/// Kind of reference
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReferenceKind {
    /// Reading a variable or calling a function
    Read,
    /// Writing to a variable
    Write,
    /// Calling a function or method
    Call,
    /// Type reference (using a class, struct, etc.)
    Type,
    /// Import statement
    Import,
}

impl ReferenceKind {
    /// Returns true if this is a read-like reference
    pub fn is_read(&self) -> bool {
        matches!(
            self,
            ReferenceKind::Read | ReferenceKind::Call | ReferenceKind::Type
        )
    }

    /// Returns true if this is a write-like reference
    pub fn is_write(&self) -> bool {
        matches!(self, ReferenceKind::Write)
    }
}

/// Type hierarchy information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchy {
    /// The symbol this hierarchy is for
    pub symbol: Symbol,
    /// Parents/super types (for inheritance)
    pub parents: Vec<TypeHierarchyNode>,
    /// Children/sub types
    pub children: Vec<TypeHierarchyNode>,
}

/// A node in the type hierarchy
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeHierarchyNode {
    /// The symbol at this node
    pub symbol: Symbol,
    /// Distance from the root (0 = immediate parent/child)
    pub distance: u32,
}

/// A document symbol extracted from source
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSymbol {
    /// The symbol
    pub symbol: Symbol,
    /// The kind of document symbol
    pub document_kind: DocumentSymbolKind,
    /// Range in the source
    pub range: crate::domain::value_objects::SourceRange,
    /// Children (for nested symbols)
    pub children: Vec<DocumentSymbol>,
}

/// Kind of document symbol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentSymbolKind {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Event,
    Operator,
    TypeParameter,
}

impl DocumentSymbolKind {
    /// Converts to a SymbolKind
    pub fn to_symbol_kind(&self) -> SymbolKind {
        match self {
            DocumentSymbolKind::Class => SymbolKind::Class,
            DocumentSymbolKind::Method => SymbolKind::Method,
            DocumentSymbolKind::Function => SymbolKind::Function,
            DocumentSymbolKind::Variable => SymbolKind::Variable,
            DocumentSymbolKind::Constant => SymbolKind::Constant,
            DocumentSymbolKind::Field => SymbolKind::Property,
            DocumentSymbolKind::Enum => SymbolKind::Enum,
            DocumentSymbolKind::Interface => SymbolKind::Interface,
            DocumentSymbolKind::Constructor => SymbolKind::Constructor,
            DocumentSymbolKind::Module | DocumentSymbolKind::Namespace => SymbolKind::Module,
            DocumentSymbolKind::TypeParameter => SymbolKind::Type,
            _ => SymbolKind::Variable,
        }
    }
}

/// Error type for code intelligence operations
#[derive(Debug, thiserror::Error)]
pub enum CodeIntelligenceError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Invalid location: {0}")]
    InvalidLocation(String),

    #[error("Language not supported: {0}")]
    LanguageNotSupported(String),

    #[error("LSP server unavailable for {language}: {message}")]
    LspUnavailable {
        language: String,
        message: String,
        install_command: String,
    },

    #[error("LSP server error: {0}")]
    LspError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Hover information for a symbol
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HoverInfo {
    /// The content (type signature, documentation, etc.)
    pub content: String,
    /// Optional documentation string
    pub documentation: Option<String>,
    /// The kind of hover result
    pub kind: HoverKind,
}

/// Kind of hover information
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HoverKind {
    /// Type information
    Type,
    /// Documentation only
    Documentation,
    /// Mixed type and documentation
    Mixed,
    /// Source code snippet (tree-sitter fallback)
    Snippet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::value_objects::SourceRange;

    struct MockCodeIntelligence;

    impl MockCodeIntelligence {
        fn new() -> Self {
            MockCodeIntelligence
        }

        fn create_test_location(file: &str, line: u32, col: u32) -> Location {
            Location::new(file, line, col)
        }

        fn create_test_symbol(name: &str, kind: SymbolKind, loc: Location) -> Symbol {
            Symbol::new(name, kind, loc)
        }

        fn create_test_source_range(
            file: &str,
            start_line: u32,
            start_col: u32,
            end_line: u32,
            end_col: u32,
        ) -> SourceRange {
            SourceRange::new(
                Location::new(file, start_line, start_col),
                Location::new(file, end_line, end_col),
            )
        }
    }

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for MockCodeIntelligence {
        async fn get_symbols(
            &self,
            path: &std::path::Path,
        ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 0, 0);
            Ok(vec![
                Symbol::new("main", SymbolKind::Function, loc.clone()),
                Symbol::new("MyStruct", SymbolKind::Class, loc),
            ])
        }

        async fn find_references(
            &self,
            location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            Ok(vec![
                Reference {
                    location: location.clone(),
                    reference_kind: ReferenceKind::Read,
                    container: Some("main".to_string()),
                },
                Reference {
                    location: Location::new("other.rs", location.line(), location.column()),
                    reference_kind: ReferenceKind::Write,
                    container: None,
                },
            ])
        }

        async fn get_hierarchy(
            &self,
            location: &Location,
        ) -> Result<TypeHierarchy, CodeIntelligenceError> {
            let symbol = Symbol::new("TestClass", SymbolKind::Class, location.clone());
            Ok(TypeHierarchy {
                symbol: symbol.clone(),
                parents: vec![TypeHierarchyNode {
                    symbol: Symbol::new(
                        "ParentClass",
                        SymbolKind::Class,
                        Location::new("parent.rs", 0, 0),
                    ),
                    distance: 1,
                }],
                children: vec![TypeHierarchyNode {
                    symbol: Symbol::new(
                        "ChildClass",
                        SymbolKind::Class,
                        Location::new("child.rs", 0, 0),
                    ),
                    distance: 1,
                }],
            })
        }

        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            Ok(Some(Location::new("definition.rs", 10, 5)))
        }

        async fn get_document_symbols(
            &self,
            path: &std::path::Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            let loc = Location::new(path.to_str().unwrap_or("test.rs"), 5, 0);
            Ok(vec![
                DocumentSymbol {
                    symbol: Symbol::new("MyFunction", SymbolKind::Function, loc.clone()),
                    document_kind: DocumentSymbolKind::Function,
                    range: SourceRange::new(
                        loc.clone(),
                        Location::new(path.to_str().unwrap_or("test.rs"), 10, 0),
                    ),
                    children: vec![],
                },
                DocumentSymbol {
                    symbol: Symbol::new("MyClass", SymbolKind::Class, loc),
                    document_kind: DocumentSymbolKind::Class,
                    range: SourceRange::new(
                        Location::new(path.to_str().unwrap_or("test.rs"), 15, 0),
                        Location::new(path.to_str().unwrap_or("test.rs"), 25, 0),
                    ),
                    children: vec![],
                },
            ])
        }

        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            Ok(Some(HoverInfo {
                content: "fn main() -> ()".to_string(),
                documentation: Some("The entry point".to_string()),
                kind: HoverKind::Mixed,
            }))
        }
    }

    #[tokio::test]
    async fn test_mock_get_definition() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.get_definition(&loc).await.unwrap();
        assert!(result.is_some());
        let def = result.unwrap();
        assert_eq!(def.file(), "definition.rs");
        assert_eq!(def.line(), 10);
        assert_eq!(def.column(), 5);
    }

    #[tokio::test]
    async fn test_mock_hover() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.hover(&loc).await.unwrap();
        assert!(result.is_some());
        let hover = result.unwrap();
        assert!(hover.content.contains("main"));
        assert!(hover.documentation.is_some());
        assert_eq!(hover.kind, HoverKind::Mixed);
    }

    #[tokio::test]
    async fn test_mock_find_references() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.find_references(&loc, true).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].reference_kind, ReferenceKind::Read);
        assert_eq!(result[1].reference_kind, ReferenceKind::Write);
    }

    #[tokio::test]
    async fn test_mock_get_symbols() {
        let mock = MockCodeIntelligence::new();
        let path = std::path::Path::new("test.rs");
        let result = mock.get_symbols(path).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name(), "main");
        assert_eq!(result[1].name(), "MyStruct");
    }

    #[tokio::test]
    async fn test_mock_get_document_symbols() {
        let mock = MockCodeIntelligence::new();
        let path = std::path::Path::new("test.rs");
        let result = mock.get_document_symbols(path).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].symbol.name(), "MyFunction");
        assert_eq!(result[0].document_kind, DocumentSymbolKind::Function);
        assert_eq!(result[1].symbol.name(), "MyClass");
        assert_eq!(result[1].document_kind, DocumentSymbolKind::Class);
    }

    #[tokio::test]
    async fn test_mock_get_hierarchy() {
        let mock = MockCodeIntelligence::new();
        let loc = Location::new("test.rs", 5, 10);
        let result = mock.get_hierarchy(&loc).await.unwrap();
        assert_eq!(result.symbol.name(), "TestClass");
        assert_eq!(result.parents.len(), 1);
        assert_eq!(result.parents[0].symbol.name(), "ParentClass");
        assert_eq!(result.children.len(), 1);
        assert_eq!(result.children[0].symbol.name(), "ChildClass");
    }

    #[tokio::test]
    async fn test_mock_none_responses() {
        struct MockNoneResponses;
        #[async_trait::async_trait]
        impl CodeIntelligenceProvider for MockNoneResponses {
            async fn get_symbols(
                &self,
                _path: &std::path::Path,
            ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn find_references(
                &self,
                _location: &Location,
                _include_declaration: bool,
            ) -> Result<Vec<Reference>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn get_hierarchy(
                &self,
                _location: &Location,
            ) -> Result<TypeHierarchy, CodeIntelligenceError> {
                let loc = Location::new("empty.rs", 0, 0);
                Ok(TypeHierarchy {
                    symbol: Symbol::new("Empty", SymbolKind::Class, loc.clone()),
                    parents: vec![],
                    children: vec![],
                })
            }
            async fn get_definition(
                &self,
                _location: &Location,
            ) -> Result<Option<Location>, CodeIntelligenceError> {
                Ok(None)
            }
            async fn get_document_symbols(
                &self,
                _path: &std::path::Path,
            ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
                Ok(vec![])
            }
            async fn hover(
                &self,
                _location: &Location,
            ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
                Ok(None)
            }
        }

        let mock = MockNoneResponses;
        let loc = Location::new("test.rs", 5, 10);
        let path = std::path::Path::new("test.rs");

        assert!(mock.get_definition(&loc).await.unwrap().is_none());
        assert!(mock.hover(&loc).await.unwrap().is_none());
        assert!(mock.get_symbols(path).await.unwrap().is_empty());
        assert!(mock.find_references(&loc, true).await.unwrap().is_empty());
        assert!(mock.get_document_symbols(path).await.unwrap().is_empty());
        let hierarchy = mock.get_hierarchy(&loc).await.unwrap();
        assert!(hierarchy.parents.is_empty());
        assert!(hierarchy.children.is_empty());
    }

    // ── LSI M4 tier types ───────────────────────────────────────────────

    #[test]
    fn test_precision_tier_display_round_trips_through_from_str() {
        for tier in [
            PrecisionTier::S0,
            PrecisionTier::S1,
            PrecisionTier::S2,
            PrecisionTier::S3,
            PrecisionTier::S4,
        ] {
            let text = tier.to_string();
            assert_eq!(text, tier.as_str());
            let parsed: PrecisionTier = text.parse().expect("FromStr must accept Display form");
            assert_eq!(parsed, tier);
        }
        assert_eq!(PrecisionTier::S0.to_string(), "S0");
        assert_eq!(PrecisionTier::S2.to_string(), "S2");
        assert_eq!(PrecisionTier::S4.to_string(), "S4");
    }

    #[test]
    fn test_precision_tier_from_str_rejects_unknown_input() {
        assert!("S5".parse::<PrecisionTier>().is_err());
        assert!("s2".parse::<PrecisionTier>().is_err());
        assert!("".parse::<PrecisionTier>().is_err());
    }

    #[test]
    fn test_precision_tier_order_is_low_to_high() {
        assert!(PrecisionTier::S0 < PrecisionTier::S1);
        assert!(PrecisionTier::S1 < PrecisionTier::S2);
        assert!(PrecisionTier::S2 < PrecisionTier::S3);
        assert!(PrecisionTier::S3 < PrecisionTier::S4);

        // Highest-first iteration is descending order.
        let mut tiers = vec![
            PrecisionTier::S0,
            PrecisionTier::S4,
            PrecisionTier::S2,
            PrecisionTier::S1,
            PrecisionTier::S3,
        ];
        tiers.sort();
        tiers.reverse();
        assert_eq!(
            tiers,
            vec![
                PrecisionTier::S4,
                PrecisionTier::S3,
                PrecisionTier::S2,
                PrecisionTier::S1,
                PrecisionTier::S0,
            ]
        );
    }

    #[test]
    fn test_provenance_class_mapping_is_pinned() {
        assert_eq!(PrecisionTier::S0.provenance_class(), Provenance::Ambiguous);
        assert_eq!(PrecisionTier::S1.provenance_class(), Provenance::Inferred);
        assert_eq!(PrecisionTier::S2.provenance_class(), Provenance::Extracted);
        // Reserved tiers are index/compiler grade; unreachable in M4.
        assert_eq!(PrecisionTier::S3.provenance_class(), Provenance::Extracted);
        assert_eq!(PrecisionTier::S4.provenance_class(), Provenance::Extracted);
    }

    #[test]
    fn test_provider_diagnostic_display_and_outcome_strings() {
        assert_eq!(ProviderOutcome::Unavailable.to_string(), "unavailable");
        assert_eq!(ProviderOutcome::Error.to_string(), "error");
        assert_eq!(ProviderOutcome::Degraded.to_string(), "degraded");

        let diagnostic = ProviderDiagnostic::new(
            "lsp",
            PrecisionTier::S2,
            ProviderOutcome::Unavailable,
            "rust-analyzer not found",
        );
        assert_eq!(diagnostic.provider, "lsp");
        assert_eq!(diagnostic.attempted_tier, PrecisionTier::S2);
        assert_eq!(diagnostic.outcome, ProviderOutcome::Unavailable);
        assert_eq!(
            diagnostic.to_string(),
            "lsp@S2 unavailable: rust-analyzer not found"
        );
    }

    #[test]
    fn test_tiered_outcome_helpers() {
        let diagnostic = ProviderDiagnostic::new(
            "tree-sitter",
            PrecisionTier::S0,
            ProviderOutcome::Error,
            "parse failed",
        );

        let served = TieredOutcome::served_with(42u32, PrecisionTier::S1, vec![diagnostic.clone()]);
        assert!(served.is_served());
        assert_eq!(served.tier(), Some(PrecisionTier::S1));
        assert_eq!(served.value(), Some(&42));
        assert_eq!(served.diagnostics().len(), 1);
        assert_eq!(served.diagnostics()[0], diagnostic);

        let plain = TieredOutcome::served(vec![1, 2], PrecisionTier::S2);
        assert_eq!(plain.tier(), Some(PrecisionTier::S2));
        assert!(plain.diagnostics().is_empty());

        let unresolved: TieredOutcome<u32> = TieredOutcome::unresolved(vec![diagnostic.clone()]);
        assert!(!unresolved.is_served());
        assert_eq!(unresolved.tier(), None);
        assert_eq!(unresolved.value(), None);
        assert_eq!(unresolved.diagnostics(), &[diagnostic]);
    }

    /// A type implementing BOTH traits must be usable through either trait
    /// object: the additive tiered trait is the zero-ripple seam.
    struct DualTraitProvider;

    #[async_trait::async_trait]
    impl CodeIntelligenceProvider for DualTraitProvider {
        async fn get_symbols(
            &self,
            path: &std::path::Path,
        ) -> Result<Vec<Symbol>, CodeIntelligenceError> {
            Ok(vec![Symbol::new(
                "legacy",
                SymbolKind::Function,
                Location::new(path.to_string_lossy().to_string(), 1, 1),
            )])
        }

        async fn find_references(
            &self,
            location: &Location,
            _include_declaration: bool,
        ) -> Result<Vec<Reference>, CodeIntelligenceError> {
            Ok(vec![Reference {
                location: location.clone(),
                reference_kind: ReferenceKind::Call,
                container: None,
            }])
        }

        async fn get_hierarchy(
            &self,
            location: &Location,
        ) -> Result<TypeHierarchy, CodeIntelligenceError> {
            Ok(TypeHierarchy {
                symbol: Symbol::new("Legacy", SymbolKind::Class, location.clone()),
                parents: vec![],
                children: vec![],
            })
        }

        async fn get_definition(
            &self,
            _location: &Location,
        ) -> Result<Option<Location>, CodeIntelligenceError> {
            Ok(None)
        }

        async fn get_document_symbols(
            &self,
            _path: &std::path::Path,
        ) -> Result<Vec<DocumentSymbol>, CodeIntelligenceError> {
            Ok(vec![])
        }

        async fn hover(
            &self,
            _location: &Location,
        ) -> Result<Option<HoverInfo>, CodeIntelligenceError> {
            Ok(None)
        }
    }

    #[async_trait::async_trait]
    impl TieredCodeIntelligenceProvider for DualTraitProvider {
        async fn get_symbols_tiered(&self, path: &std::path::Path) -> TieredOutcome<Vec<Symbol>> {
            TieredOutcome::served(
                vec![Symbol::new(
                    "tiered",
                    SymbolKind::Function,
                    Location::new(path.to_string_lossy().to_string(), 2, 2),
                )],
                PrecisionTier::S0,
            )
        }

        async fn find_references_tiered(
            &self,
            _location: &Location,
            _include_declaration: bool,
        ) -> TieredOutcome<Vec<Reference>> {
            TieredOutcome::unresolved(vec![ProviderDiagnostic::new(
                "lsp",
                PrecisionTier::S2,
                ProviderOutcome::Unavailable,
                "no server",
            )])
        }

        async fn get_hierarchy_tiered(&self, _location: &Location) -> TieredOutcome<TypeHierarchy> {
            TieredOutcome::unresolved(vec![])
        }

        async fn get_definition_tiered(
            &self,
            _location: &Location,
        ) -> TieredOutcome<Option<Location>> {
            TieredOutcome::served(None, PrecisionTier::S2)
        }

        async fn get_document_symbols_tiered(
            &self,
            _path: &std::path::Path,
        ) -> TieredOutcome<Vec<DocumentSymbol>> {
            TieredOutcome::served(vec![], PrecisionTier::S0)
        }

        async fn hover_tiered(&self, _location: &Location) -> TieredOutcome<Option<HoverInfo>> {
            TieredOutcome::served(None, PrecisionTier::S0)
        }
    }

    #[tokio::test]
    async fn test_tiered_trait_is_additive_seam_alongside_legacy_trait() {
        let provider = DualTraitProvider;
        let path = std::path::Path::new("test.rs");
        let loc = Location::new("test.rs", 1, 1);

        // Legacy trait object still works untouched.
        let legacy: &dyn CodeIntelligenceProvider = &provider;
        let legacy_symbols = legacy.get_symbols(path).await.unwrap();
        assert_eq!(legacy_symbols[0].name(), "legacy");
        assert!(legacy.get_definition(&loc).await.unwrap().is_none());

        // Tiered trait object observes tier attribution.
        let tiered: &dyn TieredCodeIntelligenceProvider = &provider;
        let outcome = tiered.get_symbols_tiered(path).await;
        assert_eq!(outcome.tier(), Some(PrecisionTier::S0));
        assert_eq!(outcome.value().unwrap()[0].name(), "tiered");

        let def = tiered.get_definition_tiered(&loc).await;
        assert!(def.is_served());
        assert_eq!(def.tier(), Some(PrecisionTier::S2));
        assert!(def.value().unwrap().is_none());

        let refs = tiered.find_references_tiered(&loc, true).await;
        assert!(!refs.is_served());
        assert_eq!(refs.diagnostics().len(), 1);
        assert_eq!(refs.diagnostics()[0].attempted_tier, PrecisionTier::S2);
    }
}
