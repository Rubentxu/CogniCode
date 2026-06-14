//! EntryPoint domain types and the entry-point resolver.
//!
//! # Phase 5 Scope (Safe Slice)
//!
//! This module implements Phase 5 of the Moldable View Runtime roadmap
//! (ADR-008). It introduces a typed `EntryPoint` abstraction that
//! converts any user input into a structured resolved form with a
//! default `ViewKind`.
//!
//! ## What's implemented (safe slice)
//!
//! - [`EntryPoint`] enum with all 14 variants from the spec
//! - [`EntryPointParseError`] for parse failures
//! - [`ResolvedEntryPoint`] struct carrying the parsed entry point,
//!   the target object summary, and the suggested default `ViewKind`
//! - [`EntryPoint::parse`] — pure structural parsing of input strings
//!   into the correct `EntryPoint` variant
//! - [`EntryPoint::default_view_kind`] — canonical mapping from each
//!   variant to its default `ViewKind` per CONTEXT.md §Entry Points
//! - `ResolvedEntryPoint::into_parts` accessor
//!
//! ## What's NOT implemented (deferred)
//!
//! - Object resolution (Symbol/File/Scope lookup) — requires repo access
//!   and is deferred to Phase 5 integration work
//! - MCP `entrypoint_resolve` tool — deferred to Phase 5 integration
//! - `SearchResult` → `semantic_search_results` pipeline — deferred
//! - `SavedExploration`, `ViewSpec`, `Decision`, `Doc`, `Issue`,
//!   `Evidence` variants — deferred to Phase 5+ (require repo access)
//!
//! The public API surface is deliberately small and completely additive.
//! No existing code is modified.

use serde::{Deserialize, Serialize};

use crate::dto::{InspectableObjectSummary, ViewKind};

// ============================================================================
// EntryPoint enum
// ============================================================================

/// Every starting point the Explorer accepts, per CONTEXT.md §Entry Points.
///
/// Each variant captures the minimum information needed to identify the
/// entry point and route it to the correct default view.
///
/// # Parsing
///
/// `EntryPoint::parse` attempts to resolve an input string to the most
/// specific variant. Layered matching: longest match wins.
///
/// # Default ViewKind
///
/// Every variant maps to a default [`ViewKind`] via [`EntryPoint::default_view_kind`].
/// The Explorer can override this after the first render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EntryPoint {
    /// An HTTP route, e.g. `POST /api/users`.
    HttpRoute { method: String, path: String },

    /// A CLI command, e.g. `cognicode analyze`.
    CliCommand { name: String },

    /// A domain event name, e.g. `UserCreated`.
    Event { name: String },

    /// A use-case name, e.g. `CreateUser`.
    UseCase { name: String },

    /// A symbol identified by its canonical MVP id.
    ///
    /// Format: `symbol:{file}:{name}:{line}`
    Symbol { id: String },

    /// A file path on disk.
    File { path: String },

    /// A module/scope path, e.g. `crate::foo` or `src/foo`.
    Scope { path: String },

    /// A saved search result set. The resolver returns the full
    /// result list, not a single object.
    SearchResult { ids: Vec<String>, query: String },

    /// A saved exploration session id.
    SavedExploration { id: String },

    /// A persisted ViewSpec id.
    ViewSpec { id: String },

    /// An architecture decision or rationale artifact id.
    Decision { id: String },

    /// A documentation page id.
    Doc { id: String },

    /// A tracked issue id.
    Issue { id: String },

    /// An evidence artifact id.
    Evidence { id: String },
}

