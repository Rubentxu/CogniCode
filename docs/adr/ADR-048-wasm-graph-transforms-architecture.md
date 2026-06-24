# ADR-048: WASM Graph Transforms Architecture

**Fecha:** 2026-06-24
**Estado:** PROPOSED
**Decisión:** Two-crate split (`cognicode-graph-algos` + `cognicode-graph-wasm`) with a one-method `GraphBuilder` trait + pure algorithm functions, tsify-generated TypeScript types, and a JSON protocol reusing frontend DTOs
**Source:** SDDK design for `wasm-graph-transforms` (registry `3960f421`)
**Depends on:** [ADR-047](ADR-047-wasm-shared-compute-amendment.md) (ACCEPTED)
**Amends:** None (ADR-047 permits WASM-for-shared-compute; this ADR specifies the architecture)
**Related:** [ADR-007](ADR-007-no-wasm-in-browser.md) (§2 amended), [ADR-031](ADR-031-linear-pagerank.md) (PageRank pure-compute)

---

## Context

ADR-047 (v0.12.16, ACCEPTED) permits WASM-for-shared-compute under four operational rules: single-source, no frontend replacement, feature-gated opt-in, and mandatory connascence audit. However, ADR-047 left the concrete architecture unspecified — it recorded only that a "NEW ADR-048 (pending)" was needed to define the crate structure.

The SDDK explore phase (engram obs-2847) and proposal (engram obs-2850) identified the blocker: 10 pure graph algorithms in `cognicode-core` are welded to the domain `CallGraph` aggregate via `CallGraphProjection`, measured at 3.2 bits of connascence. This coupling prevents compiling the algorithms to `wasm32` without dragging the entire domain layer (30+ non-WASM dependencies: sqlx, tokio, tree-sitter, etc.) into the browser bundle.

The design phase verified the code and ran a spike: `graph_analytics.rs:78-177` (page_rank) operates on `Vec<Vec<usize>>` after a one-time petgraph setup — the loop body has NO petgraph API calls. `god_nodes` (`:248-272`) only consumes `HashMap<usize, f64>`. A spike crate (`/tmp/opencode/spike-petgraph-wasm/`, engram obs-2856) confirmed petgraph + serde + thiserror compile to `wasm32-unknown-unknown` in 5.07s. The frontend DTOs (`GraphNode`, `GraphEdge`, `GodNodeEntry`) serve directly as the WASM JSON protocol without a mapping layer.

## Decision

### 1. Two-crate split

| Crate | Target(s) | Role |
|-------|-----------|------|
| `cognicode-graph-algos` | native + wasm32 | `GraphBuilder` trait (1 method) + pure algorithm functions (PageRank, god_nodes) + JSON adjacency builder |
| `cognicode-graph-wasm` | wasm32 only | Thin bindgen shim (`wasm_bindgen` + `serde-wasm-bindgen` + `tsify`), no algorithm logic |

`cognicode-graph-algos` has zero domain dependencies. `petgraph` is an optional feature (`petgraph-adapter`, `default = []`) so the WASM bundle stays small — petgraph never enters the WASM dependency tree. `cognicode-core` enables `petgraph-adapter` for its `CallGraphProjection: GraphBuilder` impl and re-exports the algorithms so all native callers are unaffected.

### 2. `GraphBuilder` trait (1 method) + pure algorithm functions

```rust
/// The only coupling between graph construction and algorithm computation.
pub trait GraphBuilder {
    fn build_adjacency(&self) -> (Vec<Vec<usize>>, Vec<usize>);
}

/// Pure functions — same .rs on native + wasm32.
pub fn page_rank(in_neighbors: &[Vec<usize>], out_degree: &[usize], n: usize, alpha: f64, max_iterations: usize) -> Vec<f64>
pub fn god_nodes(scores: &HashMap<usize, f64>, percentile: f64) -> Vec<(usize, f64)>
```

Spike evidence (obs-2856) confirmed the PageRank body at `graph_analytics.rs:78-177` operates on `Vec<Vec<usize>>` and `Vec<f64>` after a one-time setup. The body has NO petgraph API calls. The only thing that varies per input type (native petgraph vs. WASM JSON) is the adjacency construction — captured by the single `build_adjacency()` method. The algorithm functions are free functions over raw slices: no `Copy` bound, no `NodeId` associated type, no trait mock needed for unit tests.

The original design proposed a 4-method `GraphLike` trait (node_count, node_ids, out_neighbors, label). Spike evidence rendered this unnecessary — the algorithms consume flat slices, not graph-navigation iterators.

### 3. JSON protocol reuses frontend DTOs + tsify generates TypeScript types

