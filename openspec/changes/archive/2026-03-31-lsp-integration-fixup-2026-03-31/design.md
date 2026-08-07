# Design: LSP Integration Fixup & End-to-End Testing

## Technical Approach

Wire `CompositeProvider` into `LspProxyService.route_operation()` to fix the incomplete task 2.5 integration. The service currently returns `None` for all operations; it must map string-based operation names (`"hover"`, `"textDocument/definition"`) to `CompositeProvider` async trait method calls. Integration tests will spawn real LSP binaries and validate the full stack.

## Architecture Decisions

### Decision: Map string operations to CompositeProvider methods

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add `route_operation` wrapper that maps strings to trait calls | Thin bridge, follows existing pattern | **Chosen** |
| Refactor `LspProxyService` to accept `CodeIntelligenceProvider` directly | More flexible, larger diff | Rejected |

**Rationale**: `route_operation()` is string-based (from MCP/CLI layer). Adding a thin mapping layer preserves the existing interface while delegating to the typed trait. Minimal diff.

### Decision: CompositeProvider lifetime management

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Store `Arc<CompositeProvider>` in `LspProxyService` | Shared ownership, simple | **Chosen** |
| Create fresh `CompositeProvider` per request | No state, but expensive (re-reads workspace) | Rejected |

**Rationale**: `LspProcessManager` inside `LspIntelligenceProvider` maintains a process pool per language. Creating a new provider per request would re-spawn processes. One `Arc` instance shared across calls is correct.

### Decision: workspace_root propagation

| Option | Tradeoff | Decision |
|--------|----------|----------|
| Add `workspace_root: PathBuf` field to `LspProxyService` | Explicit, traceable | **Chosen** |
| Use `std::env::current_dir()` at runtime | Implicit, fragile | Rejected |

**Rationale**: `CompositeProvider::new()` requires `workspace_root`. Making it explicit in the service constructor ensures the dependency is visible and testable.

## Data Flow

```
route_operation("hover", params)
       │
       ▼
LspProxyService::route_operation()
       │
       ▼
CompositeProvider::hover(&location)
       │
       ├── LspIntelligenceProvider::hover()
       │         │
       │         ▼
       │    LspProcessManager::request()
       │         │
       │         ▼
       │    rust-analyzer / pyright (real process)
       │
       └── TreesitterFallbackProvider::hover()  [on LSP failure]
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `src/application/services/lsp_proxy_service.rs` | Modify | Add `CompositeProvider`, implement `route_operation()` delegation |
| `tests/integration/lsp_integration_test.rs` | Create | E2E tests with real rust-analyzer, pyright binaries |

## Interfaces / Contracts

```rust
// LspProxyService constructor signature changes to accept workspace_root
pub struct LspProxyService {
    lsp_client: Arc<LspClient>,
    analysis_service: Arc<AnalysisService>,
    servers: Arc<RwLock<HashMap<String, LspServerConfig>>>,
    proxy_enabled: bool,
    composite_provider: Option<Arc<CompositeProvider>>,  // NEW
    workspace_root: Option<PathBuf>,                      // NEW
}

// route_operation maps string → Location → CompositeProvider method
pub fn route_operation(
    &self,
    operation: &str,
    params: &serde_json::Value,
) -> Result<Option<serde_json::Value>, LspProxyError> {
    let provider = match self.composite_provider.as_ref() {
        Some(p) => p,
        None => return Ok(None),
    };

    let location = self.extract_location(params)?;  // helper to parse file/line/col

    match operation {
        "hover" | "textDocument/hover" => {
            let result = provider.hover(&location).await?;
            Ok(result.map(|h| serde_json::to_value(h).ok()).flatten())
        }
        // ... similar for definition, references
        _ => Ok(None),
    }
}
```

**Note**: `extract_location` helper parses `params` (expected format: `{ "textDocument": { "uri": "file://..." }, "position": { "line": N, "character": M } }`) into `Location`.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | `route_operation` dispatches to correct provider method | Mock `CompositeProvider` with `Arc<dyn CodeIntelligenceProvider>` |
| Unit | `extract_location` parses LSP params correctly | Test various JSON shapes |
| Integration | rust-analyzer hover returns real type info | `#[ignore]` if binary missing, temp Rust project |
| Integration | pyright goto-definition works | `#[ignore]` if binary missing, temp Python project |

### Integration Test Structure

```rust
// tests/integration/lsp_integration_test.rs

fn rust_analyzer_available() -> bool {
    std::process::Command::new("rust-analyzer").arg("--version").output().is_ok()
}

fn pyright_available() -> bool {
    std::process::Command::new("pyright").arg("--version").output().is_ok()
}

#[tokio::test]
#[ignore = "requires rust-analyzer binary"]
async fn test_rust_analyzer_hover() {
    let temp_dir = tempfile::tempdir().unwrap();
    // create temp Rust file with a function
    let service = LspProxyService::new(
        Arc::new(AnalysisService::new()),
        temp_dir.path().to_path_buf(),
    );
    service.enable_proxy_mode();
    // send hover request, verify response contains function signature
}

#[tokio::test]
#[ignore = "requires pyright binary"]
async fn test_pyright_goto_definition() {
    // similar structure for pyright
}
```

## Migration / Rollout

No migration required:
- `composite_provider: Option<Arc<CompositeProvider>>` defaults to `None` (existing behavior unchanged until `enable_proxy_mode_with_provider()` is called)
- Existing unit tests pass unchanged (proxy disabled → returns `None`)

## Open Questions

- [ ] Should `LspProxyService` require `workspace_root` at construction, or accept it later? → **Accept at construction** (simpler, no partial states)
- [ ] Do we need a builder pattern for optional `workspace_root`? → **No** — existing `new()` + setter pattern is sufficient
