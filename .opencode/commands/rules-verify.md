---
description: Verify implemented rules against quality, metadata, SARIF, and agent gates
agent: rule-orchestrator
---

Verify rules batch "$ARGUMENTS".

WORKFLOW:
1. Read state, implementation progress, test report, and benchmark report.
2. Launch `rule-reviewer` for precision, metadata, SARIF, and agent semantics.
3. Launch `rule-commit-orchestrator` to prepare a commit plan for approved rules.
4. Present CRITICAL/WARNING/SUGGESTION findings and next actions.

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Batch: $ARGUMENTS
- Artifact store mode: engram

Do not create commits unless the user explicitly asks.
