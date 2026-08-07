# Plan 018: Build the shell foundation and progressive entry rail

> **Executor instructions**: Follow this plan step by step. Run every
> verification command before moving on. Stop if the shell refactor forces a
> replacement of the current pane model rather than an evolution of it.

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/012-knowledge-layer-ports-and-spotter-wave2.md`, `plans/016-ui-visible-capability-validation.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

The current Explorer shell exposes useful capabilities but still reads like a
tool frame with controls rather than a progressive moldable workbench. This
slice establishes the structural shell and the calm entry surface users need
before deeper knowledge and decision features can feel coherent.

## Current state

- `apps/explorer-ui/src/components/ShellLayout.tsx` already defines a top bar,
  primary/secondary zones, and responsive behavior.
- `apps/explorer-ui/src/components/Spotter.tsx` is the strongest current entry
  mechanism.
- `apps/explorer-ui/src/components/LandingWorkbench/*` already provides pieces
  of a start surface (`StartFromSection`, `ResumeSection`,
  `InvestigationsSection`).
- `PRODUCT.md` and `ADR-013` require a progressive, calm, object-first entry.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI unit tests | `npm --prefix apps/explorer-ui test -- Shell Spotter LandingWorkbench` | exit 0 |
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |
| E2E | `npm --prefix apps/explorer-ui run test:e2e:functional -- --grep "landing|spotter"` | exit 0 |

## Scope

**In scope**:
- `apps/explorer-ui/src/components/ShellLayout.tsx`
- `apps/explorer-ui/src/components/Shell.tsx`
- `apps/explorer-ui/src/components/LandingWorkbench/*`
- `apps/explorer-ui/src/components/Spotter.tsx`

**Out of scope**:
- pane detail behavior
- decision/evidence/artifact right-rail composition
- diagram artifact persistence

## Steps

1. Refactor the shell into explicit entry/navigation/representation shell zones.
2. Replace any dashboard-like first impression with a progressive start rail.
3. Surface Start from / Recent / Investigations / Saved work as first-class
   entry actions.
4. Keep Spotter as the dominant keyboard-first and visual entry path.

## Test plan

- Add/adjust unit tests for shell zones and landing composition.
- Add one functional E2E proving a user can enter through the start surface or
  Spotter without confusion.

## Done criteria

- [ ] The shell entry surface feels progressive, not dashboard-like
- [ ] Spotter remains prominent and accessible
- [ ] Start rail sections are visible and keyboard reachable
- [ ] All verification commands exit 0

## STOP conditions

- Entry work requires new backend capabilities not covered by Plan 012
- The shell becomes denser instead of calmer after the changes
