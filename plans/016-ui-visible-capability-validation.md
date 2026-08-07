# Plan 016: Enforce UI-visible capability completion gates

## Status

- **Priority**: P1
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: docs
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

CogniCode keeps paying the same tax: backend capabilities exist before users can
discover and use them. This plan hardens the product process so future features
cannot be called done unless they are visible, usable, and interaction-tested
in Explorer UI.

## Current state

- `docs/adr/ADR-012-ui-visible-capability-contract.md` defines the contract.
- The repo already has strong UI testing infrastructure in
  `apps/explorer-ui/package.json` and `apps/explorer-ui/e2e/*`.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| UI unit tests | `npm --prefix apps/explorer-ui test` | exit 0 |
| E2E coverage | `npm --prefix apps/explorer-ui run test:e2e:coverage` | exit 0 |
| Rust tests | `cargo test -p cognicode-explorer --lib` | exit 0 |

## Scope

**In scope**:
- contribution/feature completion docs
- roadmap completion language
- feature templates / acceptance language
- UI and E2E gates for moldable capabilities

**Out of scope**:
- implementing specific features from other plans

## Steps

1. Add a feature-completion checklist template for new moldable capabilities.
2. Require discoverable + inspectable + usable + validated acceptance language
   in roadmap and planning docs.
3. Add at least one reusable test checklist or scaffold for capability E2E.

## Done criteria

- [ ] The completion contract is documented and reusable
- [ ] Roadmap/features explicitly reference interaction validation
- [ ] New work can reuse the gate instead of inventing it each time
