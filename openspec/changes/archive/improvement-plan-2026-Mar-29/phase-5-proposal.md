# Proposal: CogniCode Phase 5 - Differentiation

> **Change Name**: `cognicode-phase-5-differentiation`
> **Created**: 2026-03-29
> **Status**: Draft
> **Depends On**: Phases 0-4 (Complete)
> **Estimated Duration**: 14 days (3 features, independently deployable)

## Intent

Add competitive differentiation features that IntelliJ and other IDEs don't provide for AI agents. Phases 0-4 built a functional MVP with standard LSP capabilities. Phase 5 introduces premium intelligence specifically designed for AI agent consumption:

1. **Context Compression** - AI agents waste tokens on verbose JSON. Compress to natural language summaries.
2. **Incremental Graph Updates** - Rebuild-every-time is slow. Update graph incrementally on file changes.
3. **LSP Proxy Mode** - Don't reinvent the wheel. Delegate basic operations to rust-analyzer/pyright, add CogniCode premium analysis on top.

**Problem Statement**:
- MCP responses are JSON-heavy, consuming AI agent token budgets
- Full graph rebuilds are expensive for large codebases
- Basic LSP features (hover, completion) already exist in mature LSPs - no need to reimplement

## Scope

### In Scope

**Feature 5.1: Context Compression (Days 1-4)**
- [ ] Add `compressed: bool` flag to all MCP tool inputs
- [ ] Create `ContextCompressor` service in application layer
- [ ] Implement compression for `get_file_symbols`, `get_call_hierarchy`, `analyze_impact`
- [ ] Summary format: file stats, function signatures, call relationships, dependencies
- [ ] Natural language output instead of raw JSON when `compressed=true`

**Feature 5.2: Incremental Graph Updates (Days 5-9)**
- [ ] Define `GraphEvent` enum in domain: `SymbolAdded`, `SymbolRemoved`, `SymbolModified`
- [ ] Create `GraphDiffCalculator` to compute symbol changes between file versions
- [ ] Add `apply_events()` method to `CallGraph` aggregate
- [ ] Implement file watcher integration in infrastructure layer
- [ ] Add in-memory graph cache that persists between agent sessions
- [ ] MCP handler for `subscribe_to_changes` (optional streaming)

**Feature 5.3: LSP Proxy Mode (Days 10-14)**
- [ ] Extend `LspClient` for subprocess management (launch external LSP)
- [ ] Implement LSP protocol adapter for rust-analyzer, pyright
- [ ] Create `LspProxyService` to route operations:
  - Delegate: hover, completion, definition, references
  - CogniCode: impact analysis, cycle detection, complexity, safe refactoring
- [ ] Add `lsp_proxy` configuration in MCP server startup
- [ ] Merge responses from external LSP + CogniCode premium analysis

### Out of Scope

- New language support beyond existing (Python, Rust, JS, TS)
- IDE client implementations (VSCode, IntelliJ plugins)
- Cloud deployment or distributed caching
- Real-time collaborative features

## Approach

### Feature Independence

Each feature is independently deployable:

```
Feature 5.1 (Compression) ──► Deploy independently
Feature 5.2 (Incremental)  ──► Deploy independently  
Feature 5.3 (LSP Proxy)    ──► Deploy independently
```

### Feature 5.1: Context Compression Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│ MCP Handler                                                       │
│ ┌─────────────────────┐    ┌─────────────────────────────────┐   │
│ │ Input: compressed=true│──►│ ContextCompressor.compress()   │   │
│ └─────────────────────┘    └─────────────────────────────────┘   │
│                                      │                            │
│                                      ▼                            │
│                            ┌─────────────────────┐               │
│                            │ Natural Language    │               │
│                            │ Summary Output      │               │
│                            └─────────────────────┘               │
└──────────────────────────────────────────────────────────────────┘
```

**Compression Example**:
```
Before (JSON):
  {"symbols": [{"name": "process_order", "kind": "Function", "line": 42},
               {"name": "validate", "kind": "Function", "line": 58},
               {"name": "compute_total", "kind": "Function", "line": 72}]}

After (Compressed):
  "order_service.rs: 3 functions (process_order@42, validate@58, compute_total@72).
   process_order calls validate + compute_total. No external deps."
