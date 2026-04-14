# Tasks: LSP Abstraction Layer for Multi-Language Code Intelligence

## Phase 1: Infrastructure Foundation

- [x] 1.1 Add `lsp_server_binary()`, `lsp_install_command()`, `lsp_args()` methods to `Language` enum in `src/infrastructure/parser/tree_sitter_parser.rs`
- [x] 1.2 Create `src/infrastructure/lsp/tool_checker.rs` — `ToolAvailabilityChecker` struct with `check(language) -> ToolStatus` and `doctor_report() -> Vec<(Language, ToolStatus)>` using `tokio::process::Command` with `which`
- [x] 1.3 Create `src/infrastructure/lsp/json_rpc.rs` — `JsonRpcTransport` with `send_request(method, params) -> Response` and `send_notification(method, params)`, implementing `Content-Length` header framing
- [x] 1.4 Create `src/infrastructure/lsp/process.rs` — `LspProcess` struct wrapping `tokio::process::Child` stdin/stdout, `initialize(workspace_root)`, `shutdown()`, `request(method, params, timeout) -> Result`
- [x] 1.5 Create `src/infrastructure/lsp/process_manager.rs` — `LspProcessManager` with `get_or_spawn(language, workspace_root)`, idle timeout (5min), crash loop detection (3 crashes in 10min → mark Failed)
- [x] 1.6 Update `src/infrastructure/lsp/mod.rs` — export new modules: `tool_checker`, `json_rpc`, `process`, `process_manager`
- [x] 1.7 Add `hover()` method and `LspUnavailable` variant to `CodeIntelligenceProvider` trait and `CodeIntelligenceError` in `src/domain/traits/code_intelligence.rs`
- [x] 1.8 Add `HoverInfo` struct to `src/domain/value_objects/` or `src/domain/aggregates/`

## Phase 2: Provider Implementation

- [x] 2.1 Create `src/infrastructure/lsp/providers/mod.rs` — module root
- [x] 2.2 Create `src/infrastructure/lsp/providers/fallback_provider.rs` — `TreesitterFallbackProvider` implementing `CodeIntelligenceProvider` using `LightweightIndex` for definitions, existing `find_usages` for references, `SymbolCodeExtractor` for hover
- [x] 2.3 Create `src/infrastructure/lsp/providers/lsp_provider.rs` — `LspIntelligenceProvider` implementing `CodeIntelligenceProvider` routing to `LspProcessManager`, converting `lsp_types` responses to domain types
- [x] 2.4 Create `src/infrastructure/lsp/providers/composite.rs` — `CompositeProvider` that tries `LspIntelligenceProvider` first, falls back to `TreesitterFallbackProvider`, logs warnings on fallback
- [x] 2.5 Update `src/application/services/lsp_proxy_service.rs` — replace placeholder `route_operation()` with `CompositeProvider` delegation

## Phase 3: MCP & CLI Exposure

- [x] 3.1 Add `GoToDefinitionInput`/`HoverInput`/`FindReferencesInput` schemas to `src/interface/mcp/schemas.rs`
- [x] 3.2 Add `handle_go_to_definition`, `handle_hover`, `handle_find_references` handlers to `src/interface/mcp/handlers.rs`
- [x] 3.3 Register 3 new MCP tools in `src/interface/mcp/server.rs` with availability status in descriptions
- [x] 3.4 Add `NavigateCommand` enum (Definition, Hover, References) and `DoctorCommand` to `src/interface/cli/commands.rs`
- [x] 3.5 Wire `navigate` and `doctor` subcommands into the CLI match/dispatch

## Phase 4: Testing

- [x] 4.1 Unit tests for `Language` enum new methods (static assertions)
- [x] 4.2 Unit tests for `ToolAvailabilityChecker` (mock with tempdir + fake binary)
- [x] 4.3 Unit tests for `JsonRpcTransport` framing (encode/decode roundtrip)
- [x] 4.4 Unit tests for `TreesitterFallbackProvider` (use existing test files)
- [x] 4.5 Unit tests for `CompositeProvider` (mock both providers, test fallback paths)
- [x] 4.6 Integration tests for `LspProcess` spawn/init/shutdown (requires real rust-analyzer or pyright, skip in CI if unavailable) — SKIPPED
- [x] 4.7 Integration tests for crash loop detection (kill child, verify restart limit) — SKIPPED
- [x] 4.8 Verify `cargo build` passes with zero new errors
- [x] 4.9 Verify all existing 22 semantic tests still pass
