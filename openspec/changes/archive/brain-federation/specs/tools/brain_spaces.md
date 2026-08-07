# Tool: `brain_spaces`

## Input Schema

```json
{
  "type": "object",
  "required": ["session_id"],
  "properties": {
    "session_id": { "type": "string", "minLength": 1 }
  }
}
```

## Output Schema (success)

```json
{
  "session_id": "string",
  "spaces": [
    {
      "id": "string",
      "name": "string",
      "kind": "Repo | Docs | Issues",
      "source_path": "string | null",
      "config": "object"
    }
  ],
  "space_count": "integer",
  "merge_candidates": [
    {
      "left": { "federated_id": "string", "label": "string", "kind": "string" },
      "right": { "federated_id": "string", "label": "string", "kind": "string" },
      "confidence": "number (0.0..=1.0)",
      "reasons": ["LabelMatch", "KindMatch", "PropertyOverlap"]
    }
  ]
}
```

## Error Codes

| Code | Cause |
|------|-------|
| `missing_required_arg` | `session_id` missing/empty |
| `session_not_found` | Session does not exist |

`provenance.source` is `"brain-session"`. `merge_candidates` is recomputed on every call (lazy, not cached).
