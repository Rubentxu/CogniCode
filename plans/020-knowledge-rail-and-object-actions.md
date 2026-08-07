# Plan 020: Introduce the knowledge rail and object-centered action zone

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/012-knowledge-layer-ports-and-spotter-wave2.md`, `plans/013-knowledge-views-pack.md`, `plans/019-pane-navigation-and-representation-coherence.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

The right side of the Explorer should not be dead air or generic utilities. It
should become the contextual rail where the active object reveals knowledge,
evidence, artifacts, and next actions. This is the bridge from exploration to
decision support.

## Current state

- `PaneInspector.tsx` already surfaces export and pin-evidence actions.
- `InvestigationsSection`, `PinEvidenceModal`, `ExportMenu`, and related hooks
  prove the ingredients exist, but they are not yet framed as one coherent
  contextual zone.
- ADRs 009–012 require knowledge and actions to be visible and usable from GUI.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI unit tests | `npm --prefix apps/explorer-ui test -- PaneInspector ExportMenu PinEvidenceModal` | exit 0 |
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |
| E2E | `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "investigation|artifact|evidence|exploration"` | exit 0 |

## Scope

**In scope**:
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`
- `apps/explorer-ui/src/components/ExportMenu.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/PinEvidenceModal.tsx`
- relevant evidence/artifact/action hooks and schemas

**Out of scope**:
- diagram generation backend
- final decision support pack composition

## Steps

1. Introduce a contextual knowledge rail tied to the active pane/object.
2. Group evidence, related docs/ADRs, artifacts, and save/export actions into a
   coherent action zone.
3. Ensure actions teach the workflow instead of hiding behind generic icons.
4. Validate the active-object → evidence/artifact/action loop through UI tests.

## Done criteria

- [ ] The active object exposes a visible knowledge/action rail
- [ ] Evidence and artifacts are reachable from the pane context
- [ ] Actions are usable without MCP/API knowledge
- [ ] Interaction tests pass
