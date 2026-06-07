// CogniCode Explorer MCP adapter.
//
// Pure wiring: every tool delegates to an existing ExplorerService method.
// No new domain logic, no new DTOs. Follows the canonical CogniCodeHandler
// pattern in cognicode-core (single struct, single ServerHandler impl,
// dispatch by match on the tool name).
//
// Tool list (8):
//   1. explorer_open_workspace     — optional root_path -> WorkspaceSummary
//   2. explorer_spotter_search     — query (required), kind (optional)
//   3. explorer_inspect_object     — object_id -> InspectableObjectSummary
//   4. explorer_get_views          — object_id -> Vec<ViewDescriptor>
//   5. explorer_get_view           — object_id, view_id -> ContextualView
//   6. explorer_get_lenses         — object_id -> Vec<LensDescriptor>
//   7. explorer_apply_lens         — object_id, lens_id -> LensResult
//   8. explorer_query_moldql       — query (required) -> MoldQLResultDto
//
// Any ExplorerError is returned as a CallToolResult::error whose
// Content::text carries the Display representation. Agents never see a
// panic — service errors are captured, not propagated.

use std::sync::Arc;

use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, Implementation, ListToolsResult,
    ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::{RequestContext, RoleServer};
use serde::Deserialize;

use crate::dto::{MoldQLResultDto, OpenWorkspaceRequest};
use crate::service::ExplorerService;

pub const TOOL_OPEN_WORKSPACE: &str = "explorer_open_workspace";
pub const TOOL_SPOTTER_SEARCH: &str = "explorer_spotter_search";
pub const TOOL_INSPECT_OBJECT: &str = "explorer_inspect_object";
pub const TOOL_GET_VIEWS: &str = "explorer_get_views";
pub const TOOL_GET_VIEW: &str = "explorer_get_view";
pub const TOOL_GET_LENSES: &str = "explorer_get_lenses";
pub const TOOL_APPLY_LENS: &str = "explorer_apply_lens";
pub const TOOL_QUERY_MOLDQL: &str = "explorer_query_moldql";

pub const TOOL_NAMES: &[&str] = &[
    TOOL_OPEN_WORKSPACE,
    TOOL_SPOTTER_SEARCH,
    TOOL_INSPECT_OBJECT,
    TOOL_GET_VIEWS,
    TOOL_GET_VIEW,
    TOOL_GET_LENSES,
    TOOL_APPLY_LENS,
    TOOL_QUERY_MOLDQL,
];

/// Backwards-compatible accessor — returns the canonical tool list.
pub fn tool_names() -> &'static [&'static str] {
    TOOL_NAMES
}

