# Delta for mcp-handler-helpers

## MODIFIED Requirements

### Requirement: ok() and ok_direct() removed

The two existing helpers
`fn ok<T: serde::Serialize>(result: &crate::ExplorerResult<T>) -> CallToolResult`
and
`fn ok_direct<T: serde::Serialize>(value: &T) -> CallToolResult`
MUST be removed from `crates/cognicode-explorer/src/mcp.rs` after this change.

(Previously: both helpers existed; the 8 explorer tools called `ok(&result)` and the 6 impact / 3 graph tools called `ok_direct(&value)`.)

#### Scenario: ok() is no longer callable

- GIVEN `ok()` has been removed
- WHEN any code (production or test) calls `ok(&result)`
- THEN the Rust compiler emits an unresolved-name error
- AND the build fails

#### Scenario: ok_direct() is no longer callable

- GIVEN `ok_direct()` has been removed
- WHEN any code (production or test) calls `ok_direct(&value)`
- THEN the Rust compiler emits an unresolved-name error
- AND the build fails

### Requirement: envelope_ok() takes tool_name and optional provenance

`envelope_ok<T: Serialize>(tool_name: &str, result: &ExplorerResult<T>, provenance: Option<ProvenanceMetadata>) -> CallToolResult`
MUST be the only success-path helper for `ExplorerResult`-returning services.

(Previously: `ok<T>(result: &ExplorerResult<T>) -> CallToolResult` took a single argument; no `tool_name`, no `provenance`.)

#### Scenario: envelope_ok signature includes tool_name

- GIVEN the new helper is defined
- WHEN the signature is inspected
- THEN the first parameter is `tool_name: &str`
- AND the third parameter is `provenance: Option<ProvenanceMetadata>`

#### Scenario: Existing dispatch arm updated to envelope_ok

- GIVEN the `TOOL_OPEN_WORKSPACE` arm previously ended with `ok(&result)`
- WHEN the change is applied
- THEN the same arm ends with `envelope_ok(TOOL_OPEN_WORKSPACE, &result, None)`
- AND no other lines in the arm change

### Requirement: envelope_ok_direct() takes tool_name and optional provenance

`envelope_ok_direct<T: Serialize>(tool_name: &str, value: &T, provenance: Option<ProvenanceMetadata>) -> CallToolResult`
MUST be the only success-path helper for raw `Serialize` payloads.

(Previously: `ok_direct<T>(value: &T) -> CallToolResult` took a single argument; no `tool_name`, no `provenance`.)

#### Scenario: envelope_ok_direct signature includes tool_name

- GIVEN the new helper is defined
- WHEN the signature is inspected
- THEN the first parameter is `tool_name: &str`
- AND the third parameter is `provenance: Option<ProvenanceMetadata>`

#### Scenario: impact_radius arm updated to envelope_ok_direct

- GIVEN the `TOOL_IMPACT_RADIUS` arm previously ended with `ok_direct(&strings)`
- WHEN the change is applied
- THEN the same arm ends with `envelope_ok_direct(TOOL_IMPACT_RADIUS, &strings, None)`
- AND no other lines in the arm change

### Requirement: err() helper unchanged

The `err(message: String) -> CallToolResult` helper MUST remain with the same
signature and behavior.

(Previously: `err()` existed alongside `ok()` and `ok_direct()`.)

#### Scenario: err() still callable

- GIVEN the change is applied
- WHEN the dispatch arms reference `err(format!("explorer_open_workspace: invalid args: {e}"))`
- THEN the compiler resolves `err` to the existing helper
- AND the error result is returned to the consumer

#### Scenario: require_graph() still callable

- GIVEN `require_graph()` is used by the 6 impact and 3 graph tools
- WHEN the change is applied
- THEN `require_graph` retains its current signature and behavior

### Requirement: All 17 dispatch arms updated

Every match arm in the dispatch function MUST use either `envelope_ok(...)` or
`envelope_ok_direct(...)`. No arm MUST retain a call to `ok()` or `ok_direct()`.

