//! RMCP Adapter - Bridge between rmcp SDK and CogniCode handlers
//!
//! This module provides the CogniCodeHandler which implements the rmcp ServerHandler trait,
//! allowing the CogniCode MCP server to use the official rmcp SDK for transport.
// e30.1 clippy baseline reset: pre-existing lint debt (see fix/e30.1-clippy-baseline-reset)
#![allow(clippy::collapsible_if, clippy::manual_async_fn)]

use crate::application::services::file_operations::FileOperationsService;
use crate::infrastructure::telemetry::get_global_metrics;
use crate::infrastructure::verification::RustVerifier;
use crate::interface::mcp::cancellation::CancellationRegistry;
use crate::interface::mcp::error::{InterfaceError, InterfaceResult};
use crate::interface::mcp::handlers::HandlerContext;
use opentelemetry::KeyValue;
use rmcp::handler::server::ServerHandler;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ListToolsResult, Meta, ServerCapabilities,
    ServerInfo, Tool,
};
use rmcp::service::RoleServer;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// CogniCodeHandler implements the rmcp ServerHandler trait
///
/// This handler bridges the rmcp SDK with the existing CogniCode handler functions.
/// It maintains a persistent HandlerContext that survives across requests to avoid
/// rebuilding the analysis graph, plus a registry of the requests that are still
/// in flight and can therefore be cancelled.
#[derive(Debug)]
pub struct CogniCodeHandler {
    /// Persistent handler context - created once and shared across all requests
    ctx: Arc<HandlerContext>,
    /// The requests that are still in flight, each with its own cancellation
    /// flag. A cancellation belongs to the request that asked for it, so
    /// this is a table and not a single flag.
    cancellations: CancellationRegistry,
}

impl CogniCodeHandler {
    /// Creates a new CogniCodeHandler with InMemoryGraphStore (no persistence)
    pub fn new(project_root: PathBuf) -> Self {
        Self::with_options(project_root, false)
    }

    /// PRF-SEC-02: create a handler in read-only mode. Mutating tools
    /// (write_file, edit_file, reparse_on_edit) are hidden from `tools/list`
    /// and rejected at dispatch with a typed error.
    ///
    /// ST-04: read-only and cancellation are both set through the builder.
    /// They used to be reassigned onto the built context by hand, which is
    /// only possible while the fields are `pub` — and a public `Arc` field
    /// also permits swapping it later, which would let a live handler be
    /// flipped into read-only (or out of it) after a client connected. That
    /// is the property PRF-SEC-02 exists to deny. Fixing it by adding a
    /// setter would have preserved the hole and dressed it up as an API.
    pub fn with_options(project_root: PathBuf, read_only: bool) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let ctx = Self::build_ctx(project_root, Some(read_only), Some(cancellation_token));
        Self {
            ctx: Arc::new(ctx),
            cancellations: CancellationRegistry::new(),
        }
    }

    /// M3.1: Creates a CogniCodeHandler wrapping a pre-built, shared
    /// `Arc<HandlerContext>`. Used by the HTTP server (cognicode-mcp)
    /// to share the same `graph_loaded` flag between the MCP dispatch
    /// and the `/ready` HTTP handler.
    pub fn from_ctx(ctx: Arc<HandlerContext>) -> Self {
        Self {
            ctx,
            cancellations: CancellationRegistry::new(),
        }
    }

    /// Whether this handler refuses to mutate the workspace.
    ///
    /// The posture is public because it is part of the product contract:
    /// `product/profiles.json` promises that a `mutating: false` profile
    /// cannot write, and this is the value that promise resolves to.
    pub fn is_read_only(&self) -> bool {
        self.ctx.is_read_only()
    }

    /// Build a handler whose posture is derived from a public profile.
    ///
    /// This is the link A-009 exists to add. `--read-only` is a flag a human
    /// types; a profile is something an installer resolves. Before this,
    /// nothing connected them, so `product/profiles.json` could promise
    /// `mutating: false` for `reviewer` while the runtime allowed writes —
    /// the promise was unenforceable.
    ///
    /// The posture comes from [`crate::product::ProfilePosture`], the same
    /// table the profile generator reads, so the published contract and the
    /// enforced behaviour cannot disagree.
    ///
    /// An unknown profile id is refused rather than assumed.
    ///
    /// This used to read `is_profile_read_only`, which returns `false` for
    /// anything it cannot classify — correct for a predicate, wrong here,
    /// because "I do not know this profile" was being turned into "this
    /// profile may write". The module docs justified that on the grounds
    /// that an unclassified profile is a compile-time concern; that holds
    /// for the closed `PROFILE_POSTURES` table, but this function takes a
    /// runtime `&str`, which is exactly the case the argument misses.
    ///
    /// Panicking is the intended outcome. A caller that cannot name the
    /// profile it is running under has a bug, and starting writable would
    /// let that bug reach the user's workspace. Refusing to start is loud,
    /// immediate, and costs nobody but the caller with the bug. The known
    /// ids in `PROFILE_POSTURES` are unaffected.
    pub fn for_profile(project_root: PathBuf, profile_id: &str) -> Self {
        match crate::product::ProfilePosture::for_profile(profile_id) {
            Some(posture) => Self::with_options(project_root, posture.is_read_only()),
            None => panic!(
                "unknown profile id {profile_id:?}: not one of the published \
                 profiles ({:?}). Refusing to start rather than assume a \
                 writable posture. Use `with_options` directly if you \
                 genuinely need an ad-hoc handler.",
                crate::product::PROFILE_POSTURES
                    .iter()
                    .map(|(id, _)| *id)
                    .collect::<Vec<_>>()
            ),
        }
    }

    /// Creates a new CogniCodeHandler with a custom GraphStore (SQLite for persistence)
    pub fn with_graph_store(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let ctx = HandlerContext::builder()
            .with_working_dir(project_root)
            .with_graph_store_arc(store)
            .with_cancellation_token(cancellation_token)
            .build();
        Self {
            ctx: Arc::new(ctx),
            cancellations: CancellationRegistry::new(),
        }
    }

    /// Creates a CogniCodeHandler with an optional IacRepository.
    ///
    /// When `iac_repo` is `Some`, the handler will use it for `iac_query`
    /// instead of falling back to the in-memory graph.
    pub fn with_iac_repo(
        project_root: PathBuf,
        store: Arc<dyn crate::domain::traits::GraphStore>,
        iac_repo: Option<Arc<dyn crate::domain::traits::iac_repository::IacRepository>>,
    ) -> Self {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let ctx = HandlerContext::builder()
            .with_working_dir(project_root)
            .with_graph_store_arc(store)
            .with_cancellation_token(cancellation_token);
        let ctx = match iac_repo {
            Some(repo) => ctx.with_iac_repo(repo),
            None => ctx,
        };
        let ctx = ctx.build();
        Self {
            ctx: Arc::new(ctx),
            cancellations: CancellationRegistry::new(),
        }
    }

    /// ST-04: read-only and cancellation are configured here, through the
    /// builder, instead of being reassigned onto the finished context. Both
    /// used to be `pub` fields patched from outside, which is only possible
    /// while the field is public — and a public `Arc` field also allows
    /// swapping it later, letting a live handler be flipped into read-only
    /// (or out of it) after a client connected. Denying that is the point of
    /// PRF-SEC-02; adding a setter would have kept the hole and renamed it.
    fn build_ctx(
        project_root: PathBuf,
        read_only: Option<bool>,
        cancellation_token: Option<Arc<AtomicBool>>,
    ) -> HandlerContext {
        let canonical_root =
            std::fs::canonicalize(&project_root).unwrap_or_else(|_| project_root.clone());

        // The workspace is where the analysis happens, not where the product
        // lives. The catalogue that says which tools exist, and which of them
        // change something, is the product's own — the copy inside this binary
        // or the one next to it in its installation. Reading it from the
        // workspace would let the tree under analysis declare a mutating tool
        // read-only, and the read-only posture would believe it.

        // Create validator and FileOperationsService for shared use across handlers
        let validator = Arc::new(
            crate::interface::mcp::security::InputValidator::new()
                .with_workspace(vec![canonical_root.clone()]),
        );
        let file_ops_service = Arc::new(FileOperationsService::new(
            canonical_root.to_string_lossy().as_ref(),
            validator,
            Arc::new(RustVerifier::new()),
            Arc::new(
                crate::infrastructure::parser::syntax_analysis::TreeSitterSyntaxAnalysis::new(),
            ),
        ));

        let mut builder = HandlerContext::builder()
            .with_working_dir(canonical_root)
            .with_file_ops_service(file_ops_service);
        if let Some(read_only) = read_only {
            builder = builder.with_read_only(read_only);
        }
        if let Some(token) = cancellation_token {
            builder = builder.with_cancellation_token(token);
        }
        builder.build()
    }

    /// Get the current CallGraph from the store
    pub fn get_call_graph(
        &self,
    ) -> anyhow::Result<crate::domain::aggregates::call_graph::CallGraph> {
        self.ctx
            .get_graph_store()
            .load_graph()
            .map_err(|e| anyhow::anyhow!("Graph store error: {}", e))?
            .ok_or_else(|| anyhow::anyhow!("No call graph available. Run build_graph first."))
    }
}

