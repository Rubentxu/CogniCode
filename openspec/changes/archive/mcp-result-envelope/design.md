# Design: MCP Result Envelope

## Technical Approach

Replace the two serialization helpers (`ok`/`ok_direct`) with envelope-aware variants (`envelope_ok`/`envelope_ok_direct`) that wrap every successful tool payload in a uniform `McpResultEnvelope` JSON shape. The envelope carries stable metadata (tool_name, version, timestamp, provenance, suggested_follow_ups) alongside the typed payload. Error results bypass the envelope entirely.

## Architecture Decisions

### Decision: Envelope struct as deserialization schema, helpers produce raw JSON

**Choice**: `McpResultEnvelope<T>` derives `Serialize + Deserialize` but helpers construct JSON via `serde_json::json!` + `serde_json::to_value`, not by building the struct directly.
**Alternatives**: (1) Build struct then serialize — requires `T: Clone` bound, regresses from current `T: Serialize`. (2) Use Value-only, no struct — loses type-safe deserialization in tests.
**Rationale**: Decouples producer (helper) from consumer (tests). Struct is the schema contract; helpers produce matching JSON. No new trait bounds on T.

### Decision: ProvenanceMetadata as separate struct

**Choice**: Dedicated `ProvenanceMetadata` with `confidence: Option<f64>` and `source: Option<String>`.
**Alternatives**: Inline fields on envelope — bloats the generic struct with optional fields most tools won't use.
**Rationale**: Keeps envelope flat (6 fields). Provenance is `None` for all 17 tools initially; separate struct makes that clean.

### Decision: Confidence validation at ProvenanceMetadata construction

**Choice**: `ProvenanceMetadata::new()` returns `Result<Self, EnvelopeError>`. The helper `build_envelope` accepts `Option<ProvenanceMetadata>` (already validated or None).
**Alternatives**: Validate in `envelope_ok` — couples validation to serialization path.
**Rationale**: Validation is a property of the metadata, not the envelope.

### Decision: Errors bypass envelope

**Choice**: `Err` branch in `envelope_ok` calls `err(e.to_string())` unchanged. `envelope_ok_direct` has no error branch (raw value).
**Alternatives**: Wrap errors in error envelope — adds complexity for no current consumer.
**Rationale**: Spec explicitly states "no envelope is emitted on service-layer errors." Matches existing behavior.

### Decision: suggested_follow_ups always empty Vec

**Choice**: Hard-coded `vec![]` in `build_envelope`. `FollowUp` struct exists for schema stability.
**Alternatives**: Parameter on helper — adds noise for zero current usage.
**Rationale**: Spec marks this out-of-scope. Empty array (not null, not absent) is the contract.

## Data Flow

```
Dispatch arm (Ok path)
    │
    ├── ExplorerResult<T> ──→ envelope_ok(tool, &result, None)
    │                              │
    │                              ├── Ok(value) → to_value(value) + envelope JSON → CallToolResult::success
    │                              └── Err(e)   → err(e.to_string())
    │
    └── &T (raw) ──→ envelope_ok_direct(tool, &value, None)
                           │
                           └── to_value(value) + envelope JSON → CallToolResult::success
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/mcp.rs` | Modify | Add `McpResultEnvelope<T>`, `ProvenanceMetadata`, `FollowUp`, `EnvelopeError` structs (~40 lines). Add `envelope_ok`, `envelope_ok_direct`, `build_envelope` helpers (~40 lines). Remove `ok` and `ok_direct` (~25 lines). Update 17 dispatch arms (1 line each). Migrate ~28 tests to unwrap envelope. Add 9 new envelope RED-gate tests (~80 lines). |

Total: ~155 net lines added, 1 file changed.

