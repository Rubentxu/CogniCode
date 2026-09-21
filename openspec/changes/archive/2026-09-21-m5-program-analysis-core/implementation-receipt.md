# Implementation Receipt — M5 — Program Analysis Core

> Cycle: `p-c1fac1fea05615c6/m5-program-analysis-core`
> Path: **A-lite** (7 WUs collapsed)
> Phase: **implementation** (closed)
> Date: 2026-09-14
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Stacked commit chain (7 commits)

| # | SHA | Subject |
|---:|---|---|
| 1 | `779e935c` | WU1 descriptors + service facade + `program-analysis-server` feature flag |
| 2 | `775b2cca` | WU2 CFG + dominators_cfg (BasicBlock adjacency) |
| 3 | `5cacaa13` | WU3 DFG + forward/backward slicing (criterion `(variable, def_site)`) |
| 4 | `553cc025` | WU4 InterprocSummary (Tarjan SCC, bottom-up DAG, fixed-point recursion, sha256 digest) |
| 5 | `5610e1f8` | WU5 Taint v1 (multi-source BFS, predecessor reconstruction, intermediates_on_chain) |
| 6 | `2a3ffe3d` | WU6 conformance harness (`canonical_corpus`, `replay_guard`, `perf_envelope`, `mcp_fixture_report`) |
| 7 | `0105c491` | openspec/changes artifacts (proposal/specs/design/tasks) |
| 8 | `2f0d48f5` | WU7 acceptance evidence — `acceptance_evidence` module exercises PUBLIC `ProgramAnalysisService::dispatch` for all 6 algorithm IDs (replay + perf + serde) |

## Deliverables shipped

### Production code (new)
- `crates/cognicode-graph-algos/src/algorithms/cfg.rs` — CFG over `FunctionLocalView`
- `crates/cognicode-graph-algos/src/algorithms/dominators.rs` — per-function dominators
- `crates/cognicode-graph-algos/src/algorithms/slicer.rs` — forward + backward slicing
- `crates/cognicode-graph-algos/src/algorithms/interproc.rs` — InterprocSummary
- `crates/cognicode-graph-algos/src/algorithms/taint.rs` — Taint v1
- `crates/cognicode-core/src/domain/analytics/cfg_descriptor.rs` — `AlgorithmDescriptor` for `cfg_per_function`
- `crates/cognicode-core/src/domain/analytics/dominators_descriptor.rs`
- `crates/cognicode-core/src/domain/analytics/slicing_descriptor.rs`
- `crates/cognicode-core/src/domain/analytics/interproc_summaries_descriptor.rs`
- `crates/cognicode-core/src/domain/analytics/taint_descriptor.rs`
- `crates/cognicode-core/src/application/program_analysis.rs` — `ProgramAnalysisService::dispatch(id, params, limits)` facade (+101 LOC)
- `crates/cognicode-core/src/application/program_analysis/conformance.rs` — harness (+14 LOC)

### Specs
- `openspec/changes/m5-program-analysis-core/specs/program-analysis-core/spec.md` (NEW)
- `openspec/changes/m5-program-analysis-core/specs/program-analysis-conformance/spec.md` (NEW)

## Test evidence (final, pre-cycle-close)

- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → **22 passed**, 0 failed
- `cargo test -p cognicode-graph-algos --lib` → 161 passed, 0 failed
- `cargo check --workspace --all-features` → exit 0
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0

## Algorithm coverage

| ID | Source of input at this cycle | Future (M5.1+) |
|---|---|---|
| `cfg_per_function` | synthetic corpus | real tree-sitter AST |
| `dominators_cfg` | synthetic corpus | real tree-sitter AST |
| `slice_forward` | synthetic corpus | real tree-sitter AST |
| `slice_backward` | synthetic corpus | real tree-sitter AST |
| `taint_flow` | synthetic corpus | real tree-sitter AST |
| `interproc_summary` | synthetic corpus | real tree-sitter AST (M5.1 lifts, M5.1b widens) |

The harness documents explicitly that tree-sitter → `FunctionLocalView` lifting is M5.1, out of scope for M5.

## Scope boundary

M5 is the algorithm layer. M5.1 (later cycle) does the tree-sitter → FunctionLocalView lift. M5.2 (later cycle) wires the algorithm IDs into the MCP server.

## Cycle state at handoff

Implementation phase complete. Verification and release phases happened in the same session that closed the ledger (sequence 9 transition). This receipt is a retrospective artifact created at housekeeping time.