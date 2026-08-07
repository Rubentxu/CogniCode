# Tool: `brain_remove_space`

## Input Schema

```json
{
  "type": "object",
  "required": ["session_id", "space_id"],
  "properties": {
    "session_id": { "type": "string", "minLength": 1 },
    "space_id": { "type": "string", "minLength": 1 }
  }
}
```

## Output Schema (success)

```json
{
  "session_id": "string",
  "space_id": "string",
  "removed": "boolean",
  "space_count_after": "integer"
}
```

## Error Codes

| Code | Cause |
|------|-------|
| `missing_required_arg` | `session_id` or `space_id` missing/empty |
| `session_not_found` | Session does not exist |

`provenance.source` is `"brain-session"` on both success and error envelopes. `removed: false` is NOT an error — the call is idempotent.
