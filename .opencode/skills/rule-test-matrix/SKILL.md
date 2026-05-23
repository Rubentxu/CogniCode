---
name: rule-test-matrix
description: Use when designing or writing fixtures and tests for CogniCode quality/security rules.
license: MIT
---

# Rule Test Matrix

## Compact rules

- Create tests before implementation whenever possible.
- Minimum matrix per rule: positives, negatives, edge cases, false-positive
  guards, and performance fixture.
- Negatives must include comments, strings, identifiers, safe APIs, generated
  code, and mitigated context when relevant.
- Edge cases include empty files, partial syntax, macros, nested constructs, and
  test-only code.
- Tests must assert metadata where valuable: rule id, message, range, entity,
  scope, remediation, and agent semantics.
- Every dashboard false positive/negative becomes a minimal regression fixture.