Input shapes consume only `id`/`label` (nodes) and `source`/`target` (edges). The frontend passes its existing `GraphNode[]` and `GraphEdge[]` arrays directly — no mapping layer. Output uses `BTreeMap<String, f64>` for deterministic cross-target parity (same input → byte-identical JSON on native and wasm32). Protocol structs carry `#[derive(Tsify)]` so `wasm-pack build` auto-generates a `.d.ts` file — no hand-maintained TypeScript definitions that could drift from Rust.

### 4. Dual-target CI

Shared test fixtures in `cognicode-graph-algos/tests/fixtures/` run via `cargo test` (native) and `wasm-pack test --node` (wasm32). Bundle size gated at <500KB gzipped (512000 bytes). The WASM build uses `--release` and `--target web`.

## Rationale

1. **Single source of truth (ADR-047 §1):** The algorithm body lives in one `.rs` file compiled to two targets. The `diff` between targets is always empty by construction — there is only one copy.

2. **Connascence reduction:** The `GraphBuilder` trait reduces CallGraph↔algos coupling from 3.2 bits to ~0.3 bits. The algorithm crate depends on a 1-method trait + pure functions, not on `CallGraph`, `SymbolId`, `CallGraphProjection`, or any domain type. petgraph↔algos coupling is behind `petgraph-adapter` — zero for WASM build.

3. **Pure functions over trait methods:** Spike evidence (obs-2856) showed the PageRank body operates on flat `Vec<Vec<usize>>` after setup. Making algorithms free functions means unit tests pass hand-built slices — no graph mock, no trait impl boilerplate. This is simpler than the original 4-method `GraphLike` design.

4. **`petgraph-adapter` feature gate (`default = []`):** Native builds enable the feature for `CallGraphProjection: GraphBuilder`. WASM builds leave it off — petgraph never enters the WASM dep tree. This keeps the WASM bundle minimal and avoids coupling the pure crate to petgraph for WASM consumers.

5. **tsify over hand-maintained `.d.ts`:** `#[derive(Tsify)]` auto-generates TypeScript types from Rust structs at build time. No drift between Rust struct fields and TypeScript interfaces.

6. **`serde-wasm-bindgen` over `serde_json`:** Direct `JsValue` ↔ Rust conversion halves deserialization overhead for large node arrays (no intermediate JSON string allocation).

## Alternatives Considered

### Option A — Standalone WASM crate with re-implemented algorithms

A single `cognicode-graph-wasm` crate containing its own PageRank/god_nodes implementation in Rust, compiled only to WASM.

**Rejected:** Violates ADR-047 §1 (single source). Two implementations (native in `cognicode-core`, WASM in `cognicode-graph-wasm`) will inevitably drift. Even with shared tests, the maintenance burden doubles and correctness is not guaranteed — a bug fixed in one target may not be fixed in the other.

### Option B — Compile `cognicode-core` subset to WASM

Add `#[cfg(target_arch = "wasm32")]` gates throughout `cognicode-core` to compile a subset to `wasm32`.

**Rejected:** `cognicode-core` has 30+ dependencies incompatible with WASM (sqlx, tokio, tree-sitter × 30 languages, rmcp, axum). Feature-gating all of them out is a massive, fragile refactor. The crate is a domain + infrastructure layer; algorithms are a small pure subset that deserves its own crate.

### Option C — Single crate with cfg gates (adopted architecture)

One new crate with `#[cfg(target_arch = "wasm32")]` sections for the bindgen exports.

**Rejected in favor of two-crate split:** A single crate mixing `cdylib` (WASM) and `rlib` (native library) crate types creates ambiguous build semantics. The WASM shim (`wasm_bindgen` macros, `serde-wasm-bindgen`) is a thin adapter concern that should not pollute the algorithm crate's dependency tree for native consumers. The two-crate split follows the same separation principle as the existing `cognicode-core` (logic) vs `cognicode-explorer` (HTTP adapter) split.

### Option D — `GraphLike` extends petgraph visitor traits (original design)

Define `GraphLike: IntoNeighbors + NodeIndexable + ...` so algorithms can call petgraph's `algo::page_rank` directly.

**Rejected:** petgraph's visitor traits return lifetime-bound iterator types (`Neighbors<'a, E>`, `NodeIndices<'a, N, E>`). If `GraphLike` extended them, the JSON adapter would need to produce petgraph-compatible iterators — requiring a full petgraph wrapper graph, defeating the purpose of the trait.

### Option E — 4-method `GraphLike` trait (original design, superseded by spike)

Define `GraphLike` with `node_count`, `node_ids`, `out_neighbors`, `label` — a richer trait surface than the adopted 1-method `GraphBuilder`.

