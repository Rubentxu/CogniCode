# Plan 013: Ship the knowledge views pack with visible Explorer interactions

> **Executor instructions**: Follow this plan step by step. Run every
> verification command before moving on.
>
> **Drift check (run first)**: `git diff --stat a130d53b..HEAD -- \
> crates/cognicode-explorer/src/domain/views.rs crates/cognicode-explorer/src/registry.rs \
> apps/explorer-ui/src/components/ObjectInspector apps/explorer-ui/src/components/ViewSpecWizard.tsx`

## Status

- **Priority**: P1
- **Effort**: L
- **Risk**: HIGH
- **Depends on**: `plans/012-knowledge-layer-ports-and-spotter-wave2.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22

## Why this matters

Once knowledge objects are discoverable, they still need strong, visible views.
This plan turns docs/ADRs/evidence from passive search results into meaningful,
inspectable knowledge surfaces inside Explorer.

## Current state

- `docs/ROADMAP.md:340-344` still lists `DocCodeAlignment`, `ConceptMap`, and
  broader living-knowledge views as part of the parity gap.
- `crates/cognicode-explorer/src/registry.rs` already wires many real
  executors, so the product pattern exists.
- `docs/adr/ADR-011-architecture-decision-support-packs.md` depends on these
  views to build decision-support packs later.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test -p cognicode-explorer --lib` | exit 0 |
| UI unit tests | `npm --prefix apps/explorer-ui test -- PaneInspector` | exit 0 |
| E2E coverage | `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "spotter|pane|view"` | exit 0 |

## Scope

**In scope**:
- `crates/cognicode-explorer/src/domain/views.rs`
- `crates/cognicode-explorer/src/registry.rs`
- `crates/cognicode-explorer/src/dto.rs`
- `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx`
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx`

**Out of scope**:
- diagram persistence/export
- contextual editor
- federated runtime objects

## Steps

1. Implement or harden `DocCodeAlignment`, `ConceptMap`, and adjacent knowledge
   views as real executors.
2. Register them with clear identity and renderer choices.
3. Add UI-visible tabs/affordances where appropriate.
4. Validate pane-based interaction and renderer behavior.

**Verify**: run all commands in the table above → exit 0.

## Test plan

- Rust executor tests for descriptor metadata and object applicability.
- Frontend tests for tab visibility and pane rendering.
- At least one E2E path from Spotter → pane → knowledge view.

## Done criteria

- [ ] Knowledge views are wired as real executors
- [ ] They are reachable from visible Explorer interactions
- [ ] Interaction tests prove the happy path

## STOP conditions

- A view requires a missing port or object family from Plan 012
- The current renderer set cannot express the view without a new renderer family

## Maintenance notes

- Keep view identity stable; these names become durable affordance contracts.
