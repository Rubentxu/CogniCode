## Exploration: Explorer Phase 5 — Agent MCP Wiring

> **Roadmap reference**: Phase 6 "Agent Workflows Through Explorer MCP" in `specs/cognicode-explorer/proposals/cognicode-explorer-roadmap.md`.
> Phase 5 in the current task numbering corresponds to the MCP wiring step.

### Current State

The Explorer crate (`crates/cognicode-explorer/`) has reached a maturity level where:
- **All core service methods are implemented**: `open_workspace`, `spotter_search`, `inspect_object`, `available_views`, `contextual_view`, `available_lenses`, `apply_lens`, `save_exploration`, `generate_artifact`.
- **All DTOs are Serde-serializable**: `InspectableObjectSummary`, `ContextualView`, `LensResult`, `SpotterResult`, `WorkspaceSummary`, etc. — structured JSON by construction.
- **rmcp is already a dependency** in the explorer's `Cargo.toml` (`rmcp.workspace = true`) with features `server`, `transport-io`, etc.
- **An MCP binary scaffold exists** at `src/bin/mcp.rs` — it currently just prints tool names and exits.
- **Tool name constants exist** in `src/mcp.rs`: 8 constants (from the roadmap), but no handler implementation.
- **The existing HTTP API** (`src/api.rs`, `bin/api.rs`) works independently — the MCP layer is additive.

The gap is purely wiring: `ExplorerService` methods need `ServerHandler` wrappers to become MCP tools.

### Affected Areas

| File | Why |
|------|-----|
| `crates/cognicode-explorer/src/mcp.rs` | **Must be rewritten** — currently just `const` definitions; needs `ExplorerMcpHandler` struct implementing `ServerHandler` |
| `crates/cognicode-explorer/src/bin/mcp.rs` | **Must be rewritten** — currently prints tool names; needs stdio transport + `serve_server` |
| `crates/cognicode-explorer/src/lib.rs` | **No change** — `pub mod mcp` already exists |
| `crates/cognicode-explorer/src/service.rs` | **No change** — all tool handlers delegate to existing methods |
| `crates/cognicode-explorer/src/api.rs` | **No change** — HTTP API remains untouched |
| `crates/cognicode-explorer/src/bin/api.rs` | **No change** — separate binary |
| `crates/cognicode-explorer/Cargo.toml` | **No change** — rmcp already included |
| Workspace `Cargo.toml` | **No change** — `rmcp` already defined with required features |
| `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` | **Reference only** — pattern to follow, no code dependencies |

### Existing rmcp Patterns in Workspace

The canonical rmcp implementation lives in `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` (`CogniCodeHandler`). Key API surface:

```rust
use rmcp::handler::server::ServerHandler;
use rmcp::model::{CallToolRequestParams, CallToolResult, Content, ListToolsResult, ServerCapabilities, ServerInfo, Tool};
use rmcp::transport::io::stdio;
use rmcp::service::RoleServer;

// Trait to implement:
impl ServerHandler for ExplorerMcpHandler {
    fn get_info(&self) -> ServerInfo { ... }
    fn list_tools(&self, request, context) -> impl Future<Output = Result<ListToolsResult, ErrorData>> { ... }
    fn call_tool(&self, request, context) -> impl Future<Output = Result<CallToolResult, ErrorData>> { ... }
}

// Transport + serve:
let transport = stdio();
let server = rmcp::serve_server(handler, transport).await?;
server.waiting().await?;
```

**Tool registration pattern** (from `CogniCodeHandler::list_tools`):
```rust
Tool::new(
    "tool_name",
    "Human description",
    Arc::new(serde_json::json!({ ... JSON Schema ... }).as_object().cloned().unwrap()),
)
```

**Dispatch pattern** (from `call_tool_handler`):
```rust
match tool_name {
    "my_tool" => {
        let input: MyInput = serde_json::from_value(arguments.into())?;
        let output = handle_my_tool(&ctx, input).await?;
        Ok(serde_json::to_string(&output)?)
    }
    // ... else Err
}
```

