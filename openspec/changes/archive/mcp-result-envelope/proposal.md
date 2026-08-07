# Proposal: MCP Result Envelope

## Intent

17 MCP tools serialize results through 2 divergent paths (`ok` for `ExplorerResult<T>`, `ok_direct` for raw `Serialize`), producing bare JSON with no envelope metadata. Phase 2 roadmap requires every tool result to carry `tool_name`, `version`, `timestamp`, `provenance`, and `suggested_follow_ups` in a stable wrapper. Agents and the explorer both need a predictable outer shape to parse results without per-tool custom logic.

## Scope

### In Scope
- Define `McpResultEnvelope<T>` generic struct in `cognicode-explorer` with fields: `tool_name`, `version`, `timestamp`, `provenance`, `payload` (generic `T`), `suggested_follow_ups`
- Replace `ok()` and `ok_direct()` helpers with single `envelope_ok()` wrapping any `Serialize` value
- Update all 17 dispatch arms to call `envelope_ok(tool_name, &result)`
- Update ~40 tests to deserialize envelope first, then assert `payload`
- Populate `provenance` from existing per-entity metadata where available (e.g., `confidence` on edges)

### Out of Scope
- Tool signatures, arg structs, dispatch match arms — unchanged
- DTO struct definitions (`WorkspaceSummary`, `LensResult`, impact DTOs) — unchanged
- Projection layer, service layer — unchanged
- `build_tool_schemas()` — unchanged
- Consumer migration tooling (deferred to design phase)
- Seeding `suggested_follow_ups` with actual values (empty array for now)

## Capabilities

### New Capabilities
- `mcp-result-envelope`: Standardized result wrapper for all 17 MCP tools carrying `tool_name`, `version`, `timestamp`, `provenance`, `payload`, `suggested_follow_ups`. Consumers parse envelope once, then inspect `payload` by tool.

### Modified Capabilities
None. Serialization-layer wrapper; no spec-level behavior changes to existing tools.

## Approach

Define `McpResultEnvelope<T>` as `#[derive(Serialize)]` in `mcp.rs`. Replace both `ok()` and `ok_direct()` with a single `envelope_ok<T: Serialize>(tool_name: &str, result: &ExplorerResult<T>)` that wraps the payload in the envelope struct. Populate `version` from `env!("CARGO_PKG_VERSION")`, `timestamp` from `chrono::Utc::now()`, `provenance` from optional `ProvenanceMetadata { confidence: Option<f64>, source: Option<String> }` passed alongside, and `suggested_follow_ups` as empty `Vec`.

Migration: incremental. Deploy `McpResultEnvelope<T>` with a `version` field so consumers detect the wrapper. The 17 dispatch arms change one line each: `ok(&result)` / `ok_direct(&value)` → `envelope_ok(TOOL_NAME, &result)`. ~40 tests change one line each: deserialize `McpResultEnvelope<ExpectedType>` and assert `.payload`. No consumer-visible regression — bare payload was never versioned.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/mcp.rs` (helpers) | Modified | Add `McpResultEnvelope<T>`, replace `ok()`/`ok_direct()` with `envelope_ok()` |
| `crates/cognicode-explorer/src/mcp.rs` (dispatch) | Modified | 17 dispatch arms: one-line helper call change each |
| `crates/cognicode-explorer/src/mcp.rs` (tests) | Modified | ~40 tests: unwrap envelope before asserting payload |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| MCP consumers (Claude Desktop, etc.) fail to parse nested envelope | Low | Envelope is additive — consumers that ignore unknown JSON fields still find `payload` at a predictable path; `version` field enables backwards-compatible detection |
| Test migration introduces assertion bugs | Medium | Each test unwrap is mechanical (`serde_json::from_str::<McpResultEnvelope<T>>(&text).payload`); no logic changes; existing assertions preserved verbatim |
| `provenance` under-populated for tools without native confidence | Low | Default `provenance: None`; Phase 2 roadmap already plans confidence propagation to all edges |

## Rollback Plan

Revert the commit. `ok()` and `ok_direct()` helpers are replaced, not removed from history. If rollback is needed, restore the two helper functions and revert dispatch arms to original calls. No database migration, no schema change, no consumer-visible breakage (bare payload never guaranteed a stable shape).

## Dependencies

None. `McpResultEnvelope<T>` lives in `cognicode-explorer` with no new crate dependencies. `chrono` is already in the workspace dependency tree.

## Success Criteria

- [ ] All 17 tools serialize results wrapped in `McpResultEnvelope<T>`
- [ ] Every envelope carries `tool_name`, `version`, `timestamp`
- [ ] All existing tests pass after mechanical unwrap update
- [ ] `cargo test -p cognicode-explorer` passes with 0 regressions
- [ ] No change to DTO struct definitions or service method signatures