**Rejected after spike evidence (obs-2856):** The spike proved the PageRank body operates on `Vec<Vec<usize>>` after a one-time setup, with NO petgraph API calls in the hot loop. A 4-method trait over-abstracts what the algorithms actually consume (flat slices). The 1-method `GraphBuilder` + pure functions design is strictly simpler: fewer trait methods, no `Copy`/`NodeId`/`Label` associated types, and algorithm unit tests need no mock at all.

## Consequences

### Positive

- **Zero duplication:** Same `.rs` compiles to native + wasm32. Single source of truth guaranteed by construction.
- **Connascence reduced 3.2→0.3 bits:** Algorithm crate depends on a 1-method trait + pure functions, not the domain aggregate.
- **Pure functions unit-testable without mocks:** Algorithms take `&[Vec<usize>]` slices; tests pass hand-built data.
- **Native callers unaffected:** `GraphAnalyticsService::page_rank` signature preserved; internal delegation is invisible.
- **Edge compute:** Browser runs PageRank/god_nodes with zero backend round-trip (200-500ms → <1ms for typical subgraphs).
- **Frontend DTOs reused:** No mapping layer between frontend types and WASM protocol.
- **tsify type safety:** Auto-generated `.d.ts` — no drift between Rust and TypeScript.
- **Deterministic output:** `BTreeMap` guarantees byte-identical JSON across targets.
- **Petgraph excluded from WASM:** `petgraph-adapter` feature off → smaller WASM bundle.

### Negative

- **Build complexity:** WASM toolchain (`wasm-pack`, `wasm-bindgen`, `tsify`, `wasm32-unknown-unknown` target) adds ~2 CI jobs.
- **Bundle size:** `.wasm` artifact must stay under 500KB gzipped; only PageRank + god_nodes in MVP. Future algorithms (SCC, community detection) must pass the same budget check.
- **Dual-target test maintenance:** Each new algorithm requires verification on both targets. Mitigated by shared fixtures + pure-function tests that run identically on both.
- **Two new crates:** Increases workspace member count. Accepted — the separation follows the existing crate discipline (`cognicode-core` vs `cognicode-explorer` vs `cognicode-mcp`).
- **`petgraph-adapter` feature gate:** Native consumers must enable the feature to get the `CallGraphProjection: GraphBuilder` impl. Default off so the algorithm crate is WASM-clean by default.

## Affected ADRs

- **ADR-047** — satisfies the "connascence audit" requirement (rule 4) and the "NEW ADR-048 (pending)" slot.
- **ADR-007** — no change (§2 already amended by ADR-047).
- **ADR-031** — no change (PageRank pure-compute is compatible; the algorithm body relocates but is unchanged).

## Validation

- [x] Spike PASSED: petgraph + serde + thiserror compile to wasm32 (obs-2856, 2026-06-24)
- [ ] `cargo test -p cognicode-graph-algos` passes (native)
- [ ] `wasm-pack test --node crates/cognicode-graph-wasm` passes (wasm32)
- [ ] Parity: same input graph → byte-identical `BTreeMap` JSON on both targets
- [ ] `cognicode-core` compiles with `cognicode-graph-algos` re-export (no native caller breakage)
- [ ] Bundle size: `.wasm` gzipped < 500KB (512000 bytes)
- [ ] tsify: `wasm-pack build` generates `.d.ts` matching Rust struct shapes
- [ ] Frontend: WASM load failure → backend fallback (integration test)
- [ ] ADR-047 rule 4 satisfied: `diff` of algorithm body between targets is empty

## References

- [ADR-047](ADR-047-wasm-shared-compute-amendment.md) — WASM-for-shared-compute amendment
- [ADR-007](ADR-007-no-wasm-in-browser.md) — No WASM in browser (§2 amended)
- [ADR-031](ADR-031-linear-pagerank.md) — Linear PageRank (pure-compute)
- `crates/cognicode-core/src/application/services/graph_analytics.rs:78` — page_rank source
- `crates/cognicode-core/src/application/services/graph_analytics.rs:248` — god_nodes source
- `crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs` — petgraph adapter (`GraphBuilder` impl target)
- `crates/cognicode-core/src/domain/aggregates/call_graph.rs:1024` — SymbolId newtype (Copy constraint)
- `apps/explorer-ui/src/api/schemas.ts` — frontend DTOs (protocol source)
- SDDK design: `sddk/wasm-graph-transforms/design.md`
- SDDK proposal: registry `3960f421`
- SDDK explore: engram obs-2847
- Spike validation: engram obs-2856 (petgraph+serde+thiserror WASM-compatible, PageRank body is pure compute)