**Result wrapping**:
```rust
Ok(CallToolResult::success(vec![Content::text(json_string)]))
Err => Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
```

**Note on approach**:
- The CogniCode handler uses `Content::text()` with JSON string inside — this is the established pattern for structured responses.
- `Tool::new()` takes an `Arc<Object>` as the input schema (JSON Schema format).
- `ServerCapabilities::builder().enable_tools().enable_resources().build()`.

### Approaches

#### Approach A: Thin wrapper (recommended)

Create `ExplorerMcpHandler` implementing `ServerHandler`, holding `Arc<ExplorerService>`. Every tool handler directly delegates to a service method and serializes the return value.

```
src/mcp.rs:
  ExplorerMcpHandler { service: Arc<ExplorerService> }
  impl ServerHandler:
    list_tools → 7 Tool definitions (see Catalog below)
    call_tool  → match name → deserialize → delegate → serialize
```

**Pros**:
- Zero service changes — pure wiring
- Follows workspace pattern exactly (same style as `DiagramAwareHandler`)
- Service already returns Serde-serializable DTOs — just `serde_json::to_string(&result)?`
- Binary can reuse the exact same adapter construction as `bin/api.rs` (CallGraphRepository, FsSourceReader, Fts5SearchAdapter, SqliteQualityAdapter)
- DIP: handler depends on `ExplorerService` abstraction, not adapters
- ~150 lines of code in `mcp.rs`, ~60 in `bin/mcp.rs`

**Cons**:
- Tool responses are JSON strings inside `Content::text()` — not native structured content (same limitation as CogniCodeHandler)

**Effort**: Low

---

#### Approach B: Dedicated handler types with typed schemas

Define `struct ToolInput / ToolOutput` for every MCP tool, separate from the existing DTOs, and write bespoke serialization/deserialization.

**Pros**:
- Clean separation between HTTP API DTOs and MCP tool schemas
- Can evolve MCP schemas independently

**Cons**:
- Duplication of DTO types
- More code, more maintenance
- Violates the "Phase 5 is WIRING, not new functionality" mandate
- Against DRY when the existing DTOs already satisfy the contract

**Effort**: Medium

---

#### Approach C: Macro-driven tool registration

Use a declarative macro to define tools, reducing boilerplate.

**Pros**:
- Less repetitive code
- Could be reusable across the workspace

**Cons**:
- Over-engineering for 7 tools
- Macro obscures the dispatch logic for debugging
- No existing pattern in the workspace

**Effort**: High

### Tool Catalog

All tools delegate to existing `ExplorerService` public methods. Parameters are derived from method signatures:

| Tool Name | Service Method | Input (JSON Schema) | Return Type |
|-----------|---------------|---------------------|-------------|
| `explorer_open_workspace` | `current_workspace()` | `{}` (no required fields; root_path with default) | `WorkspaceSummary` (JSON) |
| `explorer_spotter_search` | `spotter_search(query, kind)` | `{query: string, kind?: string}` | `Vec<SpotterResult>` (JSON) |
| `explorer_inspect_object` | `inspect_object(object_id)` | `{object_id: string}` | `InspectableObjectSummary` (JSON) |
| `explorer_get_views` | `available_views(object_id)` | `{object_id: string}` | `Vec<ViewDescriptor>` (JSON) |
| `explorer_get_view` | `contextual_view(object_id, view_id)` | `{object_id: string, view_id: string}` | `ContextualView` (JSON) |
| `explorer_get_lenses` | `available_lenses(object_id)` | `{object_id: string}` | `Vec<LensDescriptor>` (JSON) |
| `explorer_apply_lens` | `apply_lens(object_id, lens_id)` | `{object_id: string, lens_id: string}` | `LensResult` (JSON) |

