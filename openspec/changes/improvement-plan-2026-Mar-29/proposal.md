# Proposal: CogniCode Improvement Plan

> **Change Name**: `cognicode-improvement-plan`
> **Created**: 2026-03-29
> **Completed**: 2026-03-30
> **Status**: COMPLETED
> **Estimated Duration**: 28 days (5 phases)
> **Actual Duration**: 2 days (accelerated implementation)

## Intent

Transform CogniCode from an empty scaffolding codebase to a functional MVP that delivers real value to AI agents through the MCP protocol. The current state has well-conceived architecture (DDD + Clean Architecture in 4 layers) but the code is mostly empty stubs returning `Vec::new()`. This change implements the **Walking Skeleton** pattern: build one complete end-to-end flow before expanding horizontally.

**Problem Statement**:
- 9 MCP handlers return empty data (`Vec::new()`)
- 3 services have no real implementation
- Tests don't compile due to `RiskThreshold` constant issues
- Duplicate types and traits cause maintenance burden
- tree-sitter 0.20 is obsolete

## Scope

### In Scope

**Phase 0: Stabilization (Days 1-2)**
- [ ] Fix `RiskThreshold` test compilation errors in `safety/mod.rs`
- [ ] Remove duplicate `DependencyRepository` trait (`dependency_graph.rs`)
- [ ] Unify MCP types (`McpError`, `McpRequest`, `McpResponse`) in `schemas.rs`
- [ ] Clean compiler warnings (dead code, async traits)

**Phase 1: Walking Skeleton (Days 3-7)**
- [ ] Refine `TreeSitterParser` with correct `file_path` in symbols
- [ ] Implement `AnalysisService.get_file_symbols()` with real parsing
- [ ] Connect `handle_get_file_symbols` MCP handler end-to-end
- [ ] Implement MCP stdin/stdout JSON-RPC loop

**Phase 2: Real Graph (Days 8-14)**
- [ ] Resolve `CallGraph` vs `PetGraphStore` duplication
- [ ] Implement symbol relationship scanner (`find_call_relationships`)
- [ ] Implement `AnalysisService.build_project_graph()`
- [ ] Connect `get_call_hierarchy`, `analyze_impact`, `check_architecture` handlers

**Phase 3: Real Refactoring (Days 15-21)**
- [ ] Implement `RenameStrategy` (Strategy Pattern)
- [ ] Implement `RefactorService.rename_symbol()` with real logic
- [ ] Connect `handle_safe_refactor` for rename operations
- [ ] Integrate `SafetyGate` validation

**Phase 4: Maturity (Days 22-28)**
- [ ] Upgrade tree-sitter to 0.24+
- [ ] Integrate `VirtualFileSystem` for syntax validation
- [ ] Add end-to-end integration tests
- [ ] Implement `find_usages` and `get_complexity` handlers

### Out of Scope

- **Phase 5: Differentiation** (Context Compression, Incremental Graph Updates, LSP Proxy Mode) — deferred to future change
- New language support beyond Python, Rust, JavaScript, TypeScript
- Performance optimization beyond basic functionality
- Production deployment infrastructure

## Approach

### Walking Skeleton Pattern

Implement one complete vertical slice before expanding:

```
Phase 1 Flow:
Agente IA → MCP JSON-RPC → Handler → AnalysisService → TreeSitterParser → Symbol[] → JSON Response
```

Each phase builds on the previous, creating a dependency chain that ensures foundational work is complete before advanced features.

### Phase Dependencies

```
Fase 0: Estabilizacion
    |
    v
Fase 1: Walking Skeleton (get_file_symbols)
    |
    +--> Fase 2: Grafo Real (call hierarchy, impact)
    |       |
    |       +--> Fase 3: Refactorizacion Real (rename)
    |               |
    |               +--> Fase 4: Madurez (tests e2e, VFS, tree-sitter upgrade)
```

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/domain/aggregates/` | Modified | `CallGraph` methods, `Symbol` enhancements |
| `src/domain/traits/` | Modified | Remove duplicate `dependency_graph.rs`, refine parser trait |
| `src/domain/services/` | Modified | Connect `ComplexityCalculator` |
| `src/application/services/` | Modified | Implement `AnalysisService`, `RefactorService`, `GraphService` |
| `src/application/dto/` | Modified | Add/complete DTOs for responses |
| `src/infrastructure/parser/` | Modified | `TreeSitterParser` improvements, relationship scanning |
| `src/infrastructure/graph/` | Modified | `PetGraphStore.to_call_graph()` implementation |
| `src/infrastructure/safety/` | Modified | Fix `RiskThreshold` tests |
| `src/infrastructure/vfs/` | Modified | Integrate VFS into refactor flow |
| `src/infrastructure/refactor/` | New | `RenameStrategy` implementation |
| `src/interface/mcp/` | Modified | All 9 handlers, stdin/stdout loop, type unification |
| `src/interface/lsp/` | Modified | LSP handlers (secondary priority) |
| `tests/e2e/` | New | End-to-end integration tests |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| tree-sitter 0.24 API changes break existing parser | High | Phase 4 dedicated to upgrade; test thoroughly |
| PetGraphStore/CallGraph unification introduces bugs | Medium | Comprehensive unit tests before integration |
| MCP protocol implementation incomplete | Medium | Follow MCP specification strictly; test with real clients |
| Scope creep delays delivery | Medium | Strict phase boundaries; Phase 5 explicitly deferred |
| Async trait Send bounds issues | Low | Use `#[async_trait]` consistently |

