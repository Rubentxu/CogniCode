# Design: e10-landing-real-data

## Technical Approach

Implement the first real landing payload entirely inside the Explorer seam.
Do **not** inject `WorkspaceSession` into `ApiState`. Instead, extend
`GraphService` / `GraphServiceImpl` with landing-focused helpers built from the
ports it already owns:

- `SymbolRepository::all_symbols()`
- `GraphQueryPort::fan_in()` / `fan_out()` / `callees()`

`landing_handler` remains an orchestrator. It asks `GraphService` for landing
symbol sets, then uses `SearchService::inspect_object()` to turn selected symbol
ids into `InspectableObjectSummary` values with consistent `available_views`.

## Architecture Decisions

### D-1: Extend `GraphService`, don't leak `WorkspaceSession` into API

**Choice**: Add landing-focused methods to `GraphService`.
**Rejected**: Thread `WorkspaceSession` through `ApiState` and call it directly
from `landing_handler`.
**Why**: `WorkspaceSession` is a large application service from core. Pushing it
into the Explorer transport layer would increase coupling and blur the
facade boundary. `GraphService` already has the ports needed to compute a
useful landing summary.

### D-2: Reuse Search summaries for `entry_points` and `hot_paths`

**Choice**: For each selected `ResolvedSymbol`, build `InspectableObjectSummary`
through `state.search.inspect_object(mvp_id)`.
**Rejected**: Rebuild the summary shape inside `landing_handler`.
**Why**: `SearchService::inspect_object()` already produces the canonical summary
shape and `available_views` list. Reusing it avoids duplicating object-summary
logic.

### D-3: MVP `god_nodes` can use a simple backend score

**Choice**: Score god nodes using a deterministic backend approximation based on
dependency centrality (e.g. `fan_in / symbol_count`) and expose them via
`GodNodeEntry { id, label, score }`.
**Rejected**: Block `e10` on full PageRank parity with the graph-analytics
service.
**Why**: The landing only needs a useful "highly depended-upon symbols" signal.
Exact parity with future `graph_analytics::god_nodes()` can come later behind
the same DTO.

### D-4: Keep edges local to the selected landing node set

**Choice**: Build edges only when both `source` and `target` are in the
selected landing-node set.
**Rejected**: Include outbound edges to off-screen nodes.
**Why**: The landing is a compressive overview. Off-screen edges create visual
noise and dangling-node semantics.

## File Changes

| File | Action | Description |
|---|---|---|
| `crates/cognicode-explorer/src/facades/mod.rs` | Modify | `GraphService` trait gets landing-focused methods |
| `crates/cognicode-explorer/src/facades/graph.rs` | Modify | implement landing entry points / hot paths / god nodes helpers |
| `crates/cognicode-explorer/src/api.rs` | Modify | `landing_handler` populates real payload |
| `crates/cognicode-explorer/src/api_graph_tests.rs` | Modify | integration tests for landing endpoint |

## Interfaces / Contracts

Proposed additions to `GraphService`:

```rust
#[async_trait]
pub trait GraphService: Send + Sync {
    async fn resolve_symbol(&self, id: &str) -> ExplorerResult<Option<ResolvedSymbol>>;
    fn graph_query(&self) -> Option<Arc<dyn GraphQueryPort>>;
    async fn build_subgraph(&self, root_id: &str, depth: u8, direction: SubgraphDirection, max_nodes: u32) -> ExplorerResult<SubgraphResponse>;
    async fn build_architecture(&self, root_path: &str) -> ExplorerResult<SubgraphResponse>;
    async fn compare_architecture(&self, root_path: &str) -> ExplorerResult<DriftReport>;

    // NEW
    async fn landing_entry_points(&self, limit: usize) -> ExplorerResult<(Vec<ResolvedSymbol>, usize)>;
    async fn landing_hot_paths(&self, limit: usize, min_fan_in: usize) -> ExplorerResult<Vec<ResolvedSymbol>>;
    async fn landing_god_nodes(&self, limit: usize) -> ExplorerResult<Vec<crate::dto::GodNodeEntry>>;
}
```

Notes:
- `landing_entry_points()` returns `(limited_items, total_count)` so the handler
  can call `apply_landing_cap(total_count)`.
- `landing_hot_paths()` returns symbols only; `fan_in` ordering is internal.
- `landing_god_nodes()` returns the DTO directly because the score is part of
  the public contract.

## Testing Strategy

Use `api_graph_tests.rs` integration patterns:

- custom `SymbolRepository` mocks with real `all_symbols()` data
- custom `GraphQueryPort` mocks with controlled `fan_in`, `fan_out`, `callees`
- `router(state).oneshot(req)` for `GET /api/workspaces/:id/landing`

Add tests for:

1. non-empty landing payload with entry points + hot paths + god nodes
2. truncation when total entry points exceed `LANDING_NODE_CAP`
3. deterministic ordering of `hot_paths`
4. edges contain no dangling endpoints
5. empty graph still returns 200 + empty collections

## Migration / Rollout

No migration required. Pure backend change, no frontend edit. If the graph is
missing, the endpoint still returns 200 with empty collections, so rollout is
safe.

## Open Questions

- Should `landing_god_nodes()` use exact `GraphAnalyticsService::god_nodes()` now,
  or keep the MVP approximation? Recommendation: keep MVP approximation for `e10`.
- Should `entry_points` be sorted by graph root order or a deterministic REST
  order (`file`, `line`, `name`)? Recommendation: deterministic REST order.
