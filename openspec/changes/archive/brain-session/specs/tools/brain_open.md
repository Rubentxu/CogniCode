# brain_open — Tool Spec

## Input Schema

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `workspace_id` | string | yes | — | Logical workspace scope. The handler does not resolve it; it is opaque to the session layer. |
| `ttl_seconds` | u32 | no | `1800` | Time-to-live in seconds. `0` means "no expiry" in Phase 2. |

### Wire shape (JSON-RPC `tools/call` arguments)

```json
{ "workspace_id": "ws-1", "ttl_seconds": 1800 }
```

## Output Schema

`McpResultEnvelope<BrainOpenPayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string (UUIDv4) | Stable handle for subsequent calls. |
| `payload.workspace_id` | string | Echoed from input. |
| `payload.created_at` | i64 (epoch ms) | Server-generated. |
| `payload.ttl` | u32 | Effective TTL after default application. |
| `payload.focus_node` | `null` | Always `null` on a fresh open. |
| `payload.history_len` | u32 | Always `0` on a fresh open. |

## Error Cases

| Condition | `error.code` | HTTP/MCP status |
|-----------|--------------|------------------|
| Missing `workspace_id` | `"missing_required_arg"` | error response |
| `workspace_id` is empty string | `"invalid_workspace_id"` | error response |
| `ttl_seconds` is negative or > `u32::MAX` | `"invalid_ttl"` | error response |

## Scenarios

See the master `brain-session/spec.md` for canonical scenarios. This file pins the input/output contract; behavior across the 6 tools is enforced by the central spec.

### Scenario: Open returns UUIDv4 session_id

- GIVEN `brain_open` is called with `workspace_id = "ws-1"`, `ttl_seconds = 60`
- WHEN the response is parsed
- THEN `payload.session_id` MUST be a string matching the UUIDv4 regex `^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$`
- AND `payload.ttl` MUST equal `60`
- AND `payload.focus_node` MUST be `null`
- AND `payload.history_len` MUST be `0`

### Scenario: Default ttl is 1800

- GIVEN `brain_open` is called with only `workspace_id`
- WHEN the response is parsed
- THEN `payload.ttl` MUST equal `1800`

### Scenario: Open evicts expired sessions

- GIVEN two expired sessions exist in the registry
- WHEN `brain_open` is called
- THEN the registry MUST contain exactly one session after the call (the new one)
- AND the count of evicted sessions MUST be `2` (surfaced for testability, optional in response)
