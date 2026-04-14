# Exploration: LSP Abstraction Layer for Multi-Language Code Intelligence

**Date:** 2026-03-31
**Status:** Explored
**Change:** `lsp-abstraction-layer-2026-03-31`

## Current State

### What exists today

| Component | Status | Location |
|-----------|--------|----------|
| `Language` enum (Rust, Python, JS, TS) | Working | `infrastructure/parser/tree_sitter_parser.rs:25` |
| `TreeSitterParser` | Working, real grammar support | `infrastructure/parser/tree_sitter_parser.rs` |
| `LspClient` | Data holder only, no process management | `infrastructure/lsp/client.rs` |
| `LspServerConfig` | Config struct, no spawning | `infrastructure/lsp/client.rs:17` |
| `LspProxyService` | Placeholder, returns `None` for all ops | `application/services/lsp_proxy_service.rs` |
| `CodeIntelligenceProvider` trait | Defined, NO implementation | `domain/traits/code_intelligence.rs` |
| `protocol.rs` conversions | Working domain↔LSP type mapping | `infrastructure/lsp/protocol.rs` |
| `lsp-types = "0.93"` | Available in Cargo.toml | `Cargo.toml:10` |
| LSP server binary | Placeholder, just logs and exits | `bin/lsp_server.rs` |

### What does NOT exist

1. **Tool availability discovery** — No code checks if `rust-analyzer`, `pyright`, etc. are installed
2. **LSP process lifecycle** — No spawning, stdio communication, or graceful shutdown
3. **LSP JSON-RPC message exchange** — No `initialize`, `textDocument/definition`, etc.
4. **Capability negotiation** — `ClientCapabilities` stored but never used
5. **Fallback strategy** — When external LSP is unavailable, no degradation path
6. **Descriptive error messages** — No "install rust-analyzer with `rustup component add rust-analyzer`" guidance
7. **Go-to-definition** — Only via tree-sitter heuristic (identifier matching), not real semantic resolution
8. **Hover/type info** — Completely missing
9. **Real find-references** — We do tree-sitter text matching, not semantic references

### Architecture constraints

- DDD + Clean Architecture (4 layers)
- All features MUST work in both CLI and MCP
- Performance targets: outline < 10ms, queries < 100ms
- tree-sitter 0.20 (upgrade to 0.24 planned separately)

## Affected Areas

- `src/infrastructure/lsp/` — Core of this change; needs process management, message routing
- `src/domain/traits/code_intelligence.rs` — Trait exists but needs implementation
- `src/application/services/lsp_proxy_service.rs` — Replace placeholder with real delegation
- `src/infrastructure/parser/tree_sitter_parser.rs` — `Language` enum needs LSP server mapping
- `src/interface/mcp/server.rs` — New MCP tools for LSP operations
- `src/interface/mcp/handlers.rs` — New handlers for go-to-definition, hover, references
- `src/interface/cli/commands.rs` — New CLI commands for LSP operations
- `Cargo.toml` — May need `tokio-process` or similar for async process management

## Research Findings

### LSP Crate Ecosystem (Rust)