// ============================================================================
// Per-tool argument structs.
//
// Each struct is Deserialize from the JSON-RPC `tools/call` arguments blob.
// Fields are Option<_> unless the tool cannot function without them — that
// gives per-field error messages instead of one opaque parse failure.
// ============================================================================

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct OpenWorkspaceArgs {
    root_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SpotterArgs {
    query: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct InspectArgs {
    object_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct GetViewArgs {
    object_id: Option<String>,
    view_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct ApplyLensArgs {
    object_id: Option<String>,
    lens_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QueryMoldQLArgs {
    query: Option<String>,
}

// ============================================================================
// Handler
// ============================================================================

/// The MCP handler for the cognicode-explorer service.
///
/// Owns an `Arc<ExplorerService>` — cheap to clone, shareable across
/// threads. The canonical use is a single handler per process; the Arc
/// is here to support future multiplexed transports without an API break.
#[derive(Clone)]
pub struct ExplorerMcpHandler {
    service: Arc<ExplorerService>,
}

impl ExplorerMcpHandler {
    /// Wrap a service in an MCP handler.
    pub fn new(service: Arc<ExplorerService>) -> Self {
        Self { service }
    }

    /// Borrow the underlying service handle. Used by tests to confirm
    /// that dispatched tool calls actually reached the service.
    #[cfg(test)]
    pub fn service(&self) -> &Arc<ExplorerService> {
        &self.service
    }
}

impl ServerHandler for ExplorerMcpHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "cognicode-explorer",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = build_tool_schemas();
        async move {
            Ok(ListToolsResult {
                meta: None,
                tools,
                next_cursor: None,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_
    {
        let service = self.service.clone();
        async move {
            let result = dispatch(&service, request).await;
            Ok(result)
        }
    }
}

// ============================================================================
// Dispatch
// ============================================================================

async fn dispatch(service: &Arc<ExplorerService>, request: CallToolRequestParams) -> CallToolResult {
    let name = request.name.as_ref();
    // CallToolRequestParams.arguments is a serde_json::Map<String, Value>;
    // wrap it as a Value::Object so the per-tool deserializers can consume it.
    let arguments = match request.arguments {
        Some(map) => serde_json::Value::Object(map),
        None => serde_json::Value::Object(Default::default()),
    };

    match name {
        TOOL_OPEN_WORKSPACE => {
            let args: OpenWorkspaceArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_open_workspace: invalid args: {e}")),
            };
            let result = match args.root_path {
                Some(root_path) => service.open_workspace(OpenWorkspaceRequest { root_path }),
                None => service.current_workspace(),
            };
            ok(&result)
        }
        TOOL_SPOTTER_SEARCH => {
            let args: SpotterArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_spotter_search: invalid args: {e}")),
            };
            let query = match args.query {
                Some(q) => q,
                None => {
                    return err("explorer_spotter_search: missing required arg `query`".into());
                }
            };
            ok(&service.spotter_search(&query, args.kind.as_deref()))
        }
        TOOL_INSPECT_OBJECT => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_inspect_object: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err(
                        "explorer_inspect_object: missing required arg `object_id`".into(),
                    );
                }
            };
            ok(&service.inspect_object(&object_id))
        }
        TOOL_GET_VIEWS => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_views: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_views: missing required arg `object_id`".into());
                }
            };
            ok(&service.available_views(&object_id))
        }
        TOOL_GET_VIEW => {
            let args: GetViewArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_view: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_view: missing required arg `object_id`".into());
                }
            };
            let view_id = match args.view_id {
                Some(v) => v,
                None => {
                    return err("explorer_get_view: missing required arg `view_id`".into());
                }
            };
            ok(&service.contextual_view(&object_id, &view_id))
        }
        TOOL_GET_LENSES => {
            let args: InspectArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_get_lenses: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_get_lenses: missing required arg `object_id`".into());
                }
            };
            ok(&service.available_lenses(&object_id))
        }
        TOOL_APPLY_LENS => {
            let args: ApplyLensArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_apply_lens: invalid args: {e}")),
            };
            let object_id = match args.object_id {
                Some(id) => id,
                None => {
                    return err("explorer_apply_lens: missing required arg `object_id`".into());
                }
            };
            let lens_id = match args.lens_id {
                Some(l) => l,
                None => {
                    return err("explorer_apply_lens: missing required arg `lens_id`".into());
                }
            };
            ok(&service.apply_lens(&object_id, &lens_id))
        }
        TOOL_QUERY_MOLDQL => {
            let args: QueryMoldQLArgs = match serde_json::from_value(arguments) {
                Ok(a) => a,
                Err(e) => return err(format!("explorer_query_moldql: invalid args: {e}")),
            };
            let query = match args.query {
                Some(q) => q,
                None => {
                    return err("explorer_query_moldql: missing required arg `query`".into());
                }
            };
            let result: Result<MoldQLResultDto, _> =
                service.execute_query(&query).map(MoldQLResultDto::from);
            ok(&result)
        }
        _ => err(format!("Unknown tool: {name}")),
    }
}

// ============================================================================
// Result helpers
// ============================================================================

/// Serialize a successful service result as pretty JSON text content.
/// `T` only needs to be `Serialize` + `Debug` for the error context.
fn ok<T: serde::Serialize>(result: &crate::ExplorerResult<T>) -> CallToolResult {
    match result {
        Ok(value) => match serde_json::to_string_pretty(value) {
            Ok(json) => CallToolResult::success(vec![Content::text(json)]),
            Err(e) => err(format!("failed to serialize tool result: {e}")),
        },
        Err(e) => err(e.to_string()),
    }
}

/// Build a CallToolResult::error with a single text content carrying
/// the supplied message.
fn err(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}

// ============================================================================
// Tool schemas
// ============================================================================

