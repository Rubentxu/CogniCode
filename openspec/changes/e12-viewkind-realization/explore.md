# Explore: e12 — ViewKind Realization (Phase 1 Wave 1)

## Context Quality: C2

The scope is well-defined in ADR-002 Phase 1 Wave 1. Existing architecture is understood
(9 wired executors, registry.rs pattern, ViewExecutor trait). Codebase exploration needed
to confirm data sources for Wave 1 ViewKinds and confirm renderer availability.

## Taxonomy

- Dominant axes: `view-coverage` × `data-source-availability`
- Knowledge: ADR-002, existing executor patterns, GraphQueryPort call graph API
- Unknown: ownership metadata availability for `OwnershipMap`; test connectivity for `TestSlice`

## What ADR-002 Says

Phase 1 Wave 1 priority order:
1. `ApiSurface`
2. `UsageExamples`
3. `DocCodeAlignment`
4. `OwnershipMap`
5. `TestSlice`
6. `DebugSlice`

## ViewExecutor Pattern (from existing code)

From `views.rs`:
- `struct FooExecutor;`
- `impl ViewDescriptor for FooExecutor` — `id()`, `title()`, `applies_to()`, `view_kind()`, `renderer_kind()`
- `impl ViewExecutor for FooExecutor` — `async fn build(&self, ctx: &ViewContext<'_>) -> ExplorerResult<ContextualView>`
- Register in `registry.rs` `REAL_EXECUTORS` map

`ContextualView` = `{ object_id, view_id, title, view_kind, blocks: Vec<ViewBlock>, relations, evidence, findings, renderer_kind }`

`ViewBlock` = `{ id: String, title: String, body: serde_json::Value }` (generic JSON block)

## Data Sources Available

- `SymbolRepository::all_symbols()` — all symbols
- `GraphQueryPort::callees()` / `callers()` — call graph relations
- `GraphQueryPort::fan_in()` / `fan_out()` — call metrics
- `SourceReader` — file content
- `QualityRepository` — quality issues

## Analysis of Wave 1 ViewKinds

### UsageExamples (HIGHEST PRIORITY — recommended for e12)

**What**: Shows real usages of a symbol (where it's called from or what it calls).

**Data source**: `GraphQueryPort::callees()` for a symbol. Already fully wired in existing executors.

**Implementation approach**:
- `UsageExamplesExecutor` applies to `Symbol`
- `build(ctx)` → `ctx.target` is `InspectionTarget::Symbol(symbol)`
- Call `graph_query.callees(&symbol.id)` to get callee symbols
- Build `ViewBlock::Table` with columns: `symbol.name`, `symbol.file`, `symbol.kind`
- `renderer_kind: RendererKind::Table`

**Effort**: ~100 LOC Rust + registry entry + frontend renderer already exists (`Table`)

**Verification**: Add to `registry.rs` REAL_EXECUTORS, ensure renderer routes to table.

### ApiSurface

**What**: Shows public API surface of a module/crate.

**Data source**: `SymbolRepository::all_symbols()` filtered by `visibility = public` and `defined_in = target_module`.

**Complication**: `ResolvedSymbol` may not have a `visibility` field. Need to check.

### OwnershipMap

**What**: Shows who owns each module/ADR/issue.

**Data source**: No ownership metadata in current schema. Would need new data source or annotation system.

**Complication**: Not implemented. Deferred.

### TestSlice

**What**: Connects entry point to tests that cover it.

**Data source**: Need to know which tests call which symbols — requires test symbol extraction in ingest pipeline.

**Complication**: Test symbols may not be in the graph yet.

### DebugSlice

**What**: Connects error/crash to probable execution paths.

**Data source**: Would need error log analysis integration.

**Complication**: Not implemented. Deferred.

### DocCodeAlignment

**What**: Compares docs/ADRs with the code that implements them.

**Data source**: Would need ADR-to-code linkage (references edges in graph).

**Complication**: ADR graph edges may not be extracted yet.

## Recommendation

**Start with `UsageExamples` for e12 MINOR**. Rationale:
1. Fully uses existing wired data sources (`GraphQueryPort`)
2. Pattern is proven (`build_callgraph` already does caller/callee queries)
3. Frontend renderer (`Table`) already wired
4. ~100 LOC — fits in a single PR
5. High user value: "where is this function used?" is a top exploration question

**ApiSurface as e12b**: Can follow immediately since it uses the same `SymbolRepository` pattern
but needs visibility filtering check first.

## Unresolved

- Does `ResolvedSymbol` have a visibility field for `ApiSurface`?
- Are test symbols in the graph for `TestSlice`?
- Are ADR-code edges extracted for `DocCodeAlignment`?

These should be answered in the propose/spec phase.
