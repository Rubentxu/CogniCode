---
description: Start a new CogniCode rules workflow batch from a topic or source
agent: rule-orchestrator
---

Start a new rules workflow batch named "$ARGUMENTS".

WORKFLOW:
1. Ensure `rules/{$ARGUMENTS}/state` registry exists in Engram.
2. Launch `rule-knowledge-researcher` for public knowledge and provenance.
3. Launch `rule-concept-normalizer` to create/deduplicate `RuleKnowledge`.
4. Launch `rule-legal-auditor` - REQUIRED before any design work.
5. Launch `rule-designer` ONLY for candidates with legal approved status.
6. Present summary, blocked items, successful candidates, and next step.

GATES:
- Design gate: concept normalized + provenance registered + **legal approved**
- No designer launch until legal-auditor marks candidates as approved

CONTEXT:
- Working directory: !`echo -n "$(pwd)"`
- Current project: !`echo -n "$(basename $(pwd))"`
- Batch/topic: $ARGUMENTS
- Artifact store mode: engram
- Topic keys prefix: rules/{$ARGUMENTS}/

Read the rule orchestrator prompt. Do NOT execute phase work inline; delegate to
rule subagents and keep `rules/{$ARGUMENTS}/state` current.