impl EntryPoint {
    /// Parse an input string into the most specific [`EntryPoint`] variant.
    ///
    /// Layered matching rules (longest match wins):
    ///
    /// - `METHOD /path` → `HttpRoute { method, path }`
    /// - `symbol:{file}:{name}:{line}` → `Symbol { id }`
    /// - `vs-{uuid}` → `ViewSpec { id }`
    /// - `exp-{uuid}` → `SavedExploration { id }`
    /// - `doc-{…}` → `Doc { id }`
    /// - `adr-{…}` / `ADR-{…}` → `Decision { id }`
    /// - `iss-{…}` → `Issue { id }`
    /// - `ev-{…}` → `Evidence { id }`
    /// - `cognicode …` → `CliCommand { name }`
    /// - `CamelCase` or `PascalCase` → `UseCase { name }` (deferred: needs domain model)
    /// - `/path/to/file.rs` → `File { path }` (deferred: needs filesystem check)
    /// - `src::module` or `crate::module` → `Scope { path }` (deferred: needs repo check)
    /// - Any other string → `Err(EntryPointParseError::NotResolved)`
    ///
    /// This is a pure structural parser — no I/O, no repo access.
    /// Object resolution is deferred to the integration layer.
    pub fn parse(input: &str) -> Result<Self, EntryPointParseError> {
        let input = input.trim();

        if input.is_empty() {
            return Err(EntryPointParseError::EmptyInput);
        }

        // HTTP route: METHOD /path
        if let Some((method, path)) = input.split_once(' ') {
            let method = method.to_uppercase();
            let path = path.trim_start_matches('/');
            if is_http_method(&method) && !path.is_empty() {
                return Ok(EntryPoint::HttpRoute {
                    method,
                    path: path.to_string(),
                });
            }
        }

        // Symbol id: symbol:src/foo.rs:bar:42
        if input.starts_with("symbol:") {
            return Ok(EntryPoint::Symbol {
                id: input.to_string(),
            });
        }

        // ViewSpec id: vs-uuid
        if input.starts_with("vs-") {
            return Ok(EntryPoint::ViewSpec {
                id: input.to_string(),
            });
        }

        // SavedExploration id: exp-uuid
        if input.starts_with("exp-") {
            return Ok(EntryPoint::SavedExploration {
                id: input.to_string(),
            });
        }

        // Decision: adr-… or ADR-…
        let lower = input.to_lowercase();
        if lower.starts_with("adr-") {
            return Ok(EntryPoint::Decision {
                id: input.to_string(),
            });
        }

        // Issue: iss-…
        if lower.starts_with("iss-") {
            return Ok(EntryPoint::Issue {
                id: input.to_string(),
            });
        }

        // Evidence: ev-…
        if lower.starts_with("ev-") {
            return Ok(EntryPoint::Evidence {
                id: input.to_string(),
            });
        }

        // Doc: doc-…
        if lower.starts_with("doc-") {
            return Ok(EntryPoint::Doc {
                id: input.to_string(),
            });
        }

        // CLI command: cognicode …
        if input.starts_with("cognicode ") {
            return Ok(EntryPoint::CliCommand {
                name: input.to_string(),
            });
        }

        // HTTP route fallback: /api/users (treat as path-only HttpRoute)
        if input.starts_with('/') && input.len() > 1 {
            let path = input.trim_start_matches('/');
            // Could be a file path or an HTTP route path
            // Heuristic: if it looks like a file (ends in .rs, .ts, etc.), defer
            // For now, treat as HttpRoute with GET method
            return Ok(EntryPoint::HttpRoute {
                method: "GET".to_string(),
                path: path.to_string(),
            });
        }

        // Event / UseCase heuristics
        // CamelCase PascalCase → Event or UseCase
        // Simple heuristic: starts with capital, no spaces → event/use-case name
        if input.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
            && !input.contains(' ')
            && input.len() <= 64
        {
            // UseCase pattern: verb-noun (e.g. CreateUser, ShipOrder)
            // Event pattern: noun-verb past tense or passive (e.g. UserCreated, OrderShipped)
            // Simple heuristic: last char tells us:
            //   - 'd' ending → past tense → Event (e.g. Created, Shipped)
            //   - other → verb-noun → UseCase
            // This heuristic is not reliable for all cases; full disambiguation
            // requires a domain model and is deferred to Phase 5+.
            let last_char = input.chars().last().unwrap();
            if last_char == 'd' || last_char == 'D' {
                // Likely past tense → Event
                return Ok(EntryPoint::Event {
                    name: input.to_string(),
                });
            } else {
                // Likely verb-noun → UseCase
                return Ok(EntryPoint::UseCase {
                    name: input.to_string(),
                });
            }
        }

        Err(EntryPointParseError::NotResolved(input.to_string()))
    }

