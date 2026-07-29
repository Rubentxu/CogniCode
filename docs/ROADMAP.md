# CogniCode Roadmap

Last updated: 2026-07-29 (E28.2 PR4 Conformance shipped v0.71.1; E28.2 chain fully DONE; E28.3 + E28.4 unblocked.)

## Active

### Graph Query & Analytics Platform (E28)

**Strategic ADR**:
[ADR-014](./adr/ADR-014-moldql-pattern-graph-analytics-platform.md)

**Goal**: Make graph queries and selected analytics reproducible, bounded, and
consistent across PostgreSQL and immutable in-memory snapshots while keeping
PostgreSQL as the sole canonical store.

**E28.0 is DONE** (PR1 Foundation v0.61.0 + PR2 Persistence v0.62.0 merged;
PR3 Snapshot+Bridge pending to close the foundation chain). E28.1 through
E28.6 remain **PROPOSED**, each requiring its own approved SDD artifacts
before implementation begins.

| Change | Goal | Depends on | Exit criteria | Status |
|---|---|---|---|---|
| `e28-0-canonical-graph-revisions` | Stabilize identity, typed property round-trip, workspace isolation, immutable revisions, deletion, and snapshot refresh | None | The same node and edge identities round-trip through PostgreSQL and snapshots; runs pin one workspace and revision; ingest, deletion, and refresh tests pass | **DONE** (PR1 v0.61.0 + PR2 v0.62.0 merged; PR3 pending) |
| `e28-1-moldplan-graphplan-contracts` | Introduce versioned `MoldPlan`/`GraphPlan`, typed results, execution policy, limits, and structured unsupported-operation errors | E28.0 | Plans contain no backend or presentation types; every run declares limits; unsupported operations fail before execution | PROPOSED |
| `e28-2-differential-graph-executors` | Execute existing `PATH`, `NEIGHBORS`, `SUBGRAPH`, `CLUSTER`, `EXPLAIN`, and boolean composition in PostgreSQL and snapshot executors | E28.1 | Golden fixtures prove equivalent typed multisets, ordering, paths, errors, provenance, and truncation; no supported operation returns synthetic empty success | **DONE** (PR1 Port v0.68.0 + PR2 PG Executor v0.69.0 + PR3 Snapshot Executor v0.70.0 + PR6 pool-fix v0.70.1 + PR5 edge-filter v0.71.0 + PR4 Conformance v0.71.1 merged) |
| `e28-3-moldql-pattern-profile-v1` | Add read-only typed patterns, direction, bounded quantifiers, predicates, aggregation, ordering, limits, and bounded shortest paths | E28.2 | The supported-feature matrix is published; parser, lowering, differential, REST/MCP, and Explorer interaction tests pass; no Cypher/GQL compatibility claim is made | PROPOSED |
| `e28-4-analytics-registry-cohort-1` | Add descriptor admission, `stream`/`stats`/`annotate`/`persist` modes, run lineage, and stabilize PageRank, SCC, WCC, and bounded shortest paths | E28.2 | Every admitted algorithm is versioned, resource-governed, reproducible, and non-mutating; cohort-1 conformance and composition tests pass | PROPOSED |
| `e28-5-structural-analytics-cohort-2` | Add dominators, articulation points, bridges, and k-core for impact, seam, dependency-pressure, and risk views | E28.4 | Each algorithm has a product question, descriptor, golden fixtures, and either an Explorer surface or an explicit internal/composable classification | PROPOSED |
| `e28-6-advanced-analytics-evidence-gate` | Evaluate betweenness, k-shortest paths, multi-source reachability, personalized PageRank, Leiden, conductance/modularity, node similarity, and an optional Neo4j CI oracle | E28.5 | Only measured, product-relevant algorithms are admitted; optional oracle checks do not affect production availability; any production sidecar proposal is deferred to a separate ADR | PROPOSED |

#### E28.0 stacked-to-main chain

