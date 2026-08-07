# brain_status — Tool Spec

## Input Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string (UUIDv4) | yes | The session handle. |

### Wire shape

```json
{ "session_id": "550e8400-..." }
```

## Output Schema

`McpResultEnvelope<BrainStatusPayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string | Echoed. |
| `payload.workspace_id` | string | The session's workspace. |
| `payload.created_at` | i64 (epoch ms) | Original creation timestamp. |
| `payload.last_activity` | i64 (epoch ms) | Refreshed to `now` by this call. |
| `payload.ttl` | u32 | Configured TTL. |
| `payload.focus_node` | string \| null | Current focus. |
| `payload.history_len` | u32 | Current history length. |
| `payload.history` | array of `HistoryEntry` | All history entries, oldest first. Each entry: `{ question: string, answer_summary: string, pattern_id: u8, ts: i64 }`. |

## Error Cases

| Condition | `error.code` |
|-----------|--------------|
| Missing `session_id` | `"missing_required_arg"` |
| Session not found | `"session_not_found"` |
| Session expired | `"session_expired"` |

## Scenarios

### Scenario: Status returns full state including history

- GIVEN a session with 2 history entries
- WHEN `brain_status(session_id = S)` is called
- THEN `payload.history_len` MUST equal `2`
- AND `payload.history` MUST be a 2-element array
- AND `payload.history[0]` MUST be the oldest entry (FIFO order)

### Scenario: Empty history is empty array, not null

- GIVEN a fresh session
- WHEN `brain_status(session_id = S)` is called
- THEN the JSON `history` field MUST be `[]`
- AND `history_len` MUST be `0`

### Scenario: Status refreshes last_activity

- GIVEN a session with `last_activity = T0`
- WHEN `brain_status` is called at `T1 > T0`
- THEN the session's `last_activity` MUST be `T1` afterward
- AND a subsequent `brain_attach` MUST report `last_activity = T1`

### Scenario: Status on unknown session is rejected

- GIVEN no session with `session_id = "bogus"` exists
- WHEN `brain_status(session_id = "bogus")` is called
- THEN the response MUST be an error envelope with `error.code = "session_not_found"`
