# Plan 019: Strengthen pane navigation and representation coherence

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/018-shell-foundation-and-entry-rail.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

CogniCode's pane model is the heart of its moldable-development UX. If panes do
not preserve identity, cause, and representation shifts clearly, the system
feels like stacked inspectors rather than navigable thought.

## Current state

- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` already
  handles object identity, tabs, breadcrumb, notes, export, and evidence pinning.
- `apps/explorer-ui/src/components/PaneStackView.tsx` provides the lateral pane
  model.
- `apps/explorer-ui/src/components/ObjectInspector/ViewTabs.tsx` and
  `PaneBreadcrumb.tsx` are the main representation and causal-navigation
  primitives.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI unit tests | `npm --prefix apps/explorer-ui test -- PaneInspector PaneBreadcrumb ViewTabs PaneStackView` | exit 0 |
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |
| E2E | `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "pane-stack|view-tabs|drilldown"` | exit 0 |

## Scope

**In scope**:
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/PaneBreadcrumb.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/ViewTabs.tsx`
- `apps/explorer-ui/src/components/PaneStackView.tsx`

**Out of scope**:
- start surface
- right-side knowledge rail
- new knowledge families

## Steps

1. Clarify active object identity, origin, and pane purpose in each pane header.
2. Make view switching feel like representation changes of the same object, not
   mini-route changes.
3. Strengthen causal breadcrumbs and visible pane relationships.
4. Normalize pane actions so they are object-centered and consistently placed.

## Test plan

- Add unit tests for pane identity and breadcrumb rendering.
- Add E2E for drill-down, lateral navigation, and representation switching.

## Done criteria

- [ ] Pane stack preserves navigation narrative clearly
- [ ] View switching is visibly coherent
- [ ] Drill-down and lateral pane opening remain stable
- [ ] All verification commands exit 0

## STOP conditions

- The current pane stack cannot support the required breadcrumb/identity model
  without state-model breakage
