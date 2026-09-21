# Spec: M5.2 — M5 algorithm IDs as MCP tools

> Change: `m5-mcp-wiring` | Cycle: `p-c1fac1fea05615c6/m5-mcp-wiring` | Phase: specify
> Goal: expose the 6 M5 program-analysis algorithm IDs through the existing
> MCP server so any MCP client (cogh, IDE adapters, custom tooling) can call
> them over JSON-RPC without depending on the synthetic conformance harness.

## Requirements

### REQ-MCP-01 — All 6 M5 algorithm IDs are listed by `tools/list`

**Given** an MCP client connected to the CogniCode MCP server
**When** the client calls `tools/list`
**Then** the response includes tools for every M5 algorithm id:
`cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`,
`taint_flow`, `interproc_summary`
**And** each tool exposes an input JSON schema with `algorithm_params` (object) and
optional `limits` (object) so the canonical corpus params can flow through unchanged.

### REQ-MCP-02 — `tools/call` dispatches to `ProgramAnalysisService::dispatch`

**Given** any of the 6 M5 tool names
**When** the client invokes `tools/call` with `arguments = { algorithm_params: <object> }`
**Then** the server MUST call
`ProgramAnalysisService::dispatch(&algorithm_id, &algorithm_params, &limits)`
**And** on `Ok(RunOutput)` return a `Content::text` payload whose JSON serialization
is byte-identical to the conformance harness output for the same fixture
(replay contract).

### REQ-MCP-03 — Error path is structured, not a string panic

**Given** an M5 tool call where dispatch returns `Err(AnalyticsError)`
**When** the server responds
**Then** the response is `CallToolResult::error(vec![Content::text(...)])`
with the error string formatted via `AnalyticsError`'s `Display` impl.
**And** no tool call may panic in the dispatch path; all errors propagate.

### REQ-MCP-04 — Negative invariant: unknown algorithm id is rejected

**Given** `tools/call` with a tool name not in the registered list (or with
`algorithm_params` that the underlying descriptor rejects)
**When** the server responds
**Then** it returns `CallToolResult::error` with a message identifying the
unknown id or the validation failure.

### REQ-MCP-05 — Rate limit + timeout reuse existing category machinery

**Given** the new M5 tools
**When** the server resolves per-tool category
**Then** each M5 tool uses `lookup_category(tool_name)` (existing helper) and
inherits the existing timeout + rate-limit defaults — no new policy knobs in
this cycle.

### REQ-MCP-06 — Handler module extraction

**Given** rmcp_adapter.rs is currently 2430 LOC
**When** the 6 new dispatch arms are added
**Then** the new arms delegate to a `program_analysis_mcp_handlers` module
under `interface/mcp/handlers/`
**And** rmcp_adapter.rs net growth from this change is < 200 LOC (so the
file stays under 2650; the original 2500 estimate was too tight for 6 arms).

### REQ-MCP-07 — Service reachability via HandlerContext

**Given** the dispatch path needs a `ProgramAnalysisService`
**When** the handler is invoked
**Then** the handler MUST obtain the service via `HandlerContext`
(extend it with an `analytics()` accessor if not already present)
**And** no new dependency on `tokio`/`sqlx` may be introduced in
`domain/analytics/program_analysis`.

## Scenarios

| ID | Scenario | Requirement |
|---|---|---|
| SCN-MCP-01 | `tools/list` enumerates the 6 M5 tool ids | REQ-MCP-01 |
| SCN-MCP-02 | `tools/call cfg_per_function` returns the same digest as conformance corpus | REQ-MCP-02 |
| SCN-MCP-03 | `tools/call taint_flow` returns the same digest as conformance corpus | REQ-MCP-02 |
| SCN-MCP-04 | `tools/call interproc_summary` returns the same digest as conformance corpus | REQ-MCP-02 |
| SCN-MCP-05 | `tools/call bad_id` returns `CallToolResult::error`, no panic | REQ-MCP-03, REQ-MCP-04 |
| SCN-MCP-06 | Dispatch failure (e.g. bad params) returns structured error | REQ-MCP-03 |
| SCN-MCP-07 | Rate-limit and timeout apply identically to M5 and pre-existing tools | REQ-MCP-05 |
| SCN-MCP-08 | rmcp_adapter.rs net growth < 100 LOC | REQ-MCP-06 |
| SCN-MCP-09 | `domain/analytics/program_analysis/*` imports remain free of `tokio`/`sqlx`/`reqwest` | REQ-MCP-07 |

## Acceptance contract

A new integration test `crates/cognicode-core/tests/mcp_m5_tools.rs` (or
under `interface/mcp/mcp_roundtrip_tests.rs`) MUST:

1. Spin up the MCP server in-process via rmcp.
2. Call `tools/list` and assert all 6 ids present.
3. Call `tools/call cfg_per_function` with the linear-chain fixture and
   assert the returned text body's SHA-256 matches the conformance
   fixture's `digest` (so the dispatcher + MCP layer stays replay-deterministic).
4. Repeat for `taint_flow` and `interproc_summary`.
5. Call `tools/call nonsense_tool` and assert `CallToolResult::error` with
   no panic.

The unit tests already present in `application/program_analysis.rs` and
the conformance corpus MUST remain green.