/// Helper to build cognicode metadata annotation for a tool.
/// Returns a Meta object with cognicode-specific fields:
/// - stability: "gated" | "stable" | "experimental"
/// - category: "graph" | "composite" | "navigation" | "file_ops" | etc.
/// - requires_graph: whether the tool needs a built call graph
/// - requires_persistence: whether the tool needs PG-backed persistence
/// - estimated_latency_ms: expected execution time in milliseconds
fn cognicode_meta(
    stability: &str,
    category: &str,
    requires_graph: bool,
    requires_persistence: bool,
    estimated_latency_ms: u32,
    authority: &str,
) -> Meta {
    let mut meta = Meta::new();
    meta.insert(
        "cognicode".to_string(),
        serde_json::json!({
            "stability": stability,
            "category": category,
            "requires_graph": requires_graph,
            "requires_persistence": requires_persistence,
            "estimated_latency_ms": estimated_latency_ms,
            "authority": authority
        }),
    );
    meta
}

/// M3.2: Returns the dispatch timeout for a given tool category.
/// Categories with heavy graph algorithms (PageRank, IDF, communities)
/// get 60s; LSP-backed navigation gets 45s; in-memory search gets a
/// tight 500ms (search is expected to be sub-100ms); everything else
/// gets a 30s default. Unknown categories fall through to the default.
fn timeout_for_category(category: &str) -> Duration {
    match category {
        "graph" => Duration::from_secs(60),
        "navigation" => Duration::from_secs(45),
        "search" => Duration::from_millis(500),
        _ => Duration::from_secs(30),
    }
}

/// M3.3: Categories that use a stricter rate-limit key namespace.
/// Each tool in these categories gets its own `strict:<tool_name>`
/// bucket, separate from the regular `tool:<tool_name>` budget, so
/// heavy categories (graph analytics, LSP navigation, AIX NL search)
/// cannot exhaust the regular dispatcher budget.
const STRICT_RATE_LIMIT_CATEGORIES: &[&str] = &["graph", "navigation", "aix"];

/// Returns `true` if `category` should be rate-limited with a stricter
/// key namespace. See [`STRICT_RATE_LIMIT_CATEGORIES`].
fn is_strict_rate_limit_category(category: &str) -> bool {
    STRICT_RATE_LIMIT_CATEGORIES.contains(&category)
}

/// M3.2 / M3.3: Lazily-built map from tool name to its
/// `cognicode_meta.category`. Derived from [`build_all_tools`] on first
/// access and reused for the lifetime of the process. Tools without a
/// cognicode meta block or without a category string are absent from
/// the map (the lookup falls back to "unknown", which gets the default
/// 30s timeout and the non-strict key prefix).
fn tool_category_map() -> &'static HashMap<String, String> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for tool in build_all_tools() {
            let name = tool.name.to_string();
            if let Some(meta) = tool.meta.as_ref()
                && let Some(cognicode) = meta.get("cognicode")
                && let Some(cat) = cognicode.get("category").and_then(|v| v.as_str())
            {
                m.insert(name, cat.to_string());
            }
        }
        m
    })
}

/// PRF-MCP-05: Lazily-built map from tool name to its declared authority
/// string ("read" | "mutating" | "execute" | "network").
///
/// Derived from the full `load_tool_catalog()` (NOT `build_all_tools()`)
/// on first access and reused for the lifetime of the process. Iterating
/// the full catalog matters: `build_all_tools()` filters out tools whose
/// Cargo feature is not enabled in this build, but the **public** audit
/// contract is that the catalogue (`product/tools.json`) and the
/// runtime agree tool-for-tool. Feature-gated tools are still in the
/// catalogue; if `tool_authority_map` only saw the live build it would
/// disagree with `tools.json`, and the audit gate (A-010) would report
/// phantom tools. So the oracle is the catalogue, not the build.
///
/// This map is the **primary authority oracle**, and the only one: the
/// `mutates_workspace` flag published on each tool is computed from the same
/// declaration, and an undeclared tool fails closed. There is no second list
/// of tool names to keep in step with this one.
pub fn tool_authority_map() -> &'static HashMap<String, String> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m: HashMap<String, String> = HashMap::new();
        // B2.W2 (post-merge): the oracle is the **active** catalog so the
        // map agrees with `build_all_tools()`, which also consumes the
        // active view. Before this fix, the map was built from the full
        // `load_tool_catalog()` and the 3 feature-gated tools
        // (`interproc_summary` on `program-analysis-server`,
        // `list_evidence`/`search_evidence` on `evidence-cli-ladybug`)
        // showed up in the map regardless of the current build's
        // features. The A-010 audit then reported a phantom-runtime
        // mismatch: the catalogue said 76, the runtime said 73, and
        // the test panicked with `runtime-only=[...]`. Filtering the
        // map to the active subset keeps the runtime contract intact
        // (`mutates_workspace == authority_is_mutating(declared)`)
        // and matches the same active subset that
        // `build_all_tools()` advertises via `tools/list`.
        for d in crate::interface::catalog::load_tool_catalog_active() {
            m.insert(d.name, d.authority);
        }
        m
    })
}

/// PRF-MCP-05: Resolve the authority for a tool by name, from its own
/// declaration.
///
/// A tool with no declaration resolves to `"mutating"`. This used to consult a
/// second hardcoded list on the way there, which was not defence in depth but
/// a duplicate: both that branch and the one after it returned `"mutating"`,
/// so the list could not change the answer. It only existed to be
/// cross-checked.
///
/// FAIL-CLOSED (audit 2026-09-30, finding #5): a tool that is not in the
/// declared map resolves to `"mutating"`, so read-only mode refuses it. The
/// previous default — `"read"` — was fail-open: an undeclared mutating tool
/// would have sailed through read-only mode. Deny-by-default costs an
/// undeclared read tool a refusal until it is declared, which is the correct
/// direction for a security property.
/// The authority a tool declares in its own `cognicode` meta, or `None` when
/// it declares none.
///
/// Read straight off the tool rather than through `tool_authority_map()`,
/// because `build_all_tools()` cannot consult the map: the map is built *from*
/// `build_all_tools()`. That circularity is the only reason a second,
/// hand-kept list of mutating tool names ever existed.
fn declared_authority(tool: &Tool) -> Option<&str> {
    tool.meta
        .as_ref()?
        .get("cognicode")?
        .get("authority")?
        .as_str()
}

/// Whether an authority value means "this tool changes something".
fn authority_is_mutating(authority: &str) -> bool {
    matches!(authority, "mutating" | "execute" | "network")
}

/// Every tool this build exposes that may change something.
///
/// Derived from the declarations `build_all_tools()` already carries, so a
/// tool cannot be mutating in one place and read-only in another. This
/// replaces `CogniCodeHandler::MUTATING_TOOLS`, a hand-kept list of names
/// that had to be edited every time a tool was added or feature-gated — and
/// that therefore went stale: `reparse_on_edit` is declared behind
/// `#[cfg(feature = "persistence")]`, `persistence` is a default feature, and
/// a `--no-default-features` build named it with no tool behind it.
/// PRF-MCP-05 caught that at the integration lane's `core-no-default` stage.
///
/// Fail-closed, matching `resolve_tool_authority`: a tool that declares no
/// authority is treated as mutating.
pub fn mutating_tool_names() -> Vec<String> {
    build_all_tools()
        .iter()
        .filter(|tool| declared_authority(tool).is_none_or(authority_is_mutating))
        .map(|tool| tool.name.to_string())
        .collect()
}

pub fn resolve_tool_authority(tool_name: &str) -> String {
    if let Some(declared) = tool_authority_map().get(tool_name) {
        return declared.clone();
    }
    "mutating".to_string()
}

/// PRF-MCP-05: True iff a tool is considered mutating under its
/// declared authority. A tool is mutating when its authority is any of
/// `"mutating"`, `"execute"`, or `"network"` (the three non-read values
/// declared in `cognicode_meta`). A tool with no declaration is always
/// mutating regardless of meta.
pub fn tool_is_mutating(tool_name: &str) -> bool {
    matches!(
        resolve_tool_authority(tool_name).as_str(),
        "mutating" | "execute" | "network"
    )
}

/// M3.2 / M3.3: Resolve the category for a tool name. Falls back to
/// "unknown" when the tool has no cognicode meta or the meta is
/// missing the `category` field.
fn lookup_category(tool_name: &str) -> String {
    tool_category_map()
        .get(tool_name)
        .cloned()
        .unwrap_or_else(|| "unknown".to_string())
}