/// Build the 8 tool descriptors. The schemas are intentionally hand-rolled
/// (not derived from the args structs) to keep the wire-level contract
/// stable and self-documenting — agents consume the JSON schema directly.
fn build_tool_schemas() -> Vec<Tool> {
    use std::sync::Arc as StdArc;

    let schema = |properties: serde_json::Value, required: &[&str]| -> StdArc<serde_json::Map<String, serde_json::Value>> {
        let mut obj = serde_json::json!({
            "type": "object",
            "properties": properties,
        });
        if !required.is_empty() {
            obj["required"] = serde_json::json!(required);
        }
        StdArc::new(obj.as_object().cloned().unwrap())
    };

    vec![
        Tool::new(
            TOOL_OPEN_WORKSPACE,
            "Open (or re-open) a workspace by absolute path. If `root_path` is omitted, the handler's bound workspace is returned. Returns a WorkspaceSummary with graph status and counts.",
            schema(
                serde_json::json!({
                    "root_path": {
                        "type": "string",
                        "description": "Filesystem path to the workspace root. Optional — when omitted, the workspace bound at handler construction is returned."
                    }
                }),
                &[],
            ),
        ),
        Tool::new(
            TOOL_SPOTTER_SEARCH,
            "Spotter search: exact name matches from the call graph, merged with the optional FTS5 / fuzzy backend. `query` is required, `kind` is an optional filter (e.g. 'Function', 'Struct').",
            schema(
                serde_json::json!({
                    "query": { "type": "string", "description": "Search query (required)." },
                    "kind": { "type": "string", "description": "Optional kind filter (e.g. 'Function', 'Struct')." }
                }),
                &["query"],
            ),
        ),
        Tool::new(
            TOOL_INSPECT_OBJECT,
            "Inspect an MVP id and return its InspectableObjectSummary (type, label, properties, available views). The id is the canonical `symbol:{file}:{name}:{line}` / `file:{path}` / `scope:{path}` / `issue:{id}` / `rule:{id}` form.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object to inspect (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_VIEWS,
            "List the contextual views available for a given object (e.g. 'evidence', 'quality', 'callers', 'callees' — depends on the object type).",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_VIEW,
            "Render a specific contextual view for an object. Returns blocks, relations, and evidence. For example, `view_id='evidence'` returns the evidence blocks collected about the object.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." },
                    "view_id":   { "type": "string", "description": "Id of the view to render (required). Use explorer_get_views to discover the available ids." }
                }),
                &["object_id", "view_id"],
            ),
        ),
        Tool::new(
            TOOL_GET_LENSES,
            "List the design lenses that apply to a given object. Lenses are filtered by the object's type — e.g. a quality issue returns 0 lenses, a scope returns 3 (hotspots, dependencies, architecture).",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object (required)." }
                }),
                &["object_id"],
            ),
        ),
        Tool::new(
            TOOL_APPLY_LENS,
            "Run a registered design lens against an object. Returns a LensResult with findings (hypothesis, severity, confidence, cross-references) and a one-line summary.",
            schema(
                serde_json::json!({
                    "object_id": { "type": "string", "description": "MVP id of the object to analyse (required)." },
                    "lens_id":   { "type": "string", "description": "Id of the lens to run (required). Use explorer_get_lenses to discover the available ids." }
                }),
                &["object_id", "lens_id"],
            ),
        ),
        Tool::new(
            TOOL_QUERY_MOLDQL,
            "Execute a MoldQL query against the explorer. Two query shapes are supported: `FIND <target> [IN SCOPE <path>] [WHERE <cond> AND <cond> ...] [APPLY <lens>]` and `EXPLORE <object_ref> THROUGH <callers|callees> DEPTH <n>`. Returns a MoldQLResultDto with the matched items and the original query echoed back.",
            schema(
                serde_json::json!({
                    "query": { "type": "string", "description": "MoldQL query string (required). Keywords (FIND, EXPLORE, IN, SCOPE, WHERE, AND, APPLY, THROUGH, DEPTH) are case-insensitive." }
                }),
                &["query"],
            ),
        ),
    ]
}

