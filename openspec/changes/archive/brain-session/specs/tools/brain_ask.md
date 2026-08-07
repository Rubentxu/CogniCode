# brain_ask — Tool Spec

## Input Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `session_id` | string (UUIDv4) | yes | The session handle. |
| `question` | string | yes, non-empty | The natural-language question. |

### Wire shape

```json
{ "session_id": "550e8400-...", "question": "what does it call?" }
```

## Output Schema

`McpResultEnvelope<BrainAskPayload>` with `provenance.source = Some("brain-session")`.

| Field | Type | Description |
|-------|------|-------------|
| `payload.session_id` | string | Echoed. |
| `payload.ask_envelope` | object | The inner `McpResultEnvelope<Value>` produced by the ask-router. Its `provenance.source` is `"ask-router"`. |
| `payload.focus_injected` | string \| null | The focus node actually injected, or `null` if none. |
| `payload.enriched_question` | string | The question after focus-node prepending, exactly as dispatched. |
| `payload.pattern_id` | u8 \| null | The classified pattern (1..=8), or `null` if classification failed. |
| `payload.history_len_after` | u32 | Session history length after this call. |

## Error Cases

| Condition | `error.code` |
|-----------|--------------|
| Missing `session_id` | `"missing_required_arg"` |
| Missing or empty `question` | `"missing_required_arg"` |
| Session not found | `"session_not_found"` |
| Session expired | `"session_expired"` |
| Inner ask-router error | Inner envelope's `provenance.confidence = 0.0`; outer envelope returns the inner verbatim under `ask_envelope`. The outer call still returns HTTP 200 (envelope carries the error). |

## Scenarios

### Scenario: Ask appends history entry on success

- GIVEN a session with empty history
- WHEN `brain_ask(session_id = S, question = "what does `foo` call?")` returns a non-error inner envelope
- THEN `history.len()` MUST equal `1`
- AND `history[0].question` MUST contain `foo`
- AND `history[0].pattern_id` MUST be in `1..=8`

### Scenario: Focus node prepended to question

- GIVEN a session with `focus_node = Some("AuthService")`
- WHEN `brain_ask(session_id = S, question = "what does it call?")` is called
- THEN `payload.enriched_question` MUST start with a backtick-quoted `AuthService` token
- AND the inner envelope's `payload.primary_result` MUST reflect a search/lookup rooted at `AuthService`

### Scenario: No focus leaves question unchanged

- GIVEN a session with `focus_node = None`
- WHEN `brain_ask(session_id = S, question = "explain `User`")` is called
- THEN `payload.enriched_question` MUST equal `"explain `User`"` byte-for-byte

### Scenario: Inner ask_envelope preserves ask-router provenance

- GIVEN a session exists
- WHEN `brain_ask` returns successfully
- THEN `payload.ask_envelope.provenance.source` MUST equal `"ask-router"`
- AND `payload.ask_envelope.provenance.confidence` MUST be in `[0.0, 1.0]`

### Scenario: Ask on unknown session returns error and does not append

- GIVEN no session with `session_id = "bogus"` exists
- WHEN `brain_ask(session_id = "bogus", question = "...")` is called
- THEN the response MUST be an error envelope with `error.code = "session_not_found"`
- AND the global registry state MUST be unchanged
- AND no new session is created as a side effect
