# ADR-044: MCP ViewSpec Tools — Quality Warnings Follow-up

## Status

PROPOSED

## Date

2026-06-22

## Context

During verification of `sddk/MCP-view-spec-tools` (ADR-008 implementation), the verify agent identified 4 non-blocking quality warnings that do not prevent v1 release but should be addressed in a follow-up iteration.

These warnings are the result of an architectural adaptation: Option A (postgres_repo direct) was implemented instead of the proposed Option B (trait in HandlerContext) due to a circular dependency between `cognicode-core` and `cognicode-explorer`.

## Decisions (Deferred)

Each warning is tracked as a potential follow-up task. No immediate decision is required — these are maintenance improvements with bounded scope.

### Warning 1: OCP Violation — Hard-coded Built-in IDs

**Description**: The handler hard-codes 8 built-in ViewSpec IDs (e.g., `OVERVIEW`, `CALL_GRAPH`, `SOURCE`, `QUALITY`, `SEAM_MAP`, `C4_CONTEXT`, `C4_CONTAINER`, `C4_COMPONENT`) which duplicate the actual descriptors in `REAL_EXECUTOR_DESCRIPTORS`.

**Impact**: Adding a new built-in view requires modifying the handler. OCP violation.

**Recommended Follow-up**: Extract a shared constant or function that derives the built-in ID set from `REAL_EXECUTOR_DESCRIPTORS` at runtime or compile time.

### Warning 2: DIP Violation — Concrete PostgresRepository Coupling

**Description**: The MCP handlers depend on the concrete `PostgresRepository` type rather than a trait boundary (`dyn ViewSpecRepository` or similar).

**Impact**: Handlers are tightly coupled to the persistence implementation. Swapping to `InMemoryViewSpecStore` for tests required a different path.

**Recommended Follow-up**: Consider introducing a `ViewSpecRepository` port trait that `PostgresRepository` implements. This would restore DIP and enable the `InMemoryViewSpecStore` test double that Option B originally envisioned.

### Warning 3: Data Loss — data_source and props Discarded

**Description**: When reading ViewSpecs from the database in the runtime path, the `data_source` and `props` columns are parsed but not stored anywhere — they are discarded after validation.

```rust
let _data_source: Option<serde_json::Value> =
    row.try_get("data_source").ok();
let _props: Option<serde_json::Value> =
    row.try_get("props").ok();
```

**Impact**: Runtime ViewSpecs cannot specify custom data sources or renderer props. Custom ViewSpecs created via the Explorer UI would not function correctly if stored in the database.

**Recommended Follow-up**: Store `data_source` and `props` in the `ViewSpecJson` struct returned by the handler, or add a separate query path that reconstructs the full `ViewSpec` from database rows.

### Warning 4: No Mock Integration Test for Runtime Path

**Description**: The `postgres_repo` is not mocked in the integration tests. All `read_view_spec` tests exercise the built-in hard-coded path, not the database runtime path.

**Impact**: If the database query logic has a bug, it would not be caught by the current test suite.

**Recommended Follow-up**: Add a mock `PostgresRepository` or use a test database to exercise the full runtime read path with both built-in and custom ViewSpecs.

## Consequences

- **Positive**: v1 ships with functional MCP tools. Users can list and read built-in ViewSpecs.
- **Negative**: The 4 quality warnings represent technical debt that will accumulate if not addressed.
- **Bounded**: All 4 warnings are well-scoped refactorings. None require architectural rethink.

## See Also

- [ADR-008](./ADR-008-moldable-view-runtime.md) — Original MCP ViewSpec tools decision
- `sddk/MCP-view-spec-tools` — Implementation branch (merged to main)