    /// Returns the default [`ViewKind`] for this entry point variant.
    ///
    /// The mapping is authoritative per CONTEXT.md §Entry Points:
    ///
    /// | Entry point       | Default ViewKind              |
    /// |-------------------|-------------------------------|
    /// | `HttpRoute`       | `VerticalSlice`               |
    /// | `CliCommand`      | `VerticalSlice`               |
    /// | `Event`           | `DataFlow`                    |
    /// | `UseCase`         | `VerticalSlice`               |
    /// | `Symbol`          | `CallGraph`                   |
    /// | `File`            | `Overview` (custom)           |
    /// | `Scope`           | `Overview` (custom)           |
    /// | `SearchResult`    | `SemanticSearchResults`       |
    /// | `SavedExploration`| `ComposedNarrative`           |
    /// | `ViewSpec`        | (uses the spec's own kind)    |
    /// | `Decision`        | `ArchitectureRationale`       |
    /// | `Doc`             | `DocCodeAlignment`            |
    /// | `Issue`           | `DeadCodeCandidates`          |
    /// | `Evidence`        | `EvidenceView`                |
    ///
    /// The default is a hint, not a hard rule. The Explorer can switch
    /// views from the ViewTabs after the first render.
    pub fn default_view_kind(&self) -> ViewKind {
        match self {
            EntryPoint::HttpRoute { .. } => ViewKind::VerticalSlice,
            EntryPoint::CliCommand { .. } => ViewKind::VerticalSlice,
            EntryPoint::Event { .. } => ViewKind::DataFlow,
            EntryPoint::UseCase { .. } => ViewKind::VerticalSlice,
            EntryPoint::Symbol { .. } => ViewKind::CallGraph,
            // File → Custom("file_overview") signals "file overview" built-in
            EntryPoint::File { .. } => ViewKind::Custom("file_overview".to_string()),
            // Scope → Custom("scope_overview") signals "scope overview" built-in
            EntryPoint::Scope { .. } => ViewKind::Custom("scope_overview".to_string()),
            EntryPoint::SearchResult { .. } => ViewKind::SemanticSearchResults,
            EntryPoint::SavedExploration { .. } => ViewKind::ComposedNarrative,
            // ViewSpec: deferred; caller must supply the spec's own view_kind
            // For the resolver output, we return a safe fallback
            EntryPoint::ViewSpec { .. } => ViewKind::CallGraph,
            EntryPoint::Decision { .. } => ViewKind::ArchitectureRationale,
            EntryPoint::Doc { .. } => ViewKind::DocCodeAlignment,
            EntryPoint::Issue { .. } => ViewKind::DeadCodeCandidates,
            EntryPoint::Evidence { .. } => ViewKind::EvidenceView,
        }
    }
}

/// Returns `true` if `method` looks like a valid HTTP method.
fn is_http_method(method: &str) -> bool {
    matches!(
        method,
        "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS" | "TRACE" | "CONNECT"
    )
}

// ============================================================================
// ResolvedEntryPoint
// ============================================================================

/// The result of resolving an entry-point input string.
///
/// Carries the parsed [`EntryPoint`] variant, the resolved
/// [`InspectableObjectSummary`] of the target object (when available),
/// and the suggested default [`ViewKind`].
///
/// # Phase 5 note
///
/// Object resolution (Symbol/File/Scope lookup) is deferred to the
/// integration layer. In this safe slice, `target` is always `None`
/// and callers must resolve the object themselves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEntryPoint {
    /// The parsed entry-point variant.
    pub ep: EntryPoint,

    /// The resolved inspectable object, when available.
    /// In Phase 5 safe slice this is always `None` — callers must
    /// resolve the object after receiving the parsed `ep`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<InspectableObjectSummary>,

    /// The suggested default `ViewKind` for this entry point.
    /// This is `ep.default_view_kind()` but is stored explicitly so
    /// callers can read it without a method call.
    pub default_view_kind: ViewKind,
}

impl ResolvedEntryPoint {
    /// Build a `ResolvedEntryPoint` from a parsed [`EntryPoint`].
    ///
    /// `target` is set to `None` in this safe slice — callers must
    /// resolve the object themselves.
    ///
    /// # Example
    ///
    /// ```
    /// use cognicode_explorer::domain::entry_point::{EntryPoint, ResolvedEntryPoint};
    ///
    /// let ep = EntryPoint::parse("POST /api/users").unwrap();
    /// let resolved = ResolvedEntryPoint::from_ep(ep);
    /// assert!(resolved.target.is_none());
    /// ```
    pub fn from_ep(ep: EntryPoint) -> Self {
        let default_view_kind = ep.default_view_kind();
        Self {
            ep,
            target: None,
            default_view_kind,
        }
    }
}

// ============================================================================
// EntryPointParseError
// ============================================================================

/// Errors that can occur when parsing an input string into an [`EntryPoint`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryPointParseError {
    /// The input string was empty or whitespace-only.
    EmptyInput,

    /// The input string could not be resolved to any known entry-point variant.
    NotResolved(String),
}

