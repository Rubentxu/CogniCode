# Design: LSP Abstraction Layer for Multi-Language Code Intelligence

## Technical Approach

Implement a thin LSP client behind the existing `CodeIntelligenceProvider` trait. The `Language` enum gets extended with server metadata. A new `ToolAvailabilityChecker` verifies binary presence. An `LspProcessManager` handles spawn/init/communicate/shutdown lifecycle via `tokio::process` and JSON-RPC over stdio. A `CompositeProvider` tries LSP first, falls back to tree-sitter on any failure.

## Architecture Decisions

### Decision: JSON-RPC Transport

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Custom JSON-RPC over stdio | Minimal deps, full control | **Chosen** |
| `async-lsp` crate | Full protocol, heavy deps | Rejected — overkill for 5 ops |
| `tower-lsp` client mode | Tower ecosystem | Rejected — designed for servers |

**Rationale**: We only need 3-5 LSP operations. A simple `Content-Length` header + JSON body parser is ~150 lines. Avoids tower/async-lsp transitive deps.

### Decision: One Process Per Language

| Option | Tradeoff | Decision |
|--------|----------|----------|
| One process per language | Simpler routing, shared cache | **Chosen** |
| One process per workspace | Language multiplexing complexity | Rejected |
| On-demand per file | Excessive spawn overhead | Rejected |

**Rationale**: Each language server (rust-analyzer, pyright) handles one language. One process per language with lazy start is simplest and matches how editors work.

### Decision: Crash Loop Detection

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Track crash count in memory | Session-scoped, simple | **Chosen** |
| Persistent state file | Survives restarts, complexity | Rejected |

**Rationale**: Crash loops are rare and session-scoped. A simple `Arc<AtomicU32>` counter with a time window is sufficient.

### Decision: Extend Language Enum vs Separate Registry

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add methods to `Language` enum | Co-located, follows existing pattern | **Chosen** |
| Separate `LanguageServerRegistry` struct | Decoupled, more flexible | Rejected |

**Rationale**: The `Language` enum already has `from_extension()`, `name()`, `function_node_type()` etc. Adding `lsp_server_binary()` and `lsp_install_command()` follows the established pattern.

## Data Flow

```
MCP/CLI Request
     │
     ▼
CompositeProvider
     │
     ├─ LSP available? ─── Yes ──→ LspIntelligenceProvider
     │                                   │
     │                                   ▼
     │                              LspProcessManager
     │                                   │
     │                              ┌────┴────┐
     │                              │ spawn   │
     │                              │ init    │
     │                              │ route   │
     │                              │ timeout │
     │                              └────┬────┘
     │                                   │
     │                              JSON-RPC over stdio
     │                                   │
     │                                   ▼
     │                            rust-analyzer / pyright / ...
     │
     └─ LSP unavailable? ──→ TreesitterFallbackProvider
                                   │
                                   ▼
                            LightweightIndex + SymbolCodeExtractor
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/infrastructure/lsp/tool_checker.rs` | Create | `ToolAvailabilityChecker` — `which` command, version extraction |
| `src/infrastructure/lsp/process.rs` | Create | `LspProcess` — spawn, init, send/receive JSON-RPC, shutdown |
| `src/infrastructure/lsp/process_manager.rs` | Create | `LspProcessManager` — pool of `LspProcess` by language, lazy start, idle timeout |
| `src/infrastructure/lsp/json_rpc.rs` | Create | `JsonRpcTransport` — `Content-Length` framing, request/response/id matching |
| `src/infrastructure/lsp/providers/mod.rs` | Create | Module root for providers |
| `src/infrastructure/lsp/providers/lsp_provider.rs` | Create | `LspIntelligenceProvider` — implements `CodeIntelligenceProvider` via LSP |
| `src/infrastructure/lsp/providers/fallback_provider.rs` | Create | `TreesitterFallbackProvider` — implements trait via tree-sitter |
| `src/infrastructure/lsp/providers/composite.rs` | Create | `CompositeProvider` — LSP-first with fallback |
| `src/infrastructure/lsp/mod.rs` | Modify | Export new modules |
| `src/infrastructure/parser/tree_sitter_parser.rs` | Modify | Add `lsp_server_binary()`, `lsp_install_command()`, `lsp_args()` to `Language` |
| `src/domain/traits/code_intelligence.rs` | Modify | Add `hover` method to trait, add `LspUnavailable` error variant |
| `src/application/services/lsp_proxy_service.rs` | Modify | Wire to `CompositeProvider`, remove placeholder logic |
| `src/interface/mcp/handlers.rs` | Modify | Add `handle_go_to_definition`, `handle_hover`, `handle_find_references` |
| `src/interface/mcp/server.rs` | Modify | Register 3 new tools |
| `src/interface/mcp/schemas.rs` | Modify | Add input/output types for new tools |
| `src/interface/cli/commands.rs` | Modify | Add `NavigateCommand`, `DoctorCommand` |

