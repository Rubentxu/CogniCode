---
description: Implement pending rules from an approved rules batch
agent: rule-orchestrator
---

Apply implementation tasks for rules batch "$ARGUMENTS".

WORKFLOW:
1. Read `rules/{$ARGUMENTS}/state`, `rule-designs`, `fixture-matrix`, and `tasks`.
2. Verify gates before implementation:
   - Legal approved
   - Fixtures defined
3. Launch `rule-implementer` for pending implementation tasks.
4. Launch `rule-test-engineer` for focused tests.
5. Launch `rule-benchmark-auditor` when tests pass.
6. Report implemented, failed, blocked, and benchmark status.

PARALLELISM:
- You may launch implementer and test-engineer in parallel if independent.
- Benchmark-auditor requires test results first.

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Batch: $ARGUMENTS
- Artifact store mode: engram
- Topic keys prefix: rules/{$ARGUMENTS}/

CRITICAL: Do not silently overwrite previous `apply-progress`; MERGE progress.
