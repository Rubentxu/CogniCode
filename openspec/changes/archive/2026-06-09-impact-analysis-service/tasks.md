# Tasks: Impact Analysis Service

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~480 (≈350 service+tests + ~60 DTOs + ~3 module glue + small edits) |
| 400-line budget risk | **Medium** |
| Chained PRs recommended | **No** (single coherent slice; service file is the bulk, ≤250 LOC impl + ~200 LOC tests fits one PR) |
| Suggested split | Single PR |
| Delivery strategy | ask-on-risk |
| Chain strategy | size-exception (only if user pushes back) |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Medium

> Forecast rationale: design approved = 1 new file (`impact_analysis.rs` ~150 LOC impl + ~200 LOC tests), 1 modified DTO file (`impact_dto.rs` +~40 LOC), 2 one-line `mod.rs` re-exports. Single PR is the cleanest unit because the service is the bulk and cannot be split without breaking compilation. If the user prefers a slice, the only natural break is **DTOs+glue (PR 1, ~50 LOC) → service+tests (PR 2, ~430 LOC)** — but this is worse for review since PR 2 carries the meaningful logic.

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | All 5 methods, both DTOs, both module re-exports, 25 tests passing | PR 1 | Single PR against `main`. Includes AC1–AC7 verification. |

## Phase 1: Foundation — DTO additions

- [ ] 1.1 In `crates/cognicode-core/src/application/dto/impact_dto.rs`, add `PathResultDto` struct (fields: `path: Vec<String>`, `total_cost: f64`, `found: bool`) with `#[derive(Debug, Clone, Serialize, Deserialize)]` and `from_path(Vec<SymbolId>, f64) -> Self` constructor that maps symbols via `s.as_str().to_string()` and sets `found: true`. Spec R7, AC2.
- [ ] 1.2 In the same file, add `SccDto` struct (fields: `members: Vec<String>`, `size: usize`) with the same derives and `from_scc(Vec<SymbolId>) -> Self` constructor. `size` is computed from `members.len()` after the map. Do NOT touch `ImpactDto` or `CycleDto`. Spec R7, AC2.
- [ ] 1.3 In `crates/cognicode-core/src/application/dto/mod.rs`, extend the existing `impact_dto` re-export line: change `pub use impact_dto::{CycleDto, ImpactDto};` to `pub use impact_dto::{CycleDto, ImpactDto, PathResultDto, SccDto};` (single-line edit). AC2, AC3-adjacent.

## Phase 2: Core service — struct and methods

- [ ] 2.1 Create `crates/cognicode-core/src/application/services/impact_analysis.rs` with the module-level docstring noting it is a stateless application service over `CallGraphProjection` (predecessor-only direction for `impact_radius`; no mutation).
- [ ] 2.2 Add the imports: `crate::domain::aggregates::call_graph::{CallGraph, SymbolId}`, `crate::infrastructure::graph::CallGraphProjection`, `crate::application::dto::impact_dto::{PathResultDto, SccDto}`.
- [ ] 2.3 Define `pub struct ImpactAnalysisService;` (zero-sized) and `impl ImpactAnalysisService { pub fn new() -> Self { Self } }`. Spec R1, AC1.
- [ ] 2.4 Implement `pub fn impact_radius(&self, graph: &CallGraph, root: &SymbolId, max_depth: usize) -> Vec<SymbolId>`: builds `CallGraphProjection::from_call_graph(graph)` and delegates to `find_impact_radius(root, max_depth)`. Spec R2.
- [ ] 2.5 Implement `pub fn has_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> bool`: delegates to `CallGraphProjection::dijkstra` and returns `is_some()`. (Cheapest path-existence check; the projection has no public `has_path`.) Spec R3.
- [ ] 2.6 Implement `pub fn shortest_path(&self, graph: &CallGraph, from: &SymbolId, to: &SymbolId) -> Option<PathResultDto>`: delegates to `dijkstra` and maps `Some((path, cost))` to `Some(PathResultDto::from_path(path, cost))`, `None` propagates. Spec R4.
- [ ] 2.7 Implement `pub fn detect_cycles(&self, graph: &CallGraph) -> Vec<Vec<SymbolId>>`: calls `strongly_connected_components()` and filters out SCCs whose `len() < 2` (excludes self-loops). Spec R5.
- [ ] 2.8 Implement `pub fn containing_component(&self, graph: &CallGraph, id: &SymbolId) -> Option<Vec<SymbolId>>`: calls `connected_components()`, returns the first component containing `id` (any-order match). Returns `None` if `id` is not in any component. Spec R6.

## Phase 3: Module wiring

