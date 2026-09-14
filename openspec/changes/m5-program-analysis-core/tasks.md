# Tasks: M5 Program Analysis Core

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~1.6–2.0k (goldens + pin digest excluded) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | 6 work units (PR 1 → PR 2 → PR 3 → PR 4 → PR 5 → PR 6) |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Test | Harness | Rollback |
|------|------|-----------|------|---------|----------|
| 1 | Domain types + ports + `ProgramAnalysisService` skeleton (D2, D3) | PR 1 | `cargo test -p cognicode-core --lib program_analysis` | `cargo check -p cognicode-core -p cognicode-graph-algos` | revert file |
| 2 | CFG + per-function dominators in `cognicode-graph-algos` (D1, D4) | PR 2 | `cargo test -p cognicode-graph-algos cfg dominators_cfg` | `wasm32 build check` | revert file |
| 3 | DFG + forward/backward slicer (D1) | PR 3 | `cargo test -p cognicode-graph-algos dfg slicer` | `cargo test -p cognicode-core --lib program_analysis` | revert file |
| 4 | Interprocedural summaries (D7) | PR 4 | `cargo test -p cognicode-graph-algos interproc_summary` | `cargo test -p cognicode-core --lib program_analysis` | revert file |
| 5 | Taint v1 with declared sources/sinks (D6) | PR 5 | `cargo test -p cognicode-core --lib taint --features program-analysis-server` | `just lsi-program-analysis` | revert file |
| 6 | Fixtures + MCP handlers + perf envelope + digest pins (D4, D5, D8) | PR 6 | `cargo test -p cognicode-core --test program_analysis_conformance` | `just lsi-program-analysis` + replay determinism + perf-budget gate | revert file |

```yaml
advisory_projection:
  metric: lines_changed
  forecast: 1600-2000
  budget: 400
  recommendation: "consider splitting if LOC > 400 (already split into 6 WUs)"
  rationale: "advisory; not blocking per ADR-0070"
```

### Open questions resolved at task granularity (defaults applied per design)

- Language coverage order: **rust first**, then ts serverless, then java jdtls-gated
- Taint sources/sinks: declared per language in `TaintPatterns` (no auto-discovery)
- MCP tool naming: **snake_case** (`get_cfg`, `slice_backward`, `taint_flow`)
- Summary cache: per-snapshot only (rebuilt on snapshot change)
- Slicing criterion vocabulary: `(variable, definition-site)` only (extend in M5+)

## Phase 1: Foundation (WU1)

- [ ] 1.1 Create `crates/cognicode-core/src/application/program_analysis_service.rs` with `ProgramAnalysisService { registry: Arc<AnalyticsRegistry>, parser: Arc<dyn SymbolRepository> }` and `run(algorithm, snapshot, params, limits) -> Result<RunOutput, AnalyticsError>`.
- [ ] 1.2 Add new `AlgorithmId` constants in `crates/cognicode-core/src/domain/analytics/lineage.rs`: `cfg_per_function`, `dfg`, `slice_forward`, `slice_backward`, `dominators_cfg`, `interproc_summary`, `taint_flow` (no serde; `from_static` only).
- [ ] 1.3 Add empty descriptor stubs in `crates/cognicode-core/src/domain/analytics/`: `cfg_descriptor.rs`, `dfg_descriptor.rs`, `slicing_descriptor.rs`, `interproc_summaries_descriptor.rs`, `taint_descriptor.rs` (each implementing `AlgorithmDescriptor` with `id`, `version`, `maturity = Maturity::Experimental`, `determinism = DeterminismKind::StrictDeterministic`).
- [ ] 1.4 Register the 7 descriptors in `crates/cognicode-core/src/domain/analytics/mod.rs` and `crates/cognicode-core/src/domain/analytics/lineage.rs::registry()`.
- [ ] 1.5 Add feature flag `program-analysis-server` in `crates/cognicode-core/Cargo.toml` (default off, gates DFG/summary/taint per D8).
- [ ] 1.6 RED: `cargo test -p cognicode-core --lib program_analysis` → assert 7 `AlgorithmId`s registered; assert `ProgramAnalysisService::run` returns `AnalyticsError::Unsupported` for unregistered ids.
- [ ] 1.7 GREEN: implement stubs minimally; `cargo test -p cognicode-core --lib program_analysis`; `cargo check -p cognicode-core` ± features.

