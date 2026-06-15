# ADR-010: Architecture Deepening Roadmap

**Fecha:** 2026-06-13  
**Estado:** ACCEPTED  
**Decisión:** Five-phase deepening roadmap to eliminate shallow modules, consolidate seams, and align the codebase with SOLID + DDD  
**Fuente:** improve-codebase-architecture grilling session  
**Confianza:** alta  

---

## Context

An architecture review identified five areas where the codebase carries unnecessary cognitive load: split-brain view discovery, a persistence seam spread across PostgreSQL direction and SQLite remnants, two god-modules (~11k lines combined), a metadata escape hatch on the symbol repository, and bootstrap duplication across binaries. All five were grilled to a concrete design decision.

## Decisions

### 1. View seam — ISP-segregated traits with supertrait

Replace the current split between `ViewDescriptorProvider` (metadata-only) and `ExplorerService::match view_id` (execution) with two traits:

- **`ViewDescriptor`** — metadata only (`id`, `title`, `applies_to`, `view_kind`, `renderer_kind`).
- **`ViewExecutor: ViewDescriptor`** — adds `async build(&self, ctx: &ViewContext) -> ExplorerResult<ContextualView>`.

The registry stores `dyn ViewExecutor`. No downcast, no `as_executor()`. Listing code calls `ViewDescriptor` methods through `dyn ViewExecutor` (the vtable includes them). Runtime ViewSpecs implement the same `ViewExecutor` trait, executing MoldQL queries inside `build()`.

The capability validates applicability inside `build()` — the service does not check `applies_to`. Migration is all-at-once: replace match arms, delete duplicated descriptor lists, consolidate in a single change.

**Rejected:** single trait (violates ISP — listing consumers would depend on `build()`). Separate traits with downcast (reproduces the `as_metadata_aware` anti-pattern). Capability-per-object-type (proliferates entries for a single semantic concept).

### 2. Persistence — PostgreSQL canonical, kill SQLite and deprecated crates

Remove `cognicode-db` and `cognicode-store-traits` from the workspace entirely. Delete the `sqlite` feature and all `#[cfg(feature = "sqlite")]` paths. PostgreSQL is the only backend.

Integration tests that used `SqliteGraphStore` migrate to `TEST_DATABASE_URL` gating (as `pg_bridge_contract.rs` already does). Unit tests use `CallGraph::new()` directly.

Create `cognicode-runtime` crate as the composition root: tracing init, rayon thread pool, PostgreSQL graph loading, service wiring. All binaries depend on it.

**Rejected:** keep SQLite as test harness (perpetuates infrastructure duplication, violates LSP — SQLite has write-path, PG does not yet, they are not substitutable). Keep deprecated crates as re-export wrappers (prolongs cognitive cost of "which import is canonical").

### 3. God-modules — vertical slices, no facade, ToolHandler registry

Eliminate `ExplorerService` entirely. Split into domain-aligned slices: `inspection`, `views`, `graph`, `named_views`, `sessions`, `search`, `moldql`. Each slice is an application service that receives only the ports it needs (ISP).

Split `ExplorerMcpHandler` (7212 lines) into a `ToolHandler` registry — same pattern as `ViewExecutor`. Each tool is a `dyn ToolHandler` registered by name. The MCP handler is a thin dispatcher (~80 lines) that looks up by tool name and delegates.

**Rejected:** thin facade ExplorerService (ISP violation — callers depend on all capabilities). Facades by family with prefix dispatch (reproduces the central match anti-pattern).

### 4. Graph queries — separate SymbolRepository from GraphQueryPort

Split navigation out of `SymbolRepository`:

- **`SymbolRepository`** — identity resolution only: `resolve`, `find_symbols_by_name`, `find_symbols_by_file`, `all_symbols`, `graph_stats`, `module_list`.
- **`GraphQueryPort`** — structural navigation with metadata: `callers`, `callees` (return `RelationEdge` with `provenance` + `confidence`), `traverse(start, direction, max_depth)`, `subgraph(roots, depth)`.

Delete `MetadataAwareRepository` and `as_metadata_aware()`. MoldQL compiles queries to `GraphQueryPort` operations.

**Rejected:** metadata in base SymbolRepository (forces all consumers to carry metadata even when irrelevant). GraphQueryPort with direct-neighbors only (forces BFS duplication in MoldQL executor, contextual graph builder, and impact radius views).

### 5. Bootstrap — absorbed by composition root

Candidate 5 requires no additional decision. Creating `cognicode-runtime` in decision 2 eliminates bootstrap duplication as a consequence: tracing, rayon, backend resolution, and service wiring all live in the composition root. Binaries become ~15-line entry points.

## Migration order

```
Phase 1 (view seam) ──────┐
                          ├──→ Phase 3 (slices + ToolHandler)
Phase 2 (persistence) ────┤
                          ├──→ Phase 4 (GraphQueryPort)
                          └──→ Phase 5 (absorbed)
```

Phases 1 and 2 are independent and can proceed in parallel. Phase 3 depends on both. Phase 4 can start in parallel with 3 but should close after.

## Consequences

- Five crate-level or module-level refactors, each touching multiple files.
- All consumers of `SymbolRepository` navigation methods must migrate to `GraphQueryPort`.
- All `ExplorerService` callers (API routes, MCP handler, tests) must depend on specific slice services.
- MCP tool definitions move from inline match arms to registered `ToolHandler` implementations.
- CONTEXT.md vocabulary updated with: `ViewDescriptor`, `ViewExecutor`, `InspectionTarget`, `ViewContext`, `GraphQueryPort`, `ToolHandler`, composition root.