- [ ] 3.1 In `crates/cognicode-core/src/application/services/mod.rs`, add the line `pub mod impact_analysis;` in alphabetical position (between `file_operations` and `lsp_proxy_service`). AC3.

## Phase 4: Unit tests (25 tests, mapped 1:1 to spec)

Add `#[cfg(test)] mod tests { ... }` at the bottom of `impact_analysis.rs`. Use a private helper `fn make_graph(builder: impl FnOnce(&mut CallGraph)) -> CallGraph` plus fixture builders for the canonical graphs below. Sort results into `BTreeSet` for order-insensitive comparisons (e.g., `impact_radius`, `detect_cycles`, `containing_component`).

> **Fixture names** (used in test bodies): `chain_dac` = `D→A, A→C`; `fan_bc` = `B→C`; `chain_abc` = `A→B, B→C`; `mutual_ab` = `A→B, B→A`; `double_mutual` = `A↔B` + `X↔Y`; `dag_abc` = `A→B→C`; `pair_cd` = `C→D` (no cross-edge to `A→B`); `weight_a_b_09` = `A→B` confidence 0.9; `weight_a_cb` = `A→C, C→B` confidence 0.5 each.

- [ ] 4.1 `test_impact_radius_bounded_predecessors` — R2 Bounded: graph `chain_dac + fan_bc`, assert `impact_radius(C, 1) == {A,B}`, `impact_radius(C, 2) == {A,B,D}`.
- [ ] 4.2 `test_impact_radius_zero_depth` — R2 + E2: any non-empty graph, `impact_radius(any, 0) == vec![]`.
- [ ] 4.3 `test_impact_radius_missing_root` — R2 + E1: `SymbolId::new("missing")`, `impact_radius(m, 10) == vec![]`, no panic.
- [ ] 4.4 `test_impact_radius_empty_graph` — R2 + E7: `CallGraph::new()` with 0 symbols, `impact_radius(any, 5) == vec![]`.
- [ ] 4.5 `test_impact_radius_max_sentinel` — R2 + E6: `chain_dac + fan_bc`, `impact_radius(C, usize::MAX) == {A,B,D}`.
- [ ] 4.6 `test_has_path_direct_transitive_no_path` — R3: `chain_abc`, assert `has_path(A,B)=true`, `has_path(A,C)=true`, `has_path(B,A)=false`.
- [ ] 4.7 `test_has_path_missing_endpoint` — R3 + E1: `has_path(known, missing)=false`, `has_path(missing, known)=false`.
- [ ] 4.8 `test_has_path_self_path` — R3 + E9: graph with `A` only, `has_path(A,A)=true`.
- [ ] 4.9 `test_shortest_path_confidence_weighted` — R4: `weight_a_b_09 + weight_a_cb`, assert `Some(PathResultDto { path: [A,B], total_cost: 0.1, found: true })` (compare field-by-field, not `PartialEq`).
- [ ] 4.10 `test_shortest_path_unreachable` — R4 + E5: `chain_abc`, `shortest_path(A, missing) == None` (use a non-existent id).
- [ ] 4.11 `test_shortest_path_missing_endpoint` — R4 + E1: `shortest_path(known, missing) == None`.
- [ ] 4.12 `test_shortest_path_nan_confidence` — E8: build an edge via `ExtractionContext::Heuristic { score: 0.0 }` (lowest valid cost) so cost = `1.0 - 0.0 = 1.0`; separately assert via unit test on `PathResultDto::from_path` that `total_cost` round-trips. Document the sanitization invariant in a comment. **Note**: raw `f64::NAN` injection is rejected by `ConfidenceRules::assign` (returns `InvalidConfidence`); `sanitize_confidence` runs in the projection constructor. The test verifies the post-sanitization cost is finite and `>= 0.0`.
- [ ] 4.13 `test_shortest_path_self_path` — E10: graph with `A` only, `shortest_path(A,A) == Some(PathResultDto { path: [A], total_cost: 0.0, found: true })`.
- [ ] 4.14 `test_detect_cycles_dag` — R5: `dag_abc`, `detect_cycles() == vec![]`.
- [ ] 4.15 `test_detect_cycles_mutual` — R5: `mutual_ab`, exactly one SCC == `{A,B}`.
- [ ] 4.16 `test_detect_cycles_self_loop_excluded` — R5 + E9: `A→A` only, `detect_cycles() == vec![]`.
- [ ] 4.17 `test_detect_cycles_multiple` — R5: `double_mutual`, `result.len() == 2` and contains `{A,B}` and `{X,Y}`.
- [ ] 4.18 `test_detect_cycles_empty` — R5 + E7: empty graph, `detect_cycles() == vec![]`.
- [ ] 4.19 `test_containing_component_member` — R6: `chain_abc + pair_cd`, `containing_component(A) == Some({A,B,C})` (order-insensitive set comparison).
- [ ] 4.20 `test_containing_component_missing` — R6 + E1: `containing_component(missing) == None`.
- [ ] 4.21 `test_containing_component_isolated` — R6: `A` alone, `containing_component(A) == Some(vec![A])`.
- [ ] 4.22 `test_path_result_dto_roundtrip` — R7: `PathResultDto { path: vec!["A","B"], total_cost: 0.1, found: true }`, `serde_json::to_string` then `from_str`, assert field equality.
- [ ] 4.23 `test_scc_dto_size_matches` — R7: `SccDto::from_scc(vec![A,B,A])` → `members == ["A","B","A"]`, `size == 3`.
- [ ] 4.24 `test_stateless_non_mutating` — R8: build graph with 5 symbols + 7 edges, snapshot `symbol_count()` and `edge_count()`, invoke all 5 methods 100×, assert counts unchanged.
- [ ] 4.25 `test_disconnected_graph_component` — E3 + E4: `chain_abc + pair_cd`, `containing_component(A) == Some({A,B,C})` and `containing_component(C) == Some({A,B,C})` (same component — `B→C` connects them); separately assert `detect_cycles() == vec![]` (no cycle).

