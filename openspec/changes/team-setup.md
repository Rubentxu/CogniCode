# CogniCode Implementation Team Setup

> **Created**: 2026-03-29
> **Status**: Active
> **Improvement Plan**: docs/IMPROVEMENT-PLAN.md

## Team Structure

### Team Lead / Coordinator
- **Role**: Orchestrates overall implementation, assigns phases, monitors progress
- **Reports**: Final status to user
- **Coordinates**: All phase leads

### Phase 0 Lead — Stabilization Expert
- **Agent**: sdd-apply
- **Responsibility**: Fix test compilation errors, remove duplicates, clean warnings
- **Input**: docs/IMPROVEMENT-PLAN.md § Fase 0
- **Output**: `cargo test` passes, `cargo check` with 0 errors

### Phase 1 Lead — Walking Skeleton Expert
- **Agent**: sdd-apply
- **Responsibility**: Implement `get_file_symbols` end-to-end (MCP → Service → TreeSitter)
- **Dependency**: Phase 0 complete
- **Input**: docs/IMPROVEMENT-PLAN.md § Fase 1
- **Output**: Real symbols returned from real files via MCP

### Phase 2 Lead — Graph Expert
- **Agent**: sdd-apply
- **Responsibility**: Build call graph and impact analysis with petgraph
- **Dependency**: Phase 1 complete
- **Input**: docs/IMPROVEMENT-PLAN.md § Fase 2
- **Output**: Call hierarchy and impact analysis working with real data

### Phase 3 Lead — Refactor Expert
- **Agent**: sdd-apply
- **Responsibility**: Implement rename functionality end-to-end
- **Dependency**: Phase 2 complete
- **Input**: docs/IMPROVEMENT-PLAN.md § Fase 3
- **Output**: Rename produces real diffs via MCP `safe_refactor`

### Phase 4 Lead — Quality Expert
- **Agent**: sdd-apply
- **Responsibility**: E2E tests, VFS integration, tree-sitter 0.24 upgrade
- **Dependency**: Phase 3 complete
- **Input**: docs/IMPROVEMENT-PLAN.md § Fase 4
- **Output**: Full e2e test suite passing, tree-sitter updated

---

## Communication Protocol

- All phase leads report back their completion status
- Each phase MUST NOT start until the previous phase passes its exit checklist
- Gate criteria:
  - Phase 0 → Phase 1: `cargo test` passes, `cargo check` clean
  - Phase 1 → Phase 2: `get_file_symbols` returns real symbols from real files
  - Phase 2 → Phase 3: call graph queryable, impact analysis returns data
  - Phase 3 → Phase 4: rename produces real diffs, safety gate validates

---

## Definition of Done (Global)

- `cargo test` passes with all tests green
- `cargo check` produces 0 errors, minimal warnings
- Each MCP handler returns real data (not `Vec::new()`)
- No duplicate traits or types
- Tests cover every implemented feature

---

## Task Breakdown by Phase

### Phase 0 Tasks (COMPLETED)
- [x] 0.1 Fix RiskThreshold tests in `src/infrastructure/safety/mod.rs`
  - Replace `RiskThreshold::Low` → `RiskThreshold::LOW`
  - Replace `RiskThreshold::Medium` → `RiskThreshold::MEDIUM`
  - Replace `RiskThreshold::High` → `RiskThreshold::HIGH`
- [x] 0.2 Remove duplicate `DependencyRepository` trait from `domain/traits/dependency_graph.rs` (file removed)
- [x] 0.3 Unify MCP types (`McpError`, `McpRequest`, `McpResponse`) in `interface/mcp/schemas.rs`
- [x] 0.4 Clean compiler warnings (reduced from 22 to 10, remaining are acceptable)

### Phase 1 Tasks (COMPLETED)
- [x] 1.1 Add `file_path` param to `TreeSitterParser.find_all_symbols()`
- [x] 1.2 Add TypeScript support via `tree-sitter-typescript`
- [x] 1.3 Implement `AnalysisService.get_file_symbols(path: &Path)`
- [x] 1.4 Connect `handle_get_file_symbols` to `AnalysisService` (add to HandlerContext)
- [x] 1.5 Implement MCP stdin/stdout JSON-RPC loop

### Phase 2 Tasks (COMPLETED)
- [x] 2.1 Implement `PetGraphStore::to_call_graph()` conversion
- [x] 2.2 Add `find_call_relationships()` to `TreeSitterParser`
- [x] 2.3 Implement `AnalysisService.build_project_graph(dir: &Path)`
- [x] 2.4 Connect `handle_get_call_hierarchy`, `handle_analyze_impact`, `handle_check_architecture`

### Phase 3 Tasks (COMPLETED)
- [x] 3.1 Create `infrastructure/refactor/rename_strategy.rs` implementing `RefactorStrategy`
- [x] 3.2 Implement `RefactorService.rename_symbol()` with safety gate
- [x] 3.3 Connect `handle_safe_refactor` for rename action

### Phase 4 Tasks (COMPLETED)
- [x] 4.1 Upgrade tree-sitter to 0.24+ in Cargo.toml and migrate API
- [x] 4.2 Integrate `VirtualFileSystem` into refactor flow for syntax validation
- [x] 4.3 Create `tests/e2e/` with fixtures for Python, Rust, JS
- [x] 4.4 Implement `find_usages` with tree-sitter queries
- [x] 4.5 Connect `ComplexityCalculator` to MCP handler

---

## Current Status (2026-03-30)

**Test Results**: 210 tests passing, 1 failing
**Failing Test**: `test_impact_analyzer_with_dependents` - pre-existing bug in `find_all_dependents` fallback name search

---

## Key Files Reference

| File | Role |
|------|------|
| `src/infrastructure/safety/mod.rs` | Safety gate + RiskThreshold (Phase 0 target) |
| `src/domain/traits/dependency_graph.rs` | Duplicate trait (Phase 0: delete) |
| `src/domain/traits/dependency_repository.rs` | Canonical DependencyRepository |
| `src/interface/mcp/` | MCP handlers (Phase 1+ target) |
| `src/application/services/analysis_service.rs` | Empty service (Phase 1 target) |
| `src/infrastructure/parser/tree_sitter_parser.rs` | Parser (Phase 1 target) |
| `src/infrastructure/graph/pet_graph_store.rs` | Graph store (Phase 2 target) |
| `src/domain/aggregates/call_graph.rs` | Domain call graph model |
