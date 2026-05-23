---
description: Turn rule dashboard feedback into regression fixtures and rule updates
agent: rule-orchestrator
---

Process rule feedback "$ARGUMENTS".

WORKFLOW:
1. Identify the affected rule, issue, or feedback item.
2. Update `rules/{batch}/state` with `feedback_open`.
3. Launch `rule-test-engineer` to create a minimal regression fixture.
4. Launch `rule-designer` if the rule strategy needs redesign.
5. Launch `rule-implementer` and `rule-benchmark-auditor` if a patch is needed.
6. Report the regression evidence and proposed fix path.

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Feedback: $ARGUMENTS
- Artifact store mode: engram