```

### Feature 5.2: Incremental Graph Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    File Watcher                                  │
│                         │                                        │
│                         ▼                                        │
│ ┌───────────────────────────────────────────────────────────┐   │
│ │ GraphDiffCalculator                                        │   │
│ │   old_symbols: [A, B, C]                                   │   │
│ │   new_symbols: [A, B', D]                                  │   │
│ │   diff: [Modified(B), Removed(C), Added(D)]               │   │
│ └───────────────────────────────────────────────────────────┘   │
│                         │                                        │
│                         ▼                                        │
│ ┌───────────────────────────────────────────────────────────┐   │
│ │ GraphEvent Emission                                        │   │
│ │   SymbolModified(B), SymbolRemoved(C), SymbolAdded(D)     │   │
│ └───────────────────────────────────────────────────────────┘   │
│                         │                                        │
│                         ▼                                        │
│ ┌───────────────────────────────────────────────────────────┐   │
│ │ CallGraph.apply_events(events)                             │   │
│ │   - Incremental update, no full rebuild                    │   │
│ └───────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Feature 5.3: LSP Proxy Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        AI Agent                                   │
│                           │                                       │
│                           ▼                                       │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │                    CogniCode MCP Server                      │  │
│ │                          │                                   │  │
│ │            ┌─────────────┴─────────────┐                    │  │
│ │            ▼                           ▼                    │  │
│ │ ┌──────────────────┐        ┌──────────────────────────┐   │  │
│ │ │ LspProxyService  │        │ CogniCode Analysis       │   │  │
│ │ │ (Delegate)       │        │ (Premium)                │   │  │
│ │ └────────┬─────────┘        └──────────────────────────┘   │  │
│ │          │                                                   │  │
│ │          ▼                                                   │  │
│ │ ┌──────────────────┐                                        │  │
│ │ │ External LSP     │                                        │  │
│ │ │ (rust-analyzer)  │                                        │  │
│ │ └──────────────────┘                                        │  │
│ └─────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘

Operation Routing:
┌─────────────────────────────────────────────────────────────────┐
│ Operation              │ Route To          │ Reason             │
├─────────────────────────────────────────────────────────────────┤
│ hover                  │ External LSP      │ Standard feature   │
│ completion             │ External LSP      │ Standard feature   │
│ go_to_definition       │ External LSP      │ Standard feature   │
│ find_references        │ External LSP      │ Standard feature   │
│ analyze_impact         │ CogniCode         │ Premium analysis   │
│ detect_cycles          │ CogniCode         │ Premium analysis   │
│ get_complexity         │ CogniCode         │ Premium analysis   │
│ safe_refactor          │ CogniCode         │ Premium analysis   │
└─────────────────────────────────────────────────────────────────┘
```

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/application/services/` | New | `ContextCompressorService`, `LspProxyService` |
| `src/domain/aggregates/call_graph.rs` | Modified | Add `apply_events()` method |
| `src/domain/events/` | New | `GraphEvent` enum, event emission |
| `src/domain/services/` | New | `GraphDiffCalculator` |
| `src/infrastructure/watcher/` | New | File watcher integration |
| `src/infrastructure/cache/` | New | In-memory graph cache |
| `src/infrastructure/lsp/client.rs` | Modified | Subprocess management, protocol adapter |
| `src/interface/mcp/schemas.rs` | Modified | Add `compressed: bool` to inputs |
| `src/interface/mcp/handlers.rs` | Modified | Compression logic, proxy routing |
| `src/interface/mcp/server.rs` | Modified | LSP proxy initialization |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Natural language compression loses critical info | Medium | Include structured fallback; AI agents can request `compressed=false` |
| Incremental updates miss edge cases | Medium | Comprehensive diff tests; fallback to full rebuild on error |
| External LSP compatibility issues | High | Test with rust-analyzer 1.0+, pyright 1.1+; graceful degradation |
| File watcher performance on large repos | Low | Debounce events; batch updates; configurable watch depth |
| LSP subprocess lifecycle management | Medium | Health checks; auto-restart; timeout handling |

## Rollback Plan

Each feature can be reverted independently via git:

```bash
# Feature 5.1 (Compression) rollback
git revert HEAD~<commits-for-5.1>

# Feature 5.2 (Incremental) rollback
git revert HEAD~<commits-for-5.2>

# Feature 5.3 (LSP Proxy) rollback
git revert HEAD~<commits-for-5.3>
```

**Tagging Strategy**:
- `v0.6-compression` (after Feature 5.1)
- `v0.7-incremental` (after Feature 5.2)
- `v0.8-lsp-proxy` (after Feature 5.3)
- `v1.0-differentiated` (all features complete)

**Feature Flags**: All features behind config flags:
```yaml
# cognicode.yaml
features:
  compression: true    # Feature 5.1
  incremental: true    # Feature 5.2
  lsp_proxy: true      # Feature 5.3