## Phase 2: Algorithms — CFG and Dominators (WU2)

- [ ] 2.1 Create `crates/cognicode-graph-algos/src/algorithms/cfg.rs` with `pub fn cfg_from_function(symbol: &Symbol, ast: &tree_sitter::Tree, source: &[u8]) -> Cfg` and `pub struct Cfg { entry: CfgNodeId, nodes: Vec<CfgNode>, edges: Vec<(CfgNodeId, CfgNodeId)> }`.
- [ ] 2.2 Implement `Cfg::canonical_bytes(&self) -> Vec<u8>` (sorted nodes, sorted outgoing edges, deterministic) and `Cfg::digest(&self) -> u64` (fnv1a64).
- [ ] 2.3 Create `crates/cognicode-graph-algos/src/algorithms/dominators_cfg.rs` with `pub fn dominators_cfg(cfg: &Cfg) -> Vec<(CfgNodeId, Option<CfgNodeId>)>` (immediate dominator per node; entry has `None`).
- [ ] 2.4 RED: `cfg_from_function` on a function with branching+looping yields deterministic CFG (sorted edges, stable entry); `dominators_cfg` on same yields expected dominator chain.
- [ ] 2.5 RED: empty function yields 1-node CFG with zero edges; resource limit (max_nodes=0) returns limit error.
- [ ] 2.6 GREEN: implement minimum; `cargo test -p cognicode-graph-algos cfg dominators_cfg`; `cargo check --target wasm32-unknown-unknown -p cognicode-graph-algos` (WASM-clean per D8).
- [ ] 2.7 Hook descriptors: `cfg_descriptor.rs` + `dominators_cfg_descriptor.rs` (modify `slicing_descriptor.rs` later) with fixture harness and PlanLimits validation.

## Phase 3: Algorithms — DFG and Slicer (WU3)

- [ ] 3.1 Create `crates/cognicode-graph-algos/src/algorithms/dfg.rs` with `pub fn dfg_from_cfg(cfg: &Cfg, source: &[u8]) -> Dfg` (definition-to-use edges; consumes CFG without re-parsing per spec).
- [ ] 3.2 Implement `Dfg::canonical_bytes(&self) -> Vec<u8>` and `Dfg::digest(&self) -> u64`.
- [ ] 3.3 Create `crates/cognicode-graph-algos/src/algorithms/slicer.rs` with `pub fn slice_backward(cfg: &Cfg, criterion: SliceCriterion) -> Vec<CfgNodeId>` and `pub fn slice_forward(cfg: &Cfg, criterion: SliceCriterion) -> Vec<CfgNodeId>` (sorted, stable output).
- [ ] 3.4 Define `pub struct SliceCriterion { variable: String, definition_site: CfgNodeId }` in `slicer.rs`.
- [ ] 3.5 RED: backward slice from a use includes the introduction site; forward slice stops at reassignment (spec scenarios).
- [ ] 3.6 GREEN: implement; `cargo test -p cognicode-graph-algos dfg slicer`; `cargo test -p cognicode-core --lib program_analysis` (descriptor plumbing).

## Phase 4: Algorithms — Interprocedural Summaries (WU4)

- [ ] 4.1 Create `crates/cognicode-graph-algos/src/algorithms/interproc_summary.rs` with `pub fn summary_for_call_graph(call_graph: &CallGraph, symbol_table: &[Symbol]) -> Vec<Summary>` (one per callable; memoized).
- [ ] 4.2 Implement recursion detection via existing `cognicode-graph-algos::algorithms::condensation::sccs(call_graph)`; recursive cycles emit `Summary { kind: SummaryKind::FixedPoint, .. }` marker instead of expanding.
- [ ] 4.3 Define `pub struct Summary { id: SummaryId, callee_symbol: Symbol, reads: Vec<String>, writes: Vec<String>, calls: Vec<SummaryId>, kind: SummaryKind }`.
- [ ] 4.4 RED: A→B call chain yields A's summary referencing B's stable id; recursive function yields `FixedPoint` marker (spec scenarios).
- [ ] 4.5 GREEN: implement; `cargo test -p cognicode-graph-algos interproc_summary`; resource bound `PlanLimits::max_summary_depth` enforced.
- [ ] 4.6 Hook `interproc_summaries_descriptor.rs` (gated `#[cfg(feature = "program-analysis-server")]` per D8).

