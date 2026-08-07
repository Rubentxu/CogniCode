# brain_close — Tool Spec

## Input Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string (UUIDv4) | yes | The session handle. |

### Wire shape

```json
{ "session_id": "550e8400-..." }
```

## Output Schema

`McpResultEnvelope<BrainClosePayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string | Echoed. |
| `payload.closed` | bool | `true` if the session existed and was removed; `false` if it did not exist. |

## Error Cases

| Condition | `error.code` |
|-----------|--------------|
| Missing `session_id` | `"missing_required_arg"` |

`brain_close` is idempotent. Closing an unknown or already-closed session returns `{ closed: false }` with HTTP 200 — NOT an error envelope.

## Scenarios

### Scenario: Close removes the session

- GIVEN an open session with `session_id = S`
- WHEN `brain_close(session_id = S)` is called
- THEN `payload.closed` MUST be `true`
- AND `payload.session_id` MUST equal `S`
- AND a subsequent `brain_attach(session_id = S)` MUST return `"session_not_found"`

### Scenario: Close is idempotent

- GIVEN `brain_close(session_id = S)` has already been called once successfully
- WHEN `brain_close(session_id = S)` is called again
- THEN `payload.closed` MUST be `false`
- AND the response MUST NOT be an error envelope
- AND the response status MUST be 200 (envelope-level success)

### Scenario: Close of unknown session is not an error

- GIVEN no session with `session_id = "bogus"` exists
- WHEN `brain_close(session_id = "bogus")` is called
- THEN `payload.closed` MUST be `false`
- AND the response MUST NOT be an error envelope

### Scenario: Close does not affect other sessions

- GIVEN two open sessions S1 and S2
- WHEN `brain_close(session_id = S1)` is called
- THEN `payload.closed` MUST be `true` for S1
- AND `brain_attach(session_id = S2)` MUST still return a valid state
- AND S2's `last_activity` MUST NOT be modified by the close
