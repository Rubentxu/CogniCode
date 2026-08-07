# Proposal: E25.1 — Architecture Decision Support Packs (Backend Composition)

## Intent
CogniCode has the architectural-decision primitives (`DecisionGraph`, `ArchitectureRationale`, `EvidencePack`, `RiskMap`, `ChangeImpactStory`) but no coherent composition surface. Two defects block decision archaeology: (1) `DecisionGraph` and `ArchitectureRationale` executors both delegate to the *identical* `build_rationale_view()` with only a title-swap — they are indistinguishable; (2) no backend orchestrator composes these views into a single pack. This change delivers the E25.1 slice: a backend fan-out pack executor + DecisionGraph differentiation, exposed in the pane stack.

## Decisions (LOCKED)

| # | Decision | Verdict | Evidence |
|---|----------|---------|----------|
| A | DecisionGraph vs ArchitectureRationale | **Differentiate** | Both call `build_rationale_view` (views.rs:3729); only `with_decision_graph_identity` re-tags (3913). CONTEXT.md DQS: `decision_graph` → `Graph` renderer. DecisionGraph builds topology graph; ArchitectureRationale stays Markdown narrative. |
| B | Pack composition model | **Backend fan-out** | Frontend composition violates "No backend logic in frontend" (CONTEXT.md). Backend `DecisionSupportPackExecutor` fans out to sub-views server-side. |

## Scope

### In Scope
- `DecisionSupportPackExecutor` — backend fan-out orchestrator (no new ports, no new tables)
- DecisionGraph differentiation: `RendererKind::Graph` + topology builder
- REST endpoint exposing the composed pack
- Pane-stack rendering of pack sub-views (E27.3 owns ContextRail; packs render in panes)
- Plan 015 status → ACTIVE; ADR-011 PROPOSED → ACCEPTED on completion

### Out of Scope
- E24 HIGH debt (non-blocking follow-up)
- ContextRail content (owned by E27.3)
- New DB tables, new ports
- ComposedNarrative wrapper for packs (future)

## Capabilities

> CONTRACT with sddk-spec. Researched against `openspec/specs/`.

### New Capabilities
- `decision-support-packs`: Backend fan-out orchestrator composing DecisionGraph + ArchitectureRationale + EvidencePack + RiskMap + ChangeImpactStory into a coherent inspectable pack, exposed via REST and rendered in the Explorer pane stack.

### Modified Capabilities
- `view-registry-backend`: DecisionGraph executor differentiated — `renderer_kind` Markdown → Graph; `build()` now constructs decision topology (ADR → Code → Tests → Docs → Evidence) instead of delegating to `build_rationale_view`.

## Approach
1. `DecisionSupportPackExecutor::build(ctx)` fans out to registered sub-executors via `ViewRegistry::get_executor()`, collecting each `ContextualView` into a pack.
2. New `ViewKind::DecisionSupportPack` + REST `GET /api/decisions/:id/support-pack`.
3. DecisionGraph: new `build_decision_topology()` using `GraphQueryPort::subgraph()` over the decision node; renderer → Graph.
4. Frontend: pack renders sub-views as lateral panes in the existing pane stack.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/domain/views.rs` | Modified | DecisionGraph differentiation + DecisionSupportPackExecutor |
| `crates/cognicode-explorer/src/registry.rs` | Modified | Wire pack executor; ViewKind enum + variant |
| `crates/cognicode-explorer/src/api.rs` | Modified | `GET /api/decisions/:id/support-pack` |
| `apps/explorer-ui/src/components/PaneInspector.tsx` | Modified | Pack → lateral panes rendering |
| `plans/015-architecture-decision-support-pack.md` | Modified | Lock decisions; status ACTIVE |
| `docs/adr/ADR-011-architecture-decision-support-packs.md` | Modified | PROPOSED → ACCEPTED on completion |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Fan-out latency (5 sub-views serialized) | Medium | Parallel `tokio::join!`; cap sub-view count |
| DecisionGraph topology empty for decisions without edges | Medium | Graceful degradation to focus-only node (existing pattern) |
| E24 debt surfaces during pack composition | Low | E24 is non-blocking; defer with ponytail marker |

## Rollback Plan
Revert the 4 modified source files + Plan 015/ADR-011 edits. No DB migrations — all data reads from existing tables. Revert ADR-011 to PROPOSED.

## Dependencies
- Existing executors: ArchitectureRationale, EvidencePack, RiskMap, ChangeImpactStory
- ADR-011 (the pack contract); Plan 015
- E27.3 owns ContextRail boundary — packs must not inject into the rail

## Success Criteria
- [ ] `DecisionSupportPackExecutor` composes ≥4 sub-views into one pack
- [ ] DecisionGraph renders as `Graph` (not Markdown); topology test green
- [ ] `GET /api/decisions/:id/support-pack` returns composed pack
- [ ] Pane stack renders pack sub-views as lateral panes
- [ ] ADR-011 moved PROPOSED → ACCEPTED
- [ ] `cargo test -p cognicode-explorer --lib` green