#### Scenario: Zero remaining ok() call sites in dispatch

- GIVEN the change is applied
- WHEN the dispatch function is grep'd for the literal `ok(&`
- THEN zero matches are found
- AND grep for `ok_direct(&` returns zero matches
- AND grep for `envelope_ok(` returns 8 matches
- AND grep for `envelope_ok_direct(` returns 9 matches

#### Scenario: One-line change per arm

- GIVEN a diff of the change
- WHEN each of the 17 arms is examined
- THEN the diff for each arm is exactly one changed line (the helper call)
- AND no other line in any arm is modified

### Requirement: Test migration is mechanical

The ~40 existing tests that call tools and assert on the success text MUST
deserialize the text as `McpResultEnvelope<ExpectedPayloadType>` (or as
`serde_json::Value` and assert on `.payload`) before asserting on payload
fields. The original payload assertions MUST be preserved verbatim.

(Previously: tests deserialized the text directly as `ExpectedPayloadType`.)

#### Scenario: Test deserializes envelope first

- GIVEN a test that previously did `let summary: WorkspaceSummary = serde_json::from_str(&text)?;`
- WHEN the change is applied
- THEN the test does `let env: McpResultEnvelope<WorkspaceSummary> = serde_json::from_str(&text)?;` followed by `let summary = env.payload;`
- AND the assertion lines that follow are byte-identical to the pre-change version

#### Scenario: Test still asserts on the same payload fields

- GIVEN a test that asserted on `summary.node_count`
- WHEN the change is applied
- THEN the assertion line is unchanged in content
- AND the assertion still passes against the same expected value

#### Scenario: Negative test (service error) unchanged

- GIVEN a test that asserts `result.is_error() == true` for a service error
- WHEN the change is applied
- THEN the test still asserts the same way — error results are NOT wrapped in envelopes

## TDD RED Gate

Before implementation, the following tests MUST fail to compile (proving the
old helpers are gone and the new ones are in place):

1. `helper_ok_removed_from_module` — references `ok::<WorkspaceSummary>(&result)`, asserts build fails
2. `helper_ok_direct_removed_from_module` — references `ok_direct::<Vec<String>>(&ids)`, asserts build fails
3. `helper_envelope_ok_present` — references `envelope_ok(TOOL_OPEN_WORKSPACE, &result, None)`, asserts it compiles
4. `helper_envelope_ok_direct_present` — references `envelope_ok_direct(TOOL_IMPACT_RADIUS, &ids, None)`, asserts it compiles
5. `dispatch_no_legacy_helpers` — runs `cargo build -p cognicode-explorer` and asserts success
6. `tests_compile_after_unwrap` — runs `cargo test -p cognicode-explorer --no-run` and asserts success

## Edge Cases

| Edge | Expected Behavior |
|------|-------------------|
| A test forgets to unwrap the envelope | Deserialization fails with a clear error |
| A test for an error result tries to unwrap the envelope | Error text is not JSON; deserialization fails — guard with `if !result.is_error()` |
| A test for `impact_*` deserializes as `ExplorerResult<...>` | Fails — payload is the raw value |
| A dispatch arm uses `envelope_ok` for a raw value | Compiles; produces envelope around `ExplorerResult`-shaped payload — caught by per-tool payload test |
| A new test added in the same change | MUST also unwrap the envelope (review-time check) |
| A test imports `McpResultEnvelope` from a wrong path | Compile error; tests in same module access directly; cross-crate use `cognicode_explorer::mcp::McpResultEnvelope` |

## Out of Scope

- The `require_graph()` helper (unchanged)
- The `err()` helper (unchanged)
- `build_tool_schemas()` (unchanged)
- The `TOOL_*` constants and `TOOL_NAMES` list (unchanged in count)
- Service-layer error variants
- Adding or removing any of the 17 tools
- Renaming any `TOOL_*` constant
- Changing the JSON-RPC `tools/list` schema for any tool