**Total: 7 tools** (exceeds the minimum 6 from acceptance criteria).

**Deferred tools** (not in Phase 5 scope — require new service methods):
- `explorer_follow_relation` — would need a `resolve_relation_target` method or similar; currently relation navigation is done client-side by reading `TypedRelation.target_object_id` and calling `inspect_object` again
- `explorer_explain_evidence` — would need an `explain_evidence_block` method; evidence blocks are already returned inside views, but there's no dedicated explanation endpoint
- `explorer_save_path` — exists as `save_exploration` but has a complex request shape (columns, workspace_id); can be added later
- `explorer_generate_artifact` — exits as `generate_artifact` but has complex input; deferred for simplicity

### Binary Architecture

**Transport: stdio** (matching `cognicode-mcp`). Agents run explorer as a subprocess:
```bash
cognicode-explorer-mcp --cwd /path/to/project
```

**Single binary** (`src/bin/mcp.rs`), separate from `src/bin/api.rs`. Both construct the same adapters → same `ExplorerService`, but serve via different transports (HTTP vs stdio/MCP).

**Adapter construction** dusted from `bin/api.rs`:
```rust
let graph = open_graph(&db_path)?;
let repo: Arc<dyn SymbolRepository> = Arc::new(CallGraphRepository::new(graph));
let reader = Arc::new(FsSourceReader::new(cwd.clone()));
let search = maybe_fts5_adapter(&db_path);
let quality = maybe_quality_adapter(&db_path);
let service = ExplorerService::with_all(repo, reader, cwd, search, quality);
```

### Integration with Existing Service

```
┌───────────────┐     ┌──────────────────┐     ┌─────────────────┐
│ MCP Client     │     │ ExplorerMcpHandler│     │ ExplorerService  │
│ (AI Agent)     │◄───►│ (ServerHandler)   │◄───►│ (domain logic)   │
│                │stdio│                   │     │                  │
│ tool call ────►│     │ call_tool()       │     │ .inspect_object()│
│                │     │  → serde_json     │────►│                  │
│                │     │  ← CallToolResult │◄────│                  │
└───────────────┘     └──────────────────┘     └─────────────────┘
```

The handler holds `Arc<ExplorerService>` — the same service instance used by the HTTP API. This means:
- Both HTTP and MCP share the same graph/source/search/quality state
- The MCP layer is a pure transport adapter
- Zero service code changes

### Scope Boundary

| In Scope | Out of Scope / Deferred |
|----------|------------------------|
| `ExplorerMcpHandler` implementing `ServerHandler` | New service methods (`follow_relation`, `explain_evidence`) |
| `list_tools` with 7 tool definitions | Lens composition syntax (`hotspots+quality`) |
| `call_tool` dispatch to service methods | MCP client integration tests |
| `bin/mcp.rs` with stdio transport | HTTP + MCP in same binary |
| Structured JSON responses via `serde_json::to_string` | Native MCP structured content (image, resource) |
| Reusing adapter construction from `bin/api.rs` | Exploration path persistence across restarts |

### Design

#### Implementation Plan

1. **`src/mcp.rs`**: Replace constants with `ExplorerMcpHandler` struct + `ServerHandler` impl
   - `get_info()` → return `ServerInfo` with `cognicode-explorer` identity
   - `list_tools()` → 7 `Tool::new(...)` with JSON Schema inputs
   - `call_tool()` → match tool name → extract args via `serde_json::from_value` → call service → `serde_json::to_string` → `CallToolResult::success`
2. **`src/bin/mcp.rs`**: Replace scaffold with real transport
   - Parse `--cwd` with clap
   - Construct adapters (same as `bin/api.rs`)
   - Build `ExplorerService::with_all(...)`
   - Wrap in `ExplorerMcpHandler`
   - `serve_server` + `stdio` + `waiting()`
