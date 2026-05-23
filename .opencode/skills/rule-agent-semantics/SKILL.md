---
name: rule-agent-semantics
description: Use when adding LLM/agent-oriented metadata, fix playbooks, review questions, or semantic chunks to CogniCode rules.
license: MIT
---

# Rule Agent Semantics

## Compact rules

- Every rule should help an AI agent answer: what is wrong, why it matters, where
  it is, how to verify, and how to fix safely.
- Include `agent_semantics`: summary, reasoning model, agent actions, safe
  autofix flag, review questions, and fix strategy.
- Include examples through `bad_example()` and `good_example()` when possible.
- `Issue` should include snippet, affected entity, scope, variable name, and
  remediation when available.
- Generate RAG-friendly chunks: explanation, false-positive model, fix playbook,
  references, and examples.
- If autofix is unsafe, say why and provide a guided refactor checklist instead.