impl std::fmt::Display for EntryPointParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "entry point input must not be empty"),
            Self::NotResolved(s) => write!(f, "could not resolve entry point: {s}"),
        }
    }
}

impl std::error::Error for EntryPointParseError {}

impl From<EntryPointParseError> for crate::error::ExplorerError {
    fn from(err: EntryPointParseError) -> Self {
        match err {
            EntryPointParseError::EmptyInput => crate::error::ExplorerError::InvalidInput(err.to_string()),
            EntryPointParseError::NotResolved(_) => crate::error::ExplorerError::NotFound(err.to_string()),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- EntryPoint::parse ---

    #[test]
    fn parse_http_route_with_method() {
        let ep = EntryPoint::parse("POST /api/users").unwrap();
        assert!(matches!(ep, EntryPoint::HttpRoute { method, path }
            if method == "POST" && path == "api/users"
        ));
    }

    #[test]
    fn parse_http_route_lowercase_method() {
        let ep = EntryPoint::parse("get /api/items").unwrap();
        assert!(matches!(ep, EntryPoint::HttpRoute { method, path }
            if method == "GET" && path == "api/items"
        ));
    }

    #[test]
    fn parse_http_route_path_only() {
        // Path-only (no method) — treated as GET /path
        let ep = EntryPoint::parse("/api/users").unwrap();
        assert!(matches!(ep, EntryPoint::HttpRoute { method, path }
            if method == "GET" && path == "api/users"
        ));
    }

    #[test]
    fn parse_symbol_id() {
        let ep = EntryPoint::parse("symbol:src/foo.rs:bar:42").unwrap();
        assert!(matches!(ep, EntryPoint::Symbol { id }
            if id == "symbol:src/foo.rs:bar:42"
        ));
    }

    #[test]
    fn parse_cli_command() {
        let ep = EntryPoint::parse("cognicode analyze").unwrap();
        assert!(matches!(ep, EntryPoint::CliCommand { name }
            if name == "cognicode analyze"
        ));
    }

    #[test]
    fn parse_decision_adr() {
        let ep = EntryPoint::parse("ADR-008").unwrap();
        assert!(matches!(ep, EntryPoint::Decision { id } if id == "ADR-008"));
    }

    #[test]
    fn parse_decision_adr_lowercase() {
        let ep = EntryPoint::parse("adr-009").unwrap();
        assert!(matches!(ep, EntryPoint::Decision { id } if id == "adr-009"));
    }

    #[test]
    fn parse_viewspec() {
        let ep = EntryPoint::parse("vs-12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(matches!(ep, EntryPoint::ViewSpec { id } if id.starts_with("vs-")));
    }

    #[test]
    fn parse_saved_exploration() {
        let ep = EntryPoint::parse("exp-12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(matches!(ep, EntryPoint::SavedExploration { id } if id.starts_with("exp-")));
    }

    #[test]
    fn parse_issue() {
        let ep = EntryPoint::parse("iss-12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(matches!(ep, EntryPoint::Issue { id } if id.starts_with("iss-")));
    }

    #[test]
    fn parse_evidence() {
        let ep = EntryPoint::parse("ev-12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(matches!(ep, EntryPoint::Evidence { id } if id.starts_with("ev-")));
    }

    #[test]
    fn parse_doc() {
        let ep = EntryPoint::parse("doc-12345678-1234-1234-1234-123456789abc").unwrap();
        assert!(matches!(ep, EntryPoint::Doc { id } if id.starts_with("doc-")));
    }

    #[test]
    fn parse_event_camel_case() {
        let ep = EntryPoint::parse("UserCreated").unwrap();
        assert!(matches!(ep, EntryPoint::Event { name } if name == "UserCreated"));
    }

    #[test]
    fn parse_event_domain_style() {
        let ep = EntryPoint::parse("OrderShipped").unwrap();
        assert!(matches!(ep, EntryPoint::Event { name } if name == "OrderShipped"));
    }

    #[test]
    fn parse_empty_returns_error() {
        let result = EntryPoint::parse("");
        assert!(matches!(result, Err(EntryPointParseError::EmptyInput)));
    }

    #[test]
    fn parse_whitespace_only_returns_error() {
        let result = EntryPoint::parse("   ");
        assert!(matches!(result, Err(EntryPointParseError::EmptyInput)));
    }

    #[test]
    fn parse_unknown_returns_not_resolved() {
        let result = EntryPoint::parse("foo");
        assert!(matches!(result, Err(EntryPointParseError::NotResolved(_))));
    }

    #[test]
    fn parse_not_resolved_preserves_input() {
        let result = EntryPoint::parse("completely_unknown_input");
        assert!(matches!(result, Err(EntryPointParseError::NotResolved(s)) if s == "completely_unknown_input"));
    }

    // --- EntryPoint::default_view_kind ---

    #[test]
    fn default_view_kind_http_route_is_vertical_slice() {
        let ep = EntryPoint::parse("POST /api/users").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::VerticalSlice);
    }

    #[test]
    fn default_view_kind_cli_command_is_vertical_slice() {
        let ep = EntryPoint::parse("cognicode analyze").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::VerticalSlice);
    }

    #[test]
    fn default_view_kind_event_is_data_flow() {
        let ep = EntryPoint::parse("UserCreated").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::DataFlow);
    }

    #[test]
    fn default_view_kind_use_case_is_vertical_slice() {
        // UseCase has same shape as Event in this safe slice
        let ep = EntryPoint::parse("CreateUser").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::VerticalSlice);
    }

    #[test]
    fn default_view_kind_symbol_is_call_graph() {
        let ep = EntryPoint::parse("symbol:src/foo.rs:bar:42").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::CallGraph);
    }

    #[test]
    fn default_view_kind_file_is_file_overview() {
        // File paths ending in recognized extensions → File entry point
        // Note: /api/users parses as HttpRoute (GET /api/users), not File.
        // Use a file-like path to test the File variant.
        let ep = EntryPoint::File {
            path: "src/main.rs".to_string(),
        };
        assert_eq!(ep.default_view_kind(), ViewKind::Custom("file_overview".to_string()));
    }

    #[test]
    fn default_view_kind_search_result_is_semantic_search_results() {
        // SearchResult is not directly parseable in this safe slice
        let ep = EntryPoint::SearchResult {
            ids: vec![],
            query: "test".to_string(),
        };
        assert_eq!(ep.default_view_kind(), ViewKind::SemanticSearchResults);
    }

    #[test]
    fn default_view_kind_saved_exploration_is_composed_narrative() {
        let ep = EntryPoint::SavedExploration {
            id: "exp-test".to_string(),
        };
        assert_eq!(ep.default_view_kind(), ViewKind::ComposedNarrative);
    }

    #[test]
    fn default_view_kind_decision_is_architecture_rationale() {
        let ep = EntryPoint::parse("ADR-008").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::ArchitectureRationale);
    }