## Interfaces / Contracts

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpResultEnvelope<T> {
    pub tool_name: String,
    pub version: String,           // env!("CARGO_PKG_VERSION") = "0.5.0"
    pub timestamp: String,         // RFC 3339 via chrono::Utc::now()
    pub provenance: Option<ProvenanceMetadata>,
    pub payload: T,
    pub suggested_follow_ups: Vec<FollowUp>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ProvenanceMetadata {
    pub confidence: Option<f64>,   // [0.0, 1.0]
    pub source: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FollowUp {
    pub tool: String,
    pub reason: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("confidence {0} out of range [0.0, 1.0]")]
    ConfidenceOutOfRange(f64),
}

// Construction helpers (private to mcp.rs)
fn envelope_ok<T: serde::Serialize>(
    tool_name: &str,
    result: &crate::ExplorerResult<T>,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult;

fn envelope_ok_direct<T: serde::Serialize>(
    tool_name: &str,
    value: &T,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult;

// Internal shared builder
fn build_envelope(
    tool_name: &str,
    payload: serde_json::Value,
    provenance: Option<ProvenanceMetadata>,
) -> CallToolResult;
```

Dispatch arm migration pattern (before/after):

```rust
// BEFORE (line 336):
ok(&result)
// AFTER:
envelope_ok(TOOL_OPEN_WORKSPACE, &result, None)

// BEFORE (line 470):
ok_direct(&strings)
// AFTER:
envelope_ok_direct(TOOL_IMPACT_RADIUS, &strings, None)
```

Test migration pattern (before/after):

```rust
// BEFORE:
let parsed: Vec<String> = serde_json::from_str(&text).expect("valid JSON array");
// AFTER:
let env: McpResultEnvelope<Vec<String>> = serde_json::from_str(&text).expect("valid envelope");
let parsed = env.payload;
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|-------------|----------|
| Unit | Envelope struct serialization, 6 fields present | 9 RED-gate tests from Domain 1 spec |
| Unit | Helper envelope_ok wraps Ok, passes Err through | Replace existing `ok_helper_*` tests |
| Unit | Helper envelope_ok_direct wraps raw values | Replace existing `ok_direct_*` tests |
| Unit | ProvenanceMetadata::new validates confidence | 2 tests (out-of-range, negative) |
| Integration | 17 dispatch arms emit correct tool_name | Per-tool payload assertion on envelope |
| Integration | ~28 existing tests unwrap envelope.payload | Mechanical migration, preserve assertions |

## TDD Sequence (RED-GREEN-REFACTOR)

**Batch 1 — Structs (RED → GREEN)**
1. Write `envelope_ok_success_wraps_payload` — references `McpResultEnvelope`, fails to compile.
2. Define structs + `ProvenanceMetadata::new`. GREEN.
3. Write remaining 8 envelope tests from Domain 1. All fail. Implement `build_envelope`. GREEN.

**Batch 2 — Helpers (RED → GREEN)**
4. Write `helper_envelope_ok_present`, `helper_envelope_ok_direct_present`. Fail.
5. Implement `envelope_ok`, `envelope_ok_direct`. GREEN.
6. Write `helper_ok_removed`, `helper_ok_direct_removed`. Remove old helpers. GREEN.

**Batch 3 — Dispatch (RED → GREEN)**
7. Write `dispatch_no_legacy_helpers` (grep for `ok(&` / `ok_direct(&`). Fails.
8. Update 17 dispatch arms. GREEN.

**Batch 4 — Test Migration (REFACTOR)**
9. Migrate ~28 tests to unwrap envelope. Run `cargo test -p cognicode-explorer`. All pass.

## Migration / Rollout

No migration required. Single atomic commit. Revert = restore `ok`/`ok_direct`, revert 17 arms, revert ~28 test unwraps.

## Open Questions

- [x] Should `McpResultEnvelope<T>` be pub beyond the crate? — Yes, per spec: "MUST be pub at module scope." Consumers outside the crate can use it for deserialization.
- [x] Should `envelope_ok`/`envelope_ok_direct` be pub? — No. Same visibility as current `ok`/`ok_direct` (private to mcp.rs). Only the struct needs to be pub.
