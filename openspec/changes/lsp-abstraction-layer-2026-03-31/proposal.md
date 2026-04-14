# Proposal: LSP Abstraction Layer for Multi-Language Code Intelligence

## Intent

CogniCode currently relies on tree-sitter for all code analysis, which lacks semantic understanding (no type resolution, no cross-file navigation, no real go-to-definition). External LSP servers (rust-analyzer, pyright, typescript-language-server) provide this intelligence perfectly. We need to integrate them as providers behind an abstraction, with tree-sitter as fallback, and provide clear errors when tools are unavailable.

## Scope

### In Scope
- Tool availability discovery (`ToolAvailabilityChecker`) with descriptive error messages and install instructions per language
- LSP process lifecycle management (`LspProcessManager`): spawn, initialize, capability negotiation, shutdown
- Implementation of existing `CodeIntelligenceProvider` trait via `LspIntelligenceProvider`
- Tree-sitter fallback provider (`TreesitterFallbackProvider`) for when no LSP is available
- Composite provider with graceful degradation
- MCP tools: `go_to_definition`, `hover`, `find_references`
- CLI commands: `cognicode navigate <subcommand>`, `cognicode doctor`
- Language server registry mapping `Language` → server binary + install instructions

### Out of Scope
- Go (gopls), C/C++ (clangd) support — deferred to future change
- `textDocument/completion` — different UX, separate change
- `textDocument/rename` — we have our own refactoring system
- Replacing existing tree-sitter call graph with LSP call hierarchy — future enhancement
- LSP server implementation (we remain a client only)
- `async-lsp` or `tower-lsp` integration — overkill for our needs

## Approach

Thin LSP client with 4-phase implementation:

1. **Tool Discovery** — detect installed servers, provide errors with install instructions
2. **Process Manager** — spawn LSP servers as child processes, JSON-RPC over stdio
3. **Provider Implementation** — `CodeIntelligenceProvider` trait routed to LSP or tree-sitter fallback
4. **Exposure** — MCP tools and CLI commands for definition, hover, references

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/infrastructure/lsp/` | Modified/New | Process manager, tool checker, JSON-RPC transport |
| `src/domain/traits/code_intelligence.rs` | Modified | May need adjustment for async fallback pattern |
| `src/application/services/lsp_proxy_service.rs` | Modified | Replace placeholder with real delegation |
| `src/infrastructure/parser/tree_sitter_parser.rs` | Modified | Add server metadata to `Language` enum |
| `src/interface/mcp/server.rs` | Modified | Register new tools |
| `src/interface/mcp/handlers.rs` | Modified | New handlers |
| `src/interface/cli/commands.rs` | Modified | New `navigate` and `doctor` subcommands |
| `src/interface/mcp/schemas.rs` | Modified | New input/output schemas |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| LSP process crashes/hangs | Medium | Watchdog timeout (30s), auto-restart, fallback |
| lsp-types version mismatch | Low | We control both sides; pin to 0.93 |
| Server startup latency (1-5s) | High | Async pre-warming on project open, status caching |
| Resource usage from multiple servers | Medium | Lazy start (only when needed), idle shutdown after 5min |
| Incomplete capability negotiation | Low | Check `ServerCapabilities` before routing; fallback if unsupported |

## Rollback Plan

All changes in new files under `infrastructure/lsp/`. Existing tree-sitter paths untouched. Rollback = delete new files, remove new MCP tool registrations, revert CLI additions. No existing behavior changes.

## Dependencies

- `tokio::process` (already in deps via tokio) for child process management
- `lsp-types = "0.93"` (already in Cargo.toml)
- No new external crates needed

## Success Criteria

- [ ] `cargo run --bin cognicode doctor` shows available/unavailable servers per language with install instructions
- [ ] `go_to_definition` MCP tool works with rust-analyzer for Rust code
- [ ] `hover` returns type info and documentation
- [ ] `find_references` returns semantic (not text-match) references
- [ ] When no LSP server installed, operations fall back to tree-sitter with a warning
- [ ] All new code covered by unit tests (mock LSP responses)
- [ ] Zero regression in existing 295 tests
