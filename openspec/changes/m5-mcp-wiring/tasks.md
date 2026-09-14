# Tasks: M5.2 — Wire M5 algorithm IDs into rmcp_adapter

> Change: `m5-mcp-wiring` | Cycle: `p-c1fac1fea05615c6/m5-mcp-wiring` | Phase: tasks

Stacked PRs (3 work units, ~600 LOC net).

## WU1 — handler module + HandlerContext accessor (~250 LOC)

**Goal**: Build the dispatch glue without yet touching `rmcp_adapter.rs`.

### Files
- `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs` (NEW, ≤ 200 LOC)
  - 6 input structs: `CfgInput`, `DominatorsCfgInput`, `SliceForwardInput`,
    `SliceBackwardInput`, `TaintFlowInput`, `InterprocSummaryInput`.
  - 6 handler fns: `handle_cfg`, `handle_dominators_cfg`, `handle_slice_forward`,
    `handle_slice_backward`, `handle_taint_flow`, `handle_interproc_summary`.
  - Each fn: parse `algorithm_params` (JSON object), build
    `ProgramAnalysisService`, call `dispatch`, serialize `RunOutput` via the
    same extractor used in `conformance::run_corpus` (extract JSON
    representation).
  - Interproc arm gated by `#[cfg(feature = "program-analysis-server")]`.
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` (MODIFY, +6 lines)
  - `pub mod program_analysis_handlers;`
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` (MODIFY, +15 LOC)
  - Add `HandlerContext::analytics(&self) -> ProgramAnalysisService` accessor
    (lazy-construct default).

### Tests
- Unit tests in `program_analysis_handlers.rs`:
  - One happy-path test per handler with the canonical corpus fixture.
  - One error-path test: malformed `algorithm_params` returns `Err(String)`.

## WU2 — register 6 tools + dispatch arms (~150 LOC)

**Goal**: Wire `build_all_tools` + `call_tool_handler` to call the new module.

### Files
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs`
  - `build_all_tools()`: append 6 `Tool::new(...)` entries with the input
    schema `{ "algorithm_params": { "type": "object" }, "limits": { "type": "object", "required": false } }`
    and `.with_meta(cognicode_meta("experimental", "analytics", false, false, 5000))`.
  - `call_tool_handler()`: add 6 match arms — each parses `{ algorithm_params, limits }`,
    calls the corresponding `crate::interface::mcp::handlers::program_analysis_handlers::*`,
    serializes the returned JSON text.
  - Interproc arm gated by `#[cfg(feature = "program-analysis-server")]`.

### Tests
- `wc -l crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` < 2500 (CI guard).

## WU3 — in-process MCP round-trip integration test (~150 LOC)

**Goal**: Real acceptance — boot the rmcp server in-process, exercise
`tools/list` + `tools/call` over the actual MCP wire format, assert
digest equality with the conformance harness.

### Files
- `crates/cognicode-core/tests/mcp_m5_tools.rs` (NEW, ~150 LOC) — or extend
  `interface/mcp/mcp_roundtrip_tests.rs` if the existing harness supports it.
  - Spin up MCP server via the existing `ServiceExt::serve` machinery.
  - Call `tools/list` with cursor pagination until all pages collected.
  - Assert all 6 M5 ids present.
  - For each of `cfg_per_function`, `taint_flow`, `interproc_summary`:
    invoke `tools/call` with the conformance fixture's `params`, compute
    SHA-256 of the returned text body, assert it matches the conformance
    digest stored in the test (recompute via the same fixture against the
    service to get the expected digest).
  - Negative test: `tools/call nonsense_tool` returns error result.

### Tests
- One integration test per assertion above (split for clarity).

## Acceptance gate (across all 3 WUs)

- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → 22 still green
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp` → all green
- `cargo test -p cognicode-core --features program-analysis-server --test mcp_m5_tools` → all green
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0
- `wc -l crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` < 2500
- grep `tokio`/`sqlx`/`reqwest` under `domain/analytics/program_analysis/**` → 0 matches

## Commit chain

```
<sha1> WU1 handler module + HandlerContext accessor
<sha2> WU2 register 6 tools + dispatch arms
<sha3> WU3 in-process MCP round-trip integration test + docs(m5.2) change artifacts
```

All three land as stacked-to-main commits.
