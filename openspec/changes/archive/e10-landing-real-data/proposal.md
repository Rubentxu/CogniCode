# Proposal: e10-landing-real-data

## Intent

Complete the backend half of the landing page so `GraphLanding` stops being a
visually rich empty shell. The frontend already has:

- a truncation banner (`e8`),
- backend contract for `truncated` / `truncated_reason` (`e8b`),
- accessible canvas + node-list fallback.

What is still missing is **real landing data**. The handler still returns only
empty stubs for `nodes`, `edges`, `entry_points`, `hot_paths`, `god_nodes`, and
`suggested_questions`. This cycle wires the first real landing summary.

## Scope

### In Scope

- Extend `GraphService` / `GraphServiceImpl` with landing-specific helpers over
  `SymbolRepository::all_symbols()` + `GraphQueryPort`.
- Populate `landing_handler` with:
  - `entry_points`
  - `hot_paths`
  - `god_nodes`
  - `nodes` = union of landing symbols
  - `edges` = only relations among selected landing nodes
- Apply `apply_landing_cap(total_entry_points)` so the banner can activate.
- Add integration tests in `api_graph_tests.rs` for:
  - non-empty landing payload
  - truncation when entry points exceed cap
  - no dangling edges
  - hot paths sorted by fan-in
  - god nodes present with scores

### Out of Scope

- Injecting `WorkspaceSession` into `ApiState`.
- Full narrative runtime / Spotter expansion / contextual editor.
- Replacing the current `LandingPayload` shape.
- Perfect PageRank parity with `graph_analytics::god_nodes()`.
- `suggested_questions` generation (can remain empty for now).

## Capabilities

### Modified Capabilities

- `graphlanding-affordances`:
  - Requirement 1 becomes materially live (banner can now activate).
  - Requirement 4 gets real backing data (`graph-node-*` buttons correspond to
    actual backend symbols).
  - Requirement 9 moves from "contract closed, data empty" to
    "contract closed, entry_points/hot_paths/god_nodes populated".

## Approach

Single backend-first PR off `main`: `feat/e10-landing-real-data`.

Key architectural rule: **do not smuggle `WorkspaceSession` into the HTTP
layer**. Use the existing Explorer facade seam. `GraphService` grows
landing-focused helpers; `landing_handler` orchestrates summaries through
`SearchService::inspect_object()`.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `crates/cognicode-explorer/src/facades/mod.rs` | Modified | `GraphService` trait gains landing helpers |
| `crates/cognicode-explorer/src/facades/graph.rs` | Modified | implement landing entry points / hot paths / god nodes helpers |
| `crates/cognicode-explorer/src/api.rs` | Modified | `landing_handler` populates real payload |
| `crates/cognicode-explorer/src/api_graph_tests.rs` | Modified | landing integration tests |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Helper logic duplicates some core semantics | Medium | Mirror `AnalysisService::get_entry_points()` and `CallGraphAnalyzer::find_hot_paths()` exactly; cite them in comments/tests |
| Unstable ordering causes flaky snapshots | Medium | Sort output deterministically (`fan_in desc`, then `file`, `line`, `name`) |
| God-node score semantics drift from future PageRank-based implementation | Low | Document as MVP approximation; future cycles can swap scoring behind same DTO |

## Rollback Plan

Single PR revert. The change is additive to the handler/facade surface and does
not alter the payload shape, only populates it. Revert restores the current
empty-stub behavior.

## Dependencies

- Depends on `e8` and `e8b` already being merged (they are).
- Enables `e12-viewkind-realization` by making the landing page a real entry
  point into the workspace graph.

## Success Criteria

- [ ] `GET /api/workspaces/:id/landing` returns non-empty `entry_points` when the graph has roots.
- [ ] `truncated` / `truncated_reason` reflect `apply_landing_cap(total_entry_points)`.
- [ ] `nodes` and `edges` are non-empty for non-empty graphs and contain no dangling edges.
- [ ] `entry_points`, `hot_paths`, and `god_nodes` are deterministic and test-covered.
- [ ] Frontend `GraphLanding` shows real buttons and can open real panes from them.
