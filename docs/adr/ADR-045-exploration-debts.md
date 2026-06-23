# ADR-045: Known Exploration-Persistence Debts

## Status

ACCEPTED

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

**Disposition**: **Defer + plan.** Recommend **remove the orphaned route** entirely rather
than implementing `load_exploration_path`, which would entrench the legacy `ExplorationPath`
shape. The implementation ADR for this debt must confirm the removal before any code lands.

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

**Disposition**: **Defer + timeline.** Unify onto `ExplorationSession` (ADR-039-aligned).
The unified model must be established before Debt 3 is addressed.

**Rationale**: Persisting the legacy shape in a new SQL table (Debt 3) would entrench it
irreversibly. Debt 2 must precede Debt 3.

## Debt 3 — In-Memory Store Lifetime

**Location**: `crates/cognicode-explorer/src/facades/persistence.rs:27,30` (two `Mutex<HashMap>`
type aliases: `ExplorationPathStore` and `ExplorationSessionStore`).

**Description**: Both the `paths` and `sessions` HashMaps are process-lifetime only. Server
restart loses all rows. There is no Postgres exploration table today.

**Disposition**: **Defer + timeline.** Postgres persistence for the **unified** model
(blocked on Debt 2). No PG exploration table exists; migration will be a separate ADR.

**Rationale**: Highest user impact (all saved explorations lost on restart). However,
Debt 2 must be resolved first to avoid entrenching the legacy `ExplorationPath` columns
model in a new SQL table.

## ADR-039 Contradiction

> **This section surfaces but does not amend ADR-039.**

ADR-039 Decision 1 mandates a hard cut from column-based navigation to pane-stack
navigation. However, the backend still writes `columns` on `ExplorationPath` (the legacy
shape returned by the LIST endpoint) and `default_navigation_mode()` returns `"column"`
(`dto.rs:447`):

```rust
fn default_navigation_mode() -> String { "column".to_string() }
```

Reconciliation of this contradiction is **out of scope for this ADR**. A separate future ADR
must address the contradiction resolution.

## Ordering Constraint: Debt 2 → Debt 3

**Debt 3 is blocked on Debt 2.** Persisting the legacy `ExplorationPath` shape in a new
Postgres table would entrench the `"column"` navigation mode permanently, making Debt 2
significantly harder. Debt 2 (model unification) must be resolved before any Postgres
persistence work begins for exploration data.

## Open Question — Debt 1 Final Fix Shape

Debt 1 has two possible resolution paths:

1. **Remove orphaned route** (recommended, zero consumers verified): Delete
   `GET /api/explorations/:id` entirely. The route's documented behaviour has no
   corresponding implementation and no consumers.
2. **Add `load_exploration_path` trait method**: Implement the doc-claimed behaviour,
   preserving the route for potential future use.

The implementation ADR for Debt 1 must confirm which path is taken before any code lands.
This ADR does not prejudge the outcome.