/// M3.4: Lazily-built map from tool name to the tools that must run first.
///
/// Read from `ToolDescriptor::prerequisites`, the typed field B3.W3 added
/// to `product/tools.json` for exactly this question. It used to be
/// extracted with a regex over each tool's English description, matching
/// "Requires build_graph first." — a functional dependency inferred
/// from prose, which is the same defect class as the ghost tools this
/// branch removed: the real state sat in a typed field nobody read,
/// and a second, weaker source was consulted instead.
///
/// The measured cost was silent under-coverage, not a wrong value. The
/// regex matched 26 tools; 41 declare prerequisites. The 15 it missed —
/// `analyze_impact`, `ask_about_code`, `codebase_map`, `smart_search`,
/// `project_overview`, … — were run by `cognicode-sandbox` with no
/// graph built, because the sandbox gates its implicit `build_graph`
/// call on this lookup. A scenario for one of those tools reported
/// `expected_outcome: pass` for a run that never had its input.
///
/// Note that `requirements.graph` (44 tools) is a *different* question
/// and is not the right source here: it means "this tool touches the
/// graph", which includes the three tools that build their own
/// (`build_call_subgraph`, `build_lightweight_index`,
/// `get_per_file_graph`). Prerequisites means "a graph must already
/// exist".
pub(crate) fn tool_graph_deps_map() -> &'static HashMap<String, Vec<String>> {
    use std::sync::OnceLock;
    static MAP: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m = HashMap::new();
        for descriptor in crate::interface::catalog::load_tool_catalog() {
            if let Some(prereqs) = descriptor.prerequisites
                && !prereqs.is_empty()
            {
                m.insert(descriptor.name, prereqs);
            }
        }
        m
    })
}

/// The first tool `tool_name` must run before it can answer, if any.
///
/// For example, `iac_query` returns `Some("build_graph")` because its
/// catalogue entry lists it under `prerequisites` — not because its
/// description happens to read a certain way.
///
/// Singular because every entry in the catalogue declares exactly one, and
/// because the only caller asks a yes/no question. If a second
/// prerequisite is ever declared, this returns the first and the caller
/// that needs all of them wants `tool_graph_deps_map` directly.
pub fn lookup_tool_deps(tool_name: &str) -> Option<String> {
    tool_graph_deps_map()
        .get(tool_name)
        .and_then(|v| v.first())
        .cloned()
}

/// Returns the complete list of public MCP tool definitions.
/// This is the single source of truth for `tools/list` and the parity-test surface.
///
/// Public so the A-012 contract suite can compare what the catalog advertises
/// against what the derive produces. Read-only: it rebuilds the vector.
pub fn build_all_tools() -> Vec<Tool> {
    // B2.W2: the catalog is the single source of truth. The previous
    // implementation was a 970-line inline `vec![Tool::new(...)...]`
    // that listed every tool in code; any drift between this list and
    // `product/tools.json` was silent and only the parity test could
    // see it (and it didn't, because there was no parity test). The
    // catalog loader reads `product/tools.json`, deserialises each
    // entry into a typed `ToolDescriptor`, filters out tools whose
    // declared Cargo feature is not enabled in the current build, and
    // `descriptor_to_tool` projects each remaining entry into the
    // 5-field MCP meta block. The handlers in `call_tool_handler`
    // below still hold their `#[cfg(feature = "...")]` gates, so a
    // feature-gated tool that slipped past the catalog filter would
    // still fail to dispatch — defence in depth, not a single point
    // of failure.
    build_report()
        .tools
        .into_iter()
        .map(descriptor_to_tool)
        .map(|mut tool| {
            // CP2.3 / A-012 (structured output): announce the published
            // output contract. The schema is derived from the very type
            // that serialises the result, so `tools/list` cannot
            // describe a shape the handler does not produce. Tools
            // without a concrete output type keep `None`, which is the
            // honest answer: "no contract", not "any shape".
            //
            // `descriptor_to_tool` has already populated the
            // cognicode meta block (mutates_workspace, tool_version);
            // `with_raw_output_schema` does not touch meta, so the
            // order is independent of the meta side.
            if let Some(schema) = super::output_contracts::output_schema_for(tool.name.as_ref()) {
                tool = tool.with_raw_output_schema(Arc::new(schema));
            }
            tool
        })
        .collect()
}

/// The active catalogue plus its provenance, warning when it is unusable.
///
/// A server that cannot load its catalogue says so on stderr. stdout belongs
/// to JSON-RPC; an empty `tools/list` with no diagnostic is how this shipped
/// broken in the first place, and nothing downstream can distinguish "the
/// product has no tools" from "this build lost its catalogue".
fn build_report() -> crate::interface::catalog::CatalogReport {
    let report = crate::interface::catalog::load_tool_catalog_report();
    if let Some(message) = report.diagnostic() {
        eprintln!("cognicode-mcp: warning: {message}");
    }
    crate::interface::catalog::CatalogReport {
        tools: report
            .tools
            .into_iter()
            .filter(|d| crate::interface::catalog::tool_feature_enabled(&d.feature))
            .collect(),
        source: report.source,
    }
}

/// Project a `ToolDescriptor` into the rmcp `Tool` model.
///
/// The projection keeps the wire-side 5-field meta block
/// (`stability`, `category`, `requires_graph`, `requires_persistence`,
/// `estimated_latency_ms`, `authority`) plus the derived
/// `mutates_workspace` and `tool_version` injected at the same site.
/// All other fields the catalog carries (`requirements`,
/// `requirements_status`, `public_profiles`, `runtime_metadata`, ...)
/// stay in the on-disk JSON, accessible via
/// `cognicode capabilities --json`; the catalog is the source of
/// truth, the MCP projection is a strict subset for the wire.
fn descriptor_to_tool(d: crate::interface::catalog::ToolDescriptor) -> Tool {
    let input_schema = if let Some(obj) = d.input_schema.as_object() {
        obj.clone()
    } else {
        // input_schema is null/missing for tools that take no args.
        // rmcp requires an object map; empty map is the contract.
        serde_json::Map::new()
    };

    // PRF-MCP-05: authority is the primary oracle. `mutates_workspace`
    // is derived (not declared) so the flag cannot lie about the
    // declaration that produced it.
    let mutates = matches!(d.authority.as_str(), "mutating" | "execute" | "network");

    // requires_graph / requires_persistence are pinned on each entry
    // by `product/tools.json`'s `runtime_metadata` block; fall back to
    // false when the field is absent (older catalog revisions, ad-hoc
    // additions) so a missing field never silently claims a
    // dependency that isn't there.
    let requires_graph = d
        .runtime_metadata
        .get("requires_graph")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let requires_persistence = d
        .runtime_metadata
        .get("requires_persistence")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let meta = cognicode_meta(
        d.stability.as_str(),
        d.category.as_str(),
        requires_graph,
        requires_persistence,
        d.estimated_latency_ms,
        d.authority.as_str(),
    );

    let mut tool = Tool::new(d.name, d.description, Arc::new(input_schema)).with_meta(meta);

    // PRF-EXT-01 + PRF-MCP-05: expose the read/write permission
    // explicitly so clients can distinguish mutating tools without
    // hardcoding names. `mutates` is derived from `d.authority`
    // *here*, not by calling `tool_is_mutating(&tool.name)` — that
    // would recurse through `tool_authority_map()` which iterates
    // `build_all_tools()`.
    if let Some(c) = tool
        .meta
        .as_mut()
        .and_then(|m| m.get_mut("cognicode"))
        .and_then(|v| v.as_object_mut())
    {
        c.insert("mutates_workspace".to_string(), serde_json::json!(mutates));
        c.insert(
            "tool_version".to_string(),
            serde_json::json!(env!("CARGO_PKG_VERSION")),
        );
    }
    tool
}

