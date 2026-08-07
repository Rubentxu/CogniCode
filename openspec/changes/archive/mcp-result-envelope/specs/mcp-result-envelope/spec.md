# mcp-result-envelope Specification

## Purpose

Standardize the wire-level shape of every MCP tool result. Every successful
`tools/call` MUST serialize as a single `McpResultEnvelope<T>` whose envelope
fields are stable across all 17 tools, allowing consumers to parse one outer
schema and dispatch to per-tool payload inspectors.

## Requirements

### Requirement: McpResultEnvelope struct shape

The system MUST define `McpResultEnvelope<T>` as a generic, `Serialize`-derived
struct with exactly six fields: `tool_name: String`, `version: String`,
`timestamp: String` (RFC 3339), `provenance: Option<ProvenanceMetadata>`,
`payload: T`, `suggested_follow_ups: Vec<FollowUp>`.

`ProvenanceMetadata` MUST carry `confidence: Option<f64>` (range `[0.0, 1.0]`)
and `source: Option<String>`. `FollowUp` MUST carry `tool: String` and
`reason: String`. Both MUST be `Serialize + Deserialize + Clone + Debug + Default`.

The struct MUST be defined at module scope in
`crates/cognicode-explorer/src/mcp.rs` and MUST be `pub`.

#### Scenario: Envelope serializes with all six fields

- GIVEN a value `payload: WorkspaceSummary` and a `tool_name = "explorer_open_workspace"`
- WHEN `envelope_ok("explorer_open_workspace", &Ok(payload), None)` is called
- THEN the resulting JSON object has keys `tool_name`, `version`, `timestamp`, `provenance`, `payload`, `suggested_follow_ups` (six, in any order)
- AND `payload` is the original `WorkspaceSummary` value verbatim

#### Scenario: Empty provenance serializes as JSON null

- GIVEN a caller passes `provenance = None`
- WHEN the envelope is serialized
- THEN the `provenance` field appears as JSON `null` (not the string `"None"`, not the string `"null"`, not absent)
- AND the field is still present in the object

#### Scenario: Suggested follow-ups default to empty array

- GIVEN a caller does not pass `suggested_follow_ups`
- WHEN `envelope_ok` constructs the envelope
- THEN `suggested_follow_ups` serializes as `[]` (empty array, never `null`, never absent)

#### Scenario: Provenance confidence out of range is rejected

- GIVEN a caller constructs `ProvenanceMetadata { confidence: Some(1.5), source: None }`
- WHEN the value is built via the constructor
- THEN the constructor returns `Err(EnvelopeError::ConfidenceOutOfRange)` (or equivalent named error)
- AND the envelope is NOT produced

### Requirement: Version field sourced from CARGO_PKG_VERSION

The `version` field MUST be populated from `env!("CARGO_PKG_VERSION")` at
envelope-construction time, MUST be a valid semver string, and MUST match the
package version of `cognicode-explorer` at compile time.

#### Scenario: Version matches package manifest

- GIVEN `Cargo.toml` of `cognicode-explorer` declares `version = "0.7.0"`
- WHEN any `envelope_ok` call serializes its result
- THEN the resulting JSON has `version: "0.7.0"` (verbatim, no prefix, no suffix)

### Requirement: Timestamp formatted as RFC 3339

The `timestamp` field MUST be the current UTC time at envelope construction,
formatted as RFC 3339, and MUST be generated via `chrono::Utc::now()`.

#### Scenario: Timestamp parses as RFC 3339

- GIVEN an envelope serialized to JSON text
- WHEN the text is deserialized by `chrono::DateTime::parse_from_rfc3339` on the `timestamp` field
- THEN the parse succeeds
- AND the parsed time is within ±2 seconds of the system clock at envelope construction

#### Scenario: Timestamp uses UTC suffix Z

- GIVEN an envelope serialized at any wall-clock time
- WHEN the JSON is inspected
- THEN `timestamp` ends with the character `Z`

### Requirement: Construction helper envelope_ok

