# CogniCode Roadmap

Last updated: 2026-07-02 (continuación sesión — PR #104 merged, relation-candidates-v1 shipped v0.45.0. e12f-ownership-map: explore + propose + spec + design completos, pendiente tasks.)

## Active

### Moldable UX + C4 Investigation + Diagram Representations

**Strategic ADRs**: [ADR-003](./adr/ADR-003-diagram-representations.md) (diagrams), [ADR-004](./adr/ADR-004-c4-investigation-model.md) (C4), [ADR-005](./adr/ADR-005-investigation-mode.md) (investigations)

**Goal**: Transform CogniCode Explorer from a multi-view inspector into a moldable-development knowledge workbench with C4 investigation, draw.io-derived diagrams, and durable knowledge artifacts.

#### Milestone E18 — Moldable UX Foundation

| Change | Goal | ADR | Status | PR |
|--------|------|-----|--------|----|
| `e18-1-landing-workbench` | Landing as entry-point workbench (Start from: Route / Symbol / Use case / Saved exploration) | ADR-005 | DONE | [#76](https://github.com/Rubentxu/CogniCode/pull/76) + [#77](https://github.com/Rubentxu/CogniCode/pull/77) |
| `e18-2-spotter-intent` | Spotter with intent actions (Open as call graph, vertical slice, kind-aware defaults, Cmd+1..N) | ADR-005 | DONE | [#78](https://github.com/Rubentxu/CogniCode/pull/78) |
| `e18-3-pane-causal-breadcrumbs` | Pane stack shows causal breadcrumbs (From / Via / Purpose) | ADR-005 | DONE | [#79](https://github.com/Rubentxu/CogniCode/pull/79) |
| `e18-4-suggestion-verbs` | SuggestionStrip evolves to 5 investigation verbs (Understand / Trace / Compare / Explain / Save) | ADR-005 | DONE | [#80](https://github.com/Rubentxu/CogniCode/pull/80) |

**E18-1 follow-ups (closed in PR #77)**:
- KindFilterChips UX bug — fixed
- Stale visual-regression snapshots — regenerated
- Pre-existing `suggestedQuestions.test.ts` failure — fixed (added `route` kind)

**E18-2 follow-ups (debt to address)**:
- cmdk `vimBindings` limitation blocks interactive E2E for Cmd+N (unit tests cover)
- 3 pre-existing Playwright failures (a11y + strict-mode close button) — technical debt

#### Milestone E19 — C4 Investigation Model

| Change | Goal | ADR | Priority |
|--------|------|-----|----------|
| `e19-1-rename-c4-toggle` | Rename "C4 Components" to "Structure" (honest labeling) | ADR-004 | HIGH | DONE | [#81](https://github.com/Rubentxu/CogniCode/pull/81) |
| `e19-2-c4-level-selector` | Level selector: Context / Container / Component / Code | ADR-004 | HIGH | DONE | [#82](https://github.com/Rubentxu/CogniCode/pull/82) |
| `e19-3-c4-overlays` | Overlays: drift + hotspots on C4 nodes | ADR-004 | HIGH | DONE | [#83](https://github.com/Rubentxu/CogniCode/pull/83) |
| `e19-4-c4-dynamic-views` | Dynamic views from investigation traces (request/event flow) | ADR-004 | MEDIUM | DONE | [#84](https://github.com/Rubentxu/CogniCode/pull/84) |
| `e19-5-expected-architecture` | Expected architecture baseline + drift governance | ADR-004 | MEDIUM | DONE | [#85](https://github.com/Rubentxu/CogniCode/pull/85) |

#### Milestone E20 — Diagram Representations

| Change | Goal | ADR | Priority |
|--------|------|-----|----------|
| `e20-1-mermaid-c4-export` | `to_mermaid()` for c4_context, c4_container, c4_component | ADR-003 | HIGH | DONE | [#86](https://github.com/Rubentxu/CogniCode/pull/86) |
| `e20-2-mermaid-trace-export` | Mermaid export for call_graph, impact_radius, decision_trace, vertical_slice | ADR-003 | HIGH | DONE | [#87](https://github.com/Rubentxu/CogniCode/pull/87) |
| `e20-3-drawio-action` | "Open in draw.io" action in C4 toolbar + pane inspector | ADR-003 | HIGH | DONE | [#88](https://github.com/Rubentxu/CogniCode/pull/88) |
| `e20-4-svg-snapshot` | SVG/PNG snapshot export for static documentation | ADR-003 | LOW | DONE | [#89](https://github.com/Rubentxu/CogniCode/pull/89) |

#### Milestone E21 — Investigation Mode ✅ COMPLETE

| Change | Goal | ADR | Priority | Status |
|--------|------|-----|----------|--------|
| `e21-1-investigation-entity` | Investigation entity + PostgreSQL tables + API | ADR-005 | HIGH | PR1✅ PR2✅ PR3✅ |
| `e21-2-pin-evidence` | "Pin as evidence" action on panes + views | ADR-005 | HIGH | ✅ PR #90 |
| `e21-3-evidence-pack-view` | `ViewKind::EvidencePack` executor | ADR-005 | MEDIUM | ✅ PR #91 |
| `e21-4-composed-narrative` | `ViewKind::ComposedNarrative` with embedded objects + diagrams | ADR-005 | MEDIUM | ✅ PR #91 |
| `e21-5-investigation-board` | Investigation board on landing page | ADR-005 | LOW | ✅ PR #88 |
| `e21-6-artifacts-in-investigation` | Mermaid/draw.io/SVG artifacts embedded in investigations | ADR-003+005 | MEDIUM | ✅ PR #91 |

**E21-1 PR details** (branch `feat/e21-1-investigation-entity`):
- PR1 ✅: PostgreSQL schema (m0013) + repo methods (`save_investigation_tx`, `load_investigation`, etc.)
- PR2 ✅: Domain entity + InvestigationStore trait + PostgresInvestigationStore + InvestigationService facade + REST API
- PR3 ✅: InvestigationBoard UI (InvestigationsSection) + investigation_id wiring + integration tests + useInvestigations hook

**E21-2 PR #90** (branch `feat/e21-1-investigation-entity`):
- ✅ Backend: `PinEvidenceRequest` type + `POST /api/investigations/:id/evidence` endpoint
- ✅ Frontend: `pinEvidence()` hook + `PinEvidenceModal` with investigation dropdown
- ✅ Pin button (📌) in `PaneInspector` header
- ✅ E2E tests for Pin Evidence flow

**E21-3+E21-4+E21-6** (branch `feat/e21-3-e21-4-e21-6`):
- E21-3: `ViewKind::EvidencePack` executor
  - Add `ViewKind::EvidencePack` to ViewKind enum
  - EvidencePackExecutor: fetch investigation.evidence and build ContextualView
  - REST handler: `GET /api/investigations/:id/evidence-pack`
  - Frontend renderer for evidence_pack
- E21-4: `ViewKind::ComposedNarrative` executor
  - Add `ViewKind::ComposedNarrative` to ViewKind enum
  - ComposedNarrativeExecutor: build markdown narrative from investigation
  - REST handler: `GET /api/investigations/:id/composed-narrative`
  - Frontend renderer with embedded diagrams (mermaid, SVG)
- E21-6: Artifacts embedded in investigations
  - POST `/api/investigations/:id/artifacts` endpoint
  - Wire Mermaid export (E20-1/E20-2) to create investigation_id artifacts
  - Wire draw.io action (E20-3) to create investigation_id artifacts
  - Wire SVG snapshot (E20-4) to create investigation_id artifacts
  - Frontend: Artifacts section in InvestigationBoard

#### Execution order

```
E18 (UX foundation)  ──→  E20 (diagrams)  ──→  E21 (investigations)
         │                        ↑
         └──→  E19 (C4)  ─────────┘
```

E18 and E19 can start in parallel. E20 depends on E19 (C4 levels inform diagram content). E21 depends on E18 + E20 (UX foundation + diagram artifacts).

#### e13-universal-spotter-wave1 — Universal Spotter (Investigation + Scope families)

| Change | Goal | Status | Branch | PR |
|--------|------|--------|--------|-----|
| `e13-universal-spotter-wave1` | Extend Spotter 6→8 families (+Investigation +Scope); fix frontend desync (dropped ViewSpec hits, phantom `route` variant) | ✅ DONE | `feat/e13-universal-spotter-wave1` | ✅ [PR #92](https://github.com/Rubentxu/CogniCode/pull/92) |

**Branch commits** (`feat/e13-universal-spotter-wave1`):
- `f80cf9e` — backend: `dto.rs` (Investigation+Scope variants) + `search.rs` (`derive_scope_results`, `InvestigationFacade` wiring) + `lib.rs` (wiring fix)
- `209a0ee` — frontend: `schemas.ts` (8-family enum, route removed) + `useSpotter.ts` (ViewSpec preserved) + `Spotter.tsx` + `IntentFooter.tsx` + `suggestedQuestions.ts` + tests
- `686bb0c` — fix: dead `_original` field, ponytail marker, scope optimization, error logging

**Verification**: 724 Rust tests ✅, 855/856 vitest ✅ (1 pre-existing failure unrelated to this change)

**Debt-verify**: PASS_WITH_WARNINGS — 0 criticals, 2 warnings deferred to follow-up `refactor/e13-followup-typed-accessors`:
- WARN-A (medium): leaky discriminated-union cast in 4 frontend sites — typed accessor helpers (`idOf`, `availableViewsOf`) consolidate narrowing
- WARN-B (low): asymmetric short-circuit in `derive_scope_results` — generalize `kind_per_family` table or drop guard

**Follow-up queued**: `refactor/e13-followup-typed-accessors` — ✅ DONE (commit c783d77)

**Semver**: PATCH — additive feature (8 families), no breaking changes

## Session Handover 2026-07-01

**e13-universal-spotter-wave1 completed, merged, and tagged v0.40.0** (PR #92, branch `feat/e13-universal-spotter-wave1`):
- Spotter now returns 8 families (was 6): +Investigation, +Scope
- Frontend desync fixed: ViewSpec hits no longer dropped, phantom `route` variant removed
- 724 Rust + 871 vitest passing; debt-verify PASS_WITH_WARNINGS
- 2 medium/low warnings deferred to `refactor/e13-followup-typed-accessors`

**E21-2 completed — PR #90 merged**. All evidence pinning functionality is DONE:
- Backend: `POST /api/investigations/:id/evidence` with investigation dropdown modal
- Frontend: `pinEvidence()` hook, PinEvidenceModal, 📌 button in PaneInspector
- E2E tests: 5 Playwright tests for pinning evidence flow

**E21-3+E21-4+E21-6** merged via PR #91:
- E21-3: `ViewKind::EvidencePack` executor + `GET /api/investigations/:id/evidence-pack`
- E21-4: `ViewKind::ComposedNarrative` executor + `GET /api/investigations/:id/composed-narrative`
- E21-6: `POST /api/investigations/:id/artifacts` (mermaid/svg/drawio)

**Architecture stack** (ADR-005 INV-1):
```
PostgreSQL tables (m0013)
  → PostgresRepository (existing methods)
    → PostgresInvestigationStore
      → InvestigationService<S> (core facade)
        → InvestigationFacade (explorer trait)
          → InvestigationServiceImpl (explorer wrapper)
            → REST handlers (api.rs)
              → ApiState.with_investigation() (runtime wiring)
```

**Key decisions made**:
- Facade trait renamed `InvestigationFacade` to avoid name conflict with core `InvestigationService`
- `time::OffsetDateTime` in domain entity (with serde+formatting+parsing features)
- `String` timestamps in repo rows (RFC 3339 format)
- `created_at` preserved on update (fetch-then-patch pattern)
- `time` crate added to workspace + cognicode-explorer dependencies

**Open debts**: None — E21 milestone COMPLETE (PR #90 + PR #91).

**Tests**: 1386 core + 724 explorer lib tests passing. 871 vitest passing.

---

## Session Handover 2026-07-01 (continuación)

**Fixes merged this continuation**:

- `postgres_quality_write_integration` 8/8: `extract_env` helper usaba `serde_json::to_value(result)` (serializaba el struct) en vez de extraer y parsear el texto interno; además faltaba `workspace_id` en cada item del payload de issues
- `Spotter.tsx`: `isSpotterHit` tenía return type malformado (`Extract<X> extends never ? ...`) — arreglado a `Exclude<>` limpio
- `IntentFooter.tsx`: documentado por qué `result.result as SpotterResult` es seguro en el branch no-viewspec
- MSW fixture (`inspectableObjectFixture`): agregadas `evidence` y `ownership-map` a `available_views` — desbloquea 2 tests en `view-tabs-coverage.spec.ts` que antes hacían skip

**Test suite**: 871 vitest ✅, 724 Rust lib tests ✅, `postgres_quality_write_integration` 8/8 ✅

**Commits**:
- `c783d77` — fix(explorer): typed-accessor return type + test helper
- `39ff851` — fix(fixtures): add evidence + ownership-map to Symbol available_views

**Debt-verify follow-ups completados**:
- e13-wave1 WARN-A: typed-accessor (`isSpotterHit` return type) ✅ DONE

**Remaining deferred**:
- `e13-wave2-universal-spotter` — bloqueado por puertos de arquitectura ausentes (DocRepository, ADR index, evidence store). Requiere SDD antes de implementar
- Bug #4 fixture fix parcial: `evidence` + `ownership-map` agregados; 2 de 11 skipped tests re-habilitados. Los otros 9 siguen skipped (views sin fixture data de body)
- Bug #5 mobile-320px: marcado `.fixme` en `responsive-full.spec.ts`. Requiere auditoría visual + fix UI

---

## Session Handover 2026-07-02

**moldql-intent-syntax-v1 completed, merged, tagged v0.44.0** (PR #103, branch `feat/moldql-intent-syntax-v1`):
- New module `src/moldql/intent.rs`: `lower_intent()` lowering/translation layer
- Pattern 1: `symbols where <cond>` → `FIND symbols WHERE <cond>`
- Pattern 2: `calls from '<id>' [depth N]` → `EXPLORE <id> THROUGH callees [DEPTH N]`
- Wired into `facades/moldql.rs` before `parser::parse()`
- 15 unit tests + 9 integration tests
- verify: PASS_WITH_WARNINGS, debt-verify: PASS_WITH_WARNINGS
- 0 critical, 2 warnings (facade match duplication, integration test coverage), 4 suggestions

**typed-overview-affordance-matrix-v1 completed, merged, tagged v0.43.0** (PR #102, branch `feat/typed-overview-affordance-matrix-v1`):
- Affordance matrix: static `AffordanceMatrix` per `InspectableObjectType`
- `GET /api/affordances/:object_type` endpoint
- `AffordanceCards` component in `PaneInspector` (empty state → quick-nav cards)
- `useAffordance` hook + Zod schemas
- 7 unit tests passing

**relation-candidates-v1 completed, merged, tagged v0.45.0** (PR #104, branch `feat/relation-candidates-v1`):
- `AnalysisService::find_dead_code()` + `candidates_for_reverse_edge()`
- reverse_edges type-blind HashMap fixed: `HashMap<SymbolId, HashSet<SymbolId>>` (was `HashMap<SymbolId, HashSet<SymbolId>>` without generic)
- 4 unit tests passing
- Verdict: PASS

---

## Session Handover 2026-06-29

Closed E19 milestone (C4 Investigation Model) + E20-1:
- E19-1 ✅ (PR #81) — Rename C4 Components → Structure
- E19-2 ✅ (PR #82) — 5-button level selector
- E19-3 ✅ (PR #83) — C4 overlays (drift + hotspots)
- E19-4 ✅ (PR #84, v0.34.0) — ComposedNarrativeExecutor
- E19-5 ✅ (PR #85, v0.35.0) — Expected architecture + boundary violation detection
- E20-1 ✅ (PR #86, v0.36.0) — C4 Mermaid export

### Pending
- E20-2: Mermaid trace export (call_graph, impact_radius, decision_trace, vertical_slice)
- E20-3: draw.io action
- E20-4: SVG/PNG snapshot
- E21-1: Investigation entity + PostgreSQL

### Key fixes applied
- NodeKind::Route #[cfg(multimodal)] guards in cognicode-core (pre-existing bug)
- Severity::Info match arm in graph.rs (cargo check --features multimodal)
- boundary_violation overlay wiring in GraphLanding.tsx (dead frontend plumbing)
- edge_style_class_for("depends_on") missing arm

### Technical debt
- PersistenceService ISP wide port (8 methods, 1 used) — pre-existing
- c4OverlaySlice.test.ts not extended for toggleBoundaryViolations

## Session Handover 2026-06-28

Closed e17 E2E coverage sprint (2 PRs, 127 E2E tests passing). Pruned 57 stale/orphaned branches. Trunk base clean.

## Session Handover 2026-06-28 (E18-1)

E18-1 LandingWorkbench closed via SDDK A-lite cycle:
- ADRs 003-005 written + ROADMAP restructured into E18-E21 milestones
- 3 stacked PRs (PR1 state, PR2 components, PR3 wiring) + 2 correction cycles
- Final verdict: PASS_WITH_WARNINGS (1 KindFilterChips UX bug + 35 stale snapshots documented)
- PR #76 opened, awaiting review. Follow-ups queued as hotfix PRs.

## Session Handover 2026-06-26

Continuing from 2026-06-25. Closed e9 (landing virtualization), e11 (truncation field naming), and started e12 (viewkind realization).

## Completed

| Change | Tag | Closed | Notes |
|--------|-----|--------|-------|
| `e9-landing-perf` | v0.26.0 | 2026-06-26 | Frontend-only: windowed virtualization of node-list fallback in `GraphLanding` when nodes > 200. Simple scroll-based window (ITEM_H=28px, 8 cols, 4 visible rows). Preserves all `data-testid` attributes and visual styling. 671 vitest + 602 Rust tests pass. |
| `e11-truncation-field-naming` | v0.26.1 | 2026-06-26 | Backend: renamed `ContextualGraphResponse.truncation_reason` → `truncated_reason`. Serde alias for backwards compat. Frontend: Zod schema + usage updated. 602 Rust + 671 vitest pass. ADR-002 updated. |
| `e12-viewkind-realization` | v0.27.0 | 2026-06-26 | UsageExamplesExecutor as 10th executor. `build_usage_examples` returns callers + callees as Table blocks. Gracefully degrades when graph_query is None. 4 new tests. Registry entry + static instance. |
| `e12b-api-surface` | v0.27.1 | 2026-06-26 | ApiSurfaceExecutor as 11th executor. Shows all scope symbols sorted by name as Table. Columns: name, kind, file, line. V1 pragmatic: no visibility filter (ResolvedSymbol has no visibility field). 4 new tests. |
| `e12c-test-slice` | v0.27.2 | 2026-06-26 | TestSliceExecutor as 12th executor. Shows test callers of a symbol via GraphQueryPort.callers() filtered by is_test_file heuristic. DocCodeAlignment deferred to Phase 2+ (EntryPoint::Doc not wired). 4 new tests. |
| `e12d-debug-slice` | v0.27.3 | 2026-06-26 | DebugSliceExecutor as 13th executor. Shows debug-relevant callers/callees filtered by name heuristic (debug,log,error,panic,dbg,trace,assert,check,verify,test,_test). Renderer: graph. 3 new tests. |
| `e12e-change-impact-story` | v0.27.4 | 2026-06-26 | ChangeImpactStoryExecutor as 15th executor. BFS upstream (callers) + downstream (callees) up to depth 3 as Table blocks. Renderer: Table. 4 new tests. PR #67 open. |
| `e17-e2e-coverage` | — | 2026-06-28 | E2E coverage sprint. PR #74: 13 McpToolsModal tests + ViewSpec save flow. PR #75: 13 new test specs (landing-real-data, pane-stack-drilldown, viewspec-wizard-full, view-tabs-coverage, spotter-multifamily, perspective-toggle-full, responsive-full, error-states-coverage, exploration-share, scan-progress, lens-panel, msw-fixture-consistency, call-graph-rendering-extended). 127 E2E passing. |
| `e17-e2e-coverage-audit` | — | 2026-06-28 | Audit branch integrated into PR #75. Same 13 specs as e17 above. Trunk pruned: 57 stale/orphaned branches removed. |

## Hygiene 2026-06-25

Closed before resuming new cycles:

- **Stashes**: 11 → 0. All 11 stashes dropped; patches preserved at `/tmp/stash-dump-2026-06-25/` (`00-refactor-error-enum.patch` ... `06-main-wip-aa8b951-e2e.patch`, 248 KB total). Notable discarded: `sdd/postgres-default-config` multimodal-docs-source (2358 insertions — was a Phase 4 spike, not aligned with current architecture).
- **Openspec changes**: 29 stale proposals moved to `openspec/changes/archive/`. Mix of incomplete proposals (no `proposal.md`) and old March/April context proposals (LSP, perf, refactoring suite, etc.). If any of those themes need to come back, they should be re-proposed with current context.
- **Branch `feat/e7-renderer-scale-evaluation`**: archived. The branch diverged from `main` by 1044 files (84811 insertions / 31801 deletions) and 0 of its commits had landed in `main`. The branch claimed "E7 is COMPLETED, WebGL adopted" but that work was never integrated; if WebGL adoption or renderer scale evaluation is needed, it should be re-scoped as a new cycle against current `main`.
- **Working tree**: clean. No uncommitted code; no untracked artifacts in `apps/explorer-ui/`.

## Completed

| Change | Tag | Closed | PR | Notes |
|--------|-----|--------|----|----|
| `typed-overview-affordance-matrix-v1` | v0.43.0 | 2026-07-02 | [#102](https://github.com/Rubentxu/CogniCode/pull/102) | Affordance matrix per InspectableObjectType. `GET /api/affordances/:object_type`. `AffordanceCards` in PaneInspector. 7 unit tests. Verdict PASS_WITH_WARNINGS. |
| `moldql-intent-syntax-v1` | v0.44.0 | 2026-07-02 | [#103](https://github.com/Rubentxu/CogniCode/pull/103) | MoldQL intent lowering layer: lowercase `symbols where` and `calls from` patterns translated to MoldQL AST before canonical parser. 15 unit + 9 integration tests. Verdict PASS_WITH_WARNINGS. |
| `relation-candidates-v1` | v0.45.0 | 2026-07-02 | [#104](https://github.com/Rubentxu/CogniCode/pull/104) | `AnalysisService::find_dead_code()` + `candidates_for_reverse_edge()`. reverse_edges type-blind HashMap fixed. 4 unit tests. Verdict PASS. |
| `refactor/view-registry-uniqueness` | v0.53.1 | 2026-07-23 | [#117](https://github.com/Rubentxu/CogniCode/pull/117) | Refactor: derive `view_kind` from trait, KSVConfig snapshot tests, HashSet deduplication. COMPLIANT on all scenarios. Verdict PASS_WITH_WARNINGS (3 carry-over warnings: W1/W2/H1). |
| `e25-decision-support-packs` | v0.55.1 | 2026-07-24 | [#121](https://github.com/Rubentxu/CogniCode/pull/121) | Decision Support Packs (E25.1): C-2 closure (ViewKind::DecisionSupportPack + executor + registry) + 7 PR2 verify fixes. Tests: 901 multimodal / 834 default all passing. Verdict PASS_WITH_WARNINGS (3 warnings, 4 suggestions). ADR-011 finalized. |
| `refactor/dup-001-get-node` | v0.55.2 | 2026-07-24 | [#122](https://github.com/Rubentxu/CogniCode/pull/122) | DUP-001 refactor: extract 3-way `get_node` match in `build_node_source_view` to use `resolve_focus_node` + `FocusResolution`. 903 multimodal / 836 default tests (was 901/834). Marker view for NotFound (graceful degradation). |
| `fix/e24-png-artifact-kind` | v0.55.3 | 2026-07-24 | [#123](https://github.com/Rubentxu/CogniCode/pull/123) | E24 PNG artifact kind fix: PNG exports were mislabeled as `kind=\"svg\"` because `addSvgArtifact` was reused for PNG content. Added `addPngArtifact()` helper and used it in ExportMenu PNG branch. ExportMenu tests: 15/15 passing. |
| `refactor/e25-viewkind-wire-tag` | v0.55.4 | 2026-07-24 | [#124](https://github.com/Rubentxu/CogniCode/pull/124) | W-001 refactor: derive `ViewKind` serde with `#[serde(rename_all = \"snake_case\")]`, eliminating triple duplication (35 variants × 3 match arms). -129 LOC delta. New ViewKind additions now require 1 edit instead of 3. 903 multimodal / 836 default tests pass. |
| `fix/topbar-shell-ids` | v0.55.5 | 2026-07-24 | [#125](https://github.com/Rubentxu/CogniCode/pull/125) | TopBar + Shell tablist `data-testid` IDs added (5 TopBar + 1 Shell) for E2E testability, a11y, and DOM debugging. Existing IDs preserved for backward compat. 916/917 vitest passing. |
| `e10-landing-real-data` | v0.25.0 | 2026-06-25 | [#60](https://github.com/Rubentxu/CogniCode/pull/60) | Landing backend now returns real semantic workspace seeds instead of empty stubs: `entry_points`, `hot_paths`, `god_nodes`, `nodes`, and `edges`. Implemented entirely through the Explorer seam (`GraphService` over `all_symbols()` + `GraphQueryPort`) without injecting `WorkspaceSession` into `ApiState`. `apply_landing_cap(total_entry_points)` now runs on real data, so the E8/E8b banner can activate on wide workspaces. 3 new landing integration tests; `api_graph_tests` 59/59 green; frontend vitest 671/671 unchanged. |
| `e8b-landing-payload-truncation` | v0.24.2 | 2026-06-25 | [#59](https://github.com/Rubentxu/CogniCode/pull/59) | Backend `LandingPayload` DTO: `+truncated: bool`, `+truncated_reason: Option<String>`. `LANDING_NODE_CAP = 50` constant. `apply_landing_cap(total)` pure helper as single source of truth. `landing_handler` calls `apply_landing_cap(0)` (handler still returns empty stubs; data wiring deferred to `e10-landing-real-data`). 9 new tests in `api_landing_truncation.rs` (5 helper boundary + 4 DTO serde), strict TDD. Banner remains dormant in production until `e10` wires real `entry_points` data through the `Graph` facade. |
| `e8-graphlanding-affordances` | v0.24.1 | 2026-06-25 | [#56](https://github.com/Rubentxu/CogniCode/pull/56) + [#57](https://github.com/Rubentxu/CogniCode/pull/57) + [#58](https://github.com/Rubentxu/CogniCode/pull/58) + [snapshot re-baseline `78b12eb`](https://github.com/Rubentxu/CogniCode/commit/78b12eb) | GraphLanding: truncation banner (dormant, awaiting `e8b`), canvas a11y (`role="application"` + `aria-label` + `tabIndex={0}`), node-list fallback of buttons, `selectObject` memoized via `useCallback`. Artifact endpoint: `/explorations/` → `/api/exploration-sessions/` aligned with ADR-040 Wave 3 (fixes pre-existing `generateArtifact` test failure). E2E: `page.route` → `addInitScript` for MSW compatibility; 24 visual-regression snapshots re-baselined. |
| `quality-stack-evolution` | v0.24.0 | 2026-06-25 | [#55](https://github.com/Rubentxu/CogniCode/pull/55) | C5 rename (`QualityIssue.file → file_path` with serde wire compat per D-1 B.1) + multi-workspace `quality_gate` scoping (`workspace_id: Option<&str>` per D-2) + quality agent ingest write-path (`QualityWritePort` trait + `PostgresQualityRepository` impl + `ingest_quality_issues` MCP tool with natural-key idempotency per D-3) |
| `quality-stack-pg-canonical` (+ v2) | v0.23.0 | 2026-06-25 | [#52](https://github.com/Rubentxu/CogniCode/pull/52) + follow-up `ad35e06` | Postgres-canonical quality stack: m0011_quality.sql migration + PostgresQualityRepository + issues_for_workspace + runtime wiring + 6 test mocks + 8 integration tests + parked-crates ADR |

## Future

Follow-ups explicitly queued by cycles closed today. Each will need its own proposal + spec before becoming Active.

| Candidate | Source cycle | Semver target | Why it exists |
|-----------|---|---|---|
| `refactor/e13-followup-typed-accessors` | e13-wave1 | PATCH | ✅ DONE PR #94 — type predicates for discriminated union |
| `impl/e13-investigation-scope-integration-test` | e13-wave1 | PATCH | ✅ DONE PR #95 — MSW fixtures + E2E coverage |
| `e13-wave2-universal-spotter` | ADR-002 Phase 2 | MINOR | Add doc/ADR/evidence families to Spotter. Needs new ports: DocRepository, ADR index, evidence store. **Blocked until ports exist.** |
| `e12f-ownership-map` | ADR-002 Phase 1 | MINOR | **ACTIVE — explore+propose+spec+design DONE**. OwnershipMap deferred: no ownership/author attribution in graph. Needs git blame (gix) + CODEOWNERS parsing. Pending: tasks + implement. |
| `e12g-risk-map` | ADR-002 Phase 1 | MINOR | RiskMap deferred: needs quality/hotspots data wired to graph. |
| `e12h-decision-trace` | ADR-002 Phase 1 | MINOR | DecisionTrace deferred: needs ADR/doc infrastructure. |

## Technical Debt

| Item | Severity | Source | Why | Status |
|------|---------|--------|-----|--------|
| 3 pre-existing Playwright failures (shell doesn't load in headless Chromium) | HIGH | E18-2 | cmdk `vimBindings` + MSW service worker not registering in headless CI (infra, not code) | Open |
| Pre-existing GraphLanding cytoscape error (canvas-of-type-2d) | LOW | unknown | unhandled canvas type in headless Chromium | Open |
| Pre-existing `postgres_quality_write_integration` failure | MEDIUM | unknown | `quality_write_unavailable_when_port_not_wired` assertion fails even with live postgres | Open |

## Strategic program: moldable exploration parity

Source of truth: [ADR-002](./adr/ADR-002-moldable-exploration-parity-program.md).

This program does **not** promise Smalltalk/Pharo image-level parity with
GToolkit. It targets **functional parity in exploration workflows**:

- objects are inspectable as first-class entities,
- each object has multiple contextual representations,
- navigation preserves narrative and supports drill-down,
- discovery is context-driven rather than menu-driven,
- durable explanations exist as executable or object-backed narratives.

### Current proven state

What is already implemented today:

- backend `ViewRegistry` + `ViewExecutor` + `ViewSpecStore`
- frontend `RendererRegistry` + `PaneStackView` + `PaneInspector`
- `MoldQL` execution + JSONata preview
- `Spotter` + `EntryPoint` / `ResolvedEntryPoint`
- WASM graph tooling (`god_nodes`, PageRank, SCC, etc.)
- real landing workspace overview (`entry_points`, `hot_paths`, `god_nodes`, nodes, edges)

What is **not** implemented yet:

- Lepiter-equivalent runtime (`ProjectDiary`, `ComposedNarrative`, `ExampleObject`)
- universal Spotter (today returns 8 families: Symbol, File, ViewSpec, SavedExploration, QualityIssue, Rule, Investigation, Scope)
- contextual editor beyond the JSONata textarea
- most catalogued `ViewKind`s as real executors (today 9 executors are wired; the catalog is much broader)

### Program phases

| Phase | Candidate | Semver target | Primary crates | Goal |
|---|---|---|---|---|
| 0 | `e9-landing-perf` | PATCH | `cognicode-explorer` | Virtualise the fallback node list if large workspaces cause DOM bloat |
| 0 | `e11-context-response-field-naming` | PATCH | `cognicode-explorer` | Harmonise `truncated_reason` vs `truncation_reason` naming without breaking the wire contract |
| 1 | `e12-viewkind-realization` | MINOR | `cognicode-explorer`, `cognicode-core`, `cognicode-graph-algos` | Convert high-value catalogued `ViewKind`s into real executors and renderers |
| 2 | `e13-universal-spotter` | MINOR | `cognicode-explorer`, `cognicode-core` | Wave 1 ✅ (Investigation + Scope, 6→8 families, frontend desync fixed). Typed-accessor WARN-A ✅. Wave 2 (DocRepository, ADR index, evidence store ports — bloqueado, requiere SDD). Wave 3 (narratives) remaining |
| 3 | `e14-narrative-runtime` | MAJOR | `cognicode-explorer`, `cognicode-core` | Implement `ProjectDiary`, `ComposedNarrative`, and `ExampleObject` as runtime artifacts, not just catalog entries |
| 4 | `e15-contextual-editor` | MINOR or MAJOR | `cognicode-explorer`, `cognicode-core` | Add a real contextual editor with references, completion, peek, and graph-aware edit workflows |
| 5 | `e16-federated-runtime-objects` | MAJOR | `cognicode-explorer`, `cognicode-core`, `cognicode-graph-algos` | Make more runtime/domain objects explorable and passable to agents as structured objects |

### View-coverage reality check

The parity gap is not abstract — it is measurable:

- `ViewRegistry.known_view_kinds()` exposes a broad catalog including
  `ComposedNarrative`, `ProjectDiary`, `ExampleObject`, `ConceptMap`,
  `EvidencePack`, `UsageExamples`, `ApiSurface`, `DocCodeAlignment`,
  `TestSlice`, `DebugSlice`, `OwnershipMap`, `RiskMap`, and more.
- The currently wired real executors in
  `crates/cognicode-explorer/src/registry.rs:336-382` are only:
  `overview`, `call-graph`, `source`, `quality`, `evidence`, `symbols`,
  `dependencies`, `hotspots`, `architecture-drift`.

`e12-viewkind-realization` should therefore begin by shipping executors for
the highest-leverage missing views:

1. `usage_examples`
2. `api_surface`
3. `doc_code_alignment`
4. `ownership_map`
5. `test_slice`
6. `debug_slice`
7. `concept_map`
8. `evidence_pack`

### Definition of parity for planning purposes

We may only claim **similar moldable exploration functionality** when all of
the following are true:

1. High-value catalogued `ViewKind`s have real executors.
2. Spotter is universal across the main object families.
3. `ProjectDiary`, `ComposedNarrative`, and `ExampleObject` are runtime
   capabilities, not just enum values and wizard options.
4. There is at least one contextual editor experience beyond a textarea.
5. Exploration outputs can be turned into durable narratives that survive
   across sessions and can be inspected by both humans and AI.

The 3 previously-listed items (`cognicode-axiom`, `cognicode-quality`, `cognicode-rule-test-harness` re-activation) were **archived** on 2026-06-25 per ADR-001 trigger (b) — moved to `docs/parked-crates/` rather than revived. See ADR-001 §Archive Action. The C5 rename, multi-workspace `quality_gate`, and quality agent ingest items shipped in v0.24.0.

## Conventions

- Roadmap entries are **date-sorted descending** within each section.
- Each entry links to: branch (Active), tag + PR (Completed), or ADR/scenario (Future).
- The `quality-stack-pg-canonical` entry includes a follow-up commit (`ad35e06`) that landed AFTER the original PR merged; both are part of the same change for the purposes of this roadmap.
- When an item shifts from Future to Completed (or to Archived), the entry is moved and the source ADR/spec is cited.