```

## Dependencies

### External
- **notify 6.0+** - File system watcher for incremental updates
- **lsp-types 0.95+** - LSP protocol types (already in use)
- **tokio 1.0+** - Async subprocess management (already in use)

### Internal
- All Phases 0-4 complete and tested
- `CallGraph` aggregate fully functional
- MCP handlers return real data

### Runtime
- External LSP servers (rust-analyzer, pyright) must be installed for Feature 5.3
- Optional: Redis for distributed caching (future enhancement)

## Success Criteria

### Feature 5.1: Context Compression
- [ ] `get_file_symbols` with `compressed=true` returns natural language summary
- [ ] Compression reduces token count by 50%+ compared to JSON
- [ ] All critical information preserved in compressed format
- [ ] `compressed=false` returns original JSON format

### Feature 5.2: Incremental Graph Updates
- [ ] File change triggers incremental update, not full rebuild
- [ ] `GraphEvent` emission covers added/removed/modified symbols
- [ ] Graph cache persists between MCP session invocations
- [ ] Incremental update is 5x+ faster than full rebuild for single file change

### Feature 5.3: LSP Proxy Mode
- [ ] rust-analyzer subprocess launched and managed correctly
- [ ] Hover/completion delegated to external LSP
- [ ] Impact analysis, cycles, complexity handled by CogniCode
- [ ] Graceful degradation when external LSP unavailable

### Overall
- [ ] `cargo test` passes with 150+ tests
- [ ] Each feature independently toggleable via config
- [ ] Documentation updated for new features
- [ ] MCP tools documentation includes `compressed` parameter

## Detailed Feature Breakdown

### Feature 5.1: Context Compression

| Task | File(s) | Acceptance |
|------|---------|------------|
| 5.1.1 Add `compressed` field | `interface/mcp/schemas.rs` | All input schemas have `compressed: Option<bool>` |
| 5.1.2 Create ContextCompressor | `application/services/context_compressor.rs` | `compress_symbols()`, `compress_hierarchy()`, `compress_impact()` |
| 5.1.3 Integrate in handlers | `interface/mcp/handlers.rs` | Conditional compression based on flag |
| 5.1.4 Add tests | `tests/compression_tests.rs` | Verify output format and info preservation |

### Feature 5.2: Incremental Graph Updates

| Task | File(s) | Acceptance |
|------|---------|------------|
| 5.2.1 Define GraphEvent | `domain/events/graph_event.rs` | `SymbolAdded`, `SymbolRemoved`, `SymbolModified` variants |
| 5.2.2 GraphDiffCalculator | `domain/services/graph_diff.rs` | Calculate symbol diff between file versions |
| 5.2.3 CallGraph.apply_events | `domain/aggregates/call_graph.rs` | Incremental mutation from events |
| 5.2.4 File watcher | `infrastructure/watcher/file_watcher.rs` | Debounced file change events |
| 5.2.5 Graph cache | `infrastructure/cache/graph_cache.rs` | In-memory persistence between sessions |
| 5.2.6 MCP integration | `interface/mcp/handlers.rs` | Use cached graph, update on changes |

### Feature 5.3: LSP Proxy Mode

| Task | File(s) | Acceptance |
|------|---------|------------|
| 5.3.1 Subprocess management | `infrastructure/lsp/subprocess.rs` | Launch, monitor, restart external LSP |
| 5.3.2 Protocol adapter | `infrastructure/lsp/protocol_adapter.rs` | rust-analyzer, pyright specific adapters |
| 5.3.3 LspProxyService | `application/services/lsp_proxy_service.rs` | Route operations to correct backend |
| 5.3.4 Response merger | `application/services/response_merger.rs` | Combine external LSP + CogniCode responses |
| 5.3.5 Configuration | `interface/mcp/server.rs` | `lsp_proxy` config section |
| 5.3.6 Graceful degradation | Multiple | Handle external LSP unavailable |

## Source of Truth

> **Parent Document**: `openspec/changes/improvement-plan-2026-Mar-29/proposal.md`
> 
> This proposal extends the parent improvement plan with Phase 5 features. Phase 5 was marked as "Out of Scope" in the parent but is now ready for implementation.
