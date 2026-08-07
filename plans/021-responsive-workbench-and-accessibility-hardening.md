# Plan 021: Harden the progressive workbench for responsive and accessibility behavior

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: MED
- **Depends on**: `plans/018-shell-foundation-and-entry-rail.md`, `plans/019-pane-navigation-and-representation-coherence.md`, `plans/020-knowledge-rail-and-object-actions.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

The shell is not done when it looks correct at desktop width. CogniCode's new
workbench contract demands strong accessibility and progressive responsive
behavior, especially because the product serves a mixed team with deep,
information-heavy tasks.

## Current state

- `PRODUCT.md` explicitly sets a high accessibility bar.
- `apps/explorer-ui/src/components/ShellLayout.tsx` already has a small-viewport
  bottom-sheet adaptation.
- The repo already contains strong Playwright suites for responsive and a11y
  coverage.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI build | `npm --prefix apps/explorer-ui build` | exit 0 |
| Responsive E2E | `npm --prefix apps/explorer-ui run test:e2e:functional -- --grep "responsive"` | exit 0 |
| Coverage E2E | `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "responsive|a11y|pane"` | exit 0 |

## Scope

**In scope**:
- shell responsive behavior
- pane accessibility and focus management
- keyboard navigation through Spotter, pane stack, actions, and rail
- reduced-motion and readability hardening

**Out of scope**:
- adding new product capabilities

## Steps

1. Audit the progressive shell on small, tablet, desktop, and ultrawide.
2. Harden focus order, keyboard affordances, and visible focus states.
3. Improve bottom-sheet / rail collapse behavior so progressive depth survives
   on smaller screens.
4. Add or update interaction tests for a11y and responsive behavior.

## Done criteria

- [ ] Responsive shell preserves the same conceptual zones
- [ ] Keyboard navigation is complete and visible
- [ ] Reduced-motion behavior remains usable
- [ ] E2E validation passes for responsive and a11y flows
