---
name: rule-legal-provenance
description: Use when auditing license, provenance, source snapshots, and legal gates for generated CogniCode rules.
license: MIT
---

# Rule Legal Provenance

## Compact rules

- Record source, URL, license, version/commit, hash when available, date, and
  data kind for every candidate.
- Classify data as `metadata`, `documentation`, `example`, `pattern`,
  `test_fixture`, or `implementation_reference`.
- Do not derive CogniCode implementation from proprietary or unclear code.
- `reference_only` means the idea can inspire a new design from first principles;
  it does not authorize copying structure or code.
- Block design/implementation when license is missing, incompatible, or unclear.
- Legal gate output must include decision, rationale, reviewer/agent, and next
  transition for each candidate.
