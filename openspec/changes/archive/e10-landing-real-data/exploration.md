# Exploration: e10-landing-real-data

## Current State

Cycle `e8` (frontend) and `e8b` (backend truncation contract) prepared the
landing page for real data but did not actually wire that data through the
Explorer backend.

Today:

- `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx` already
  renders:
  - a truncation banner (`graph-landing-warning`),
  - an accessible canvas (`role="application"`, `aria-label`, `tabIndex=0`),
  - a fallback node-list of buttons (`graph-node-*`).
- `LandingPayload` now includes `truncated` + `truncated_reason`
  (cycle `e8b`, v0.24.2).
- But `landing_handler` still returns only empty stubs:
  `nodes: []`, `edges: []`, `entry_points: []`, `hot_paths: []`,
  `god_nodes: []`, `suggested_questions: []`.

The result is that the landing page still does **not** act like a moldable
overview of the workspace. The backend contract exists, but there is no real
semantic content behind it.

## Seam Analysis

### What `cognicode-core` already knows

`cognicode-core` already computes the key ingredients we need:

- `WorkspaceSession::get_entry_points()` —
  `crates/cognicode-core/src/application/workspace_session.rs:1198-1203`
- `WorkspaceSession::get_hot_paths()` —
  `crates/cognicode-core/src/application/workspace_session.rs:1443-1474`
- `AnalysisService::get_entry_points()` semantics —
  `analysis_service.rs:959-971` (`graph.roots()`)
- `CallGraphAnalyzer::find_hot_paths()` semantics —
  `call_graph_analyzer.rs:22-49` (sort by `fan_in` desc, drop `fan_in == 0`)
- `graph_analytics::god_nodes()` / `graph_insights` / PageRank services in
  core already exist, though not yet surfaced through the Explorer facade.

### What `cognicode-explorer` already has

The Explorer runtime already injects enough to compute a first real landing,
without reaching into `WorkspaceSession` directly:

- `SymbolRepository::all_symbols()` —
  `crates/cognicode-explorer/src/ports/symbol_repository.rs:93-101`
- `GraphQueryPort::fan_in()` / `fan_out()` / `callees()` —
  `cognicode-core/src/domain/traits/graph_query_port.rs:105-145`
- `SearchService::inspect_object()` to build consistent
  `InspectableObjectSummary` values with `available_views` already populated —
  `facades/search.rs:132-151` + `inspect_symbol_impl()` at `272-299`
- `GraphService` already injected into `ApiState` by runtime —
  `crates/cognicode-runtime/src/lib.rs:174-188`

### The missing seam

`landing_handler` currently only sees `ApiState`:

```rust
pub struct ApiState {
    pub workspace: Arc<dyn WorkspaceService>,
    pub search: Arc<dyn SearchService>,
    pub view: Arc<dyn ViewService>,
    pub persistence: Arc<dyn PersistenceService>,
    pub moldql: Arc<dyn MoldQLService>,
    pub graph: Arc<dyn GraphService>,
    pub ingest: Option<Arc<IngestController>>,
}
```

There is **no** `WorkspaceSession` in `ApiState`, and `WorkspaceServiceImpl`
is intentionally thin (`open_workspace` / `current_workspace` only).

So the correct seam for `e10` is **NOT** to smuggle `WorkspaceSession` into
the HTTP handler. The correct seam is to extend `GraphService` with landing-
specific graph summaries that can be computed from the ports it already has:

- `SymbolRepository::all_symbols()`
- `GraphQueryPort::fan_in()` / `fan_out()` / `callees()`

That keeps the current crate boundaries intact and avoids turning
`landing_handler` into a graph-analysis god function.

## Recommended Scope

### In scope for `e10`

1. Extend `GraphService` / `GraphServiceImpl` with landing-focused helpers:
   - top entry points
   - hot paths
   - god nodes (simple backend version is acceptable)
   - edges among the selected landing nodes
2. Populate `landing_handler` with real data:
   - `entry_points`
   - `hot_paths`
   - `god_nodes`
   - `nodes` = union of the selected symbols
   - `edges` = only relations among selected landing nodes
3. Apply `apply_landing_cap(total_entry_points)` using the cap already
   introduced in `e8b`.
