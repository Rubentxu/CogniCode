# brain_attach — Tool Spec

## Input Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string (UUIDv4) | yes | The session handle returned by `brain_open` or `brain_attach`. |

### Wire shape

```json
{ "session_id": "550e8400-e29b-41d4-a716-446655440000" }
```

## Output Schema

`McpResultEnvelope<BrainAttachPayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string | Echoed from input. |
| `payload.workspace_id` | string | The session's workspace. |
| `payload.created_at` | i64 (epoch ms) | Original creation timestamp. |
| `payload.last_activity` | i64 (epoch ms) | Refreshed to `now` by this call. |
| `payload.ttl` | u32 | Configured TTL. |
| `payload.focus_node` | string \| null | Current focus. |
| `payload.history_len` | u32 | Current history length. |

## Error Cases

| Condition | `error.code` |
|-----------|--------------|
| Missing `session_id` | `"missing_required_arg"` |
| `session_id` not found | `"session_not_found"` |
| Session has expired (lazy TTL) | `"session_expired"` |

## Scenarios

### Scenario: Attach returns current state

- GIVEN a session with `focus_node = Some("X")` and 2 history entries
- WHEN `brain_attach(session_id = S)` is called
- THEN `payload.focus_node` MUST equal `Some("X")`
- AND `payload.history_len` MUST equal `2`
- AND `payload.last_activity` MUST be ≥ the previous value

### Scenario: Attach refreshes last_activity

- GIVEN a session with `last_activity = T0`
- WHEN `brain_attach` is called at time `T1 > T0`
- THEN the session's `last_activity` MUST be `T1`
- AND a subsequent `brain_ask` MUST see `last_activity == T1` at the start of its call

### Scenario: Attach to expired session evicts and errors

- GIVEN a session whose `last_activity + ttl < now`
- WHEN `brain_attach(session_id = S)` is called
- THEN the response MUST be an error envelope with `error.code = "session_expired"`
- AND the registry MUST NOT contain the session afterward