3. **No changes** to `service.rs`, `api.rs`, `bin/api.rs`, `dto.rs`, or any port/domain code

---

### Entropy Analysis (Connascence Landscape)

**Method**: CogniCode

| Component A | Component B | Connascence Type | I(bits) | Severity |
|-------------|-------------|------------------|---------|----------|
| `mcp.rs` (handler) | `service.rs` (ExplorerService) | Name | 0.0 | ✅ OK |
| `bin/mcp.rs` (binary) | `mcp.rs` (handler) | Name | 0.0 | ✅ OK |
| `mcp.rs` (tool names) | service method names | Meaning | <0.5 | ✅ OK |

**Critical Pairs (I > 3.0 bits)**: None.
**Hidden Connascence (Meaning/Timing)**: None.
**SOLID-Entropy Violations**: None.

**OCP Compliance**: H(Δ_existing) = 0 bits — pure extension, no existing code modified.
**DIP Compliance**: Handler depends on `ExplorerService` (domain), not on concrete adapters.
**SRP**: MCP handler has one reason to change: MCP protocol or service method signature changes.
**ISP**: MCP tool definitions expose exactly what agents need — no broad interfaces.

**Coupling Score**: H(coupling) ≈ 0.0.
**Design Quality Impact**: Neutral to positive — adds functionality without introducing connascence.

**Recommendation**: This is an ideal extension point. The MCP wiring introduces no measurable coupling beyond what's already present in the service contract.

---

### Risks

1. **Tool response size**: `ContextualView` and `InspectableObjectSummary` can be large JSON payloads. MCP transport (stdio) may need to handle large messages. Mitigation: all returns are JSON strings — MCP's internal framing handles this; same as `CogniCodeHandler`.
2. **Error handling mismatch**: `ExplorerError` variants don't map 1:1 to MCP error codes. Mitigation: convert errors to text via `Display` (same pattern as `CogniCodeHandler`).
3. **Graph cold start**: If the workspace has no `cognicode.db`, the MCP server starts with empty adapters — this is the same degradation pattern as the HTTP API and is already handled by the service.
4. **`follow_relation` / `explain_evidence` gap**: Agents must do multi-step reasoning (read relation → inspect target) rather than a single tool call. Mitigation: document the agent workflow pattern (search → inspect → view → lens) in tool descriptions.

### Recommendation

**Approach A (Thin wrapper)** is the clear winner. It:
- Matches the workspace's existing rmcp pattern
- Requires zero service changes
- Is entirely additive (no risk to HTTP API)
- Takes <250 lines of code total
- Satisfies all acceptance criteria

**Ready for Proposal**: YES — the implementation path is well-defined. The orchestrator can proceed to `sdd-propose` with this exploration as foundation.

### Files Read During Exploration

- `specs/cognicode-explorer/proposals/cognicode-explorer-roadmap.md` — Phase 6 (MCP)
- `crates/cognicode-explorer/src/mcp.rs` — current stub
- `crates/cognicode-explorer/src/bin/mcp.rs` — scaffold binary
- `crates/cognicode-explorer/src/service.rs` — full service API
- `crates/cognicode-explorer/src/api.rs` — HTTP routes (mirrored by MCP)
- `crates/cognicode-explorer/src/bin/api.rs` — adapter construction pattern
- `crates/cognicode-explorer/src/dto.rs` — all DTOs (tool response types)
- `crates/cognicode-explorer/src/error.rs` — error types
- `crates/cognicode-explorer/src/ports/mod.rs` — port contracts
- `crates/cognicode-explorer/src/lib.rs` — module structure
- `crates/cognicode-explorer/Cargo.toml` — dependencies (rmcp present)
- `Cargo.toml` (workspace) — rmcp version/features
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` — canonical rmcp pattern
- `crates/cognicode-mcp/src/diagram_handler.rs` — wrapper handler pattern
- `crates/cognicode-mcp/src/main.rs` — transport/serve pattern