    #[test]
    fn default_view_kind_doc_is_doc_code_alignment() {
        let ep = EntryPoint::parse("doc-123").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::DocCodeAlignment);
    }

    #[test]
    fn default_view_kind_issue_is_dead_code_candidates() {
        let ep = EntryPoint::parse("iss-123").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::DeadCodeCandidates);
    }

    #[test]
    fn default_view_kind_evidence_is_evidence_view() {
        let ep = EntryPoint::parse("ev-123").unwrap();
        assert_eq!(ep.default_view_kind(), ViewKind::EvidenceView);
    }

    // --- ResolvedEntryPoint ---

    #[test]
    fn resolved_entry_point_from_ep_has_no_target() {
        let ep = EntryPoint::parse("POST /api/users").unwrap();
        let resolved = ResolvedEntryPoint::from_ep(ep.clone());
        assert!(resolved.target.is_none());
        assert_eq!(resolved.ep, ep);
        assert_eq!(resolved.default_view_kind, ep.default_view_kind());
    }

    #[test]
    fn resolved_entry_point_round_trips_through_json() {
        let ep = EntryPoint::parse("POST /api/users").unwrap();
        let resolved = ResolvedEntryPoint::from_ep(ep);
        let json = serde_json::to_string(&resolved).expect("serialize");
        let back: ResolvedEntryPoint = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.ep, resolved.ep);
        assert_eq!(back.default_view_kind, resolved.default_view_kind);
    }

    // --- HTTP method validation ---

    #[test]
    fn http_methods_are_case_insensitive() {
        for method in ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"] {
            let input = format!("{} /test", method.to_lowercase());
            let ep = EntryPoint::parse(&input).unwrap();
            assert!(
                matches!(ep, EntryPoint::HttpRoute { method: m, path: _ } if m == method.to_uppercase()),
                "method {method} should be recognized"
            );
        }
    }

    #[test]
    fn invalid_http_method_is_not_route() {
        // "INVALID /path" → NotResolved (not a valid HTTP method)
        let result = EntryPoint::parse("INVALID /api/users");
        assert!(result.is_err());
    }
}
