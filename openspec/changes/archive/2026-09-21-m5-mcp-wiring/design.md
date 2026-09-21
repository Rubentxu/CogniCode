# Design: M5.2 — Wire M5 algorithm IDs into rmcp_adapter

> Change: `m5-mcp-wiring` | Cycle: `p-c1fac1fea05615c6/m5-mcp-wiring` | Phase: design

## Architecture (additive; no boundary change)

```
┌────────────────────┐  tools/list   ┌────────────────────┐
│  MCP client (cogh, │ ────────────► │  CogniCode MCP     │
│  IDE adapter, …)   │ ◄──────────── │  server (rmcp)     │
└────────────────────┘  tools/call   └────────┬───────────┘
                                              │
                                              ▼
                              ┌──────────────────────────────┐
                              │  rmcp_adapter::call_tool_    │
                              │  handler  (existing match)   │
                              │   ↳ "cfg_per_function" arm   │
                              │   ↳ "dominators_cfg" arm     │
                              │   ↳ "slice_forward" arm      │
                              │   ↳ "slice_backward" arm     │
                              │   ↳ "taint_flow" arm         │
                              │   ↳ "interproc_summary" arm  │
                              └──────────┬───────────────────┘
                                         ▼
                              ┌──────────────────────────────┐
                              │ interface/mcp/handlers/      │
                              │ program_analysis_handlers.rs │  NEW
                              │  fn handle_cfg(...) etc.     │
                              └──────────┬───────────────────┘
                                         ▼
                              ┌──────────────────────────────┐
                              │ application/program_analysis  │
                              │  ProgramAnalysisService       │
                              │  ::dispatch(id, params, lim)  │
                              └──────────────────────────────┘
```

## Decisions

### D1 — Reuse `ProgramAnalysisService` as the single dispatch entry point

**Choice**: every new MCP tool calls `ProgramAnalysisService::dispatch` and
serializes the `RunOutput` through the same JSON extraction the conformance
harness uses (`PageRank|Scc|Wcc|...` variants).
**Why**: keeps the dispatcher boundary consistent with WU7
`acceptance_evidence::public_dispatcher_accepts_every_m5_algorithm_id`; the
integration test can assert byte-identical digests between MCP and direct calls.
**Alternatives**: (a) re-implement the algorithm in the handler layer (rejected
— duplicates M5 surface); (b) expose descriptors individually (rejected —
bypasses the PlanLimits + worker machinery).

### D2 — Extract a `program_analysis_handlers` module under `interface/mcp/handlers/`

**Choice**: new file `crates/cognicode-core/src/interface/mcp/handlers/
program_analysis_handlers.rs` (≤ 200 LOC) holds the 6 handler fns and
shared input structs.
**Why**: keeps `rmcp_adapter.rs` under 2500 LOC (currently 2430).
**Pattern**: mirror the existing per-tool handler pattern (`handle_get_file_symbols`
etc.) so reviewers don't learn a new convention.

### D3 — Tool naming: algorithm id verbatim

**Choice**: MCP tool name = algorithm id (`cfg_per_function`,
`dominators_cfg`, …).
**Why**: zero translation surface; matches what `ProgramAnalysisService::m5_ids()`
returns. Clients already use this exact id in their payload.
**Trade-off**: no `program_analysis_` prefix. Acceptable: the spec / descriptors
carry the cohort-5 identity already; tool graph + meta carry the `analytics`
category.

### D4 — HandlerContext gets an `analytics()` accessor

**Choice**: add `pub fn analytics(&self) -> &ProgramAnalysisService` on
`HandlerContext`. The service is cheap to construct (stateless descriptors),
so the accessor can lazily build one if not provided.
**Why**: avoids threading a new constructor arg through 4 constructors.
**Constraint**: `ProgramAnalysisService::new()` is cheap — no I/O — so the lazy
default is safe. If the runtime already has an instance (e.g. for tooling
that caches snapshot state), the existing wiring wins.

### D5 — Input shape: `{ algorithm_params: <object>, limits: <object>? }`

**Choice**: every new tool accepts a single required `algorithm_params`
object whose schema matches the conformance fixture's `params` JSON. An
optional `limits` object is reserved for future PlanLimits overrides.
**Why**: keeps the canonical-corpus replay contract intact
(`fixtures[i].params` plugs straight into `tools/call`'s arguments).

### D6 — No new ports, schemas, or descriptors

**Choice**: 100% reuse of existing M5 descriptors (`CfgDescriptor`,
`DominatorsCfgDescriptor`, `SlicingDescriptor`, `TaintDescriptor`,
`InterprocSummaryDescriptor`). No new schema, no new port.
**Why**: M5 is delivered; M5.2 only changes the access surface.

## Component → file mapping

| Component | File | Status |
|---|---|---|
| MCP tool registrations | `interface/mcp/rmcp_adapter.rs::build_all_tools()` | MODIFY (add 6 entries, < 100 LOC) |
| Dispatch arms | `interface/mcp/rmcp_adapter.rs::call_tool_handler` | MODIFY (6 arms, ~30 LOC) |
| Handler module | `interface/mcp/handlers/program_analysis_handlers.rs` | NEW (≤ 200 LOC, 6 handler fns + 6 input structs) |
| HandlerContext accessor | `interface/mcp/handlers/mod.rs` or `interface/mcp/rmcp_adapter.rs` | MODIFY (≤ 20 LOC) |
| Integration test | `interface/mcp/mcp_roundtrip_tests.rs` (extend) or new `tests/mcp_m5_tools.rs` | NEW (~150 LOC) |
| Architectural rule check | `cargo clippy --workspace --all-features --all-targets -- -D warnings` | VERIFY |
| Domain purity check | grep `crates/cognicode-core/src/domain/analytics/program_analysis` for `tokio`/`sqlx`/`reqwest` | VERIFY |

## Risks & mitigations

- **R1**: `run_interproc_summary` is feature-gated behind
  `program-analysis-server`. The MCP tool must mirror that gate.
  **Mitigation**: gate the entire `program_analysis_handlers` module +
  its registration with `#[cfg(feature = "program-analysis-server")]` for
  the interproc arm; the other 5 stay always-on.
- **R2**: rmcp_pagination — the new 6 tools push `build_all_tools()` past
  the 20-tool page boundary. **Mitigation**: confirm pagination still works
  (existing test should cover it; if not, add one).
- **R3**: Input validation mismatch — `parameter_aliases` BC layer may
  rewrite a key that the M5 descriptor doesn't recognize.
  **Mitigation**: M5 tools are new; document in spec that no alias is
  applied (BC layer is no-op for them).

## Acceptance gate

| Requirement | Check |
|---|---|
| REQ-MCP-01 | `mcp_m5_tools.rs` integration test asserts 6 ids in `tools/list` |
| REQ-MCP-02 | Same test: `tools/call cfg_per_function/taint_flow/interproc_summary` |
|              | digest == conformance corpus digest |
| REQ-MCP-03/04 | `tools/call bad_id` → error result, no panic |
| REQ-MCP-05 | Reuse existing `lookup_category` + `timeout_for_category` |
| REQ-MCP-06 | `wc -l crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` < 2500 |
| REQ-MCP-07 | grep returns 0 for `tokio`/`sqlx`/`reqwest` under `domain/analytics/program_analysis` |
