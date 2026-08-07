# brain_focus — Tool Spec

## Input Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string (UUIDv4) | yes | The session handle. |
| `node` | string \| null | yes | Entity token to focus on, or `null` to clear. Must be non-empty when a string. |

### Wire shape

```json
{ "session_id": "...", "node": "AuthService" }   // set
{ "session_id": "...", "node": null }            // clear
```

## Output Schema

`McpResultEnvelope<BrainFocusPayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string | Echoed. |
| `payload.focus_node` | string \| null | The new focus value. |
| `payload.last_activity` | i64 (epoch ms) | Refreshed timestamp. |

## Error Cases

| Condition | `error.code` |
|-----------|--------------|
| Missing `session_id` | `"missing_required_arg"` |
| Missing `node` field | `"missing_required_arg"` |
| `node` is an empty string | `"invalid_focus_node"` |
| Session not found | `"session_not_found"` |
| Session expired | `"session_expired"` |

## Scenarios

### Scenario: Setting focus updates the session

- GIVEN a session with `focus_node = None`
- WHEN `brain_focus(session_id = S, node = "AuthService")` is called
- THEN `payload.focus_node` MUST equal `Some("AuthService")`
- AND `payload.last_activity` MUST be > the previous value

### Scenario: Clearing focus is honored

- GIVEN a session with `focus_node = Some("Foo")`
- WHEN `brain_focus(session_id = S, node = null)` is called
- THEN `payload.focus_node` MUST equal `None`

### Scenario: Empty string focus is rejected

- GIVEN a session with `focus_node = None`
- WHEN `brain_focus(session_id = S, node = "")` is called
- THEN the response MUST be an error envelope with `error.code = "invalid_focus_node"`
- AND `focus_node` MUST remain `None`

### Scenario: Focus on unknown session is rejected

- GIVEN no session with `session_id = "bogus"` exists
- WHEN `brain_focus(session_id = "bogus", node = "X")` is called
- THEN the response MUST be an error envelope with `error.code = "session_not_found"`