| Crate | Purpose | Maturity |
|-------|---------|----------|
| `lsp-types` (0.93, already used) | Type definitions for LSP protocol | High |
| `tower-lsp` | Build LSP servers (we'd be a client) | High |
| `async-lsp` | Client+Server framework with middleware | High, 2265 snippets |
| `lsp-server` | Low-level message handling | Medium |

### LSP Operations We Need (by priority)

**Tier 1 — High value, maps to existing needs:**
| Operation | LSP Method | Our Equivalent | Gap |
|-----------|-----------|----------------|-----|
| Go to definition | `textDocument/definition` | None (tree-sitter heuristic) | Full |
| Find references | `textDocument/references` | `find_usages` (text match) | Semantic quality |
| Hover/type info | `textDocument/hover` | None | Full |
| Document symbols | `textDocument/documentSymbol` | `get_outline` (tree-sitter) | Already covered |
| Workspace symbols | `workspace/symbol` | `semantic_search` | Already covered |

**Tier 2 — Medium value, enhances premium features:**
| Operation | LSP Method | Benefit |
|-----------|-----------|---------|
| Call hierarchy | `textDocument/prepareCallHierarchy` | Semantic call graph (vs tree-sitter) |
| Type definition | `textDocument/typeDefinition` | Navigate to type source |
| Implementation | `textDocument/implementation` | Trait→impl navigation |
| Rename | `textDocument/rename` | Semantic rename (vs text) |

### Language Server Mapping

| Language | Recommended Server | Install Command | Priority |
|----------|-------------------|-----------------|----------|
| Rust | `rust-analyzer` | `rustup component add rust-analyzer` | P0 |
| Python | `pyright` | `npm install -g pyright` | P1 |
| TypeScript | `typescript-language-server` | `npm install -g typescript-language-server typescript` | P1 |
| JavaScript | `typescript-language-server` | (same as TS) | P1 |
| Go | `gopls` | `go install golang.org/x/tools/gopls@latest` | P2 |
| C/C++ | `clangd` | System package manager | P2 |

## Approaches

### Approach 1: Thin LSP Client with Tool Discovery (Recommended)

Build a minimal LSP client that:
1. Discovers available language servers at startup
2. Spawns them as child processes via `tokio::process::Command`
3. Communicates via stdin/stdout JSON-RPC
4. Implements `CodeIntelligenceProvider` trait
5. Falls back to tree-sitter when server unavailable

**Architecture:**
```
┌──────────────────────────────────────────────┐
│              CodeIntelligenceProvider         │  (domain trait - already exists)
│  get_definition / find_references / hover    │
└──────────────────┬───────────────────────────┘
                   │
        ┌──────────┴──────────┐
        ▼                     ▼
┌───────────────┐   ┌─────────────────┐
│ LspProvider   │   │ TreesitterFallback│
│ (if server    │   │ (always available)│
│  available)   │   │                   │
└───────┬───────┘   └───────────────────┘
        │
        ▼
┌───────────────────────────────┐
│     LspProcessManager         │
│  - spawn/process lifecycle    │
│  - JSON-RPC message exchange  │
│  - capability negotiation     │
└───────────────────────────────┘
        │
        ▼
┌───────────────────────────────┐
│     ToolAvailabilityChecker   │
│  - which/command -v check     │
│  - version validation         │
│  - install instructions       │
└───────────────────────────────┘
```

- **Pros:**
  - Reuses existing `CodeIntelligenceProvider` trait
  - Clean fallback: tree-sitter always works, LSP enhances when available
  - Minimal new dependencies (just tokio process, already have lsp-types)
  - Descriptive errors with install instructions
  - Each layer independently testable
- **Cons:**
  - LSP process lifecycle management adds complexity
  - Need careful timeout/error handling for unresponsive servers
  - First invocation latency (server startup)
- **Effort:** Medium (3-5 sessions)

### Approach 2: Use `async-lsp` as Client Framework

Use the `async-lsp` crate which provides full client+server middleware:
1. Add `async-lsp` dependency
2. Implement the `LanguageServer` trait as a client
3. Use built-in lifecycle, routing, and middleware
4. Connect to external servers via transport layer

- **Pros:**
  - Full protocol compliance (all LSP 3.17 operations)
  - Middleware stack for logging, error handling, retries
  - Type-safe request/response mapping
  - Less boilerplate for JSON-RPC
- **Cons:**
  - Heavy dependency (many transitive deps)
  - `async-lsp` is designed primarily for servers, client use is less documented
  - Requires tower ecosystem understanding
  - Overkill for the 5 operations we actually need
  - Version compatibility with our `lsp-types = "0.93"`
- **Effort:** Medium-High (5-8 sessions)

### Approach 3: Hybrid — Tree-sitter Enhanced with Lightweight Semantic Queries

Instead of full LSP client, enhance tree-sitter analysis:
1. Add scope-aware identifier resolution to tree-sitter parsing
2. Implement definition jumping via project-wide symbol index (already have `LightweightIndex`)
3. Add type inference heuristics for hover-like info
4. Keep LSP as future enhancement, not current priority

- **Pros:**
  - No external process dependency (fully self-contained)
  - Zero latency (no server startup)
  - Works offline, no install requirements
  - Builds on existing tree-sitter infrastructure
- **Cons:**
  - Will never match semantic accuracy of real LSP (macros, generics, cross-file type inference)
  - Complex to get right for each language
  - Duplicates effort that language servers already do perfectly
- **Effort:** Low-Medium (2-3 sessions for basic, ongoing for accuracy)

## Recommendation

**Approach 1 (Thin LSP Client) as primary, with Approach 3 elements as built-in fallback.**

Rationale:
- CogniCode's value proposition is being a **smart proxy** over existing tools
- Reimplementing what rust-analyzer already does perfectly is wasteful
- The `CodeIntelligenceProvider` trait already exists and is the right abstraction
- Tree-sitter fallback ensures CogniCode always works, even without external servers
- Tool availability checking with descriptive errors is a must-have for UX

**Implementation phases:**

### Phase 1: Tool Discovery + Error Infrastructure
- `ToolAvailabilityChecker` — detect installed language servers
- `LspError` with install instructions per language
- `Language` enum extension with server metadata
- No process management yet — just availability checking

### Phase 2: LSP Process Manager
- `LspProcess` — spawn, initialize, shutdown lifecycle
- JSON-RPC message framing over stdio
- Capability negotiation (store `ServerCapabilities`)
- Connection pooling (one process per language, reuse)

### Phase 3: CodeIntelligenceProvider Implementation
- `LspIntelligenceProvider` — routes to appropriate LSP process
- `TreesitterFallbackProvider` — uses existing tree-sitter when LSP unavailable
- `CompositeProvider` — tries LSP first, falls back to tree-sitter
- Wire into MCP handlers and CLI commands

### Phase 4: MCP/CLI Exposure
- New MCP tools: `go_to_definition`, `hover`, `find_references`
- New CLI commands: `cognicode navigate definition <file:line:col>`, `hover`, `references`
- Status command: `cognicode doctor` — shows available tools per language

## Risks

1. **LSP process stability** — External servers can crash, hang, or consume too much memory. Need watchdog/timeout.
2. **Version compatibility** — Different LSP server versions may have different capabilities. Need version checks.
3. **Multi-process coordination** — Running multiple LSP servers simultaneously requires careful resource management.
4. **Latency on first use** — Server startup can take 1-5 seconds. Need async initialization or pre-warming.
5. **lsp-types version** — We're on 0.93, latest async-lsp may require newer. Verify compatibility before committing.

## Ready for Proposal

**Yes.** The exploration is complete. The next step is to create the SDD proposal with the recommended Approach 1 + fallback strategy, structured in 4 phases.
