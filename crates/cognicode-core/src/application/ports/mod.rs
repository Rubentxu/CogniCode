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

/// Port for the call graph that a session and an analysis service agree on.
///
/// `GraphCache` is genuinely infrastructure — an `ArcSwap` over a versioned
/// ring with a pluggable snapshot provider — and `AnalysisService` used to name
/// it in a public signature, which put a concrete cache in application code's
/// hands. Worse, `WorkspaceSession` built one and handed it over, so the
/// application decided which cache implementation the product ships.
///
/// The port is four methods because four is what the call sites actually use,
/// measured rather than mirrored: `get` and `set` in `AnalysisService`,
/// `current_id`/`get_at` in `CachedGraphStore`, `subscribe` in `WorkspaceSession`. The
/// other thirteen methods on the concrete cache — retention, provider wiring,
/// incremental updates, per-checkpoint reads — are used by other layers that
/// are allowed to name the concrete type, and a port that re-exported all of
/// them would be a second copy of the class rather than a seam.
///
/// Every type in these signatures is domain vocabulary: `CallGraph` and
/// `GraphEvent` are domain, and `CheckpointId` is already
/// `domain::value_objects::CheckpointId` re-exported through
/// `infrastructure::graph::checkpoint`.
pub trait SharedGraph: Send + Sync {
    /// The current graph. Cheap: it hands back a shared pointer, not a copy.
    fn get(&self) -> std::sync::Arc<crate::domain::aggregates::CallGraph>;

    /// Publish a new head and notify subscribers.
    ///
    /// Named `replace` rather than `set` because the concrete cache returns the
    /// new `CheckpointId` and no caller in the application layer ever read it;
    /// `CachedGraphStore` asks for it explicitly through [`Self::current_id`]
    /// when it needs it. Two methods with one name and different signatures
    /// would also have shadowed each other at every concrete call site.
    fn replace(&self, graph: crate::domain::aggregates::CallGraph);

    /// Revision of the current head, if there is one.
    fn current_id(&self) -> Option<crate::domain::value_objects::CheckpointId>;

    /// The graph as of a specific revision, while that revision is still
    /// retained.
    ///
    /// Snapshot isolation (ADR-035) is a property the store promises its
    /// callers, so a reader that pinned an older revision needs to be able to
    /// go back to it. `None` means the revision was evicted or never existed;
    /// the port does not pretend those are the same thing.
    fn get_at(
        &self,
        id: crate::domain::value_objects::CheckpointId,
    ) -> Option<std::sync::Arc<crate::domain::aggregates::CallGraph>>;

    /// Feed of graph-change events, for callers that need to react to a rebuild.
    fn subscribe(&self) -> tokio::sync::broadcast::Receiver<crate::domain::events::GraphEvent>;
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

/// Port for finding symbols by name.
///
/// The session used to hold an `Option<SemanticSearchService>` behind a lock
/// and build it itself, which put a concrete index in the application's hands
/// and made the choice of index part of application code. Three methods,
/// because three is what it called: index a workspace, index one file, answer
/// a query.
pub trait SymbolSearch: Send + Sync {
    /// Index every parseable file under `root`.
    fn index_workspace(&self, root: &std::path::Path) -> AppResult<()>;

    /// Index (or re-index) one file.
    fn index_file(&self, path: &std::path::Path) -> AppResult<()>;

    /// Symbols matching `query`, best first.
    fn search(
        &self,
        query: &crate::domain::value_objects::SymbolSearchQuery,
    ) -> Vec<crate::domain::aggregates::symbol::Symbol>;
}

/// Port for reading the source a symbol occupies.
pub trait SymbolSource: Send + Sync {
    /// The source text of the symbol starting at `line`/`column` in `path`.
    fn source_at(&self, path: &std::path::Path, line: u32, column: u32) -> AppResult<String>;
}