The system MUST provide a single helper
`envelope_ok<T: Serialize>(tool_name: &str, result: &ExplorerResult<T>, provenance: Option<ProvenanceMetadata>) -> CallToolResult`
that wraps the inner value in the envelope. On `Ok(value)`, it MUST build the
envelope and return a success result. On `Err(e)`, it MUST return the existing
`err(e.to_string())` path — no envelope is emitted on service-layer errors.

#### Scenario: Successful service result wraps payload in envelope

- GIVEN `service.spotter_search(...)` returns `Ok(vec_of_results)`
- WHEN the dispatch arm calls `envelope_ok(TOOL_SPOTTER_SEARCH, &Ok(vec_of_results), None)`
- THEN the returned `CallToolResult` is a success result
- AND its `Content::text` JSON has the envelope outer shape
- AND `payload` equals the original `vec_of_results`

#### Scenario: Service error returns bare error result, not envelope

- GIVEN `service.spotter_search(...)` returns `Err(ExplorerError::WorkspaceClosed)`
- WHEN the dispatch arm calls `envelope_ok(TOOL_SPOTTER_SEARCH, &Err(ExplorerError::WorkspaceClosed), None)`
- THEN the returned `CallToolResult` is an error result
- AND no envelope struct is constructed or serialized

### Requirement: Envelope for raw Serialize values

The system MUST provide
`envelope_ok_direct<T: Serialize>(tool_name: &str, value: &T, provenance: Option<ProvenanceMetadata>) -> CallToolResult`
that wraps a raw `Serialize` value (not wrapped in `ExplorerResult`) in the
same envelope. The five `impact_*` tools MUST use it.

#### Scenario: Raw value wraps in envelope

- GIVEN `let ids: Vec<String> = svc.impact_radius(...)`
- WHEN the dispatch arm calls `envelope_ok_direct(TOOL_IMPACT_RADIUS, &ids, None)`
- THEN the success `Content::text` JSON has the envelope outer shape
- AND `payload` is the `Vec<String>`

### Requirement: Backwards-compatible detection via version

A consumer MUST be able to detect whether a JSON response uses the envelope
by inspecting the presence of the `version` field at the top level. The
envelope MUST be additive — never remove or rename the six top-level fields
without a major version bump in `version`.

#### Scenario: Bare payload is distinguishable from envelope

- GIVEN a JSON object missing the `version` field
- WHEN a consumer checks `obj.get("version").is_some()`
- THEN it returns `false` (legacy bare payload)
- AND a JSON object with `"version": "0.7.0"` returns `true` (envelope)

#### Scenario: All 17 tools emit the same envelope outer shape

- GIVEN the full `TOOL_NAMES` list of 17 entries
- WHEN each tool is dispatched with valid arguments and the service returns `Ok`
- THEN every emitted JSON shares the exact same set of top-level keys
- AND `tool_name` matches the constant used at the dispatch arm

## TDD RED Gate

Before implementation lands, the following tests MUST exist and MUST fail:

1. `envelope_ok_success_wraps_payload`
2. `envelope_ok_provenance_none_serializes_as_null`
3. `envelope_ok_follow_ups_default_empty`
4. `envelope_ok_timestamp_rfc3339_utc`
5. `envelope_ok_version_matches_pkg`
6. `envelope_ok_err_returns_error_result`
7. `envelope_ok_direct_raw_value`
8. `envelope_ok_provenance_confidence_out_of_range`
9. `envelope_version_field_detects_wrapper`

## Edge Cases

| Edge | Expected Behavior |
|------|-------------------|
| Empty payload (`Vec::new()`) | `payload: []`; envelope fields still present |
| `provenance = None` | `provenance: null` in JSON (key present, value null) |
| `suggested_follow_ups` not provided | Defaults to `[]` |
| Large payload (>1 MB) | `to_string_pretty`; no truncation in helper (out of scope) |
| Service-layer error | Envelope NOT constructed; `err()` is called |
| `confidence` boundary `0.0` / `1.0` | Accepted; only values outside `[0.0, 1.0]` rejected |
| Negative `confidence` | Rejected with named error |

## Out of Scope

- Wire-level protocol negotiation
- `version` field auto-bumping on field additions
- `suggested_follow_ups` populated with real values (always empty for now)
- Compression of large payloads
- Localization of strings
- `chrono` replacement