## Interfaces / Contracts

```rust
// New method on existing Language enum
impl Language {
    pub fn lsp_server_binary(self) -> &'static str { ... }
    pub fn lsp_install_command(self) -> &'static str { ... }
    pub fn lsp_args(self) -> &'static [&'static str] { ... }
}

// Tool availability
pub struct ToolStatus {
    pub available: bool,
    pub binary_path: Option<PathBuf>,
    pub version: Option<String>,
    pub install_command: &'static str,
}

pub trait ToolAvailabilityChecker: Send + Sync {
    fn check(&self, language: Language) -> ToolStatus;
    fn doctor_report(&self) -> Vec<(Language, ToolStatus)>;
}

// New trait method
pub trait CodeIntelligenceProvider: Send + Sync {
    // ... existing methods ...
    async fn hover(&self, location: &Location) -> Result<Option<HoverInfo>, CodeIntelligenceError>;
}

// Hover result (new type)
pub struct HoverInfo {
    pub content: String,
    pub documentation: Option<String>,
    pub kind: HoverKind, // Type, Documentation, Mixed
}

// Process manager
pub struct LspProcessManager {
    processes: RwLock<HashMap<Language, LspProcessHandle>>,
    crash_counts: DashMap<Language, CrashRecord>,
}

pub struct LspProcessHandle {
    child: Child,
    capabilities: ServerCapabilities,
    last_activity: Instant,
    request_id: AtomicU64,
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `ToolAvailabilityChecker` | Mock `which` via tempdir + fake binary |
| Unit | `JsonRpcTransport` framing | Encode/decode roundtrip tests |
| Unit | `Language` enum methods | Static assertions |
| Unit | Provider trait implementations | Mock `LspProcessManager` |
| Integration | `CompositeProvider` | Start real pyright/rust-analyzer, send requests |
| Integration | `LspProcessManager` lifecycle | Spawn, init, request, shutdown sequence |
| Integration | Crash loop detection | Kill child process, verify restart logic |
| CLI | `cognicode doctor` | Run against real system |
| CLI | `cognicode navigate` | Run against real Rust/Python project |

## Migration / Rollout

No migration required. All changes are additive:
- New files under `infrastructure/lsp/`
- New methods on existing types (non-breaking)
- New MCP tools (opt-in for clients)
- New CLI commands (opt-in for users)
- Existing tree-sitter paths remain the default fallback

## Open Questions

- [ ] Should `LspProcessManager` be a singleton (`Arc`) or created per-request? → **Singleton** (one process pool per application lifetime)
- [ ] Should we pre-warm servers on `initialize` or lazy-start on first request? → **Lazy start** (simpler, no wasted resources for unused languages)
- [ ] JSON-RPC notifications (e.g., `textDocument/didOpen`) — do we need to sync file state? → **Phase 2** — for now, servers index on their own
