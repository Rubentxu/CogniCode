---
name: rule-operational-registry
description: Use when updating rules/{batch}/state, lifecycle states, batch metrics, or rule processing status.
license: MIT
---

# Rule Operational Registry

## Compact rules

- `rules/{batch}/state` is the operational source of truth.
- Track three identities: `concept_id`, `candidate_id`, and `rule_id`.
- Preserve every attempt; never delete failed or blocked work silently.
- Valid results per phase: `success`, `failed`, `blocked`, `duplicate`.
- State transitions must be append-only with timestamp, agent, gate, reason, and
  artifact links.
- Recalculate batch metrics after every phase: discovered, normalized,
  duplicates, blocked, implemented, tested, benchmarked, approved, published,
  failed by type, and coverage added.
- Before saving, merge with previous state. Do not overwrite progress from other
  agents or earlier batches.
- Loss of registry continuity is CRITICAL and must stop the workflow.
