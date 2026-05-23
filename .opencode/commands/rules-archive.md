---
description: Archive or publish a verified rules workflow batch
agent: rule-orchestrator
---

Archive rules batch "$ARGUMENTS".

WORKFLOW:
1. Read all artifacts under `rules/$ARGUMENTS/`.
2. Confirm all required gates are closed or explicitly waived.
3. Update registry states to `published`, `deprecated`, or `archived` as needed.
4. Produce `rules/$ARGUMENTS/archive-report` with metrics and entropy/quality trend.
5. Remind the user about ignored docs or files that require `git add -f`.

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Batch: $ARGUMENTS
- Artifact store mode: engram
