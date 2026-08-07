# Plan 015: Compose architecture decision support packs

## Status

- **Priority**: P2
- **Effort**: L
- **Risk**: MED
- **Depends on**: `plans/013-knowledge-views-pack.md`, `plans/014-diagram-artifacts-workflow.md`
- **Category**: direction
- **Planned at**: commit `a130d53b`, 2026-07-22
- **Status**: ACTIVE — SDDK change `e25-decision-support-packs` (proposal locked 2026-07-24)
- **SDD change**: `openspec/changes/e25-decision-support-packs/proposal.md`
- **ADR**: `docs/adr/ADR-011-architecture-decision-support-packs.md` (PROPOSED → ACCEPTED on completion)

## Locked decisions (E25.1 slice)

| # | Decision | Verdict | Rationale |
|---|----------|---------|-----------|
| A | DecisionGraph vs ArchitectureRationale | **Differentiate** | Both delegate to `build_rationale_view` (views.rs:3729); only title differs. DecisionGraph → `Graph` renderer with topology (ADR→Code→Tests→Docs→Evidence); ArchitectureRationale stays Markdown narrative. |
| B | Pack composition model | **Backend fan-out** | Frontend composition violates "No backend logic in frontend". `DecisionSupportPackExecutor` fans out to sub-views server-side. |

## Constraints
- E25.1 scope only — E24 HIGH debt is a non-blocking follow-up
- E27.3 owns ContextRail; packs render in pane stack, not rail
- No new DB tables; no new ports
- ADR-011 → ACCEPTED at completion

## Why this matters

CogniCode's long-term value is not another isolated view. It is giving users a
grounded, inspectable answer to architectural questions. This plan composes the
existing views into a decision-support experience that users can actually act
on inside Explorer.

## Current state

- `DecisionGraph`, `ArchitectureRationale`, `EvidencePack`, `RiskMap`, and
  `ChangeImpactStory` already exist or are being hardened.
- `docs/adr/ADR-011-architecture-decision-support-packs.md` defines the pack
  contract.

## Commands you will need

| Purpose | Command | Expected on success |
|---|---|---|
| Rust tests | `cargo test -p cognicode-explorer --lib` | exit 0 |
| UI tests | `npm --prefix apps/explorer-ui test -- PaneInspector` | exit 0 |
| E2E | `npm --prefix apps/explorer-ui run test:e2e:coverage -- --grep "decision|evidence|risk|impact"` | exit 0 |

## Scope

**In scope** (E25.1):
- `DecisionSupportPackExecutor` — backend fan-out orchestrator
- DecisionGraph differentiation (Markdown → Graph + topology builder)
- REST endpoint `GET /api/decisions/:id/support-pack`
- Pane-stack rendering of pack sub-views

**Out of scope**:
- E24 HIGH debt (follow-up)
- ContextRail content (E27.3)
- New DB tables, new ports
- ComposedNarrative wrapper (future)
- contextual editor
- federated runtime objects

## Steps

1. Implement `DecisionSupportPackExecutor` backend fan-out over registered sub-executors.
2. Differentiate DecisionGraph: new `build_decision_topology()` via `GraphQueryPort::subgraph()`.
3. Add `ViewKind::DecisionSupportPack` + REST endpoint.
4. Render pack sub-views as lateral panes in Explorer.
5. Move ADR-011 PROPOSED → ACCEPTED.
6. Validate user interaction, not just executor existence.

## Done criteria

- [ ] A user can open a decision and reach rationale, evidence, risk, and
      impact from the GUI via the pane stack
- [ ] DecisionGraph renders as a graph (not Markdown)
- [ ] The flow is covered by interaction tests
- [ ] ADR-011 is ACCEPTED
