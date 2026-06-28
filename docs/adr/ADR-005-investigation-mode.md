# ADR-005: Investigation Mode — Knowledge Artifacts as First-Class Entities

**Status**: PROPOSED  
**Date**: 2026-06-28  
**Deciders**: User, orchestrator session 2026-06-28

## Context

CogniCode Explorer currently supports:
- **Exploration** — navigate objects via Spotter, panes, views
- **Sharing** — `ShareExplorationButton` saves navigation state as a URL
- **Custom views** — `ViewSpecWizard` creates user-defined views

But it does NOT support **structured knowledge building**:
- No way to attach a **goal** to an exploration session
- No way to **pin evidence** (a view, a pane, a finding)
- No way to **write conclusions** linked to code objects
- No way to **bundle** diagrams, evidence, and narrative into a reusable artifact
- No way to **resume** an investigation with context intact

The `ExplorationSession` DTO (`crates/cognicode-explorer/src/dto.rs:427`)
models raw navigation events. It captures *what happened* but not *why*.

GToolkit's Lepiter provides living documentation with embedded objects,
evaluable snippets, and linked narratives. CogniCode's `ViewKind` catalog
already reserves `ComposedNarrative`, `ProjectDiary`, `EvidencePack`, and
`ConceptMap` — but none are implemented.

The gap is between **browsing** and **understanding**.

## Decision

**Introduce an Investigation entity as a first-class knowledge artifact that
bundles exploration, evidence, narrative, and diagrams into a resumable,
shareable, and durable structure.**

### 1. Investigation entity

```typescript
interface Investigation {
  id: string;
  title: string;
  goal: string;              // What question are we answering?
  status: "active" | "completed" | "archived";
  entryPoint: string;       // Object that started the investigation
  panes: PaneRef[];         // Pinned panes with context
  evidence: EvidenceItem[]; // Pinned views, findings, notes
  artifacts: Artifact[];    // Diagrams, Mermaid, draw.io exports
  narrative: string;        // Markdown conclusion / summary
  relatedADRs: string[];    // Linked decisions
  createdAt: string;
  updatedAt: string;
}

interface EvidenceItem {
  id: string;
  objectId: string;
  viewId: string;
  note: string;              // Why this matters
  pinnedAt: string;
}

interface Artifact {
  id: string;
  kind: "mermaid" | "drawio" | "svg" | "markdown";
  title: string;
  content: string;           // Mermaid source, draw.io XML, SVG, markdown
  generatedFrom: string;     // ViewKind + objectId that produced it
}
```

### 2. Investigation lifecycle

```
[Start Investigation]
  ↓
[Explore: open panes, trace paths, inspect objects]
  ↓
[Pin Evidence: "this view proves X", "this call shows Y"]
  ↓
[Generate Artifacts: Mermaid diagram, draw.io export, summary]
  ↓
[Write Conclusion: narrative linking evidence to goal]
  ↓
[Complete / Share / Archive]
```

### 3. UI surfaces

#### a. Investigation Mode toggle
When active, the Explorer shows:
- **Investigation sidebar** (left or right): goal, pinned evidence, artifacts
- **Pin button** on every pane: "Add to investigation"
- **Investigation actions** in SuggestionStrip: "Save as evidence", "Generate diagram"

#### b. Evidence Pack view
A `ViewKind::EvidencePack` that renders collected evidence as a structured
document with linked objects, notes, and embedded diagrams.

#### c. Composed Narrative
A `ViewKind::ComposedNarrative` that renders a markdown narrative with
embedded object links, view references, and diagram artifacts. This is the
Lepiter-equivalent: a navigable story made of objects and explanations.

#### d. Investigation Board
A lightweight board showing all active and completed investigations for the
workspace. Accessible from the landing page.

### 4. Integration with existing systems

| System | Integration |
|--------|-------------|
| Spotter | Investigations appear as search results |
| Pane Stack | "Pin as evidence" action per pane |
| ViewTabs | "Add to investigation" in overflow menu |
| Share | Share button produces investigation URL, not just panes |
| C4 (ADR-004) | Dynamic traces can be pinned as evidence |
| Diagrams (ADR-003) | Mermaid/draw.io exports become artifacts |
| ExplorationSession | Upgraded to carry investigationId |

### 5. Backend persistence

Investigations are persisted in PostgreSQL (same as ViewSpecs and
ExplorationSessions). New tables:

- `investigations` — id, workspace_id, title, goal, status, narrative, timestamps
- `investigation_evidence` — investigation_id, object_id, view_id, note, pinned_at
- `investigation_artifacts` — investigation_id, kind, title, content, generated_from

### 6. Investigation → draw.io → expected architecture

A completed investigation can produce:
- A **curated diagram** (draw.io export refined by hand)
- That diagram becomes the **expected architecture** baseline
- Future drift detection compares inferred C4 against that baseline

This closes the loop: **investigation → knowledge → governance**.

## Alternatives considered

### A. Enhance ExplorationSession instead of new entity
- Pros: Less schema change.
- Cons: ExplorationSession is raw navigation events; investigations are
  semantic knowledge artifacts. Conflating them degrades both.
- **Rejected** — different concerns, different lifecycles.

### B. Use ViewSpecs for investigations
- Pros: Already persisted, already in ViewTabs.
- Cons: ViewSpecs are view definitions, not knowledge bundles. Wrong abstraction.
- **Rejected** — wrong entity shape.

### C. External tool (Notion, Confluence) for knowledge
- Pros: No implementation effort.
- Cons: Loses linkage to live code objects; knowledge rots when code changes.
- **Rejected** — CogniCode's value is live, code-linked knowledge.

## Consequences

### Positive
- Users build durable, shareable knowledge artifacts.
- Onboarding: "read this investigation" instead of "explore the codebase".
- Evidence-backed decisions: ADRs reference investigations.
- Diagrams and narratives are part of the product, not external tools.
- Closes the moldable-development loop: explore → understand → explain.

### Negative
- Significant new entity + persistence + UI.
- Risk of scope creep (notebooks, collaborative editing, etc.).
  **Mitigation**: v1 is single-user, markdown narrative, no real-time collaboration.
- Narrative quality depends on user effort.
  **Mitigation**: AI-assisted narrative drafting from evidence (future).

## Implementation phases

| Phase | Deliverable |
|-------|-------------|
| INV-1 | Investigation entity + persistence + API |
| INV-2 | Investigation sidebar UI + pin evidence |
| INV-3 | Evidence Pack view (`ViewKind::EvidencePack`) |
| INV-4 | Composed Narrative view (`ViewKind::ComposedNarrative`) |
| INV-5 | Investigation Board on landing |
| INV-6 | Diagram artifacts embedded in investigations |
| INV-7 | Expected architecture from completed investigation |

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md) — Moldable exploration program
- [ADR-003](./ADR-003-diagram-representations.md) — Diagram representations
- [ADR-004](./ADR-004-c4-investigation-model.md) — C4 investigation model
- `CONTEXT.md` — EvidencePack, ComposedNarrative, ProjectDiary ViewKinds
- `crates/cognicode-explorer/src/dto.rs:427` — ExplorationSession
- `apps/explorer-ui/src/components/ShareExplorationButton.tsx` — current share flow
- GToolkit Lepiter — pages with evaluable snippets and linked objects
