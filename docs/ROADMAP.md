# CogniCode Roadmap

Last updated: 2026-06-26 (e9 + e11 + e12 + e12b + e12c + e12d + e12e completed; e12e PR open)

## Active

_(none — all cycles closed)_

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

## Hygiene 2026-06-25

Closed before resuming new cycles:

- **Stashes**: 11 → 0. All 11 stashes dropped; patches preserved at `/tmp/stash-dump-2026-06-25/` (`00-refactor-error-enum.patch` ... `06-main-wip-aa8b951-e2e.patch`, 248 KB total). Notable discarded: `sdd/postgres-default-config` multimodal-docs-source (2358 insertions — was a Phase 4 spike, not aligned with current architecture).
- **Openspec changes**: 29 stale proposals moved to `openspec/changes/archive/`. Mix of incomplete proposals (no `proposal.md`) and old March/April context proposals (LSP, perf, refactoring suite, etc.). If any of those themes need to come back, they should be re-proposed with current context.
- **Branch `feat/e7-renderer-scale-evaluation`**: archived. The branch diverged from `main` by 1044 files (84811 insertions / 31801 deletions) and 0 of its commits had landed in `main`. The branch claimed "E7 is COMPLETED, WebGL adopted" but that work was never integrated; if WebGL adoption or renderer scale evaluation is needed, it should be re-scoped as a new cycle against current `main`.
- **Working tree**: clean. No uncommitted code; no untracked artifacts in `apps/explorer-ui/`.

## Completed

| Change | Tag | Closed | PR | Notes |
|--------|-----|--------|----|----|
| `e10-landing-real-data` | v0.25.0 | 2026-06-25 | [#60](https://github.com/Rubentxu/CogniCode/pull/60) | Landing backend now returns real semantic workspace seeds instead of empty stubs: `entry_points`, `hot_paths`, `god_nodes`, `nodes`, and `edges`. Implemented entirely through the Explorer seam (`GraphService` over `all_symbols()` + `GraphQueryPort`) without injecting `WorkspaceSession` into `ApiState`. `apply_landing_cap(total_entry_points)` now runs on real data, so the E8/E8b banner can activate on wide workspaces. 3 new landing integration tests; `api_graph_tests` 59/59 green; frontend vitest 671/671 unchanged. |
| `e8b-landing-payload-truncation` | v0.24.2 | 2026-06-25 | [#59](https://github.com/Rubentxu/CogniCode/pull/59) | Backend `LandingPayload` DTO: `+truncated: bool`, `+truncated_reason: Option<String>`. `LANDING_NODE_CAP = 50` constant. `apply_landing_cap(total)` pure helper as single source of truth. `landing_handler` calls `apply_landing_cap(0)` (handler still returns empty stubs; data wiring deferred to `e10-landing-real-data`). 9 new tests in `api_landing_truncation.rs` (5 helper boundary + 4 DTO serde), strict TDD. Banner remains dormant in production until `e10` wires real `entry_points` data through the `Graph` facade. |
| `e8-graphlanding-affordances` | v0.24.1 | 2026-06-25 | [#56](https://github.com/Rubentxu/CogniCode/pull/56) + [#57](https://github.com/Rubentxu/CogniCode/pull/57) + [#58](https://github.com/Rubentxu/CogniCode/pull/58) + [snapshot re-baseline `78b12eb`](https://github.com/Rubentxu/CogniCode/commit/78b12eb) | GraphLanding: truncation banner (dormant, awaiting `e8b`), canvas a11y (`role="application"` + `aria-label` + `tabIndex={0}`), node-list fallback of buttons, `selectObject` memoized via `useCallback`. Artifact endpoint: `/explorations/` → `/api/exploration-sessions/` aligned with ADR-040 Wave 3 (fixes pre-existing `generateArtifact` test failure). E2E: `page.route` → `addInitScript` for MSW compatibility; 24 visual-regression snapshots re-baselined. |
| `quality-stack-evolution` | v0.24.0 | 2026-06-25 | [#55](https://github.com/Rubentxu/CogniCode/pull/55) | C5 rename (`QualityIssue.file → file_path` with serde wire compat per D-1 B.1) + multi-workspace `quality_gate` scoping (`workspace_id: Option<&str>` per D-2) + quality agent ingest write-path (`QualityWritePort` trait + `PostgresQualityRepository` impl + `ingest_quality_issues` MCP tool with natural-key idempotency per D-3) |
| `quality-stack-pg-canonical` (+ v2) | v0.23.0 | 2026-06-25 | [#52](https://github.com/Rubentxu/CogniCode/pull/52) + follow-up `ad35e06` | Postgres-canonical quality stack: m0011_quality.sql migration + PostgresQualityRepository + issues_for_workspace + runtime wiring + 6 test mocks + 8 integration tests + parked-crates ADR |

## Future

Follow-ups explicitly queued by cycles closed today. Each will need its own proposal + spec before becoming Active.

| Candidate | Source cycle | Semver target | Why it exists |
|-----------|---|---|---|
| `e12f-ownership-map` | ADR-002 Phase 1 | MINOR | OwnershipMap deferred: no ownership/author attribution in graph. Needs git blame or author annotation as node property. |
| `e12g-risk-map` | ADR-002 Phase 1 | MINOR | RiskMap deferred: needs quality/hotspots data wired to graph. |
| `e12h-decision-trace` | ADR-002 Phase 1 | MINOR | DecisionTrace deferred: needs ADR/doc infrastructure. |

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
- universal Spotter (today it returns only `Symbol` and `ViewSpec`)
- contextual editor beyond the JSONata textarea
- most catalogued `ViewKind`s as real executors (today 9 executors are wired; the catalog is much broader)

### Program phases

| Phase | Candidate | Semver target | Primary crates | Goal |
|---|---|---|---|---|
| 0 | `e9-landing-perf` | PATCH | `cognicode-explorer` | Virtualise the fallback node list if large workspaces cause DOM bloat |
| 0 | `e11-context-response-field-naming` | PATCH | `cognicode-explorer` | Harmonise `truncated_reason` vs `truncation_reason` naming without breaking the wire contract |
| 1 | `e12-viewkind-realization` | MINOR | `cognicode-explorer`, `cognicode-core`, `cognicode-graph-algos` | Convert high-value catalogued `ViewKind`s into real executors and renderers |
| 2 | `e13-universal-spotter` | MINOR | `cognicode-explorer`, `cognicode-core` | Expand Spotter to docs, ADRs, evidence, issues, saved explorations, narratives, examples, and more object families |
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