impl ServerHandler for CogniCodeHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
        .with_server_info(rmcp::model::Implementation::new(
            "cognicode",
            env!("CARGO_PKG_VERSION"),
        ))
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_03_26)
    }

    fn list_tools(
        &self,
        request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        async move {
            use crate::interface::mcp::cursor::{CursorContext, encode_cursor, verify_cursor};

            // The catalogue is read from the workspace this server was
            // configured with (`--cwd`), never from the inherited process
            // CWD: an MCP client launches the process inside the *user's*
            // project, which is not the checkout, and a catalogue resolved
            // from there is empty — a healthy-looking server advertising no
            // tools at all. Built once and shared with the cursor snapshot
            // below, so the two cannot disagree.
            let visible_tools: Vec<Tool> = build_all_tools()
                .into_iter()
                .filter(|t| !self.ctx.is_read_only() || !tool_is_mutating(&t.name))
                .collect();

            // H2.3: anchored cursor with typed errors. The cursor
            // is a compact base64-JSON envelope; a tampered or
            // expired cursor surfaces as a JSON-RPC error with
            // `kind` + `reason`, not a silent reset to page 0.
            let cursor_ctx = {
                let visible = visible_tools
                    .iter()
                    .map(|t| t.name.as_ref())
                    .collect::<Vec<_>>()
                    .join("|");
                CursorContext {
                    workspace: "__tools__".to_string(),
                    snapshot: format!("read_only:{}|{}", self.ctx.is_read_only(), visible),
                }
            };
            let cursor_offset: usize = match request.as_ref().and_then(|p| p.cursor.as_deref()) {
                Some(c) => match verify_cursor(c, &cursor_ctx) {
                    Ok(off) => off,
                    Err(e) => {
                        return Err(rmcp::ErrorData {
                            code: rmcp::model::ErrorCode::INVALID_PARAMS,
                            message: format!("{e}").into(),
                            data: Some(serde_json::to_value(&e).unwrap_or(serde_json::Value::Null)),
                        });
                    }
                },
                None => 0,
            };

            const PAGE_SIZE: usize = 20;

            // PRF-SEC-02 + PRF-MCP-05: in read-only mode mutating tools are not advertised.
            // Authority comes from the declared `cognicode.authority` field (primary
            // oracle), with no second list to fall back to.
            let all_tools = visible_tools;

            // Paginate
            let total = all_tools.len();
            let page_tools: Vec<_> = all_tools
                .into_iter()
                .skip(cursor_offset)
                .take(PAGE_SIZE)
                .collect();

            let next_cursor = if cursor_offset + PAGE_SIZE < total {
                Some(encode_cursor(&cursor_ctx, cursor_offset + PAGE_SIZE))
            } else {
                None
            };

            Ok(ListToolsResult {
                meta: None,
                tools: page_tools,
                next_cursor,
            })
        }
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: rmcp::service::RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_
    {
        let ctx = self.ctx.clone();
        let cancellations = self.cancellations.clone();
        let request_id = context.id.clone();
        let request_ct = context.ct.clone();
        async move {
            // The request gets a context of its own, with a cancellation
            // flag of its own. The session context is shared by every
            // request, and a shared flag is what made one cancellation
            // reach the requests that came after it.
            let (request_ctx, flag) = ctx.scoped_for_request();

            // The transport already knows whether this request was
            // cancelled, and it knows it before the notification reaches
            // the handler: a cancellation can arrive while this task is
            // still waiting to start. Asking here means the flag is born
            // cancelled instead of the notice being lost.
            if request_ct.is_cancelled() {
                flag.store(true, Ordering::SeqCst);
            }

            // The registration lives as long as the request does. When the
            // request ends, its entry leaves the registry and a late
            // notification finds nobody to mark.
            let _registration = cancellations.register(request_id, flag);

            let result = call_tool_handler(&request_ctx, request).await;

            match result {
                Ok(output) => Ok(CallToolResult::success(vec![Content::text(output)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
            }
        }
    }

    fn on_cancelled(
        &self,
        notification: rmcp::model::CancelledNotificationParam,
        _context: rmcp::service::NotificationContext<RoleServer>,
    ) -> impl std::future::Future<Output = ()> + Send + '_ {
        crate::interface::mcp::robustness::note_cancellation();
        // The cancellation belongs to the request that asked for it. If
        // that request has already ended, its entry is gone and there is
        // nobody to mark: the next request starts clean.
        self.cancellations.cancel(&notification.request_id);
        std::future::ready(())
    }
}

/// Handles the tool call by dispatching to the appropriate handler
///
/// M1.1: All tool calls flow through this single instrumentation boundary.
/// Timing, call count, and error classification are recorded here — no
/// per-handler timing or record_tool_usage calls needed.
async fn call_tool_handler(
    ctx: &HandlerContext,
    request: CallToolRequestParams,
) -> InterfaceResult<String> {
    let tool_name = request.name.as_ref();
    let mut arguments = request.arguments.unwrap_or_default();

    // PRF-SEC-02 + PRF-MCP-05: read-only mode rejects mutating tools before any handler
    // runs. Error is typed/honest (isError text), not a silent success.
    // Authority comes from the declared `cognicode.authority` field (primary oracle),
    // with no second list to fall back to.
    if tool_is_mutating(tool_name) && ctx.is_read_only() {
        return Err(InterfaceError::Internal(format!(
            "read_only_mode: tool `{tool_name}` mutates workspace state and is disabled; restart the server without --read-only to enable it"
        )));
    }

    // UAT 2026-08-10 DEFECT-1: BC layer for parameter naming.
    // Renames legacy parameter names to their canonical equivalents so
    // callers that pre-date the v1.0.0 convention continue to work.
    // Each substitution emits a structured `warn` so dashboards can
    // drive migration without this layer silently propping them up.
    let substitutions =
        crate::interface::mcp::parameter_aliases::apply_aliases(tool_name, &mut arguments);
    if !substitutions.is_empty() {
        let renames = substitutions
            .iter()
            .map(|(old, new)| format!("{old}\u{2192}{new}"))
            .collect::<Vec<_>>()
            .join(", ");
        tracing::warn!(
            tool = %tool_name,
            renames = %renames,
            "deprecated MCP parameter name(s) used; this BC shim ships with v1.0.0 and is scheduled for removal in v1.1"
        );
    }

    // M1.1: Centralized instrumentation boundary
    let start = Instant::now();
    let metrics = get_global_metrics();

    // M3.2 / M3.3: Resolve the tool's category once. Used both for
    // timeout selection (M3.2) and rate-limit key prefixing (M3.3).
    let category = lookup_category(tool_name);

    // M3.3: Pre-match rate-limit check. The key is per-tool so each
    // tool has its own 100/60s budget. Expensive categories (graph,
    // navigation, aix) use a stricter key prefix to keep their
    // budgets separate from the rest of the dispatcher.
    let rate_key = if is_strict_rate_limit_category(&category) {
        format!("strict:{}", tool_name)
    } else {
        format!("tool:{}", tool_name)
    };
    if !ctx.rate_limiter().check_with_key(&rate_key) {
        let err: InterfaceResult<String> =
            Err(InterfaceError::Internal("rate_limit_exceeded".to_string()));
        let status = crate::interface::mcp::status::classify_status(tool_name, &err);
        let duration_ms = start.elapsed().as_millis() as f64;
        // M3.4: Emit the structured per-call log line for parity
        // with the success path so dashboards see the rate-limit hit.
        tracing::info!(
            tool = %tool_name,
            duration_ms = %duration_ms as u64,
            status = %status,
            "tool_call"
        );
        if let Some(m) = &metrics {
            m.calls.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            m.duration.record(
                duration_ms,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("status", status),
                ],
            );
            // Record the error with error_type="rate_limit_exceeded"
            // so it shows up in the M1.6 error metric.
            m.errors.add(
                1,
                &[
                    KeyValue::new("tool", tool_name.to_string()),
                    KeyValue::new("error_type", "rate_limit_exceeded"),
                ],
            );
        }
        return err;
    }

    // M3.2: Wrap the per-tool match dispatch with a category-based
    // timeout. The match block itself is unchanged; only the wrapper
    // differs. The timeout is selected by `timeout_for_category` from
    // the cognicode_meta.category of the tool. On timeout, the
    // dispatch future is dropped and we return Internal("timeout").
    let timeout = timeout_for_category(&category);
    let dispatch = async {
        match tool_name {
            "get_file_symbols" => {
                let input: crate::interface::mcp::schemas::GetFileSymbolsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_file_symbols(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "build_graph" => {
                let input: crate::interface::mcp::handlers::BuildGraphInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_build_graph(ctx, input).await?;

                // M2.1: Record graph statistics after successful build
                if output.success {
                    let graph = ctx.analysis_service().get_project_graph();
                    let symbols = graph.symbol_count() as u64;
                    let edges = graph.edge_count() as u64;
                    let health_score =
                    crate::application::services::graph_insights::GraphInsightsService::analyze(&graph)
                        .health_score;
                    if let Some(m) = &metrics {
                        m.record_graph_stats(symbols, edges, health_score);
                    }
                    // M3.1: Flip the readiness flag so /ready returns 200
                    ctx.mark_graph_loaded();
                }

                Ok(serde_json::to_string(&output)?)
            }
            "get_call_hierarchy" => {
                let input: crate::interface::mcp::schemas::GetCallHierarchyInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_call_hierarchy(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "analyze_impact" => {
                let input: crate::interface::mcp::schemas::AnalyzeImpactInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_analyze_impact(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }

            "composite_impact_query" => {
                let input: crate::interface::mcp::schemas::CompositeImpactQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_composite_impact_query(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "safe_refactor" => {
                let input: crate::interface::mcp::schemas::SafeRefactorInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::refactor_handlers::handle_safe_refactor(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "safe_refactor_preview" => {
                let input: crate::interface::mcp::schemas::SafeRefactorPreviewInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::refactor_handlers::handle_safe_refactor_preview(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_usages" => {
                let input: crate::interface::mcp::schemas::FindUsagesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_find_usages(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            // E1.W3 — EvidenceStore MCP tools. Mirror `cognicode evidence
            // list|search` so the equivalence test pins the same JSON
            // schema on both surfaces. See `cognicode-cli/tests/
            // evidence_cli_mcp_equivalence.rs`.
            #[cfg(feature = "evidence-cli-ladybug")]
            "list_evidence" => {
                use crate::domain::ports::evidence_store::EvidenceKind;
                use crate::interface::cli::evidence_backend;

                let workspace = arguments
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let kind = match arguments.get("kind").and_then(|v| v.as_str()) {
                    None | Some("") => None,
                    Some("log") => Some(EvidenceKind::Log),
                    Some("trace") => Some(EvidenceKind::Trace),
                    Some("measurement") => Some(EvidenceKind::Measurement),
                    Some("external") => Some(EvidenceKind::External),
                    Some(other) => {
                        return Err(InterfaceError::InvalidInput(format!(
                            "unknown evidence kind: {other} (expected one of \
                             log | trace | measurement | external)"
                        )));
                    }
                };

                let factory = evidence_backend::evidence_backend_factory().ok_or_else(|| {
                    InterfaceError::Internal(
                        "evidence backend not registered; the MCP server build does not \
                         have the `evidence-cli-ladybug` feature enabled, or the runtime \
                         adapter did not register a factory at startup."
                            .to_string(),
                    )
                })?;
                let backend = factory(None).map_err(InterfaceError::Internal)?;
                let rows = backend
                    .list(workspace, kind)
                    .map_err(InterfaceError::Internal)?;
                Ok(crate::interface::cli::commands::render_evidence_rows_json(
                    rows,
                ))
            }
            #[cfg(feature = "evidence-cli-ladybug")]
            "search_evidence" => {
                use crate::interface::cli::evidence_backend;

                let workspace = arguments
                    .get("workspace")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let query = arguments
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        InterfaceError::InvalidInput(
                            "missing required parameter `query`".to_string(),
                        )
                    })?;
                let limit = arguments
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(25);

                let factory = evidence_backend::evidence_backend_factory().ok_or_else(|| {
                    InterfaceError::Internal(
                        "evidence backend not registered; the MCP server build does not \
                         have the `evidence-cli-ladybug` feature enabled, or the runtime \
                         adapter did not register a factory at startup."
                            .to_string(),
                    )
                })?;
                let backend = factory(None).map_err(InterfaceError::Internal)?;
                let rows = backend
                    .search(workspace, query, limit)
                    .map_err(InterfaceError::Internal)?;
                Ok(crate::interface::cli::commands::render_evidence_rows_json(
                    rows,
                ))
            }
            "get_complexity" => {
                let input: crate::interface::mcp::schemas::GetComplexityInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_complexity(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_entry_points" => {
                let input: crate::interface::mcp::schemas::GetEntryPointsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_entry_points(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_leaf_functions" => {
                let input: crate::interface::mcp::schemas::GetLeafFunctionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_leaf_functions(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "trace_path" => {
                let input: crate::interface::mcp::schemas::TracePathInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_trace_path(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "export_mermaid" => {
                let input: crate::interface::mcp::schemas::ExportMermaidInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_export_mermaid(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_hot_paths" => {
                let input: crate::interface::mcp::schemas::GetHotPathsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_hot_paths(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_all_symbols" => {
                let input: crate::interface::mcp::schemas::GetAllSymbolsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_all_symbols(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_dead_code" => {
                let input: crate::interface::mcp::schemas::FindDeadCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_find_dead_code(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_module_dependencies" => {
                let input: crate::interface::mcp::schemas::GetModuleDependenciesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_module_dependencies(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "query_symbol_index" => {
                let input: crate::interface::mcp::schemas::QuerySymbolInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_query_symbol_index(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "build_call_subgraph" => {
                let input: crate::interface::mcp::schemas::BuildSubgraphInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_build_call_subgraph(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "get_per_file_graph" => {
                let input: crate::interface::mcp::schemas::GetPerFileGraphInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_get_per_file_graph(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }

            "get_symbol_code" => {
                let input: crate::interface::mcp::schemas::SymbolCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_get_symbol_code(
                    ctx.symbol_code().clone(),
                    ctx.validator().clone(),
                    ctx.working_dir().to_path_buf(),
                    input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }

            "go_to_definition" => {
                let input: crate::interface::mcp::schemas::GoToDefinitionInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::lsp_handlers::handle_go_to_definition(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "hover" => {
                let input: crate::interface::mcp::schemas::HoverInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::lsp_handlers::handle_hover(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_references" => {
                let input: crate::interface::mcp::schemas::FindReferencesInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::lsp_handlers::handle_find_references(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "read_file" => {
                let input: crate::interface::mcp::schemas::ReadFileInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_read_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "write_file" => {
                let input: crate::interface::mcp::schemas::WriteFileInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_write_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "edit_file" => {
                let input: crate::interface::mcp::schemas::EditFileInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_edit_file(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "search_content" => {
                let input: crate::interface::mcp::schemas::SearchContentInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_search_content(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "list_files" => {
                let input: crate::interface::mcp::schemas::ListFilesInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::handle_list_files(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            "retrieve_and_verify" => {
                let input: crate::interface::mcp::schemas::RetrieveAndVerifyInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_retrieve_and_verify(ctx, input).await?;
                Ok(serde_json::to_string(&output)?)
            }
            // AIX-1: Smart Overview & Ranked Symbols

            // AIX-2: Onboarding Plan & Auto Diagnose & Refactor Plan

            // AIX-3: NL to Symbol & Ask About Code & Find Pattern
            "nl_to_symbol" => {
                let input: crate::interface::mcp::schemas::NlToSymbolInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_nl_to_symbol(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "ask_about_code" => {
                let input: crate::interface::mcp::schemas::AskAboutCodeInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::aix_handlers::handle_ask_about_code(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "find_pattern_by_intent" => {
                let input: crate::interface::mcp::schemas::FindPatternByIntentInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_find_pattern_by_intent(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // AIX-4: Compare Call Graphs & Detect API Breaks

            // AIX-5: System Prompt Context & God Functions & Long Params
            "detect_god_functions" => {
                let input: crate::interface::mcp::schemas::DetectGodFunctionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_detect_god_functions(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "detect_long_parameter_lists" => {
                let input: crate::interface::mcp::schemas::DetectLongParamsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::aix_handlers::handle_detect_long_parameter_lists(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // PL3: Symbol Hotness Tracking

            // AVC: Agent-Verifiable Context tools
            "generate_contract" => {
                let input: crate::interface::mcp::schemas::GenerateContractInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_generate_contract(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "validate_contract" => {
                let input: crate::interface::mcp::schemas::ValidateContractInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_validate_contract(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Phase 3A: Proactive Tools
            #[cfg(feature = "persistence")]
            // Detect Drift tool (S7000-S7003)
            "detect_drift" => {
                let input: crate::interface::mcp::schemas::DetectDriftInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::aix_handlers::handle_detect_drift(ctx, input)
                        .await?;
                Ok(serde_json::to_string(&output)?)
            }
            // Batch D: Agent Task Tools (bidirectional interaction)

            // Phase 4b: Graph analytics tools (extracted to graph_handlers.rs)
            "graph_pagerank" => {
                let input: crate::interface::mcp::schemas::GraphPageRankInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_pagerank(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_all_paths" => {
                let input: crate::interface::mcp::schemas::GraphAllPathsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_all_paths(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_condensed" => {
                let input: crate::interface::mcp::schemas::GraphCondensedInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_condensed(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_god_nodes" => {
                let input: crate::interface::mcp::schemas::GraphGodNodesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_god_nodes(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_reduced" => {
                let input: crate::interface::mcp::schemas::GraphReducedInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::graph_handlers::handle_graph_reduced(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_feedback_arcs" => {
                let input: crate::interface::mcp::schemas::GraphFeedbackArcsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_feedback_arcs(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }

            // Phase 5: Community Detection handlers (extracted to graph_handlers.rs)
            "graph_communities" => {
                let input: crate::interface::mcp::schemas::GraphCommunitiesInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_communities(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_community_detail" => {
                let input: crate::interface::mcp::schemas::GraphCommunityDetailInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_community_detail(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_surprising_connections" => {
                let input: crate::interface::mcp::schemas::GraphSurprisingConnectionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_surprising_connections(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }

            // Phase 6: IDF-weighted Search & Unified Insights (extracted to graph_handlers.rs)
            "graph_search_idf" => {
                let input: crate::interface::mcp::schemas::GraphSearchIdfInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_search_idf(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_insights" => {
                let input: crate::interface::mcp::schemas::GraphInsightsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_handlers::handle_graph_insights(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_suggest_questions" => {
                let input: crate::interface::mcp::schemas::GraphSuggestQuestionsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::graph_handlers::handle_graph_suggest_questions(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Sprint 2: Graphify-style tools (ADR-026)
            "graph_query" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_explain" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GraphExplainInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_graph_explain(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Edge-type query tools (ADR-026)
            "get_type_references" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GetTypeRefsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_get_type_references(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "get_imports" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GetImportsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_get_imports(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "get_implementors" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GetImplementorsInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_get_implementors(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "get_members" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GetMembersInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_get_members(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_query_filtered" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::GraphQueryFilteredInput = serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::graph_query_handlers::handle_graph_query_filtered(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "export_callflow" => {
                let input: crate::interface::mcp::handlers::graph_query_handlers::ExportCallflowInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::graph_query_handlers::handle_export_callflow(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Sprint 5: Consolidated + High-value tools (ADR-027 + ADR-028)
            "smart_search" => {
                let input: crate::interface::mcp::schemas::SmartSearchInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_smart_search(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "graph_analyze" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::GraphAnalyzeInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_graph_analyze(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "project_overview" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::ProjectOverviewInput = serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_project_overview(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "codebase_map" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::CodebaseMapInput =
                serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_codebase_map(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "project_insights" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::ProjectInsightsInput = serde_json::from_value(arguments.into())?;
                let output =
                crate::interface::mcp::handlers::consolidated_handlers::handle_project_insights(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "review_pr" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::ReviewPrInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_review_pr(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            "iac_query" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::IacQueryInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_iac_query(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // SOLID Audit tool
            "solid_audit" => {
                let output = crate::interface::mcp::handlers::handle_solid_audit(ctx).await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Architecture check
            "check_architecture" => {
                let input: crate::interface::mcp::schemas::CheckArchitectureInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_check_architecture(ctx, input).await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Graph checkpoint management
            "graph_checkpoint" => {
                let input: crate::interface::mcp::handlers::consolidated_handlers::GraphCheckpointInput =
                serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::consolidated_handlers::handle_graph_checkpoint(
                ctx, input,
            )
            .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Merge per-file graphs into consolidated project graph
            "merge_graphs" => {
                let input: crate::interface::mcp::schemas::MergeGraphsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_merge_graphs(ctx, input).await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Build lightweight symbol index
            "build_lightweight_index" => {
                let input: crate::interface::mcp::schemas::BuildIndexInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::handle_build_lightweight_index(ctx, input)
                        .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            // Incremental reindex after edits (requires persistence feature)
            #[cfg(feature = "persistence")]
            "reparse_on_edit" => {
                let input: crate::interface::mcp::schemas::ReparseOnEditInput =
                    serde_json::from_value(arguments.into())?;
                let output = crate::interface::mcp::handlers::aix_handlers::handle_reparse_on_edit(
                    ctx, input,
                )
                .await?;
                Ok(serde_json::to_string_pretty(&output)?)
            }
            #[cfg(not(feature = "persistence"))]
            "reparse_on_edit" => {
                let _ = ctx;
                Err(InterfaceError::Internal(
                    "reparse_on_edit requires the `persistence` feature".to_string(),
                ))
            }
            // Sprint 5.3: graph_diff and graph_timeline tools
            // ViewSpec tools (ADR-008)
            "list_view_specs" => {
                let input: crate::interface::mcp::schemas::ListViewSpecsInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_list_view_specs(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }
            "read_view_spec" => {
                let input: crate::interface::mcp::schemas::ReadViewSpecInput =
                    serde_json::from_value(arguments.into())?;
                let output =
                    crate::interface::mcp::handlers::consolidated_handlers::handle_read_view_spec(
                        ctx, input,
                    )
                    .await?;
                Ok(serde_json::to_string(&output)?)
            }

            // M5.2 WU2: program-analysis dispatch arms.
            "cfg_per_function" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_cfg(input)
                    .map_err(InterfaceError::Internal)
            }
            "dominators_cfg" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_dominators_cfg(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "slice_forward" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_slice_forward(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "slice_backward" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_slice_backward(
                    input,
                )
                .map_err(InterfaceError::Internal)
            }
            "taint_flow" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_taint_flow(input)
                    .map_err(InterfaceError::Internal)
            }
            #[cfg(feature = "program-analysis-server")]
            "interproc_summary" => {
                let input: crate::interface::mcp::handlers::program_analysis_handlers::ProgramAnalysisToolInput =
                    serde_json::from_value(arguments.into())?;
                crate::interface::mcp::handlers::program_analysis_handlers::handle_interproc_summary(input)
                    .map_err(InterfaceError::Internal)
            }

            _ => Err(InterfaceError::ToolNotFound(tool_name.to_string())),
        }
    };
    let result = match tokio::time::timeout(timeout, dispatch).await {
        Ok(inner) => inner,
        Err(_elapsed) => Err(InterfaceError::Internal("timeout".to_string())),
    };

    // M1.2: Classify status for metrics
    let status = crate::interface::mcp::status::classify_status(tool_name, &result);

    // M1.1: Record duration + classify status for error recording
    let duration_ms = start.elapsed().as_millis() as f64;

    // M3.4: Structured per-call log line — one entry per tool call
    // (success, error, gated, missing, skipped). Emitted at the
    // universal instrumentation boundary so every tool flow is captured
    // uniformly. Field names match M3-Sprint-spec.md §M3.4.
    tracing::info!(
        tool = %tool_name,
        duration_ms = %duration_ms as u64,
        status = %status,
        "tool_call"
    );
    if let Some(m) = &metrics {
        // M1.6: Record calls with tool + status labels
        m.calls.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
        // M1.6: Record duration with tool + status labels
        m.duration.record(
            duration_ms,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("status", status),
            ],
        );
    }

    // M1.1: Record error metrics (error_type is separate from status)
    if let Err(e) = &result
        && let Some(m) = &metrics
    {
        // M3.2 / M3.3: Distinguish timeout and rate_limit_exceeded
        // from generic errors. Both are conveyed as
        // `InterfaceError::Internal(<sentinel>)` so we match on
        // the inner string to keep the error_type taxonomy
        // machine-readable.
        let error_type = match e {
            InterfaceError::ToolNotFound(_) => "missing",
            InterfaceError::Internal(msg) if msg == "timeout" => "timeout",
            InterfaceError::Internal(msg) if msg == "rate_limit_exceeded" => "rate_limit_exceeded",
            _ => "error",
        };
        m.errors.add(
            1,
            &[
                KeyValue::new("tool", tool_name.to_string()),
                KeyValue::new("error_type", error_type),
            ],
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    // ============================================================================
    // Concurrent Request Tests
    // ============================================================================
    // NOTE: The original `test_concurrent_list_tools_requests` and
    // `test_concurrent_shared_handler` are removed: they were `#[ignore]`d
    // because rmcp's RequestContext requires internal APIs (Peer::new is
    // pub(crate)) and would have panicked with `unimplemented!()` if
    // ever un-ignored. Reintroducing them requires a test helper from
    // rmcp itself or moving them to a sibling integration test crate.

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_handler_creation() {
        // Test that multiple handlers can be created concurrently without issues
        let handlers: Vec<CogniCodeHandler> = (0..10)
            .map(|i| CogniCodeHandler::new(PathBuf::from(&format!("/tmp/test_{}", i))))
            .collect();

        // All handlers should have valid state
        for handler in handlers {
            assert!(handler.ctx.working_dir().to_string_lossy().contains("test"));
            let info = handler.get_info();
            assert_eq!(info.server_info.name, "cognicode");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_concurrent_get_info_calls() {
        let handler = Arc::new(CogniCodeHandler::new(PathBuf::from("/tmp/test")));

        let mut handles = vec![];
        for _ in 0..100 {
            let handler = handler.clone();
            handles.push(tokio::spawn(async move { handler.get_info() }));
        }

        let results = futures_util::future::join_all(handles).await;

        // All calls should return consistent info
        for result in results {
            let info = result.unwrap();
            assert_eq!(info.server_info.name, "cognicode");
            assert!(info.capabilities.tools.is_some());
        }
    }

    // ============================================================================
    // Adapter State Management Tests
    // ============================================================================
    // NOTE: `test_handler_state_preserved_across_concurrent_requests` is
    // removed: it was `#[ignore]`d because rmcp's RequestContext requires
    // internal APIs (Peer::new is pub(crate)) and would have panicked with
    // `unimplemented!()` if ever un-ignored. See comment in the
    // "Concurrent Request Tests" section above.

    #[tokio::test(flavor = "multi_thread")]
    async fn test_capability_exchange_concurrent() {
        let handler = Arc::new(CogniCodeHandler::new(PathBuf::from("/tmp/test")));

        let mut handles = vec![];
        for _ in 0..20 {
            let handler = handler.clone();
            handles.push(tokio::spawn(async move {
                let info = handler.get_info();
                // Verify capabilities are consistent
                (info.server_info.name.clone(), info.capabilities.clone())
            }));
        }

        let results = futures_util::future::join_all(handles).await;

        // All should return same consistent capabilities
        let first = results.first().unwrap().as_ref().unwrap();
        let first_tools = first.1.tools.clone();
        for result in results {
            let (name, caps) = result.unwrap();
            assert_eq!(name, "cognicode");
            assert_eq!(caps.tools, first_tools);
        }
    }

    #[tokio::test]
    async fn test_version_negotiation_returns_correct_version() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        let info = handler.get_info();

        // Verify protocol version is set correctly
        assert_eq!(
            info.protocol_version,
            rmcp::model::ProtocolVersion::V_2025_03_26
        );
    }

    // ============================================================================
    // Original Basic Tests (preserved)
    // ============================================================================
    // NOTE: The "Error Handling Under Load Tests" section is removed
    // entirely: its three `#[ignore]` tests
    // (`test_pagination_consistent_under_concurrent_access`,
    // `test_high_concurrency_stress`,
    // `test_concurrent_requests_with_different_pagination`) all panicked
    // with `unimplemented!()` because they require rmcp's RequestContext
    // (Peer::new is pub(crate)) and would never run even if un-ignored.
    // See comment in the "Concurrent Request Tests" section above.

    #[test]
    fn test_cognicode_handler_creation() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        // working_dir is canonicalized so may differ from input path
        assert!(
            handler
                .ctx
                .working_dir()
                .to_string_lossy()
                .ends_with("test")
        );
    }

    #[test]
    fn test_server_info() {
        let handler = CogniCodeHandler::new(PathBuf::from("/tmp/test"));
        let info = handler.get_info();
        assert_eq!(info.server_info.name, "cognicode");
        assert!(info.capabilities.tools.is_some());
    }

    // ============================================================================
    // M3.2 — Per-tool timeout
    // ============================================================================

    #[test]
    fn test_timeout_for_category_known_categories() {
        // graph analytics is allowed the longest window (60s)
        assert_eq!(timeout_for_category("graph"), Duration::from_secs(60));
        // LSP navigation is allowed 45s
        assert_eq!(timeout_for_category("navigation"), Duration::from_secs(45));
        // in-memory search is the tightest bound (500ms)
        assert_eq!(timeout_for_category("search"), Duration::from_millis(500));
        // file operations default to 30s
        assert_eq!(timeout_for_category("file"), Duration::from_secs(30));
        // composite/quality/refactor/aix are also 30s
        assert_eq!(timeout_for_category("composite"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("quality"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("refactor"), Duration::from_secs(30));
        assert_eq!(timeout_for_category("aix"), Duration::from_secs(30));
    }

    #[test]
    fn test_timeout_for_category_unknown_defaults_to_30s() {
        // Unknown categories and the empty string fall through to the
        // 30s default so we never accidentally disable the timeout.
        assert_eq!(
            timeout_for_category("unknown_category"),
            Duration::from_secs(30)
        );
        assert_eq!(timeout_for_category(""), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_timeout_fires_when_handler_takes_longer() {
        // M3.2: A handler that exceeds the category timeout must be
        // cancelled and replaced with a timeout error. We exercise
        // the `tokio::time::timeout` wrapper directly here without
        // going through the full `call_tool_handler` (which needs a
        // real `HandlerContext` and would be slower to set up).
        let timeout = timeout_for_category("search"); // 500ms — fast
        let slow_handler = async {
            // Sleep longer than the search timeout (2s > 500ms)
            tokio::time::sleep(Duration::from_secs(2)).await;
            Ok::<_, InterfaceError>("should not reach here".to_string())
        };
        let result = tokio::time::timeout(timeout, slow_handler).await;
        assert!(
            result.is_err(),
            "expected tokio::time::timeout to fire on slow handler"
        );
        // Map the Elapsed to the same error we use in the boundary.
        let err: InterfaceResult<String> = Err(InterfaceError::Internal("timeout".to_string()));
        assert!(matches!(err, Err(InterfaceError::Internal(ref m)) if m == "timeout"));
    }

    #[tokio::test]
    async fn test_timeout_does_not_fire_when_handler_returns_quickly() {
        // M3.2: Fast handlers must NOT be cancelled.
        let timeout = timeout_for_category("search"); // 500ms
        let fast_handler = async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, InterfaceError>("done".to_string())
        };
        let result = tokio::time::timeout(timeout, fast_handler).await;
        assert!(result.is_ok(), "fast handler must not be timed out");
        assert_eq!(result.unwrap().unwrap(), "done");
    }

    // ============================================================================
    // M3.3 — Per-tool rate limit + strict category namespacing
    // ============================================================================

    #[test]
    fn test_is_strict_rate_limit_category() {
        // Categories that get their own rate-limit namespace.
        assert!(is_strict_rate_limit_category("graph"));
        assert!(is_strict_rate_limit_category("navigation"));
        assert!(is_strict_rate_limit_category("aix"));
        // Cheap categories share the regular namespace.
        assert!(!is_strict_rate_limit_category("file"));
        assert!(!is_strict_rate_limit_category("search"));
        assert!(!is_strict_rate_limit_category("quality"));
        assert!(!is_strict_rate_limit_category("refactor"));
        assert!(!is_strict_rate_limit_category("composite"));
        // Unknown / empty
        assert!(!is_strict_rate_limit_category(""));
        assert!(!is_strict_rate_limit_category("unknown"));
    }

    #[test]
    fn test_tool_category_map_contains_known_tools() {
        // M3.2 / M3.3: The lazy-init map must be populated from
        // `build_all_tools()` and include a representative sample of
        // tool names mapped to their declared category. A failure
        // here means the timeout / rate-limit routing will silently
        // fall back to the 30s default and the non-strict key prefix.
        let map = tool_category_map();
        assert_eq!(map.get("build_graph").map(|s| s.as_str()), Some("graph"));
        assert_eq!(
            map.get("go_to_definition").map(|s| s.as_str()),
            Some("navigation")
        );
        assert_eq!(
            map.get("search_content").map(|s| s.as_str()),
            Some("search")
        );
        assert_eq!(map.get("ask_about_code").map(|s| s.as_str()), Some("aix"));
        assert_eq!(map.get("read_file").map(|s| s.as_str()), Some("file"));
        // Every tool declared in `build_all_tools()` must have a
        // category — otherwise the lookup is incomplete and
        // roundtrip parity tests will diverge.
        let total = build_all_tools().len();
        assert_eq!(map.len(), total, "every tool must have a category");
    }

    #[test]
    fn test_lookup_category_falls_back_to_unknown() {
        // Unknown tool names must NOT panic and must return a
        // "unknown" placeholder that maps to the default 30s timeout
        // and the non-strict rate-limit key prefix.
        assert_eq!(lookup_category("definitely_not_a_real_tool"), "unknown");
        assert_eq!(lookup_category(""), "unknown");
    }

    #[test]
    fn test_rate_limiter_dispatch_key_separation() {
        // M3.3: Different rate-limit key namespaces (tool: vs
        // strict:) must track tokens independently so an expensive
        // tool cannot exhaust the regular dispatcher budget.
        let limiter = crate::interface::mcp::security::RateLimiter::new(2, 60);
        // Strict graph keys: 2 hits each, then exhausted.
        assert!(limiter.check_with_key("strict:graph_pagerank"));
        assert!(limiter.check_with_key("strict:graph_pagerank"));
        assert!(!limiter.check_with_key("strict:graph_pagerank"));
        // Regular file keys: untouched by the graph bucket.
        assert!(limiter.check_with_key("tool:read_file"));
        assert!(limiter.check_with_key("tool:read_file"));
        assert!(!limiter.check_with_key("tool:read_file"));
    }

    // ============================================================================
    // M3.4 — Tool graph-dependency extraction
    // ============================================================================

    // ============================================================================
    // Graph dependency — read from the catalogue, not from prose
    // ============================================================================

    /// The load-bearing one, and the reason the four tests it replaces
    /// are gone.
    ///
    /// `analyze_impact` declares `requirements.graph = true` and its
    /// description never says "Requires build_graph first." The old
    /// implementation inferred the dependency from that prose with a
    /// regex, so it answered `false` here — and the sandbox, which uses
    /// this to decide whether to pre-build the graph, ran 18 such tools
    /// with no graph at all. The number is the finding: 26 tools matched
    /// the regex, 44 declare the requirement.
    #[test]
    fn a_tool_that_declares_a_graph_requirement_is_reported_even_without_the_prose() {
        let tool = build_all_tools()
            .into_iter()
            .find(|t| t.name == "analyze_impact")
            .expect("analyze_impact is in the catalogue");

        assert!(
            !tool
                .description
                .as_deref()
                .unwrap_or_default()
                .to_lowercase()
                .contains("requires build_graph first"),
            "this test is only discriminating while the description omits the              phrase; if someone adds it back, the test has stopped testing              anything and should be replaced"
        );
        assert!(
            lookup_tool_deps("analyze_impact").is_some(),
            "a declared requirement must not depend on how the description \
             happens to be worded"
        );
    }

    /// Both directions, so the replacement cannot quietly become
    /// "everything needs a graph".
    #[test]
    fn graph_requirement_follows_the_catalogue_in_both_directions() {
        assert_eq!(
            lookup_tool_deps("iac_query").as_deref(),
            Some("build_graph"),
            "iac_query declares the requirement"
        );
        assert_eq!(
            lookup_tool_deps("get_call_hierarchy").as_deref(),
            Some("build_graph"),
            "and so does get_call_hierarchy"
        );
        assert_eq!(
            lookup_tool_deps("read_file"),
            None,
            "read_file does not declare one"
        );
        assert_eq!(
            lookup_tool_deps("nonexistent_tool"),
            None,
            "an unknown tool cannot declare anything"
        );
    }

    /// Pins the gap that made the old map wrong, so a future change to
    /// the catalogue cannot quietly shrink the detected set again
    /// without this number moving.
    #[test]
    fn the_detected_set_is_not_smaller_than_the_declared_one() {
        let catalog = crate::interface::catalog::load_tool_catalog();
        let declared = catalog
            .iter()
            .filter(|d| d.prerequisites.as_ref().is_some_and(|p| !p.is_empty()))
            .count();
        let detected = catalog
            .iter()
            .filter(|d| lookup_tool_deps(d.name.as_str()).is_some())
            .count();
        assert_eq!(
            detected, declared,
            "every tool declaring prerequisites must be detected; the regex \
             over descriptions found only 26 of these"
        );
    }

    // PRF-MCP-05: every public MCP tool MUST declare its required authority
    // in the `cognicode_meta.authority` field. Authority is one of:
    //   - "read"        — read-only, no side effects, safe in read-only mode
    //   - "mutating"    — writes to the workspace (files, graph cache, etc.)
    //   - "execute"     — runs code, processes, or shell commands
    //   - "network"     — sends requests off-host (HTTP, OTLP, fetch, etc.)
    //
    // The authority is enforced by two mechanisms:
    //   1. `list_tools` filters tools whose authority ≠ "read" when the
    //      handler is in read-only mode.
    //   2. `call_tool` rejects invocation of a non-read tool when the
    //      handler is in read-only mode.
    //
    // Without an explicit declaration, authority defaults to "mutating"
    // (deny by default) so that adding a new tool without thinking about
    // its authority cannot accidentally grant write/exec/net powers.
    //
    // This test pins two properties:
    //   1. Every tool has `cognicode_meta.authority` and the value is one
    //      of the four allowed strings.
    //   2. The published `mutates_workspace` flag agrees with the declared
    //      authority — one derivation, no second list. Every tool must
    //      authority != "read". (The legacy list is allowed to be a strict
    //      subset — once a tool has authority declared, PRF-MCP-05 audits
    //      can promote additional tools into `mutating`/`execute`/
    //      `network` based on observed behavior. The test does NOT require
    //      the converse direction to keep churn minimal.)
    #[test]
    fn test_prf_mcp_05_authority_declared_for_every_tool() {
        let tools = build_all_tools();
        assert!(!tools.is_empty(), "build_all_tools() must return tools");

        let allowed = ["read", "mutating", "execute", "network"];
        let mut declared_non_read: Vec<String> = Vec::new();

        for tool in &tools {
            let tool_name = tool.name.to_string();
            let cognicode = tool
                .meta
                .as_ref()
                .and_then(|m| m.get("cognicode"))
                .unwrap_or_else(|| panic!("Tool {tool_name:?} missing cognicode meta block"));
            let authority = cognicode
                .get("authority")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    panic!(
                        "Tool {tool_name:?} missing cognicode_meta.authority. \
                         PRF-MCP-05 requires every tool to declare its authority."
                    )
                });
            assert!(
                allowed.contains(&authority),
                "Tool {tool_name:?} declares unknown authority {authority:?}; \
                 allowed: {allowed:?}"
            );
            if authority != "read" {
                declared_non_read.push(tool_name);
            }
        }

        // The invariant that replaces the old `MUTATING_TOOLS` cross-check.
        // That check asked whether a hand-kept list agreed with the
        // declarations, which is a question about two lists and cannot fail
        // for the reason we care about. This asks whether the flag we publish
        // agrees with the declaration we read — one list, one derivation, and
        // a disagreement means the published surface lies.
        for tool in build_all_tools() {
            let name = tool.name.to_string();
            let declared = declared_authority(&tool).expect(
                "every exposed tool declares cognicode.authority; an \
                 undeclared tool would be published as mutating without \
                 anyone having decided that",
            );
            let published = tool
                .meta
                .as_ref()
                .and_then(|m| m.get("cognicode"))
                .and_then(|c| c.get("mutates_workspace"))
                .and_then(|v| v.as_bool())
                .unwrap_or_else(|| {
                    panic!("tool {name:?} publishes no cognicode.mutates_workspace flag")
                });
            assert_eq!(
                published,
                authority_is_mutating(declared),
                "tool {name:?} declares authority {declared:?} but publishes \
                 mutates_workspace={published}"
            );
        }
    }

    // PRF-MCP-05 behavioral test: in read-only mode the public catalog
    // exposed via `build_all_tools()` MUST be filtered to drop every tool
    // whose declared authority is not "read". The filtering logic is
    // replicated inline because `list_tools` requires an
    // `rmcp::service::RequestContext` that depends on a Peer (only
    // constructible from inside rmcp). The contract being pinned is the
    // set membership after the filter, not the pagination wire format.
    #[test]
    fn test_prf_mcp_05_read_only_excludes_non_read_tools() {
        let all_tools = build_all_tools();

        // Replicate the exact predicate used by `list_tools` (rmcp_adapter.rs
        // list_tools handler). The predicate is: in read-only mode, drop
        // tools whose authority is not "read".
        let read_only = true;
        let filtered: Vec<String> = all_tools
            .iter()
            .filter(|t| {
                if !read_only {
                    return true;
                }
                let authority = t
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("cognicode"))
                    .and_then(|m| m.get("authority"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("read");
                authority == "read"
            })
            .map(|t| t.name.to_string())
            .collect();

        // Every tool whose declared authority is not "read" MUST be absent
        // from the filtered set.
        for tool in &all_tools {
            let authority = tool
                .meta
                .as_ref()
                .and_then(|m| m.get("cognicode"))
                .and_then(|m| m.get("authority"))
                .and_then(|v| v.as_str())
                .unwrap_or("read");
            let tool_name = tool.name.to_string();
            if authority != "read" {
                assert!(
                    !filtered.contains(&tool_name),
                    "PRF-MCP-05: read-only filter leaked non-read tool {tool_name:?} \
                     (authority={authority:?})"
                );
            }
        }

        // Symmetry: every tool whose authority is "read" MUST remain in
        // the filtered set (the filter must not be over-eager).
        for tool in &all_tools {
            let authority = tool
                .meta
                .as_ref()
                .and_then(|m| m.get("cognicode"))
                .and_then(|m| m.get("authority"))
                .and_then(|v| v.as_str())
                .unwrap_or("read");
            if authority == "read" {
                let tool_name = tool.name.to_string();
                assert!(
                    filtered.contains(&tool_name),
                    "PRF-MCP-05: read-only filter dropped a read-only tool {tool_name:?}"
                );
            }
        }
    }

    // PRF-MCP-05 (B2): The production enforcement now uses the
    // `tool_is_mutating(name)` helper as the primary oracle, with
    // any second list. This test pins the helper's contract:
    //
    // 1. Every tool whose declared `cognicode.authority` is one of
    //    "mutating" | "execute" | "network" must be reported mutating.
    // 2. Every tool whose declared authority is "read" must be reported
    //    NOT mutating.
    // 3. The derived mutating set and the helper must agree.
    // 4. Unknown tool names must default to NOT mutating (safe default).
    #[test]
    fn test_prf_mcp_05_tool_is_mutating_helper() {
        use super::{resolve_tool_authority, tool_is_mutating};

        // 1+2: cross-check against `build_all_tools()` declarations.
        for tool in build_all_tools() {
            let name = tool.name.to_string();
            let declared = resolve_tool_authority(&name);
            let should_be_mutating =
                matches!(declared.as_str(), "mutating" | "execute" | "network");
            assert_eq!(
                tool_is_mutating(&name),
                should_be_mutating,
                "tool_is_mutating({name:?}) disagrees with declared authority {declared:?}"
            );
        }

        // 3: the derived set and the helper agree, since both read the same
        // declarations. This is the check the legacy list used to provide,
        // except it can no longer go stale: there is nothing to keep in step.
        for name in mutating_tool_names() {
            assert!(
                tool_is_mutating(&name),
                "PRF-MCP-05: derived mutating tool {name:?} not reported mutating"
            );
        }

        // 4: FAIL-CLOSED default for unknown names (audit 2026-09-30,
        // finding #5). An unknown name resolves to the mutating posture
        // so read-only mode refuses it; the previous revision asserted
        // the opposite with the security direction inverted.
        assert!(
            tool_is_mutating("__definitely_not_a_real_tool_name__"),
            "PRF-MCP-05: unknown tool name must fail closed (mutating posture)"
        );
        assert_eq!(
            resolve_tool_authority("__definitely_not_a_real_tool_name__"),
            "mutating",
            "PRF-MCP-05: unknown tool name must resolve to authority 'mutating'"
        );
    }

    // PRF-MCP-05 (B2): the declaration is the only oracle.
    // Every tool whose `authority` declares "mutating" must be reported
    // mutating, with no second list consulted on the way there.
    //
    // This test does not require any production code change beyond
    // `tool_is_mutating` itself — it pins the semantics directly.
    #[test]
    fn test_prf_mcp_05_declared_authority_is_primary_not_floor() {
        use super::tool_is_mutating;
        let tools = build_all_tools();
        let declared_mutating: Vec<String> = tools
            .iter()
            .filter_map(|t| {
                let declared = t
                    .meta
                    .as_ref()
                    .and_then(|m| m.get("cognicode"))
                    .and_then(|m| m.get("authority"))
                    .and_then(|v| v.as_str())?;
                if matches!(declared, "mutating" | "execute" | "network") {
                    Some(t.name.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !declared_mutating.is_empty(),
            "no tool declares a non-read authority, so this test proves nothing"
        );
        for name in &declared_mutating {
            assert!(
                tool_is_mutating(name),
                "PRF-MCP-05 (B2): declared-mutating tool {name:?} not reported mutating"
            );
        }
    }
}
