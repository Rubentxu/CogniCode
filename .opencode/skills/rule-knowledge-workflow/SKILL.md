---
name: rule-knowledge-workflow
description: Use when researching, importing, normalizing, or deduplicating public static-analysis rule knowledge for CogniCode rules.
license: MIT
---

# Rule Knowledge Workflow

## Compact rules

- Separate public knowledge from implementation. Facts, metadata, taxonomy, and
  descriptions are allowed; third-party implementation code is not copied.
- Every raw candidate needs URL, source name, version/commit if available,
  license, extraction date, and data kind.
- Normalize into `RuleKnowledge`: domain, category, languages, severity,
  precision target, detection strategy, tests, performance, Axiom mapping, and
  `agent_semantics`.
- Deduplicate by problem meaning, not by title. One `concept_id` may aggregate
  many external `candidate_id`s.
- Preserve false-positive conditions and exclusion logic as first-class data.
- Any uncertain source becomes `reference_only` or `legal_blocked` before design.
