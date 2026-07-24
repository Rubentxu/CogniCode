# ADR-011: Architecture decision support through explainable packs

**Status**: ACCEPTED  
**Date**: 2026-07-22  
**Accepted**: 2026-07-24 (SDDK e25-decision-support-packs design)  
**Deciders**: User, OpenCode orchestrator  

## Context

Most AI architecture tools either summarize code or generate diagrams. They do
not reliably answer architectural questions with grounded evidence.

CogniCode already has the primitives to do better:

- `DecisionGraph`
- `ArchitectureRationale`
- `EvidencePack`
- `ChangeImpactStory`
- `RiskMap`
- `ComposedNarrative`

But these primitives are still too independent. Users need a coherent product
surface for architectural decision-making, not a loose collection of views.

## Decision

We will introduce **architecture decision support packs**: coherent, inspectable
bundles that combine decision, code, evidence, impact, and risk.

### A support pack must answer

1. What decision exists?
2. What code/doc/evidence supports it?
3. What parts of the system are affected if it changes?
4. What risks or contradictions are visible right now?
5. How can the user continue the investigation from the UI?

### Initial pack composition

- `DecisionGraph`
- `ArchitectureRationale`
- `EvidencePack`
- `RiskMap`
- `ChangeImpactStory`
- optional `ComposedNarrative` wrapper for review/sharing

### Product rule

No decision-support capability counts as complete unless it is:

- directly reachable from Explorer UI;
- inspectable in pane-based navigation;
- usable without requiring raw MCP/tool invocation;
- validated by interaction tests for the main user path.

## Alternatives considered

### 1. Add more standalone views and let users compose mentally

Rejected. This increases feature count but not user decision support.

### 2. Build a chat-only architecture assistant

Rejected. Chat without inspectable, reproducible objects weakens traceability.

### 3. Keep decision support as docs/ADRs outside Explorer

Rejected. That would abandon the moldable-development goal.

## Consequences

### Positive

- Architectural reasoning becomes reproducible and inspectable.
- ADRs and evidence move from passive text to active system objects.
- Review and onboarding improve because the tool can show “why” and “what
  changes if we move this.”

### Negative

- Requires stronger consistency between graph, docs, ADRs, and UI states.
- Raises the bar for regression testing and UX coherence.

### Mitigations

- Deliver pack composition incrementally.
- Reuse existing views before adding new taxonomy.

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md)
- [ADR-006](./ADR-006-functional-gtoolkit-parity.md)

---

## Revision Note: E25.1 Closure (2026-07-24)

E25.1 shipped with the following implementation commitments, verified by SDDK:

### DecisionSupportPack Implementation Rules

1. **Discoverable**: `DecisionSupportPackExecutor` registered in `REAL_EXECUTORS` map with
   `ViewKind::DecisionSupportPack` and `applies_to: [DecisionArtifact]`. The view appears
   in Explorer UI listings for Decision targets.

2. **Persistent Provenance**: Pack panes carry `PaneStatus` (`Ok | Degraded | Failed`)
   so partial failure never propagates silently. The five-pane structure is stable
   regardless of which panes succeed or fail.

3. **GUI-visible**: Decision targets in Explorer can select "Decision Support Pack" from
   the view picker, opening a new inspector pane with the five-pane composite view.

4. **Regeneration Contract**: The pack builder is pure (no side effects). Re-running the
   same `decision_id` with the same graph state produces the same output. Provenance
   is preserved per-pane through `PaneStatus` — callers can distinguish fresh builds
   from degraded ones.

### E25.1 Deliverables

| Deliverable | PR | Status |
|-------------|-----|--------|
| DecisionGraph topology builder + RendererKind::Graph | PR 1 | ✅ Merged |
| DecisionSupportPackBuilder + GET /api/decisions/:id/support-pack | PR 2 | ✅ Merged |
| DecisionSupportPackExecutor + ADR-011 revision + W-2 test | PR 3 | ✅ Applied |
