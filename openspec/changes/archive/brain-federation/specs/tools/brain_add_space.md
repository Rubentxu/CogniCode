# Tool: `brain_add_space`

## Input Schema

```json
{
  "type": "object",
  "required": ["session_id", "space"],
  "properties": {
    "session_id": { "type": "string", "minLength": 1 },
    "space": {
      "type": "object",
      "required": ["id", "name", "kind"],
      "properties": {
        "id": { "type": "string", "minLength": 1, "pattern": "^[^:]+$" },
        "name": { "type": "string", "minLength": 1 },
        "kind": { "enum": ["Repo", "Docs", "Issues"] },
        "source_path": { "type": "string" },
        "config": { "type": "object" }
      }
    }
  }
}
```

## Output Schema (success)

```json
{
  "session_id": "string",
  "space_id": "string",
  "name": "string",
  "kind": "Repo | Docs | Issues",
  "space_count_after": "integer"
}
```

## Error Codes

| Code | Cause |
|------|-------|
| `missing_required_arg` | `session_id` or `space` missing/empty |
| `session_not_found` | Session does not exist |
| `invalid_space_id` | `space.id` is empty or contains `::` |
| `invalid_space_kind` | `space.kind` not in `Repo | Docs | Issues` |
| `duplicate_space` | Space id already registered in session |

`provenance.source` is `"brain-session"` on both success and error envelopes.
