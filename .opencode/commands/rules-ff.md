---
description: Fast-forward rules planning through designs, fixtures, and tasks
agent: rule-orchestrator
---

Fast-forward planning for rules batch "$ARGUMENTS".

WORKFLOW:
1. Check existing artifacts under `rules/{$ARGUMENTS}/`.
2. Verify gates for each phase - do not skip blocked phases.
3. Run dependency-ready planning phases:
   - `rule-concept-normalizer` if concepts are missing.
   - `rule-legal-auditor` if legal review is missing.
   - `rule-designer` for viable candidates (requires legal approved).
   - `rule-test-engineer` to create fixture matrix.
4. Produce/update `rules/{$ARGUMENTS}/tasks` for implementation.
5. Present a combined summary after all planning phases.

DYNAMIC DECISIONS:
- You may repeat or skip phases based on entropy_score.
- If >50% rules fail a phase, repeat it.
- If <20% rules fail, continue.

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Batch: $ARGUMENTS
- Artifact store mode: engram
- Topic keys prefix: rules/{$ARGUMENTS}/

Keep `rules/{$ARGUMENTS}/state` as the operational registry. Merge progress,
do not overwrite.
