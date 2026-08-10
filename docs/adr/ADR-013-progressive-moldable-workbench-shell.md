# ADR-013: Progressive moldable workbench shell for CogniCode Explorer

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-22  
**Deciders**: User, OpenCode orchestrator  

## Context

CogniCode Explorer already contains many meaningful capabilities: Spotter,
EntryPoints, PaneInspector, ViewExecutors, investigations, evidence, diagrams,
DecisionGraph, RiskMap, and narrative objects. The current risk is not lack of
features but lack of a shell that presents them as one coherent moldable
environment.

The product goal is not to copy GToolkit visually. The goal is to adapt the
functional strengths of GToolkit to CogniCode's architecture and use cases:

- object-centered exploration,
- progressive disclosure,
- multiple synchronized representations,
- pane-based navigation,
- and architecture knowledge made visible.

The current UI still risks reading as a collection of screens and tools rather
than one progressive workbench for visual thinking.

## Decision

CogniCode Explorer will adopt a **progressive moldable workbench shell**.

### Core shell principles

1. **Progressive before dense**
   - the starting surface must be calm and understandable;
   - depth appears through drill-down, not through default clutter.

2. **Pane-first navigation**
   - panes are the primary navigation grammar for object exploration;
   - users should be able to preserve narrative while moving laterally.

3. **Object-first actions**
   - views, affordances, narratives, diagrams, and evidence are entered from an
     object context whenever possible.

4. **One workbench, many representations**
   - code, graph, architecture, evidence, docs, and decision views must feel
     structurally related, not like separate mini-apps.

5. **GUI-visible completion**
   - every shell capability must be discoverable, inspectable, usable, and
     validated by user interaction tests.

### Shell zones

The Explorer shell should be organized into stable zones:

- **Entry zone** — Spotter, recent explorations, investigations, saved work,
  and context-aware starts.
- **Navigation zone** — pane stack, causal breadcrumbs, active object path.
- **Representation zone** — contextual views (graph, table, code, markdown,
  artifacts, diagrams).
- **Knowledge zone** — evidence, ADRs, docs, narratives, concept maps.
- **Action zone** — create custom view, pin evidence, export diagram, compare,
  explain, save.

These are conceptual zones, not a requirement for literal permanent sidebars.

## Alternatives considered

### 1. Dashboard-first shell

Rejected. A dashboard organizes widgets, not exploratory thought.

### 2. GToolkit visual mimicry

Rejected. CogniCode needs functional inspiration, not aesthetic imitation.

### 3. Chat-first architecture UI

Rejected. Chat can assist, but the primary environment must remain inspectable,
visible, and reproducible.

## Consequences

### Positive

- Gives current and future capabilities one navigable home.
- Makes the product more understandable to mixed teams.
- Creates a stable UX target for Spotter, diagrams, narratives, and decision
  support work.

### Negative

- Requires shell-level refactoring, not just view additions.
- Raises the bar for UX consistency and interaction testing.

### Mitigations

- Ship the shell in slices.
- Reuse existing PaneStack, Spotter, affordances, and Investigation flows.
- Keep the first iteration focused on shell structure, not visual ornament.

## References

- [ADR-002](./ADR-002-moldable-exploration-parity-program.md)
- [ADR-006](./ADR-006-functional-gtoolkit-parity.md)
- [ADR-012](./ADR-012-ui-visible-capability-contract.md)
- `PRODUCT.md`

## Implementation Log

- **2026-08-10 (E31-C)**: Progressive workbench shell (E27) implemented. PaneStack, LandingWorkbench, ContextualPanel, OnboardingWizard shipped. Documentation in docs/E27-progressive-workbench-shell.md.