## Rollback Plan

Each phase is independent and can be rolled back by reverting git commits:

```bash
# If Phase N fails, rollback to end of Phase N-1
git revert HEAD~<commits-in-phase>
# or
git reset --hard <phase-N-1-tag>
```

**Tagging Strategy**: Create git tags at phase completion:
- `v0.1-stabilization` (end of Phase 0)
- `v0.2-walking-skeleton` (end of Phase 1)
- `v0.3-real-graph` (end of Phase 2)
- `v0.4-real-refactor` (end of Phase 3)
- `v0.5-mvp` (end of Phase 4)

## Dependencies

- **External**: tree-sitter 0.24+, tree-sitter-python, tree-sitter-rust, tree-sitter-javascript, tree-sitter-typescript
- **Internal**: All phases depend on Phase 0 completion (tests must compile)
- **Runtime**: Tokio async runtime, petgraph for graph storage

## Metrics Tracking

| Metric | Start | Phase 1 | Phase 2 | Phase 3 | Phase 4 (Target) |
|--------|-------|---------|---------|---------|------------------|
| MCP Handlers Functional | 0/9 | 1/9 | 4/9 | 5/9 | **8/9** |
| Services with Real Logic | 0/4 | 1/4 | 2/4 | 3/4 | **4/4** |
| Tests Passing | ~30 | ~45 | ~65 | ~80 | **120+** |
| Lines of Real Code | ~500 | ~1500 | ~3000 | ~4500 | **~6000** |

## Success Criteria

- [ ] `cargo test` passes with 120+ tests
- [ ] `cargo check` has 0 warnings (or justified dead_code only)
- [ ] `get_file_symbols` returns real parsed symbols from Python/Rust/JS files
- [ ] `get_call_hierarchy` traverses real call graph
- [ ] `analyze_impact` calculates real dependency impact
- [ ] `safe_refactor` with `action: rename` generates real TextEdits
- [ ] MCP server responds to `tools/list` and `tools/call` via stdin/stdout
- [ ] tree-sitter upgraded to 0.24+
- [ ] VFS validates syntax before applying refactors

## Detailed Phase Breakdown

### Phase 0: Stabilization (Days 1-2)

| Task | File(s) | Acceptance |
|------|---------|------------|
| 0.1 Fix RiskThreshold tests | `infrastructure/safety/mod.rs` | `cargo test` compiles |
| 0.2 Remove duplicate trait | `domain/traits/dependency_graph.rs`, `domain/traits/mod.rs` | `cargo check` clean |
| 0.3 Unify MCP types | `interface/mcp/server.rs`, `interface/mcp/schemas.rs` | Single type definition |
| 0.4 Clean warnings | Multiple | `cargo check` 0 warnings |

### Phase 1: Walking Skeleton (Days 3-7)

| Task | File(s) | Acceptance |
|------|---------|------------|
| 1.1 TreeSitterParser refinement | `infrastructure/parser/tree_sitter_parser.rs` | Correct file_path in symbols |
| 1.2 AnalysisService.get_file_symbols | `application/services/analysis_service.rs` | Real parsing, language detection |
| 1.3 MCP handler connection | `interface/mcp/handlers/` | End-to-end symbol extraction |
| 1.4 stdin/stdout MCP loop | `interface/mcp/server.rs` | JSON-RPC protocol works |

### Phase 2: Real Graph (Days 8-14)

| Task | File(s) | Acceptance |
|------|---------|------------|
| 2.1 Resolve CallGraph/PetGraphStore | `infrastructure/graph/petgraph_store.rs`, `domain/aggregates/call_graph.rs` | Bidirectional conversion |
| 2.2 Relationship scanner | `infrastructure/parser/tree_sitter_parser.rs` | Detect a→b, a→c, b→c calls |
| 2.3 build_project_graph | `application/services/analysis_service.rs` | Index full directory |
| 2.4 Connect graph handlers | `interface/mcp/handlers/` | Real graph data in responses |

### Phase 3: Real Refactoring (Days 15-21)

| Task | File(s) | Acceptance |
|------|---------|------------|
| 3.1 RenameStrategy | `infrastructure/refactor/rename_strategy.rs` | Generate TextEdits for all refs |
| 3.2 RefactorService | `application/services/refactor_service.rs` | Validate, preview, execute |
| 3.3 safe_refactor handler | `interface/mcp/handlers/` | Real diffs returned |

### Phase 4: Maturity (Days 22-28)

| Task | File(s) | Acceptance |
|------|---------|------------|
| 4.1 tree-sitter upgrade | `Cargo.toml`, parser files | All tests pass on 0.24+ |
| 4.2 VFS integration | `infrastructure/vfs/`, refactor flow | Syntax validation |
| 4.3 E2E tests | `tests/e2e/` | Multi-language coverage |
| 4.4 find_usages | Parser, handler | All references found |
| 4.5 get_complexity | `domain/services/`, handler | Real metrics |

## Source of Truth

> **Reference Document**: `docs/IMPROVEMENT-PLAN.md`
> 
> This proposal is derived from and should remain consistent with the improvement plan document. In case of discrepancy, `docs/IMPROVEMENT-PLAN.md` takes precedence.