4. Keep `graph_status` semantics unchanged (always 200, no 503; empty data when
   graph missing).
5. Keep `suggested_questions` as-is (can stay empty for now).

### Out of scope for `e10`

- Full narrative / diary / Lepiter-equivalent runtime.
- Generic `WorkspaceSession` exposure in `ApiState`.
- Full PageRank / community visualization pipeline. A lightweight `god_nodes`
  implementation is enough.
- New frontend UX — E8 already shipped the client surface.

## Data Contract Recommendation

### Entry points

Match `AnalysisService::get_entry_points()` semantics:

- symbols with `fan_in == 0`
- return them as `InspectableObjectSummary`
- `nodes` use `GraphNode` with symbol label/kind/file/line
- sort deterministically for stable API output (`file`, `line`, `name`) if the
  underlying graph order is not guaranteed.

### Hot paths

Match `CallGraphAnalyzer::find_hot_paths()` semantics:

- rank all symbols by `fan_in`
- keep only `fan_in > 0`
- sort `fan_in desc`
- cap the list (`limit`, likely 10)
- `min_fan_in` likely 2 to reduce noise on tiny graphs

### God nodes

For `e10`, a simple backend approximation is acceptable:

- rank by a simple score aligned with current codebase semantics:
  `fan_in / symbol_count`
- return top `N` with `score: f64`
- use existing `GodNodeEntry { id, label, score }`

This matches the spirit of `graph_analytics::god_nodes()` without forcing the
landing endpoint to depend on the entire graph analytics service at this stage.

### Edges

Edges should be **only** those whose `source` and `target` are both in the
selected landing-node set. That keeps the graph small, renderable, and avoids
the dangling-edge problem already solved in subgraph tests.

## Test Strategy Recommendation

Reuse the pattern already used in `api_graph_tests.rs`:

- `make_test_api_state()` at `520-529`
- custom `SymbolRepository` mocks (`WideRepo`, `ContextualRepo`)
- custom `GraphQueryPort` mocks (`WideGraphQueryPort`, `ContextualGraphQueryPort`)
- `router(state).oneshot(req)` integration tests

This means `e10` can be tested without DB, without `WorkspaceSession`, and
without changing runtime wiring.

## Approaches

### Approach A — Extend `GraphService` only (recommended)

Add landing-specific graph helpers to the Explorer facade and let the HTTP
handler stay an orchestrator.

- **Pros**: preserves architecture, fully testable in `api_graph_tests.rs`,
  no runtime surgery, no `WorkspaceSession` leak into explorer API layer.
- **Cons**: duplicates a bit of logic already present in `core`
  (`get_entry_points`, `find_hot_paths`) until a deeper facade unification is
  done.
- **Effort**: Medium.

### Approach B — Inject `WorkspaceSession` into `ApiState`

Wire `WorkspaceSession` directly through runtime and call its methods from the
handler.

- **Pros**: reuses core logic directly.
- **Cons**: leaks a large application service into the transport layer,
  increases runtime coupling, makes tests and ownership blurrier.
- **Effort**: Medium/High.

### Approach C — Do only entry points, leave hot paths/god nodes empty

- **Pros**: smallest patch.
- **Cons**: landing remains half-populated; E8 frontend already styles hot and
  god nodes, so we'd still be under-delivering on the intended UX.
- **Effort**: Low.

## Recommendation

**Approach A**.

Implement a first real landing using `GraphService` helpers over
`all_symbols()` + `GraphQueryPort` and let the handler orchestrate summaries
via `SearchService::inspect_object()`.

This gives us:

- real landing nodes
- real entry points
- real hot paths
- god nodes good enough for MVP
- a working truncation banner
- no runtime-architecture regression

## Risks

- **MVP id mapping duplication**: `SearchService::inspect_object()` expects MVP
  ids; `landing_handler` will need a `ResolvedSymbol -> MVP id` helper unless we
  expose one. Small duplication risk.
- **Stability of ordering**: `graph.roots()` order is not explicitly specified.
  We should sort REST output deterministically.
- **God node semantics drift**: simple backend god-node scoring may not match
  future PageRank-based semantics exactly. Acceptable if documented as MVP.

## Ready for Proposal

**Yes.** The seam is clear, the scope is moderate, and the implementation is
testable with current mocks and runtime wiring.
