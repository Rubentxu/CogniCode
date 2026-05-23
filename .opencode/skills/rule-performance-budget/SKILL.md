---
name: rule-performance-budget
description: Use when designing, implementing, or benchmarking CogniCode rules with layer, preflight, and per-file budgets.
license: MIT
---

# Rule Performance Budget

## Compact rules

- Reuse parsed AST and shared `RuleContext`; never reparse per rule.
- Use `required_keywords()` for cheap preflight when possible.
- Set `layer()` honestly: cheap local rules first, expensive project/data-flow
  rules later.
- Compile queries/regex once with `OnceLock` or `LazyLock`.
- Initial budgets per file: regex/token 1 ms, AST query 3 ms, visitor 5 ms,
  metric 2 ms, call graph 10 ms, taint/data-flow 25 ms.
- Stop or redesign on repeated full traversals, unbounded interprocedural search,
  or benchmark regression.
- Benchmark report must include measured time, budget, delta, skipped files, and
  recommendation per rule.
