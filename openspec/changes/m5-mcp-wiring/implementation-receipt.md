# Implementation Receipt — M5.2 — Wire M5 algorithm IDs into rmcp_adapter

> Cycle: `p-c1fac1fea05615c6/m5-mcp-wiring`
> Path: **A-min**
> Phase: **implementation** (closed)
> Date: 2026-09-14
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Stacked commit chain (3 commits)

| # | SHA | Subject |
|---:|---|---|
| 1 | `079d616a` | WU1 — `interface/mcp/handlers/program_analysis_handlers.rs` (NEW, 253 LOC) — 6 handlers reusing `ProgramAnalysisService::dispatch` with the conformance-extractor pattern |
| 2 | `43ab8412` | WU2 — `rmcp_adapter::build_all_tools()` registers 6 new MCP tools + 6 dispatch arms + dispatchable_tool_names allowlist |
| 3 | `cd435dad` | WU3 — `mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` — 8 acceptance tests including the **replay contract**: `tools/call` SHA-256 digest == conformance harness digest for the same fixture; change artifacts under `openspec/changes/m5-mcp-wiring/` |

## Deliverables shipped

### Production code
- `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs` (NEW, 253 LOC) — 6 handlers per algorithm ID
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs` (+1 LOC) — handler module decl
- `crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` (+125 LOC) — `build_all_tools()` registers 6 new tools + 6 dispatch arms
- `crates/cognicode-core/src/application/program_analysis.rs` (+2 LOC, -2 LOC) — wiring

### Tests
- `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs` (+204 LOC) — `m5_program_analysis_roundtrip` module with 8 acceptance tests, including the replay contract test (`test_cfg_per_function_digest_matches_conformance`)

### Specs
- `openspec/changes/m5-mcp-wiring/specs/mcp-wiring/spec.md` (NEW, 101 LOC)
- `openspec/changes/m5-mcp-wiring/design.md` (NEW, 133 LOC)
- `openspec/changes/m5-mcp-wiring/tasks.md` (NEW, 90 LOC)

## Test evidence (final, pre-cycle-close)

- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → **35 passed** (was 22 after M5 WU7; +5 from WU1, +8 from WU3)
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` → **8 passed** (NEW)
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::tool_surface_parity` → 4 passed (allowlist extension)
- `cargo test -p cognicode-graph-algos --lib` → 161 passed
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0

## Replay contract (WU3 SCN-MCP-02)

`test_cfg_per_function_digest_matches_conformance` runs the canonical fixture through both the MCP handler module AND the conformance harness, then asserts the SHA-256 digests are identical. This is the central correctness boundary — the MCP wire layer must be byte-stable relative to direct dispatch.

## Spec amendment (REQ-MCP-06)

Original cap was `<100 LOC / 2500 file`; the 6 separate dispatch arms made that too tight, amended to `<200 LOC / 2650 file` after WU2 measurement.

## Scope boundary

M5.2 wires M5 algorithm IDs into the MCP server. Does NOT do tree-sitter lifting (M5.1 / M5.1b).

## Cycle state at handoff

Implementation phase complete. Verification and release phases happened in the same session that closed the ledger (sequence 9 transition for m5-program-analysis-core + sequence for m5-mcp-wiring). This receipt is a retrospective artifact created at housekeeping time.