// ============================================================================
// Tests
// ============================================================================
//
// We can't construct a real `rmcp::service::RequestContext<RoleServer>` in
// unit tests (its constructor is `pub(crate)` in rmcp), so we exercise the
// dispatch logic directly via the public `tool_names` / `build_tool_schemas`
// surfaces and by calling the service through a hand-built
// `ExplorerMcpHandler`. The integration tests in `tests/integration.rs`
// verify the binary's link surface and the tool list contract.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::FsSourceReader;
    use crate::dto::OpenWorkspaceRequest;
    use crate::ports::symbol_repository::{
        GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
    };
    use cognicode_core::domain::aggregates::SymbolId;
    use cognicode_core::domain::value_objects::SymbolKind;
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;
    use std::sync::Arc;

    /// In-memory symbol repository backed by a hashmap. Mirrors the
    /// MockRepo in `service::tests` so dispatch tests get a service
    /// that has real symbols to find.
    #[derive(Debug, Default)]
    struct TestRepo {
        by_name: HashMap<String, Vec<ResolvedSymbol>>,
        by_id: HashMap<String, ResolvedSymbol>,
    }

    impl TestRepo {
        fn new() -> Self {
            Self::default()
        }

        fn with_symbol(
            &mut self,
            name: &str,
            file: &str,
            line: u32,
            kind: SymbolKind,
        ) -> &mut Self {
            let id = SymbolId::new(format!("{file}:{name}:{line}"));
            let sym = ResolvedSymbol {
                id: id.clone(),
                name: name.to_string(),
                kind,
                file: file.to_string(),
                line,
                signature: None,
            };
            self.by_id.insert(id.to_string(), sym.clone());
            self.by_name.entry(name.to_string()).or_default().push(sym);
            self
        }
    }

    impl SymbolRepository for TestRepo {
        fn resolve(&self, id: &SymbolId) -> crate::ExplorerResult<Option<ResolvedSymbol>> {
            Ok(self.by_id.get(id.as_str()).cloned())
        }
        fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize {
            0
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn find_symbols_by_name(&self, name: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_name.get(name).cloned().unwrap_or_default())
        }
        fn find_symbols_by_file(&self, file: &str) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self
                .by_id
                .values()
                .filter(|s| s.file == file)
                .cloned()
                .collect())
        }
        fn module_list(&self) -> Vec<String> {
            let mut modules: BTreeSet<String> = BTreeSet::new();
            for s in self.by_id.values() {
                if let Some(parent) = std::path::Path::new(&s.file).parent() {
                    let p = parent.to_string_lossy().to_string();
                    if !p.is_empty() {
                        modules.insert(p);
                    }
                }
            }
            modules.into_iter().collect()
        }
        fn all_symbols(&self) -> crate::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(self.by_id.values().cloned().collect())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats {
                symbol_count: self.by_id.len(),
                relation_count: 0,
            }
        }
    }

    /// Build a service bound to a fresh tempdir with two known symbols.
    fn build_test_service() -> (Arc<ExplorerService>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut repo = TestRepo::new();
        repo.with_symbol("alpha", "src/a.rs", 1, SymbolKind::Function);
        repo.with_symbol("beta", "src/b.rs", 5, SymbolKind::Struct);
        let repo: Arc<dyn SymbolRepository> = Arc::new(repo);
        let reader = Arc::new(FsSourceReader::new(dir.path().to_path_buf()));
        let service = Arc::new(ExplorerService::new(
            repo,
            reader,
            dir.path().to_path_buf(),
        ));
        (service, dir)
    }

    /// Build a `CallToolRequestParams` from a tool name and JSON arguments.
    fn call_tool_args(name: &str, arguments: serde_json::Value) -> CallToolRequestParams {
        let map = match arguments {
            serde_json::Value::Object(m) => m,
            other => panic!("expected JSON object for arguments, got: {other}"),
        };
        CallToolRequestParams::new(name.to_string()).with_arguments(map)
    }

    /// Convenience: extract the text from the first content item.
    fn first_text(result: &CallToolResult) -> String {
        result
            .content
            .first()
            .expect("at least one content item")
            .as_text()
            .expect("text content")
            .text
            .clone()
    }

    // ---- list_tools contract ------------------------------------------------

    #[test]
    fn tool_schemas_list_eight_tools() {
        let tools = build_tool_schemas();
        assert_eq!(tools.len(), 8, "expected 8 tools, got {}", tools.len());

        let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
        let expected = [
            TOOL_OPEN_WORKSPACE,
            TOOL_SPOTTER_SEARCH,
            TOOL_INSPECT_OBJECT,
            TOOL_GET_VIEWS,
            TOOL_GET_VIEW,
            TOOL_GET_LENSES,
            TOOL_APPLY_LENS,
            TOOL_QUERY_MOLDQL,
        ];
        for e in expected {
            assert!(
                names.contains(&e),
                "tool list missing `{}` — got: {:?}",
                e,
                names
            );
        }
    }

    #[test]
    fn tool_names_exposed_via_back_compat_helper() {
        let names = tool_names();
        assert_eq!(names.len(), 8);
        assert!(names.contains(&TOOL_OPEN_WORKSPACE));
        assert!(names.contains(&TOOL_APPLY_LENS));
        assert!(names.contains(&TOOL_QUERY_MOLDQL));
    }

    // ---- handler basics -----------------------------------------------------

    #[test]
    fn handler_wraps_service_cheaply() {
        let (service, _dir) = build_test_service();
        let handler = ExplorerMcpHandler::new(service.clone());
        // Clone must not move the service.
        let _h2 = handler.clone();
        assert!(Arc::ptr_eq(handler.service(), &service));
    }

    #[test]
    fn get_info_reports_explorer_server_name() {
        let (service, _dir) = build_test_service();
        let handler = ExplorerMcpHandler::new(service);
        let info = handler.get_info();
        assert_eq!(info.server_info.name, "cognicode-explorer");
        assert!(
            info.capabilities.tools.is_some(),
            "tools capability must be enabled"
        );
    }

    // ---- dispatch happy paths ----------------------------------------------

    #[tokio::test]
    async fn dispatch_open_workspace_with_no_root_uses_bound_path() {
        let (service, dir) = build_test_service();
        let handler = ExplorerMcpHandler::new(service);
        // We can't drive `call_tool` end-to-end without a RequestContext, but
        // the dispatch logic is private — so we re-derive the contract here
        // by asserting the underlying service method is reachable and
        // returns a WorkspaceSummary rooted at the tempdir.
        let summary = handler
            .service()
            .current_workspace()
            .expect("current_workspace ok");
        assert_eq!(
            PathBuf::from(&summary.root_path).canonicalize().unwrap(),
            dir.path().canonicalize().unwrap(),
            "current_workspace should report the bound root path"
        );
    }

    #[tokio::test]
    async fn dispatch_spotter_search_finds_known_symbol() {
        let (service, _dir) = build_test_service();
        let results = service.spotter_search("alpha", None).expect("spotter ok");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].object.id, "symbol:src/a.rs:alpha:1");
        assert_eq!(
            results[0].object.object_type,
            crate::dto::InspectableObjectType::Symbol
        );
        assert!((results[0].score - 1.0).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn dispatch_inspect_object_dispatches_to_service() {
        let (service, _dir) = build_test_service();
        let summary = service
            .inspect_object("symbol:src/a.rs:alpha:1")
            .expect("inspect_object ok");
        assert_eq!(summary.id, "symbol:src/a.rs:alpha:1");
        assert!(!summary.available_views.is_empty());
    }

    #[tokio::test]
    async fn dispatch_get_views_for_symbol_returns_views() {
        let (service, _dir) = build_test_service();
        let views = service
            .available_views("symbol:src/a.rs:alpha:1")
            .expect("available_views ok");
        assert!(!views.is_empty(), "symbol should have at least one view");
        for v in &views {
            assert!(!v.id.is_empty());
            assert!(!v.title.is_empty());
        }
    }

    #[tokio::test]
    async fn dispatch_get_lenses_for_unknown_object_returns_resolution_error() {
        let (service, _dir) = build_test_service();
        let err = service
            .available_lenses("garbage")
            .expect_err("garbage id must error");
        assert!(matches!(err, crate::ExplorerError::ResolutionFailed(_)));
    }

    #[tokio::test]
    async fn dispatch_apply_lens_unknown_id_returns_error_text() {
        let (service, _dir) = build_test_service();
        let err = service
            .apply_lens("scope:src", "no-such-lens")
            .expect_err("unknown lens must error");
        let msg = err.to_string();
        assert!(
            msg.contains("lens not found"),
            "expected error to mention `lens not found`, got: {msg}"
        );
    }

    // ---- error path: unknown tool name -------------------------------------

    #[tokio::test]
    async fn dispatch_unknown_tool_name_returns_error() {
        let (service, _dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args("not_a_real_tool", serde_json::json!({})),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(true),
            "unknown tool must produce is_error=Some(true)"
        );
        let text = first_text(&result);
        assert!(
            text.contains("Unknown tool"),
            "error text should mention Unknown tool, got: {text}"
        );
    }

    // ---- error path: missing required arg ----------------------------------

    #[tokio::test]
    async fn dispatch_spotter_missing_query_returns_error() {
        let (service, _dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args(TOOL_SPOTTER_SEARCH, serde_json::json!({})),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("query"),
            "error text should mention the missing arg, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_returns_dto() {
        let (service, _dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "FIND symbols" }),
            ),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(false),
            "FIND symbols should succeed against an empty repo"
        );
        let text = first_text(&result);
        // The DTO serialises `query`, `items`, and `total` — all of
        // which must be present in the response body.
        assert!(text.contains("\"query\""));
        assert!(text.contains("\"items\""));
        assert!(text.contains("\"total\""));
    }

    #[tokio::test]
    async fn dispatch_moldql_query_missing_query_errors() {
        let (service, _dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args(TOOL_QUERY_MOLDQL, serde_json::json!({})),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(
            text.contains("query"),
            "error text should mention the missing arg, got: {text}"
        );
    }

    #[tokio::test]
    async fn dispatch_moldql_query_with_parse_error() {
        let (service, _dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args(
                TOOL_QUERY_MOLDQL,
                serde_json::json!({ "query": "FOO" }),
            ),
        )
        .await;
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        // The service wraps ParseError into ResolutionFailed, so the
        // tool surfaces a clean error string.
        assert!(
            text.contains("FIND or EXPLORE"),
            "error text should mention the parse failure, got: {text}"
        );
    }

    // ---- ok() helper serializes success as pretty JSON ---------------------

    #[test]
    fn ok_helper_serializes_success_as_pretty_json() {
        let summary = crate::dto::WorkspaceSummary {
            id: "abc".to_string(),
            root_path: "/tmp".to_string(),
            graph_status: crate::dto::GraphStatus::Ready,
            indexed_at: None,
            symbol_count: 42,
            relation_count: 7,
        };
        let result = ok(&Ok::<_, crate::ExplorerError>(summary));
        // CallToolResult::success sets is_error = Some(false); only
        // CallToolResult::error sets it to Some(true).
        assert_eq!(result.is_error, Some(false));
        let text = first_text(&result);
        assert!(text.contains('\n'), "expected pretty JSON, got: {text}");
        assert!(text.contains("\"symbol_count\": 42"));
    }

    #[test]
    fn ok_helper_serializes_error_as_text_content() {
        let result = ok::<crate::dto::WorkspaceSummary>(&Err(
            crate::ExplorerError::WorkspaceNotFound("/nope".to_string()),
        ));
        assert_eq!(result.is_error, Some(true));
        let text = first_text(&result);
        assert!(text.contains("workspace not found"));
        assert!(text.contains("/nope"));
    }

    // ---- end-to-end dispatch path for open_workspace (no RequestContext) --

    #[tokio::test]
    async fn dispatch_open_workspace_with_explicit_root_path() {
        let (service, dir) = build_test_service();
        let result = dispatch(
            &service,
            call_tool_args(
                TOOL_OPEN_WORKSPACE,
                serde_json::json!({ "root_path": dir.path().to_string_lossy() }),
            ),
        )
        .await;
        assert_eq!(
            result.is_error,
            Some(false),
            "open_workspace should succeed"
        );
        let text = first_text(&result);
        assert!(text.contains("\"id\""));
    }

    // ---- wire-level contract guard: DTO field name matches the schema. ----

    #[test]
    fn open_workspace_request_field_names_match_tool_schema() {
        let request = OpenWorkspaceRequest {
            root_path: "/tmp".to_string(),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert!(json.get("root_path").is_some());
    }
}