## Phase 5: Verification (AC sweep)

- [ ] 5.1 `cargo build -p cognicode-core` compiles clean (AC1, AC3, AC6).
- [ ] 5.2 `cargo test -p cognicode-core --lib impact_analysis` runs the 25 new tests green (AC4, R10).
- [ ] 5.3 `cargo test -p cognicode-core` — full test suite, zero regressions on pre-slice tests (AC4).
- [ ] 5.4 `cargo clippy --all-targets -p cognicode-core -- -D warnings` clean (AC5, R8).
- [ ] 5.5 `git diff crates/cognicode-core/Cargo.toml` is empty (AC7, R9).
- [ ] 5.6 `git diff crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs crates/cognicode-core/src/domain/services/{impact_analyzer,cycle_detector}.rs crates/cognicode-core/src/infrastructure/graph/pet_graph_store.rs` is empty (AC6, R8).
- [ ] 5.7 `git diff --stat` reports changed files matching design exactly: 1 new file (`impact_analysis.rs`), 2 modified files (`impact_dto.rs`, `dto/mod.rs`), 1 modified file (`services/mod.rs`).

## Dependencies Between Tasks

```
Phase 1 (DTOs) ─┐
                ├─► Phase 2 (service impl) ─► Phase 3 (mod wiring)
                │                                       │
                └─► Phase 4 (tests: 4.22, 4.23 use DTOs) ┘
                                                        │
                                                        ▼
                                                  Phase 5 (verify)
```

- 2.x requires 1.x complete (service uses `PathResultDto`, `SccDto`).
- 3.1 requires 2.1 (file must exist before module is declared).
- 4.22 + 4.23 require 1.1 + 1.2 complete.
- 4.x requires 2.x + 3.1 complete (service must be in module tree).
- Phase 5 requires all of Phase 1–4.

## Estimated Line Counts (per task, LOC delta)

| Task | Approx LOC | Notes |
|------|-----------|-------|
| 1.1 | ~15 | struct + impl `from_path` |
| 1.2 | ~12 | struct + impl `from_scc` |
| 1.3 | 1 | one-line edit |
| 2.1–2.3 | ~12 | module doc + imports + struct + `new` |
| 2.4 | ~6 | `impact_radius` |
| 2.5 | ~6 | `has_path` (via `dijkstra` + `is_some`) |
| 2.6 | ~8 | `shortest_path` (delegation + map) |
| 2.7 | ~5 | `detect_cycles` (filter) |
| 2.8 | ~10 | `containing_component` (search) |
| 3.1 | 1 | one-line add |
| 4.1–4.25 | ~250 | 25 tests × ~10 LOC avg (helpers + bodies) |
| 5.x | 0 | commands only |
| **Total** | **~326** (impl+tests+glue) + ~50 DTO + ~5 wiring ≈ **~480** |

## PR Sizing Verdict

Single PR is appropriate:
- **Atomic**: service cannot be split; DTOs and `mod` wiring are mechanical.
- **Reviewable**: bulk is in one new file with clear per-test spec mapping.
- **Rollback**: revert one new file + 2 one-line reverts (per design's rollback plan).

If the user later asks to chain, the natural break is PR 1 = DTOs + `mod` re-exports (~50 LOC, trivial), PR 2 = service + tests (~430 LOC) — but this is worse review ergonomics, so recommend single PR by default.
