# Plan 017: Reshape Explorer into a progressive moldable workbench shell

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the STOP conditions section occurs, stop and
> report — do not improvise.
>
> **Drift check (run first)**: `git diff --stat a130d53b..HEAD -- \
> apps/explorer-ui/src/components apps/explorer-ui/src/hooks \
> apps/explorer-ui/src/api crates/cognicode-explorer/src docs/ROADMAP.md PRODUCT.md docs/adr`

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/012-knowledge-layer-ports-and-spotter-wave2.md`, `plans/016-ui-visible-capability-validation.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

CogniCode Explorer already has many real capabilities, but the interface still
does not present them as one coherent moldable environment. The user asked for
a shell more aligned with GToolkit's progressive workbench model, adapted to
CogniCode's own product and architecture. Without this shell work, new views and
knowledge capabilities will continue to feel like disconnected features.

## Current state

- `PRODUCT.md` defines the register as `product` for a mixed team, with
  progressive depth, object-centered exploration, visual thinking, and concept
  maps as core direction.
- `docs/adr/ADR-013-progressive-moldable-workbench-shell.md` defines the shell
  contract.
- `apps/explorer-ui/src/components/Spotter.tsx` already acts as a primary entry
  mechanism.
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` already
  hosts contextual representations.
- `apps/explorer-ui/src/components/LandingWorkbench/InvestigationsSection.tsx`
  already gives one visible entry to durable exploration.
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx` proves
  in-situ authoring exists, but the shell does not yet frame it clearly.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI unit tests | `npm --prefix apps/explorer-ui test` | exit 0 |
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |
| E2E coverage | `npm --prefix apps/explorer-ui run test:e2e:coverage` | exit 0 |
| Rust compile guard | `cargo check -p cognicode-explorer` | exit 0 |

## Scope

**In scope**:
- `apps/explorer-ui/src/components/Spotter.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`
- `apps/explorer-ui/src/components/LandingWorkbench/InvestigationsSection.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx`
- adjacent shell/layout/navigation files in `apps/explorer-ui/src/components/`
- schemas/hooks needed for shell state

**Out of scope**:
- adding unrelated backend capabilities
- replacing PaneStack with a new navigation model
- shipping the full contextual editor

## Git workflow

- Branch: `advisor/017-progressive-workbench-shell` (or repo-equivalent)
- Commit style: conventional commits
- Do NOT push or open a PR unless explicitly instructed.

## Steps

### Step 1: Define the shell zones in code

Refactor the shell so the UI has explicit, understandable zones:

- entry zone,
- navigation zone,
- representation zone,
- knowledge zone,
- action zone.

Do not implement these as arbitrary permanent chrome unless they improve the
flow. The purpose is structural clarity.

**Verify**: `npm --prefix apps/explorer-ui build` → exit 0.

### Step 2: Make the entry experience progressive

Reshape the starting experience so a user can enter from:

- Spotter,
- investigation/saved work,
- relevant suggested objects,
- high-signal current tasks.

The first screen must feel calm, not dashboard-noisy.

**Verify**: `npm --prefix apps/explorer-ui test -- Spotter` → exit 0.

### Step 3: Strengthen pane-based narrative navigation

Improve pane stack comprehension with stronger causal breadcrumbs, object
identity, active view identity, and visible relationship to previous panes.

**Verify**: `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "pane|navigation|spotter"` → exit 0.

### Step 4: Surface the knowledge/action zones visibly

Ensure users can discover and use:

- evidence,
- diagrams/artifacts,
- narratives,
- custom-view creation,
- save/compare/explain actions,

from within the shell, not by hidden API flows.

**Verify**: `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "view|exploration|artifact|investigation"` → exit 0.

### Step 5: Validate the moldable interaction loop

Add or update interaction tests to prove the main user story:

1. enter through a meaningful object,
2. inspect it,
3. switch representation,
4. navigate laterally,
5. save or continue the exploration.

**Verify**: `npm --prefix apps/explorer-ui run test:e2e:coverage` → exit 0.

## Test plan

- Use existing Vitest component tests for local behavior.
- Use Playwright for the primary user journey and interaction validation.
- Model E2E structure after:
  - `e2e/pane-stack-drilldown.spec.ts`
  - `e2e/view-tabs-coverage.spec.ts`
  - `e2e/spotter-multifamily.spec.ts`
  - `e2e/exploration-share.spec.ts`

## Done criteria

- [ ] Explorer presents a coherent progressive workbench shell
- [ ] Entry, navigation, representation, knowledge, and action zones are clear
- [ ] Main flow is usable from the GUI without hidden tool knowledge
- [ ] Interaction tests prove the shell contract
- [ ] `npm --prefix apps/explorer-ui build` exits 0
- [ ] `npm --prefix apps/explorer-ui run test:e2e:coverage` exits 0

## STOP conditions

- The shell refactor requires replacing PaneStack instead of evolving it
- Knowledge/action surfaces depend on missing backend capabilities from Plan 012
- The redesign collapses progressive disclosure and creates a denser dashboard
  instead of a moldable workbench

## Maintenance notes

- Keep GToolkit as functional inspiration, not visual imitation.
- Reviewers should scrutinize whether the shell improves comprehension, not only
  whether it looks cleaner.
- This plan should coordinate tightly with plans 012, 013, 014, and 016.