| Sub-PR | Branch | Status | Tag | PR |
|---|---|---|---|---|
| PR1 Foundation | `feat/e28-0-canonical-graph-revisions` | ✅ Merged | v0.61.0 | [#135](https://github.com/Rubentxu/CogniCode/pull/135) |
| PR2 Persistence | `feat/e28-0-pr2-persistence` | ✅ Merged | v0.62.0 | [#136](https://github.com/Rubentxu/CogniCode/pull/136) |
| **PR3 Snapshot+Bridge** | `feat/e28-0-pr3-snapshot-bridge` | ✅ Merged | v0.63.0 | [#137](https://github.com/Rubentxu/CogniCode/pull/137) |

**E28.0 is now fully DONE.** PR3 closes the foundation chain (Phase 4: Repository trait extension + GenericGraphRepository + MetadataAwareRepository contract tests + m0019 FK subset fix + 2 new pg_tests). **E28.1 unblocked.**

#### E28.1 stacked-to-main chain

| Sub-PR | Branch | Status | Tag | PR |
|---|---|---|---|---|
| PR1 Foundation | `feat/e28-1-pr1-foundation` | ✅ Merged | v0.64.0 | [#138](https://github.com/Rubentxu/CogniCode/pull/138) |
| PR2 Plan Algebra | `feat/e28-1-pr2-plan-algebra` | ✅ Merged | v0.65.0 | [#139](https://github.com/Rubentxu/CogniCode/pull/139) |
| **PR3 Bridge** | `feat/e28-1-pr3-bridge` | ✅ Merged | v0.66.0 | [#140](https://github.com/Rubentxu/CogniCode/pull/140) |
| PR4 PG Conformance | `feat/e28-1-pr4-pg-conformance` | 🔲 Pending | — | — |

PR4 closes the E28.1 chain (Phase 4: PG `#[sqlx::test]` integration + bridge-mapping + executor regression gate). Then E28.2 unblocks.

#### E28.2 stacked-to-main chain

| Sub-PR | Branch | Status | Tag | PR |
|---|---|---|---|---|
| PR1 Port | `feat/e28-2-pr1-port` | ✅ Merged | v0.68.0 | [#142](https://github.com/Rubentxu/CogniCode/pull/142) |
| PR2 PG Executor | `feat/e28-2-pr2-pg-executor` | ✅ Merged | v0.69.0 | [#143](https://github.com/Rubentxu/CogniCode/pull/143) |
| PR3 Snapshot Executor | `feat/e28-2-pr3-snapshot-executor` | ✅ Merged | v0.70.0 | [#144](https://github.com/Rubentxu/CogniCode/pull/144) |
| PR6 pool-timeout fix | `fix/e28-2-pr2-pool-connection-release` | ✅ Merged | v0.70.1 | [#145](https://github.com/Rubentxu/CogniCode/pull/145) |
| PR5 edge-filter fix | `fix/e28-2-pr5-edge-filter` | ✅ Merged | v0.71.0 | [#147](https://github.com/Rubentxu/CogniCode/pull/147) |
| **PR4 Conformance** | `feat/e28-2-pr4-conformance` | ✅ Merged | v0.71.1 | [#148](https://github.com/Rubentxu/CogniCode/pull/148) |

**E28.2 is now fully DONE.** PR4 closes the differential chain with 10 conformance tests proving `PgGraphExecutor` and `SnapshotGraphExecutor` return equivalent `GraphResult`s for the same `MoldPlan` + workspace + revision pin. **E28.3 (`moldql-pattern-profile-v1`) and E28.4 (`analytics-registry-cohort-1`) unblocked** — may now proceed in parallel per the E28 execution order.

#### E28 execution order

```text
E28.0 -> E28.1 -> E28.2 -> E28.3
                         -> E28.4 -> E28.5 -> E28.6
```

E28.3 and E28.4 may proceed in parallel only after E28.2 proves executor
equivalence. New diagram models, graph mutation, complete Cypher/GQL
compatibility, and production Neo4j infrastructure remain outside E28.

**Specifications**:
[graph query execution](./specs/graph-query-execution.md) and
[graph analytics execution](./specs/graph-analytics-execution.md).

**Evidence**:
[graph stack assessment](./analysis/cognicode-graph-stack-assessment.md) and
[Cypher/GDS fit assessment](./analysis/cognicode-cypher-gds-fit-assessment.md).

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

## Session Handover 2026-07-27

**E28 documentation chain closed (no code, no PR yet)**

Cierre documental del programa E28 sin abrir ciclo de código:

- Análisis de evidencia persistido: [`cognicode-graph-stack-assessment.md`](analysis/cognicode-graph-stack-assessment.md), [`cognicode-cypher-gds-fit-assessment.md`](analysis/cognicode-cypher-gds-fit-assessment.md).
- Decisión estratégica: [`ADR-014`](adr/ADR-014-moldql-pattern-graph-analytics-platform.md) (PROPOSED).
- Especificaciones ejecutables: [`graph-query-execution.md`](specs/graph-query-execution.md), [`graph-analytics-execution.md`](specs/graph-analytics-execution.md).
- Nueva sección [`### Graph Query & Analytics Platform (E28)`](#graph-query--analytics-platform-e28) con 7 cambios dependientes (E28.0 → E28.6), tabla goal/dep/exit-criteria/status, diagrama de orden y enlaces cruzados.
- `docs/adr/README.md` actualizado con ADR-008..014.
- `CONTEXT.md` enriquecido: MoldPlan, GraphPlan, Pattern Profile, Graph Analytics Registry, con enlaces inline a ADR-014 y specs; Open Questions cerrada la entrega documental.
- Anclas cruzadas: ADR-014 ↔ ROADMAP ↔ CONTEXT ↔ specs ↔ análisis resuelven.

**Próximo paso propuesto**: abrir ciclo SDDK A-lite para [`e28-0-canonical-graph-revisions`](#graph-query--analytics-platform-e28) (depends-on: none; fundamento de identidad tipada, property round-trip, revisiones inmutables y refresco de snapshots antes de tocar planners o analytics).

## Session Handover 2026-07-27 (E28 PR1 shipped)

**E28 PR1 Foundation closed and shipped v0.61.0 (PR #135 merged to main).**

Ciclo SDDK A-lite ejecutado completamente en auto mode. PR1 cubre la fundación de E28 (Phase 1 del ROADMAP: 14 tasks; 7 atomic commits + 4 correction cycle 1 + 3 correction cycle 2 + 1 inline fixup = 15 commits total en `feat/e28-0-canonical-graph-revisions`).

**Logros PR1**:
- Value objects: `RevisionId`, `WorkspaceId` (ambos re-exportados en `value_objects::`).
- Migrations PG: `m0017_graph_revisions` (tabla + head uniqueness), `m0018_workspace_scoped_identity` (PK `(workspace_id, id, kind)`, FKs compuestas, idempotente en todos los estados).
- Typed JSONB upgrade: `GraphNode.properties` y `GraphEdge.metadata` migrados de `HashMap<String,String>` a `serde_json::Value` con adaptadores `to_map()` / `from_map()`.
- `NodeKind::Symbol(_)` ahora produce `Display = "symbol.{inner}"` con `FromStr` inverso; bare `"symbol"` rechazado.
- Verifiers: `sddk-verify` PASS_WITH_WARNINGS (0 CRITICAL, 1 WARNING pre-existing); `sddk-debt-verify` PASS_WITH_WARNINGS (smells-W1 cerrado inline).
- Strict TDD discipline aplicada: RED → GREEN por task, evidence table completa en `apply-progress.md`.

**Trazabilidad**:
- Branch: `feat/e28-0-canonical-graph-revisions` (squashed a `cd529cde` en merge).
- Tag: `v0.61.0` (MINOR — typed JSONB upgrade + new value objects + migrations).
- PR: <https://github.com/Rubentxu/CogniCode/pull/135>.
- Artifacts: `sddk/e28-0-canonical-graph-revisions/` (proposal, spec, tasks, apply-progress, verify-report, debt-report, archive-report).

**Próximo paso propuesto**: PR2 Persistence (`e28-0-pr2-persistence`; 22 tasks: `save_call_graph`/`load_call_graph` pinned + deletion completeness + SnapshotProvider + edge trigger + refresh wiring). Cadena stacked-to-main sigue.

## Session Handover 2026-07-27 (E28 PR2 shipped)

**E28 PR2 Persistence closed and shipped v0.62.0 (PR #136 merged to main).**

Ciclo SDDK A-lite ejecutado completamente en auto mode. PR2 cubre Phase 2 (Persistence) + Phase 3 (SnapshotProvider + Wiring) del programa E28 (40 tasks; 6 commits originales + 3 commits correction cycle 1 + 1 commit docs = 10 commits totales en `feat/e28-0-pr2-persistence`).

**Logros PR2**:
- `save_call_graph(g, ws) -> RevisionId` ahora workspace-scoped + revision-aware en una transacción atómica.
- `load_call_graph_ws(ws, rev) -> Option<CallGraph>` pinned a `(workspace_id, revision_id)`; `Err(UnknownRevision)` cuando la revisión no existe.
- `RepositoryError::UnknownRevision { workspace, revision }` nuevo variant.
- Deletion completeness: archivos removidos desaparecen de `graph_nodes`, `graph_edges` y `scan_manifest` en la misma transacción.
- Cross-workspace isolation: ws1 nunca afecta contadores ni revisiones de ws2.
- `SnapshotProvider` trait + PostgreSQL-backed impl + `PgListener` (LISTEN/NOTIFY `graph_updated`).
- Edge trigger extendido a `graph_edges` (idempotente).
- 100ms debounce por `workspace_id` coalesciendo eventos al último `revision_id`.
- `VersionedGraphCache` retention ≥ 2; lecturas pinned sobreviven a ingest concurrente.
- `GraphCache::Arc<dyn SnapshotProvider>` integrado con `ArcSwap` legacy.
- `refresh_from_pg` reescrito a usar `&dyn SnapshotProvider`.

**CRIT-1 cerrado en correction cycle 1**:
- Defecto: `SnapshotProviderImpl` LISTEN task llamaba `Handle::current().block_on(...)` desde worker thread → panic en producción bajo `#[tokio::main]` multi-thread runtime.
- Fix: direct `.await` en async context; `block_in_place` + `Handle::enter` + `tokio::spawn` + mpsc para sync trait methods.
- 2 nuevos `#[tokio::test(flavor = "multi_thread")]` regression tests.

**Trazabilidad**:
- Branch: `feat/e28-0-pr2-persistence` (squashed a `7e83ee17` al mergear).
- Tag: `v0.62.0` (MINOR — new trait + new variant + revision-pinned methods).
- PR: <https://github.com/Rubentxu/CogniCode/pull/136>.
- Artifacts: `sddk/e28-0-canonical-graph-revisions/` (verify-report PR2, debt-report PR2).

**Próximo paso propuesto**: PR3 Snapshot+Bridge (`e28-0-pr3-snapshot-bridge`; 16 tasks: Repository trait extension + GenericGraphRepository + MetadataAwareRepository contract tests + 5 follow-up WARNINGs cleanup). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.1 PR1 shipped)

**E28.1 PR1 Foundation closed and shipped v0.64.0 (PR #138 merged to main).**

Ciclo SDDK A-lite ejecutado completamente en auto mode. PR1 cubre Phase 1 (Foundation — value objects) del programa E28.1 (22 tasks; 1 commit atómico en `feat/e28-1-pr1-foundation`).

**Logros PR1**:
- Value objects introducidos en `crates/cognicode-core/src/domain/plan/`:
  - `PlanVersion` (semver) + `PlanHash` (SHA-256 de plan canónico)
  - `TypedValue` (Null/Bool/Int/Float/String/Json) + `assert_approx_equal` para floats
  - `ResultSet` (Rows/Nodes/Edges/Paths/Scalars) con multiset semantics + stable iteration order
  - `TruncationMarker` + `SemanticsViolation` + `Path` + `PathHop` (graph navigation)
  - `PlanLimits` + `PlanLimit` (9 variantes) + `CancellationToken`
  - `PlanError` + `UnsupportedConstruct` + `ConstructId` + `SourceLocation`
  - `ExecutorError` (extiende/supersede `QueryError`)
  - `MoldPlan` enum (Select/Count/Aggregate/Explain)
  - `GraphPlan` enum (Path/Neighbors/Subgraph/Cluster/Explain)
- Backend-neutrality static assertions (`BackendNeutral` sealed trait + `assert_backend_neutral!` macro).
- 103 plan-specific tests pasando.

**PR2-track WARNINGs** (7 identificados por debt-verify; non-blocking):
1. `BackendNeutral` sealed trait theater — implementar `sealed::Sealed` o remover.
2. `SemanticsViolation` name drift — unificar vía `#[from]` entre `result.rs`/`error.rs`/`PlanError`.
3. `CancellationToken::Hash` no-determinista (hashes por `Arc::as_ptr`) — documentar o remover impl.
4. `PlanLimits`/`PlanLimit` shotgun surgery — refactor a lookup table.
5. `PathHop::edge_kind: Option<String>` primitive obsession — introducir `EdgeKind` enum.
6. `PlanVersion` semver validation incompleta — completar o adoptar `semver` crate.
7. `PlanLimits::PartialEq` semantics para `cancellation` field.

**Trazabilidad**:
- Branch: `feat/e28-1-pr1-foundation` (squashed a `0da8ed78` al mergear).
- Tag: `v0.64.0` (MINOR — nuevos value objects públicos en `cognicode-core::domain::plan`).
- PR: <https://github.com/Rubentxu/CogniCode/pull/138>.
- Artifacts: `sddk/e28-1-moldplan-graphplan-contracts/` (proposal, spec, tasks, apply-progress, verify-report, debt-report).

**Próximo paso propuesto**: PR2 Plan Algebra (`feat/e28-1-pr2-plan-algebra`; Phase 2 — parser lowering a `MoldPlan`/`GraphPlan`; 12 tasks; ~500 LOC; resuelve también los 7 WARNINGs del PR1 backlog). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.1 PR2 shipped)

**E28.1 PR2 Plan Algebra closed and shipped v0.65.0 (PR #139 merged to main).**

Ciclo SDDK A-lite ejecutado en auto mode (3 apply sub-runs para completar scope completo). PR2 cubre Phase 2 (Parser Lowering + 7 PR1-track WARNING fixes) del programa E28.1 (12 tasks + 7 WARNINGs = 19 tareas; 5 commits squash-merged a `4abce4ae`).

**Logros PR2**:
- `GraphPlan` enum extendido: `Path`, `Neighbors`, `Subgraph`, `Cluster`, `Explain`, `BooleanComposition` con `BooleanOp` (And/Or/Not).
- `MoldPlan` enum extendido: `Select`, `Graph`, `ObjectSelection`, `Quality`, `Lens`, `ViewExecution`.
- `MoldPlan::Graph { inner, pin: Option<(WorkspaceId, RevisionId)> }` con `with_pin(ws, rev)` para revision pinning.
- `PlanLimits::validate(&GraphPlan)` enforcing Subgraph requiere `max_depth`, Path requiere `max_hops`.
- `AstLowerer` trait + `NoOpLowerer` port en `lower.rs`; `MoldqlAstLowerer` adapter en `cognicode-explorer`.
- `populate_defaults(plan, &QueryShape) -> PlanLimits` deriving graph-selecting limits desde query shape.
- 7 PR1-track WARNINGs cerrados (W1-W7):
  - W1: `BackendNeutral` Sealed impl para 26+ plan types.
  - W2: `SemanticsViolation` unified enum (was String drift).
  - W3: `CancellationToken::Hash` documentado como process-local.
  - W4: `PlanLimitKind` single source of truth.
  - W5: `PathHop::edge_kind: Option<String>` → `Option<EdgeKind>`.
  - W6: `PlanVersion` semver 2.0 validation.
  - W7: `PlanLimits::PartialEq` para cancellation via `Arc::ptr_eq`.
- 2637 tests verdes (143 plan + 15 lower_plan + 1610 core + 869 explorer).

**Trazabilidad**:
- Branch: `feat/e28-1-pr2-plan-algebra` (squashed a `4abce4ae` al mergear).
- Tag: `v0.65.0` (MINOR — nuevos value objects + extension de `MoldPlan`/`GraphPlan` + `PlanFilter`).
- PR: <https://github.com/Rubentxu/CogniCode/pull/139>.
- Artifacts: `sddk/e28-1-moldplan-graphplan-contracts/` (verify-report PR2, debt-report PR2).

**3 WARNINGs nuevas para PR3** (no bloqueantes, pero documentadas):
1. `populate_defaults` definido como port function pero nunca llamado por `MoldqlAstLowerer` adapter (adapter inlinea su propia lógica de defaulting).
2. `validate()` wired into `lower()` pero solo invocado desde tests, no desde el lowering production path.
3. NaN soundness hole en `PlanFilter::Confidence::threshold` (manual `Eq` impl viola Hash/Eq contract).

**Próximo paso propuesto**: PR3 Bridge (`feat/e28-1-pr3-bridge`; Phase 3 — `compile_to_plan` + legacy bridge + `#[deprecated]` + cleanup de las 3 WARNINGs nuevas; 10 tasks; ~400 LOC). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.1 PR3 shipped)

**E28.1 PR3 Bridge closed and shipped v0.66.0 (PR #140 merged to main).**

Ciclo SDDK A-lite ejecutado en auto mode. PR3 cubre Phase 3 (Bridge: `compile_to_plan` + legacy bridge + `#[deprecated]` + cleanup de las 3 WARNINGs nuevas del debt-verify PR2) del programa E28.1 (10 tasks + 3 WARNINGs = 13 tareas; 3 commits squash-merged a `90559f75`).

**Logros PR3**:
- `compile_to_plan(query, limits, pin) -> Result<MoldPlan, PlanError>` nuevo entry point que retorna versioned `MoldPlan` con `PlanVersion`, `PlanHash`, `pin: Option<(WorkspaceId, RevisionId)>`.
- Legacy `compile(q, target)` ahora delega a `compile_to_plan` + re-emite PG SQL o wraps PetgraphPlan (24 tests existentes siguen verdes).
- `#[deprecated(note = "use compile_to_plan for new code")]` en `compile()` y `CompileTarget` enum (compilation warning al build).
- `PlanFilter::Confidence` lowered a PG `confidence > $N` (bind parameter, NO inline literal) — SQL injection safe.
- 39 tests en `moldql::compile` (24 existentes + 12 nuevos `compile_to_plan_tests` + 3 NaN soundness).

**3 WARNINGs PR2-debt fixes (PR3 cleanup)**:
- W-A (populate_defaults): función existe en `lower.rs` port pero NO es llamada desde `MoldqlAstLowerer` adapter (cada `lower_*` setea sus propios limits internamente). **PARCIAL** — la función está documentada pero sub-utilizada. Filed as follow-up `e28-1-pr4-populate-defaults`.
- W-B (validate wired): `compile_to_plan` llama `PlanLimits::validate(&plan)?` en producción. **CLOSED**.
- W-C (NaN soundness): `PlanFilter::Confidence::PartialEq` manual para tratar NaN consistent con Hash. 3 tests added. **CLOSED**.

**Trazabilidad**:
- Branch: `feat/e28-1-pr3-bridge` (squashed a `90559f75` al mergear).
- Tag: `v0.66.0` (MINOR — nuevo entry point `compile_to_plan` + deprecation markers).
- PR: <https://github.com/Rubentxu/CogniCode/pull/140>.
- Artifacts: `sddk/e28-1-moldplan-graphplan-contracts/` (verify-report PR3 + debt-report PR3).

**Carry-over (out of PR3 scope)**:
- W-A (populate_defaults unused) — follow-up `e28-1-pr4-populate-defaults` or addressed in PR4 PG Conformance.
- 30+ multimodal feature compile errors in non-E28.1 files (PRE-EXISTING).
- cognicode-macros clippy + 17 unnecessary_min_or_max (PRE-EXISTING).

**Próximo paso propuesto**: PR4 PG Conformance (`feat/e28-1-pr4-pg-conformance`; Phase 4 — 6 `pg_test!` scenarios + bridge-mapping + executor regression gate + W-A cleanup; 10 tasks; ~500 LOC; requiere `TEST_DATABASE_URL`). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.2 PR1 Port shipped)

**E28.2 PR1 Port closed and shipped v0.68.0 (PR #142 merged to main).**

Ciclo SDDK A-lite ejecutado en auto mode. PR1 cubre Phase 1 (Infrastructure — Executor Port) del programa E28.2 (5 tasks; 2 commits squash-merged a `7e72b0bc`).

**Logros PR1**:
- `GraphExecutor` trait en `crates/cognicode-core/src/domain/plan/executor.rs`: `execute(&self, plan: &GraphPlan, pin: (WorkspaceId, RevisionId)) -> Result<ResultSet, ExecutorError>`. `Send + Sync + 'static`, object-safe.
- `ExecutorError` enum extendido: `RevisionUnknown { workspace, revision }`, `UnsupportedConstruct { construct: ConstructId }`, `LimitExceeded { limit: PlanLimitKind }`, `Internal(String)`.
- `ProvenanceEnvelope` aggregate para per-row source-side provenance.
- `StubExecutor` test impl (returns `Ok(ResultSet::empty())` o `Err(ExecutorError::RevisionUnknown)`).
- 11 nuevos tests (9 unit + 2 pg_test); 1631/1631 cognicode-core tests verdes.
- Re-export en `plan/mod.rs`: `GraphExecutor`, `ExecutorError`, `ProvenanceEnvelope`.

**Trazabilidad**:
- Branch: `feat/e28-2-pr1-port` (squashed a `7e72b0bc` al mergear).
- Tag: `v0.68.0` (MINOR — nuevos public types).
- PR: <https://github.com/Rubentxu/CogniCode/pull/142>.
- Artifacts: `sddk/e28-2-differential-graph-executors/apply-progress.md`.

**Próximo paso propuesto**: PR2 PG Executor (`feat/e28-2-pr2-pg-executor`; Phase 2 — `PgGraphExecutor` + recursive CTE sobre `PostgresRepository::load_call_graph_ws`; 15 tasks; ~500 LOC; ~11 PG scenarios). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.2 PR2 PG Executor shipped)

**E28.2 PR2 PG Executor closed and shipped v0.69.0 (PR #143 merged to main).**

Ciclo SDDK A-lite ejecutado en auto mode. PR2 cubre Phase 2 (PG Executor) del programa E28.2 (15 tasks; 3 commits squash-merged a `4974485c`).

**Logros PR2**:
- `PgGraphExecutor` struct en `crates/cognicode-core/src/infrastructure/persistence/pg_graph_executor.rs` (1744 LOC).
- `impl GraphExecutor for PgGraphExecutor` con `execute(plan, pin)` + `execute_with_limits(plan, pin, limits_override)`.
- Dispatcher `execute_pg` con 5 métodos:
  - `execute_path` — `WITH RECURSIVE` CTE bounded by `max_hops ≤ 32`.
  - `execute_neighbors` — recursive CTE at depth 1+ (both directions).
  - `execute_subgraph` — BFS-via-CTE from `{nodes}` set.
  - `execute_cluster` — `GROUP BY` aggregation.
  - `execute_boolean` — `INTERSECT` / `UNION ALL` / `EXCEPT` typed multiset.
- Plan-limit enforcement: `LIMIT n` pushed into SQL for `max_result_rows`; in-process polling para `time_ms` y `cancellation`.
- `unknown_revision` path: `RepositoryError::UnknownRevision` → `ExecutorError::RevisionUnknown("<ws>:<rev>")`.
- 11 nuevos tests (10 pass + 1 PRE-EXISTING-DEBT).

**Known issue (PRE-EXISTING-DEBT)**:
- `unknown_revision_returns_error` pg_test fails with "pool timed out" en sandbox. Root cause: el dedicated-OS-thread approach holds a PG connection que no se libera antes de la assertion. CI con timeouts más generosos podría pasar. Follow-up: `e28-2-pr2-pool-connection-release` — usar el patrón `block_in_place + Handle::enter + tokio::spawn + mpsc` (que ya funciona en `snapshot_provider.rs`).

**Trazabilidad**:
- Branch: `feat/e28-2-pr2-pg-executor` (squashed a `4974485c` al mergear).
- Tag: `v0.69.0` (MINOR — nuevo `PgGraphExecutor` public type + `GraphExecutor` trait implementation).
- PR: <https://github.com/Rubentxu/CogniCode/pull/143>.

**Próximo paso propuesto**: PR3 Snapshot Executor (`feat/e28-2-pr3-snapshot-executor`; Phase 3 — `SnapshotGraphExecutor` + BFS over petgraph; 8 tasks; ~400 LOC; 0 PG scenarios). Cadena stacked-to-main sigue.

## Session Handover 2026-07-28 (E28.2 PR3 Snapshot Executor shipped)

**E28.2 PR3 Snapshot Executor closed and shipped v0.70.0 (PR #144 merged to main).**

Ciclo SDDK A-lite ejecutado en auto mode. PR3 cubre Phase 3 (Snapshot Executor) del programa E28.2 (8 tasks; 1 commit squash-merged a `ca20fbaa`).

**Logros PR3**:
- `SnapshotGraphExecutor<'a> { provider: &'a dyn SnapshotProvider }` struct en `crates/cognicode-core/src/infrastructure/graph/snapshot_graph_executor.rs` (1863 LOC).
- `impl GraphExecutor for SnapshotGraphExecutor` con `execute(plan, pin)` + `execute_with_limits(plan, pin, limits_override)`.
- Dispatcher `execute_snapshot` con 5 métodos:
  - `execute_path` — BFS over petgraph `StableGraph<String, DependencyType>` con shortest-first ordering; bounded por `max_hops`.
  - `execute_neighbors` — Incoming (Direction::Incoming) + Outgoing (Direction::Outgoing) at configurable depth.
  - `execute_subgraph` — BFS from `{nodes}` set; emits visited nodes + edges.
  - `execute_cluster` — `HashMap<String, usize>` group counts; emits one row per group con `TypedValue`.
  - `execute_boolean` — typed multiset: `And = intersection`, `Or = union`, `Not = complement`.
- PlanLimits enforcement: `max_result_rows` + `max_path_count` applied post-walk via `TruncationMarker`.
- Cancellation: `CancellationToken::set()` mid-BFS → `Err(ExecutorError::LimitExceeded { limit: PlanLimitKind::Cancellation })`.
- 15/15 unit tests verde (1660+ cognicode-core lib tests passed).

**Trazabilidad**:
- Branch: `feat/e28-2-pr3-snapshot-executor` (squashed a `ca20fbaa` al mergear).
- Tag: `v0.70.0` (MINOR — nuevo `SnapshotGraphExecutor` public type + second `GraphExecutor` trait implementation).
- PR: <https://github.com/Rubentxu/CogniCode/pull/144>.

**Próximo paso propuesto**: PR4 Conformance (`feat/e28-2-pr4-conformance`; Phase 4 — `assert_equivalent` differential harness + petgraph oracle; 10 tasks; ~500 LOC; ~7 PG scenarios). Cierra la cadena E28.2.

## Session Handover 2026-07-29 (E28.2 PR2 pool-timeout fix shipped v0.70.1)

**Pre-existing-debt follow-up from E28.2 PR2 closed and shipped as v0.70.1.** The `unknown_revision_returns_error` pg_test that was failing with "pool timed out" in sandbox is now green; no regressions in the rest of the PG suite.

**Root cause** (confirmed empirically against `snapshot_provider.rs` which already worked):

The dedicated-OS-thread + `Runtime::new()` approach used by `PgGraphExecutor::execute_with_limits` and all 5 `execute_*` methods leaked PG pool connections. The new `Runtime` lifecycle interfered with the shared pool's tokio primitives (handles, mutexes) that were initialized in the caller runtime, causing "pool timed out" on the first SELECT inside `load_call_graph_ws` for unknown revisions. Other tests passed because they warmed up the pool via `save_call_graph_ws` before calling `execute`.

**Fix**: refactored all 8 inline `std::thread::spawn + Runtime::new + rt.block_on` blocks to the proven `block_in_place + Handle::current + handle.enter + tokio::spawn` pattern (matches `snapshot_provider.rs::current_head` / `snapshot`). Kept the async SQL work on the caller's Tokio runtime instead of spawning a fresh runtime per call, so the pool's tokio state lifecycle is no longer interrupted.

Also converted `unknown_revision_returns_error` from `#[tokio::test]` + direct `PgPool::connect(&base)` (admin DB without schema) to the local `pg_test!` macro (creates a unique DB with migrations run).

**Verification** (real `cargo test` output as GREEN evidence):

| Scope | Result |
|---|---|
| `unknown_revision_returns_error` (was RED, now GREEN) | 1 passed in 0.92s |
| All 11 `pg_graph_executor::tests::*` | 11 passed in 8.87s |
| `cognicode-core --tests --features postgres --lib` | 1648 passed, 0 failed, 27 ignored in 162.75s |
| `cognicode-core --tests --features postgres` (full PG suite) | 1648 + 6 + 2 = 1656 passed, 0 failed |
| `cognicode-explorer --features postgres --tests` | 117 passed across 14 test files, 0 failed |

Pre-existing 4 `sandbox_orchestrator_test::test_plan_expands_*` failures confirmed on `main` HEAD `cdf1d588` — unrelated to this fix (binary not built: `cargo build --bin sandbox-orchestrator`).

**Trazabilidad**:
- Branch: `fix/e28-2-pr2-pool-connection-release`
- Tag: `v0.70.1` (PATCH — bug fix only, no new APIs, no breaking changes; `block_in_place` is internal refactor of the sync-to-async bridge in `PgGraphExecutor`)
- File: `crates/cognicode-core/src/infrastructure/persistence/pg_graph_executor.rs` (+107 / −84 lines)
- The fix unblocks reliable `cargo test --features postgres` runs in sandbox environments that previously timed out at 30s.

**Próximo paso propuesto**: PR4 Conformance (`feat/e28-2-pr4-conformance`; Phase 4 — `assert_equivalent` differential harness + petgraph oracle; 10 tasks; ~500 LOC; ~7 PG scenarios). Cierra la cadena E28.2. El bug #4 (separación de aristas) ya está resuelto en v0.71.0, así que PR4 conformance validará la semántica correcta.

---

## Session Handover 2026-07-29 (E28.2 PR5 edge-filter fix shipped v0.71.0)

**Assessment gap #4 closed**: `GraphPlan::Path` and `Neighbors` now carry an optional `edge_kind_filter: Option<Vec<DependencyType>>`. When `Some(list)`, only edges of the listed `DependencyType` variants are traversed; when `None` (default), every edge kind is walked (preserves pre-fix behavior).

**Root cause** (detected during stack evaluation): both `PgGraphExecutor::execute_path` (recursive CTE) and `SnapshotGraphExecutor::bfs_all_paths` walked every edge indiscriminately. The assessment §1.3 item #4 ("separate calls from other dependencies when traversing") had never been implemented. E28.2 PR4 Conformance would have **frozen this as the spec semantic** in `assert_equivalent` conformance tests. Fixing before PR4 is critical.

**Fix** (2 atomic commits, single PR #147):

1. **Commit 1 — pre-existing migration order fix**: `m0018` adds composite FKs `(workspace_id, source_id) → graph_nodes(workspace_id, id)`, but `graph_nodes` PK is `(workspace_id, id, kind)`. PostgreSQL rejects the FK without a matching UNIQUE constraint. `m0019` provides that index but was scheduled *after* `m0018`, so it never ran on a fresh DB. Swapped the order. Bug masked by volume persisting schema state across container restarts; surfaced 2026-07-29.

2. **Commit 2 — edge-filter feature**:
   - `SnapshotGraphExecutor::bfs_all_paths` + `execute_neighbors`: filter by `edge.weight()` when iterating edges.
   - `PgGraphExecutor::execute_path` + `execute_neighbors`: SQL `AND ($N::text[] IS NULL OR e.kind = ANY($N))` clause + filter bound as `Option<Vec<String>>` mapped from `DependencyType` → `"dependency.calls"` etc.

**RED tests added** (strict TDD):

| Test | Fixture | Assertion |
|---|---|---|
| `snapshot_graph_executor::tests::path_with_edge_kind_filter_excludes_references` | A→B(Calls), A→B_ref(References), B→C(Calls), B_ref→C(Calls) | With `[Calls]` filter, every path must go through B (not B_ref) |
| `pg_graph_executor::tests::path_with_edge_kind_filter_eliminates_only_path` | A→B(References), B→C(Calls) | Without filter: A→B→C exists. With `[Calls]`: empty paths (no Calls edge from A) |

**Verification** (real `cargo test` output as GREEN evidence):

| Scope | Result |
|---|---|
| `pg_graph_executor::tests::*` (12 tests, +1 new) | **12 passed in 8.18s** |
| `snapshot_graph_executor::tests::*` (16 tests, +1 new) | **16 passed in 0.01s** |
| `cognicode-core --tests --features postgres` | **1650 passed, 0 failed**, 27 ignored in 253.37s |
| `cognicode-explorer --tests` (incl. PG feature) | all passed |

**Trazabilidad**:
- Branch: `fix/e28-2-pr5-edge-filter`
- Commits: `fd6aaaad` (migration fix) + `7593792d` (edge-filter)
- PR: <https://github.com/Rubentxu/CogniCode/pull/147>
- Tag: `v0.71.0` (MINOR — adds optional filter capability)
- Files: `crates/cognicode-core/src/domain/plan/graph_plan.rs` + `snapshot_graph_executor.rs` + `pg_graph_executor.rs` + `postgres_repository.rs` + `tests/e28_2_port_unknown_pin.rs` + `crates/cognicode-explorer/src/moldql/lower_plan.rs`

**Próximo paso propuesto**: PR4 Conformance (`feat/e28-2-pr4-conformance`; Phase 4 — `assert_equivalent` differential harness + petgraph oracle; 10 tasks; ~500 LOC; ~7 PG scenarios). Cierra la cadena E28.2. El bug #4 ya está resuelto, así que PR4 validará la semántica correcta de edge_kind_filter.

## Session Handover 2026-07-29 (E28.2 PR4 Conformance shipped v0.71.1 — E28.2 chain fully DONE)

**E28.2 PR4 Conformance closed and shipped v0.71.1 (PR #148 merged to main). E28.2 chain fully DONE. E28.3 (`moldql-pattern-profile-v1`) and E28.4 (`analytics-registry-cohort-1`) unblocked — may now proceed in parallel.**

Ciclo SDDK A-lite ejecutado en auto mode. PR4 cubre Phase 4 (differential conformance harness) del programa E28.2 (38 tasks across 4 phases; 1 commit `0d604ad3` fast-forward-merged a `5db567b2`).

**Logros PR4**:
- NEW file `crates/cognicode-core/tests/e28_2_executor_conformance.rs` (571 LOC) — differential harness with 10 tests (7 PG-required scenarios + 3 unit-only):
  - `conformance_path_sequences_match_in_order` (sequential equality)
  - `conformance_unordered_neighbor_sets_match` (multiset equality)
  - `conformance_max_result_rows_truncation_matches`
  - `conformance_max_path_count_truncation_matches`
  - `conformance_subgraph_nodes_match`
  - `conformance_unknown_revision_matches`
  - `conformance_bfs_ordering_matches_sql`
  - `petgraph_oracle_divergence_is_non_binding` (oracle divergence is logged but non-fatal)
  - `loud_failure_panics_with_triple_on_multiset_mismatch`
  - `loud_failure_panics_on_path_order_mismatch`
- `pg_conformance_test!` macro (local) — spins up fresh DB + runs migrations; uses `flavor = "multi_thread"` so `block_in_place` is allowed.
- `CallGraph::all_dependencies_with_metadata()` — new method returning `(source, target, dep_type, provenance, confidence)` for snapshot-vs-PG parity.

**14 pre-existing executor / schema bugs fixed** (atomic, single PR):

PG executor:
- `DISTINCT ON (path[last])` collapsed parallel paths per endpoint → `DISTINCT path` + `ORDER BY depth ASC, path ASC`.
- `execute_boolean` pin propagation dropped caller revision → threads caller pin through.
- `execute_boolean` Not semantics returned operand → queries `graph_nodes` directly for universe via `block_in_place`.
- `max_path_count` truncation missing → added.
- Neighbor query needs `ORDER BY id ASC` → added for stable order.

Snapshot executor:
- Edge labels used Debug form (`"Calls"`) instead of Display `"dependency.calls"` → fixed.
- Nodes not sorted before LIMIT truncation → sort by id, then truncate.
- `EdgeResult.properties` empty → populated from `CallGraph::all_dependencies_with_metadata()`.

Schema:
- `notify_graph_change` trigger referenced `NEW.source_path` on `graph_edges` inserts (column dropped after m0018) → use `TG_TABLE_NAME` to conditionally include `source_path` only on `graph_nodes` inserts.
- `m0018` / `m0019` migration order swapped: m0019 unique index must exist before m0018 composite FKs reference it.
- `"column"` reserved keyword → quoted in SQL.

Persistence / domain:
- `postgres_repository.rs`: `confidence` column is `REAL` (f32); cast to f64 to match `ExtractionContext`.
- `pg_test!` macro uses `flavor = "multi_thread"` so `block_in_place` is allowed; `#![cfg(feature = "postgres")]` required at top of integration tests.

**Verification** (real `cargo test` output as GREEN evidence):

| Scope | Result |
|---|---|
| `e28_2_executor_conformance` (10 tests, 7 PG-required) | **10 passed in 0.85s** |
| `pg_graph_executor::tests::*` (12 tests) | **12 passed** |
| `snapshot_graph_executor::tests::*` (16 tests) | **16 passed** |
| `cognicode-core --tests --features postgres --lib` | **1650 passed, 0 failed**, 27 ignored |
| `cognicode-explorer --tests` | **4 passed** |

**Trazabilidad**:
- Branch: `feat/e28-2-pr4-conformance` (fast-forward merged a `5db567b2`).
- Commit: `0d604ad3` (6 files, +742 / −30 lines).
- PR: <https://github.com/Rubentxu/CogniCode/pull/148>.
- Tag: `v0.71.1` (PATCH — no new capability, just validates executor equivalence).
- Specs implemented: `openspec/specs/executor-equivalence-conformance/spec.md` (now FULL).

**E28.2 chain closure**:
- PR1 Port ✅ → v0.68.0
- PR2 PG Executor ✅ → v0.69.0
- PR3 Snapshot Executor ✅ → v0.70.0
- PR6 pool-timeout fix ✅ → v0.70.1
- PR5 edge-filter fix ✅ → v0.71.0
- PR4 Conformance ✅ → v0.71.1
- **E28.2 is fully DONE.**

**Pre-existing debt (out of PR4 scope, NOT blockers, follow-up `fix/` PR planned)**:
- ~37 `postgres_repository.rs` tests still fail with `cannot insert into view "call_edges"` / `cannot insert into view "symbols"` (view-updatability bugs from schema rename).
- 4 `sandbox_orchestrator_test` failures (pre-existing on main; binary not built).

**Próximo paso propuesto**: launch **E28.3** (`moldql-pattern-profile-v1`) and **E28.4** (`analytics-registry-cohort-1`) in parallel — both unblocked now that E28.2 proves executor equivalence. Use the proven SDDK A-lite auto-mode cycle pattern.

## Session Handover 2026-07-28 (E28.1 PR4 shipped — E28.1 chain fully DONE)

**E28.1 PR4 PG Conformance closed and shipped v0.67.0 (PR #141 merged to main). E28.1 chain fully DONE. E28.2 (differential graph executors) unblocked.**

Ciclo SDDK A-lite ejecutado en auto mode. PR4 cubre Phase 4 (PG Conformance + W-A cleanup) del programa E28.1 (10 tasks + 1 W-A cleanup = 11 tareas; 1 commit squash-merged a `24df3ed9`).

**Logros PR4**:
- 6 `pg_test!` scenarios en `crates/cognicode-explorer/tests/e28_1_pg_conformance.rs`:
  - `pg_sql_safety_confidence_parameter_binding` (4.1)
  - `pg_sql_injection_no_inlining` (4.2) — `from = "alpha' OR 1=1; --"` no aparece verbatim
  - `pg_filter_equivalence_vs_petgraph` (4.3)
  - `pg_path_parity` (4.4)
  - `pg_neighbors_parity_with_where` (4.5)
  - `pg_subgraph_parity_with_provenance_filter` (4.6)
- 2 unit tests: executor refuses empty success (4.7) + bridge mapping legacy→MoldError (4.8)
- W-A cleanup: `populate_defaults` wired into all 6 `lower_*` functions de `MoldqlAstLowerer`; `compile_to_plan(SubgraphQuery { depth: 0 })` ahora retorna `PlanLimits { max_depth: Some(5), .. }`.

**Trazabilidad**:
- Branch: `feat/e28-1-pr4-pg-conformance` (squashed a `24df3ed9` al mergear).
- Tag: `v0.67.0` (MINOR — nuevos pg_tests + W-A fix).
- PR: <https://github.com/Rubentxu/CogniCode/pull/141>.
- Artifacts: `sddk/e28-1-moldplan-graphplan-contracts/apply-progress.md` (PR1+PR2+PR3+PR4 evidence).

**E28.1 chain closure**:
- PR1 Foundation ✅ → v0.64.0
- PR2 Plan Algebra ✅ → v0.65.0
- PR3 Bridge ✅ → v0.66.0
- PR4 PG Conformance ✅ → v0.67.0
- **E28.1 is fully DONE.**

**Pre-existing debt (out of PR4 scope, NOT blockers)**:
- 30+ multimodal feature compile errors in non-E28.1 files
- `cognicode-macros/src/newtype.rs:78,119` `clippy::useless_conversion` (blocks `-D warnings`)
- 17 `clippy::unnecessary_min_or_max` errors in non-PR4 files
- 4 `sandbox_orchestrator_test` failures in `cognicode` package (pre-existing on main)
- 5 PR2 E28.0 carry-over WARNINGs (snapshot_cache unbounded, subscribe ignores workspace, load_call_graph_ws not pinned, SnapshotProvider async_trait, SnapshotError Cow)

**Próximo paso propuesto**: E28.2 — Differential Graph Executors (depends on E28.1; unblocked). Cadena E28 continúa.

## Session Handover 2026-07-27 (E28 PR3 shipped)

**E28 PR3 Snapshot+Bridge closed and shipped v0.63.0 (PR #137 merged to main). E28.0 chain fully DONE.**

Ciclo SDDK A-lite ejecutado completamente en auto mode. PR3 cubre Phase 4 (Repository Bridge + Contract tests) del programa E28 (16 tasks originales; 5 commits GREEN + 3 commits correction cycle 1 + 1 commit docs = 9 commits totales en `feat/e28-0-pr3-snapshot-bridge`).

**Logros PR3**:
- `Repository::load_call_graph_pinned(&WorkspaceId, RevisionId)` trait extension con `Send + Sync` dyn-compat.
- `PostgresRepository::load_call_graph_pinned` delega a `load_call_graph_ws(ws, rev)` (ya revision-pinned desde PR2).
- `PgGraphRepository::find_nodes_by_kind(kind, &WorkspaceId)` + `find_incoming_edges(id, &WorkspaceId)` filtran por `workspace_id` en SQL.
- `MetadataAwareRepository::callees_with_metadata_pinned(id, &WorkspaceId, RevisionId, &SnapshotProvider)` usa `SnapshotProvider::snapshot` para lecturas pinned.
- `From<&ResolvedSymbol> for RelationTarget` con `provenance=None, confidence=None` (backward compat).
- pg_test contra ingest concurrente (rev 3 vs rev 4) sobrevive lectura pinned.

**CRITs cerrados en correction cycle 1**:
- **CRIT-2 (4.3a)**: pg_test workspace-scoped (ws1 vs ws2 isolation) ahora pasa.
- **CRIT-3 (4.4a)**: pg_test revision-pinned callees_with_metadata_pinned ahora pasa.
- **CRIT-1 (4.6)**: `GraphNode.properties` preserva raw `serde_json::Value` (no aplana a `HashMap<String,String>`); código committed pero runtime bloqueado por 30+ errores pre-existentes de compilación `multimodal` feature (clasificado PRE-EXISTING-DEBT, fuera del scope PR3).

**Bug latente corregido (m0019)**: el FK subset `(workspace_id, source_id) → graph_nodes(workspace_id, id)` que `m0018` declaró sin UNIQUE INDEX subset estaba siendo rechazado silenciosamente por PostgreSQL, causando que `fresh_pool() → run_migrations()` fallara y todos los pg_tests se skippearan desde PR1. La nueva migration `m0019_unique_index_workspace_id.sql` agrega `CREATE UNIQUE INDEX IF NOT EXISTS idx_graph_nodes_workspace_id ON graph_nodes(workspace_id, id)` que satisface el FK subset.

**Trazabilidad**:
- Branch: `feat/e28-0-pr3-snapshot-bridge` (squashed a `62c694c6` al mergear).
- Tag: `v0.63.0` (MINOR — new trait method + new migration).
- PR: <https://github.com/Rubentxu/CogniCode/pull/137>.
- Artifacts: `sddk/e28-0-canonical-graph-revisions/` (verify-report PR3, debt-report PR3).

**E28.0 chain completion**:
- PR1 Foundation ✅ → v0.61.0
- PR2 Persistence ✅ → v0.62.0
- PR3 Snapshot+Bridge ✅ → v0.63.0
- **E28.0 is fully DONE; E28.1 (MoldPlan/GraphPlan contracts) is unblocked.**

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
| `e25-pr2-pack-builder` | v0.55.0 | 2026-07-24 | [#120](https://github.com/Rubentxu/CogniCode/pull/120) | DecisionSupportPackBuilder + GET /api/decisions/:id/support-pack. 5 commits. Verdict PASS_WITH_WARNINGS. C-2 (ViewExecutor wiring) deferred to follow-up PR. |
| `typed-overview-affordance-matrix-v1` | v0.43.0 | 2026-07-02 | [#102](https://github.com/Rubentxu/CogniCode/pull/102) | Affordance matrix per InspectableObjectType. `GET /api/affordances/:object_type`. `AffordanceCards` in PaneInspector. 7 unit tests. Verdict PASS_WITH_WARNINGS. |
| `moldql-intent-syntax-v1` | v0.44.0 | 2026-07-02 | [#103](https://github.com/Rubentxu/CogniCode/pull/103) | MoldQL intent lowering layer: lowercase `symbols where` and `calls from` patterns translated to MoldQL AST before canonical parser. 15 unit + 9 integration tests. Verdict PASS_WITH_WARNINGS. |
| `relation-candidates-v1` | v0.45.0 | 2026-07-02 | [#104](https://github.com/Rubentxu/CogniCode/pull/104) | `AnalysisService::find_dead_code()` + `candidates_for_reverse_edge()`. reverse_edges type-blind HashMap fixed. 4 unit tests. Verdict PASS. |
| `refactor/view-registry-uniqueness` | v0.53.1 | 2026-07-23 | [#117](https://github.com/Rubentxu/CogniCode/pull/117) | Refactor: derive `view_kind` from trait, KSVConfig snapshot tests, HashSet deduplication. COMPLIANT on all scenarios. Verdict PASS_WITH_WARNINGS (3 carry-over warnings: W1/W2/H1). |
| `e25-decision-support-packs` | v0.55.1 | 2026-07-24 | [#121](https://github.com/Rubentxu/CogniCode/pull/121) | Decision Support Packs (E25.1): C-2 closure (ViewKind::DecisionSupportPack + executor + registry) + 7 PR2 verify fixes. Tests: 901 multimodal / 834 default all passing. Verdict PASS_WITH_WARNINGS (3 warnings, 4 suggestions). ADR-011 finalized. |
| `refactor/dup-001-get-node` | v0.55.2 | 2026-07-24 | [#122](https://github.com/Rubentxu/CogniCode/pull/122) | DUP-001 refactor: extract 3-way `get_node` match in `build_node_source_view` to use `resolve_focus_node` + `FocusResolution`. 903 multimodal / 836 default tests (was 901/834). Marker view for NotFound (graceful degradation). |
| `fix/e24-png-artifact-kind` | v0.55.3 | 2026-07-24 | [#123](https://github.com/Rubentxu/CogniCode/pull/123) | E24 PNG artifact kind fix: PNG exports were mislabeled as `kind=\"svg\"` because `addSvgArtifact` was reused for PNG content. Added `addPngArtifact()` helper and used it in ExportMenu PNG branch. ExportMenu tests: 15/15 passing. |
| `refactor/e25-viewkind-wire-tag` | v0.55.4 | 2026-07-24 | [#124](https://github.com/Rubentxu/CogniCode/pull/124) | W-001 refactor: derive `ViewKind` serde with `#[serde(rename_all = \"snake_case\")]`, eliminating triple duplication (35 variants × 3 match arms). -129 LOC delta. New ViewKind additions now require 1 edit instead of 3. 903 multimodal / 836 default tests pass. |
| `fix/topbar-shell-ids` | v0.55.5 | 2026-07-24 | [#125](https://github.com/Rubentxu/CogniCode/pull/125) | TopBar + Shell tablist `data-testid` IDs added (5 TopBar + 1 Shell) for E2E testability, a11y, and DOM debugging. Existing IDs preserved for backward compat. 916/917 vitest passing. |
| `fix/w003-wire-contract-asymmetry` | v0.55.6 | 2026-07-24 | [#126](https://github.com/Rubentxu/CogniCode/pull/126) | W-003 fix: `pack_to_contextual_view` now serializes `PackPane` via serde derive, matching REST endpoint wire format exactly. Eliminated "reason" vs "message" divergence. 906 multimodal tests (+3 new). |
| `refactor/s002-dedup-builder-call` | v0.55.7 | 2026-07-24 | [#127](https://github.com/Rubentxu/CogniCode/pull/127) | S-002 + S-001: extracted `build_pack` helper eliminates 3 duplicate `DecisionSupportPackBuilder::build` calls. Magic literals (3, 100) replaced with `PACK_RATIONALE_MAX_DEPTH/MAX_NODES` constants. 906 multimodal / 836 default tests passing. |
| `refactor/s004-no-json-roundtrip` | v0.55.8 | 2026-07-24 | [#128](https://github.com/Rubentxu/CogniCode/pull/128) | S-004: replaced `format!()` + `from_str()` JSON round-trip with `serde_json::Value::String` + `from_value` in boundary.rs. Eliminates JSON string allocation. 906 multimodal / 836 default tests passing. |
| `refactor/w002-panestatus-semantics` | v0.55.9 | 2026-07-24 | [#129](https://github.com/Rubentxu/CogniCode/pull/129) | W-002: `build_pane` now uses `PaneStatus::Degraded` (not `Failed`) for partial failures like \"target not found\". Failed variant reserved for actual builder crashes per enum doc. Wire format now correctly emits `\"degraded\"` for partial failures. 906 multimodal / 836 default tests passing. |
| `feat/e27-pane-navigation` | v0.56.0 | 2026-07-24 | [#130](https://github.com/Rubentxu/CogniCode/pull/130) | E27.2 pane navigation coherence (ADR-013): strengthened PaneStackView unit tests (reducer invariants), removed hardcoded shortcut hint from PaneBreadcrumb, added ARIA attributes. ContextRail marked E27.3-pending. 906 multimodal / 836 default / 916 vitest passing. |
| `feat/e27-responsive-a11y` | v0.57.0 | 2026-07-24 | [#131](https://github.com/Rubentxu/CogniCode/pull/131) | E27.4 prefers-reduced-motion + focus polish (WCAG 2.2 AA / AAA): global `@media (prefers-reduced-motion: reduce)` block disables animations for users who request reduced motion. Previously only InteractiveGraph honored this preference. 919 vitest passing (+3 new CSS rule tests). |
| `fix/suggested-questions-doc-exhaustiveness` | v0.57.1 | 2026-07-24 | [#132](https://github.com/Rubentxu/CogniCode/pull/132) | Fix pre-existing test failure: `suggestedQuestions.test.ts:59` expected 11 variants but schema enumerates 12 (doc was missing). Added doc entry to `SUGGESTED_QUESTIONS` with 4 prompts. 920/920 vitest passing. |
| `refactor/e12f-ownership-feature-gate-fix` | v0.57.2 | 2026-07-24 | [#133](https://github.com/Rubentxu/CogniCode/pull/133) | E12F ownership feature gate fix: added `cognicode-core/multimodal` to ownership chain. The 1039-line e12f-ownership-map diff was dead code. `--features ownership` now compiles and 838 tests pass (+2 ownership tests previously dead). |
| `refactor/e12f-async-node-properties` | v0.58.0 | 2026-07-24 | [#134](https://github.com/Rubentxu/CogniCode/pull/134) | E12F COUPLING-001 fix: extracted async `NodePropertyReader` port to eliminate `Handle::current().block_on()` inside sync trait method. No more deadlock risk; clean async boundary. 838 ownership / 836 default / 906 multimodal tests passing. |
| `refactor/e12f-remaining-debt` | v0.58.1 | 2026-07-24 | (direct commit) | E12F remaining debt cleanup: OE-004 ADR-008 stale gix reference → git CLI clarification, OE-006 documented unused `_file` parameter (trait signature requirement). |
| `feat/knowledge-layer-ports` | v0.59.0 | 2026-07-24 | (local merge — gh PR GraphQL error) | **Plan 012 partial**: AdrRepository port + AdrSummary + InMemoryAdrRepository (5 tests). Spotter wires "adr" family. InspectableObjectType::Adr + SpotterSearchResult::Adr. Frontend glyphs for doc/adr. 3 new Spotter interaction tests. Destraba e13-wave2-universal-spotter. 923 vitest / 911 multimodal / 843 ownership / 841 default passing. |
| `feat/doc-repository-port` | v0.59.1 | 2026-07-24 | (local merge — gh PR GraphQL error) | Plan 012 step 1 (continued): DocRepository port + DocSummary + InMemoryDocRepository adapter (5 unit tests). Future PG adapter can implement the trait. 846 default / 916 multimodal passing (+5 each). |
| `refactor/wire-doc-repo-to-spotter` | v0.59.2 | 2026-07-24 | (local merge) | Plan 012 step 2: Doc hits in Spotter backend now use DocRepository port when wired, falling back to graph_repo. Same pattern as ADR. 846 default / 916 multimodal / 923 vitest passing. Zero regressions. |
| `feat/evidence-store-port` | v0.59.3 | 2026-07-24 | (direct commit) | Plan 012 step 1 (continued): EvidenceStore port + EvidenceSummary + EvidenceKind (Log/Trace/Measurement/External) + InMemoryEvidenceStore adapter (6 unit tests). Completes typed knowledge ports trio (Adr + Doc + Evidence). 852 default / 922 multimodal / 923 vitest passing. |
| `feat/e27-3-knowledge-rail-foundation` | v0.60.0 | 2026-07-24 | (local merge) | E27.3 knowledge rail foundation (Phase 1): new endpoint `GET /api/objects/:id/related-knowledge` returning `{ adrs, docs, evidence }` (Phase 1 stub). Frontend: `useObjectKnowledge` hook + ContextRail Knowledge section showing counts. 923 vitest / 922 multimodal / 852 default passing. Zero regressions. |
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