## Phase 5: Taint v1 (WU5)

- [ ] 5.1 Create `crates/cognicode-core/src/domain/analytics/taint_descriptor.rs` with `pub struct TaintPatterns { per_language: HashMap<Language, Vec<Pattern>> }` where `Pattern { kind: PatternKind, match_shape: MatchShape }` and `PatternKind ∈ {Source, Sink, Untaint}`.
- [ ] 5.2 Declare initial patterns for Rust: `std::fs::read*`, `std::env::args*`, `std::env::var*` (sources); `std::fs::write*`, `println!` (sinks); explicit `_ = sanitize(...)` calls (untaint).
- [ ] 5.3 Create `crates/cognicode-graph-algos/src/algorithms/taint_flow.rs` with `pub fn taint_flow(dfg: &Dfg, patterns: &TaintPatterns, snapshot: SnapshotDescriptor) -> Vec<TaintPath>` (forward flow-sensitive; one path per sink reached).
- [ ] 5.4 Define `pub struct TaintPath { source: TaintSite, sink: TaintSite, intermediates: Vec<TaintSite>, tier: ProvenanceTier }`; tier derivation: LSP symbol id match → Extracted, local resolver match → Inferred, tree-sitter only → Ambiguous.
- [ ] 5.5 RED: source-to-sink path emitted; untaint breaks path; tier label matches declaration (spec scenarios).
- [ ] 5.6 GREEN: implement; `cargo test -p cognicode-core --lib taint --features program-analysis-server`; `just lsi-program-analysis`.

## Phase 6: Conformance, MCP, Perf Envelope (WU6)

- [ ] 6.1 Create `sandbox/fixtures/lsi-program-analysis/{rust,ts,java}/` with `expected.json` per algorithm (per spec conformance: rust full; ts serverless; java jdtls-gated).
- [ ] 6.2 Create `crates/cognicode-core/tests/program_analysis_conformance.rs` with 6 digest pins: `PINNED_CFG_DIGEST`, `PINNED_DFG_DIGEST`, `PINNED_SLICE_DIGEST`, `PINNED_DOMINATORS_DIGEST`, `PINNED_SUMMARY_DIGEST`, `PINNED_TAINT_DIGEST`.
- [ ] 6.3 RED: each algorithm kind fails its pin if its output digest drifts; replay-guard fails on second-run byte diff (spec scenarios).
- [ ] 6.4 GREEN: implement harness; `cargo test -p cognicode-core --test program_analysis_conformance`; first capture pins digests (fixtures byte-identical).
- [ ] 6.5 Create `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs` with snake_case tools: `get_cfg`, `get_dfg`, `slice_backward`, `slice_forward`, `dominators_per_function`, `summary_call`, `taint_flow`; each annotated `#[cognicode_meta(...)]` per M2/M3 pattern.
- [ ] 6.6 Register handlers in `crates/cognicode-core/src/interface/mcp/handlers/mod.rs`; round-trip tests `cargo test -p cognicode-core --lib mcp_handlers --features program-analysis-server`.
- [ ] 6.7 Extend `perf-budget.toml` with 6 new entries (per algorithm: max wall-clock + max nodes); wire into `just perf` budget gate.
- [ ] 6.8 Add `just lsi-program-analysis` recipe: per-language fixtures + digest check + perf-budget compare.
- [ ] 6.9 `cargo check -p cognicode-core` default / `--features evidence-kernel` / `--features evidence-kernel,program-analysis-server`; `cargo check -p cognicode-explorer`/`-p cognicode-runtime`; `just lint` EXIT 0; `cargo fmt --check` clean; `wasm32` build check for graph-algos (D8).
- [ ] 6.10 Update `.agent/TESTING-STATE.md` (Active Change + handoff section).