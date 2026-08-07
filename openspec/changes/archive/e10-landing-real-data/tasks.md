# Tasks: e10-landing-real-data

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~120 Rust + ~120 tests |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | n/a |
| Decision needed before apply | No |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: n/a
400-line budget risk: Low

## Phase 1: RED — landing integration tests

- [ ] 1.1 Add landing endpoint integration tests to `crates/cognicode-explorer/src/api_graph_tests.rs`
- [ ] 1.2 Add a `LandingRepo` mock implementing `all_symbols()` with a small graph (roots + hot nodes)
- [ ] 1.3 Add a `LandingGraphQueryPort` mock implementing `fan_in`, `fan_out`, and `callees`
- [ ] 1.4 Add `landing_app()` helper using `make_test_api_state()`
- [ ] 1.5 Test: non-empty landing returns real `entry_points`, `hot_paths`, `god_nodes`, `nodes`, `edges`
- [ ] 1.6 Test: `truncated === true` when total entry points exceed `LANDING_NODE_CAP`
- [ ] 1.7 Test: `hot_paths` sorted by `fan_in` desc and excludes `fan_in == 0`
- [ ] 1.8 Test: edges contain no dangling endpoints
- [ ] 1.9 Run `cargo test -p cognicode-explorer landing_` (or targeted file) — confirm RED

## Phase 2: GREEN — graph facade + handler

- [ ] 2.1 Extend `GraphService` trait in `crates/cognicode-explorer/src/facades/mod.rs` with `landing_entry_points`, `landing_hot_paths`, `landing_god_nodes`
- [ ] 2.2 Implement the new methods in `crates/cognicode-explorer/src/facades/graph.rs`
- [ ] 2.3 `landing_entry_points(limit)` should filter `all_symbols()` by `fan_in == 0`, sort deterministically, and return `(limited, total)`
- [ ] 2.4 `landing_hot_paths(limit, min_fan_in)` should rank by `fan_in` desc, filter `fan_in > 0` and `>= min_fan_in`, and return `ResolvedSymbol`s
- [ ] 2.5 `landing_god_nodes(limit)` should rank by a deterministic backend score and return `GodNodeEntry { id, label, score }`
- [ ] 2.6 In `crates/cognicode-explorer/src/api.rs`, update `landing_handler` to call the new methods and `apply_landing_cap(total_entry_points)`
- [ ] 2.7 Same handler, build `nodes` as the deduplicated union of selected symbols
- [ ] 2.8 Same handler, build `edges` only when both endpoints are in the selected set
- [ ] 2.9 Same handler, build `entry_points` and `hot_paths` summaries via `state.search.inspect_object(mvp_id)`
- [ ] 2.10 Run `cargo test -p cognicode-explorer` — confirm GREEN for new landing tests and no regressions in related subgraph/contextual tests

## Phase 3: Verify & close

- [ ] 3.1 Run `cargo check --workspace --tests`
- [ ] 3.2 Run `npx vitest run` in `apps/explorer-ui` to confirm no frontend regression
- [ ] 3.3 Push branch and open PR against `main`
- [ ] 3.4 Squash-merge after review
- [ ] 3.5 Tag `v0.25.0` if we classify this as MINOR (recommended, since the landing endpoint becomes materially more capable)
- [ ] 3.6 Archive the change and update `docs/ROADMAP.md`
