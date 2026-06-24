# ADR-045: Known Exploration-Persistence Debts

## Status

ACCEPTED

## Resolution Summary

**Debt 1** (`get_exploration` Mis-wire) and **Debt 2** (Dual Model) were resolved
in **Phase 1** (sddk/ADR-045-phase1-debt1-debt2, commit `2f65940` base).

- **Debt 1**: Orphaned `GET /api/explorations/:id` route + three handlers unregistered.
  No consumers exist — route was confirmed dead code.
- **Debt 2**: `ExplorationPath` model removed entirely; all production and test code
  unified onto `ExplorationSession`. `ExplorationColumn`, `SaveExplorationRequest` also
  removed. Frontend `useExplorations` no longer exposes `saveExploration`.

**Debt 3** (In-Memory Store Lifetime) is **RESOLVED v0.12.6** — Postgres
`exploration_sessions` table added (ADR-045 Phase 2, commit `e6ef208`).

## Resolution Evidence

- Branch: `sddk/ADR-045-phase1-debt1-debt2`
- Route removal: `crates/cognicode-explorer/src/api.rs` (T1.4)
- DTO cleanup: `crates/cognicode-explorer/src/dto.rs` (T1.1–T1.3)
- Frontend cleanup: `apps/explorer-ui/src/` (T2.1–T2.7, T3.1–T3.2, T4.1–T4.3)

## Date

2026-06-23

## Related

- [ADR-039 — Explorer Navigation Model](./ADR-039-explorer-navigation-model.md)
- [ADR-040 — Graph View Renderer](./ADR-040-graph-view-renderer.md)
- [ADR-044 — MCP ViewSpec Tools Follow-up](./ADR-044-mcp-viewspec-followup.md)

## Context

During the implementation of the E4.5 LIST-endpoint for explorations, three pre-existing
architectural debts in the exploration-persistence layer were identified. These debts span
the `get_exploration` route wiring, the dual model coexistence (`ExplorationPath` vs
`ExplorationSession`), and the in-memory store lifetime. This ADR surfaces and defers all
three items and serves as the canonical cross-reference for the existing KNOWN-DEBT rustdoc
in `persistence.rs:209-224`.

## Debt 1 — `get_exploration` Mis-wire

**Location**: `crates/cognicode-explorer/src/api.rs:776-788`

**Description**: The route `GET /api/explorations/:id` is documented as "return a previously
saved exploration path" but the handler calls `load_exploration_session` and returns an
`ExplorationSession`. This is a latent inconsistency — no consumer of the doc-claimed
behaviour exists (see below).

**Consumers verified absent**: There is no `load_exploration_path` trait method anywhere in
the codebase. The frontend does not call `GET /api/explorations/:id` directly; it uses
`GET /explorations/:id/artifacts` or works from in-hand LIST data.

**Disposition**: **✅ RESOLVED — Phase 1.** Orphaned `GET /api/explorations/:id` route
removed (T1.4). Three handlers unregistered: `save_exploration`, `get_exploration`,
`generate_artifact`. Zero prod consumers confirmed — route was dead code.

**Rationale**: Latent only — no live consumer has been observed. Adding the missing
`load_exploration_path` path would re-entrench the legacy model.

## Debt 2 — Dual Model (`ExplorationPath` vs `ExplorationSession`)

**Location**: `crates/cognicode-explorer/src/dto.rs:362-477` (legacy `ExplorationPath`);
`crates/cognicode-explorer/src/dto.rs:432+` (pane-stack `ExplorationSession` per ADR-040
Wave 3); approximately 8 frontend files in `apps/explorer-ui/` that use `ExplorationPath`.

**Description**: `ExplorationPath` (legacy `columns` model) and `ExplorationSession`
(pane-stack model) coexist with parallel save/list/restore code paths. The LIST endpoint
returns `ExplorationPath` because the frontend schema (`z.array(explorationPathSchema)`)
still expects that shape.

**Disposition**: **✅ RESOLVED — Phase 1.** `ExplorationPath`, `ExplorationColumn`,
`SaveExplorationRequest` removed from `dto.rs` (T1.1). All production and test code
unified onto `ExplorationSession`. Frontend SWR hook no longer exposes `saveExploration`
(T2.3). `generate_artifact` repointed to session store (T1.3).

**Rationale**: Persisting the legacy shape in a new SQL table (Debt 3) would entrench it
irreversibly. Debt 2 must precede Debt 3.

## Debt 3 — In-Memory Store Lifetime

**Location**: `crates/cognicode-explorer/src/facades/persistence.rs:27,30` (two `Mutex<HashMap>`
type aliases: `ExplorationPathStore` and `ExplorationSessionStore`).

**Description**: Both the `paths` and `sessions` HashMaps are process-lifetime only. Server
restart loses all rows. There is no Postgres exploration table today.

**Disposition**: **✅ RESOLVED — Phase 2 (v0.12.6).** `exploration_sessions` Postgres table
added (`schema_postgres.sql`). `PersistenceServiceImpl` now delegates to
`PostgresRepository` when available, with in-memory fallback. Sessions survive server
restarts. In-memory store retained as fallback for dev/test without PG.

**Resolution Evidence**:
- Schema: `crates/cognicode-core/src/infrastructure/persistence/schema_postgres.sql` (T1.1)
- Repository methods: `crates/cognicode-core/src/infrastructure/persistence/postgres_repository.rs` (T1.2–T1.5)
- Facade delegation: `crates/cognicode-explorer/src/facades/persistence.rs` (T1.7–T1.8)
- Runtime wiring: `crates/cognicode-runtime/src/lib.rs:153` (T1.9)
- Contract tests: `crates/cognicode-explorer/tests/pg_exploration_session_contract.rs` (T1.10)

## ADR-039 Contradiction

> **This section is retained for historical record — the contradiction is now resolved.**

ADR-039 Decision 1 mandates a hard cut from column-based navigation to pane-stack
navigation. Previously, `default_navigation_mode()` returned `"column"` (`dto.rs:447`),
contradicting the pane-stack mandate.

**Resolution**: `default_navigation_mode()` was removed entirely in Phase 1 (T1.1).
`ExplorationPath` (the only type that used `"column"`) has been removed. The ADR-039
contradiction is resolved.

## Ordering Constraint: Debt 2 → Debt 3

**Debt 3 is blocked on Debt 2.** Persisting the legacy `ExplorationPath` shape in a new
Postgres table would entrench the `"column"` navigation mode permanently, making Debt 2
significantly harder. Debt 2 (model unification) must be resolved before any Postgres
persistence work begins for exploration data.

## Open Question — Debt 1 Final Fix Shape

**Resolved (Phase 1)**: Path 1 chosen — orphaned `GET /api/explorations/:id` route
removed entirely. Zero consumers verified; no future use anticipated.
