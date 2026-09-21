# Exploration Report — e41 — DFG conformance coverage

> Change: `e41-lsi-dfg-conformance-coverage` | Cycle: housekeeping | Date: 2026-09-15

## Problem statement

The M5 conformance corpus (`program_analysis::conformance::canonical_corpus()`)
covered 6 of the 7 M5 algorithm kinds at the fixture level:

| Algorithm | Fixture count |
|-----------|---------------|
| `cfg_per_function` | 2 |
| `dominators_cfg` | 1 |
| `slice_forward` | 1 |
| `slice_backward` | 1 |
| `interproc_summary` | 1 (gated by `program-analysis-server`) |
| `taint_flow` | 2 |
| `dfg` | **0** ← gap |

The `dfg` dispatch path exists at `application/program_analysis.rs:330` (`run_dfg`)
and is wired into the algorithm → AlgorithmId map at `program_analysis.rs:223`
(both gated by `#[cfg(feature = "program-analysis-server")]`), but the conformance
corpus had no fixture registered for `algorithm = "dfg"`. This meant:

- `run_corpus()` never exercised the DFG dispatch.
- `replay_guard_is_byte_identical_for_entire_corpus` never hashed a DFG output.
- `perf_envelope_publishes_median_p95_max` never measured DFG dispatch latency.
- `mcp_roundtrip_tests::test_m5_tools_listed` listed DFG but no fixture proved it.

This was an accidental gap from the e38 M5 baseline (interproc_summary was wired
into the conformance corpus at the same time as the other algorithms, but DFG was
left as "stub only" and never picked up).

## Codebase reconnaissance

### Dispatch (`program_analysis.rs`)

`fn run_dfg(&self, params, _limits)` (line 330) accepts:

- `function_id: &str`
- `cfg_digest: &str` (required by `DfgParams::validate`)
- `statements: Vec<Statement>` (with `id`, `defs`, `uses`)

Output: `RunOutput::PageRank(json!({"algorithm": "dfg", "function_id", "edge_count", "edges"}))`.

### Descriptor (`domain/analytics/program_analysis/dfg_descriptor.rs`)

`DfgParams::validate` requires `function_id` and `cfg_digest` keys in the params
JSON object — fixtures MUST include both.

### Conformance corpus (`application/program_analysis/conformance.rs`)

The fixture corpus is a `vec![ConformanceFixture { ... }]`. Each fixture is dispatched
in `run_corpus` and the output is hashed in `replay_guard`.

The dispatch map (line 178) maps `algorithm: &str` to an `AlgorithmId`:
```
"cfg_per_function" => CFG_PER_FUNCTION.clone(),
"dominators_cfg" => DOMINATORS_CFG.clone(),
"slice_forward" => SLICE_FORWARD.clone(),
"slice_backward" => SLICE_BACKWARD.clone(),
"taint_flow" => TAINT_FLOW.clone(),
#[cfg(feature = "program-analysis-server")]
"interproc_summary" => INTERPROC_SUMMARY.clone(),
```

`"dfg"` was missing.

## Solution shape

Two minimal, scoped changes to `conformance.rs`:

1. Register `"dfg" => DFG.clone()` in the dispatch map, gated by
   `#[cfg(feature = "program-analysis-server")]` (mirroring interproc_summary).
2. Add 2 fixtures covering the basic emission contract:
   - `linear_def_use_chain`: 4-statement chain x→y→z, single linear flow.
   - `diamond_diamond_diamond`: 5-statement diamond, multi-use convergence
     (statement 3 uses both `a` and `b`).

Both fixtures declare `cfg_digest` so `DfgParams::validate` passes.

## Out of scope

- Pinning per-fixture digests (the existing `replay_guard` already covers
  byte-identical replay across the full corpus; per-fixture digests are
  a separate slice if the design wants them).
- Performance budgets beyond the existing median/p95/max envelope (DFG now
  contributes to that envelope automatically).
- Cross-producer fixtures (LSP + tree-sitter dual-source DFG; deferred to
  M5+ once the cross-producer test (CP-3) lands).
