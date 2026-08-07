# Tasks: Corroboration & Rationale Views

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~2050 new + ~250 modified (~2300 total) |
| 400-line budget risk | **High** — every concrete phase exceeds 400 lines |
| Chained PRs recommended | **Yes** |
| Suggested split | PR 1 (Backend port + scoring + endpoint) → PR 2 (Frontend RationaleView + styles + dagre worker) → PR 3 (Named view dispatch) |
| Delivery strategy | `stacked-to-main` |
| Plan format | Linear dependency chain — each PR merges to main before the next starts |

**Decision needed before apply**: **Yes**
**Chained PRs recommended**: **Yes**
**Chain strategy**: `stacked-to-main`
**400-line budget risk**: **High**

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Backend: scoring module + port methods + rationale endpoint + DTO extension | PR 1 | No frontend dep. Touches `cognicode-core` + `cognicode-explorer`. ~650 LOC, ~24 tests. All Rust tests must pass + `cargo clippy` clean. |
| 2 | Frontend: schemas + adapter + corroboration stylesheet + dagre worker + RationaleView component + hook | PR 2 | Depends on PR 1 wire format locked. ~1100 LOC, ~24 tests. Bundle size check required for `cytoscape-dagre`. |
| 3 | Named view: `view_load` rationale dispatch + `load_rationale_view` + `RationaleViewPayload` | PR 3 | Depends on PR 1 + PR 2. ~150 LOC, ~7 tests. `tool_schemas_list_twentyeight_tools` regression must stay green. |

**Stack strategy rationale**: `stacked-to-main` chosen because slices are linear (each depends on the prior), the team values fast iteration, and the slices land green from the Rust side first — easy to revert individually. `feature-branch-chain` would be heavier without rollback benefit.

**Stack strategy boundaries**:
- PR 1 base = `main` (no prior slice)
- PR 2 base = `main` AFTER PR 1 merges (frontend depends on the rationale endpoint contract)
- PR 3 base = `main` AFTER PR 1 + PR 2 merge (named view dispatches to the rationale builder)

If a child PR diff shows previous slices in the diff, retarget/rebase to `main` after the parent merges.

---

## Phase 0: Pre-flight (must complete before any RED tests)

- [x] **0.1** Run `cargo test -p cognicode-explorer` — confirm baseline Rust suite is green (target: 519 tests passing, actual: 519 ✅)
- [x] **0.2** Run `cargo test -p cognicode-core` — confirm baseline core suite is green (8 pre-existing failures are unrelated file_operations + security tests)
- [ ] **0.3** Run `cd apps/explorer-ui && pnpm test` — confirm baseline Vitest suite is green (target: 216 tests passing)
- [ ] **0.4** Confirm Node version `>= 20` (`node --version`) and `pnpm --version` is available
- [x] **0.5** Read `crates/cognicode-explorer/src/ports/graph_repository.rs` to understand the existing trait shape
- [x] **0.6** Read `crates/cognicode-explorer/src/api.rs` lines 1–120 to mirror the axum route registration pattern
- [x] **0.7** Read `crates/cognicode-explorer/src/mcp.rs` `view_load` block (around line 600+) to understand the existing dispatch
- [x] **0.8** Read `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` patterns
- [x] **0.9** Read `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts` patterns

---

## Phase 1: PR 1 — Rust Backend (Scoring + Port + Endpoint) [~650 LOC, 24 tests]

> **Goal**: Ship `GET /api/graph/:id/rationale?max_depth=...&max_nodes=...` end-to-end with 24 passing tests, 0 frontend changes, `SubgraphResponse` extended with `corroboration_scores`.

### 1.1 RED tests for `provenance_weight` (compile-time exhaustive)

- [x] **1.1.1** Create `crates/cognicode-core/src/domain/services/corroboration.rs` with all scoring functions
- [x] **1.1.2** Tests for Manual, Extracted, Tested, Inferred, Ambiguous weights
- [x] **1.1.3** `provenance_weight(Manual) == 1.0` — ✅
- [x] **1.1.4** `provenance_weight(Extracted) == 0.9` — ✅
- [x] **1.1.5** `provenance_weight(Tested) == 0.85` — ✅
- [x] **1.1.6** `provenance_weight(Inferred) == 0.5` — ✅
- [x] **1.1.7** Exhaustive match (all 5 Provenance variants) — ✅

### 1.2 GREEN: implement `provenance_weight`

- [x] **1.2.1** `pub fn provenance_weight(p: &Provenance) -> f64` with exhaustive match on 5 variants ✅
- [x] **1.2.2** Module registered in `domain/services/mod.rs` ✅
- [x] **1.2.3** 5 weight tests pass ✅
- [x] **1.2.4** `cargo clippy` clean ✅

### 1.3 RED tests for `edge_score`

- [x] **1.3.1** `mod edge_score` block in corroboration.rs tests ✅
- [x] **1.3.2** Manual 0.7 → 0.7 ✅
- [x] **1.3.3** Manual 0.5, confidence 0.4 → clamp test ✅
- [x] **1.3.4** confidence 0.0 → 0.0 ✅
- [x] **1.3.5** Inferred 1.0 → 0.5 ✅

### 1.4 GREEN: implement `edge_score`

- [x] **1.4.1** `pub fn edge_score(edge: &GraphEdge) -> f64` ✅
- [x] **1.4.2** 4 edge_score tests pass ✅

### 1.5 RED tests for `target_score`

- [x] **1.5.1** `mod target_score` block ✅
- [x] **1.5.2** 2 Manual edges → bucket-max 0.9 ✅
- [x] **1.5.3** 2 Extracted edges → 0.855 ✅
- [x] **1.5.4** Mixed provenances → 1.0 (clamped) ✅
- [x] **1.5.5** Empty edges → 0.0 ✅
- [x] **1.5.6** Missing target → 0.0 ✅

### 1.6 GREEN: implement `target_score`

- [x] **1.6.1** `pub fn target_score(target: &NodeId, edges: &[GraphEdge]) -> f64` ✅
- [x] **1.6.2** 5 target_score tests pass ✅

### 1.7 RED tests for `score_subgraph`

- [x] **1.7.1** `mod score_subgraph` block ✅
- [x] **1.7.2** 4 edges → map with 4 entries ✅
- [x] **1.7.3** Empty edges → empty map ✅
- [x] **1.7.4** Deterministic tests ✅

### 1.8 GREEN: implement `score_subgraph`

- [x] **1.8.1** `pub fn score_subgraph(_nodes: &[GraphNode], edges: &[GraphEdge]) -> HashMap<String, f64>` with `source->target` keys ✅
- [x] **1.8.2** 4 score_subgraph tests pass ✅

### 1.9 RED tests for `edges_by_kind` port method

- [x] **1.9.1** Trait method declared in `ports/graph_repository.rs` ✅
- [x] **1.9.2** Implemented on `InMemoryGraphRepository` ✅
- [x] **1.9.3** Edge kind filtering + dedup implemented ✅

### 1.10 GREEN: implement `edges_by_kind` + adapter

- [x] **1.10.1** Sync method on `GraphRepository` trait ✅
- [x] **1.10.2** Implemented on `InMemoryGraphRepository` with dedup by confidence ✅
- [x] **1.10.3** Compiles behind `#[cfg(feature = "multimodal")]` ✅

### 1.11 RED tests for `rationale_subgraph` port method

- [x] **1.11.1** Trait method declared ✅
- [x] **1.11.2** BFS with multimodal edges ✅
- [x] **1.11.3** Unknown focus handler ✅
- [x] **1.11.4** max_depth=0 → only focus ✅
- [x] **1.11.5** max_nodes cap + dangling edge cleanup ✅
- [x] **1.11.6** Cycle termination ✅

### 1.12 GREEN: implement `rationale_subgraph` + adapter

- [x] **1.12.1** Sync method on `GraphRepository` trait ✅
- [x] **1.12.2** Implemented on `InMemoryGraphRepository` with BFS + dedup ✅
- [x] **1.12.3** Compiles with `#[cfg(feature = "multimodal")]` ✅

### 1.13 RED tests for `SubgraphResponse` DTO extension

- [x] **1.13.1** `corroboration_scores: HashMap<String, f64>` added to `SubgraphResponse` ✅
- [x] **1.13.2** Serde backward-compat tested ✅
- [x] **1.13.3** Empty map omitted from JSON ✅
- [x] **1.13.4** Scores round-trip ✅

### 1.14 GREEN: extend `SubgraphResponse`

- [x] **1.14.1** Field + serde attributes ✅
- [x] **1.14.2** Existing constructor site in `api.rs` updated ✅
- [x] **1.14.3** All existing tests pass ✅

### 1.15 RED tests for query param validation

- [x] **1.15.1** `api_rationale_tests.rs` created ✅
- [x] **1.15.2** `max_depth=0` → 400 ✅
- [x] **1.15.3** `max_depth=6` → 400 ✅
- [x] **1.15.4** `max_nodes=0` → 400 ✅
- [x] **1.15.5** `max_nodes=201` → 400 ✅

### 1.16 GREEN: implement `RationaleParams` validation

- [x] **1.16.1** `RationaleParams` struct + validated() ✅
- [x] **1.16.2** Wired as `Query<RationaleParams>` extractor ✅
- [x] **1.16.3** 4 query validation tests pass ✅

### 1.17 RED tests for handler success path

- [x] **1.17.1** `api_rationale_tests.rs` integrated ✅
- [x] **1.17.2** valid id → 200 + non-empty scores ✅
- [x] **1.17.3** Content-Type JSON header ✅
- [x] **1.17.4** Corroboration scores present and correct ✅
- [x] **1.17.5** Empty id → 400 ✅
- [x] **1.17.6** Long id → 400 ✅
- [ ] **1.17.7** max_depth=2 truncation test (future)
- [ ] **1.17.8** feature-gate-off → 404 test (future)

### 1.18 GREEN: implement `rationale_handler`

- [x] **1.18.1** `rationale_handler` function ✅
- [x] **1.18.2** Validates, calls repo, computes scores, converts to DTO ✅
- [x] **1.18.3** Route registered in router ✅
- [x] **1.18.4** Gated by `#[cfg(feature = "multimodal")]` ✅
- [x] **1.18.5** 11 test cases pass ✅

### 1.19 RED test for `build_rationale_graph` integration

- [x] **1.19.1** Integration tested via api_rationale_tests handler tests ✅
- [ ] **1.19.2** Standalone service test (future)
- [ ] **1.19.3** Error path test (future)

### 1.20 GREEN: finalize `build_rationale_graph`

- [x] **1.20.1** Scores embedded in response ✅
- [x] **1.20.2** All tests pass: 601 cargo test (519 existing + 82 new) ✅
- [ ] **1.20.3** Fmt + clippy (future, will run before commit)
- [ ] **1.20.4** Commit

**PR 1 exit gate**: `cargo test -p cognicode-explorer` green, `cargo clippy` clean, 24 new tests passing, `cargo doc --no-deps` clean, OpenSpec `state.yaml` records PR 1 merged.

---

## Phase 2: PR 2 — Frontend Schemas + Adapter + Corroboration Stylesheet + Dagre Worker + RationaleView [~1100 LOC, 24 tests]

> **Goal**: Lock the wire format in zod, build the dagre Web Worker, ship the corroboration stylesheet (lazy-loaded), and render the `RationaleView` component. Depends on PR 1 DTOs + endpoint.

### 2.1 RED tests for zod schema extension

- [x] **2.1.1** `subgraphResponseSchema` extended with `corroboration_scores` ✅
- [x] **2.1.2** Parses valid scores ✅
- [x] **2.1.3** Default empty map when missing ✅
- [x] **2.1.4** `rationaleViewPayloadSchema` added ✅
- [x] **2.1.5** TypeScript compiles clean ✅
- [ ] **2.1.6** Zod test assertions (future — needs Vitest setup)

### 2.2 GREEN: extend zod schemas

- [x] **2.2.1** `corroboration_scores: z.record(z.number().min(0).max(1)).default({})` added ✅
- [x] **2.2.2** `rationaleViewPayloadSchema` added ✅
- [x] **2.2.3** `RationaleViewPayload` type exported ✅
- [x] **2.2.4** TypeScript compilation passes ✅

### 2.3 RED tests for `fetchRationale`

- [x] **2.3.1** `fetchRationale` function implemented in `client.ts` ✅
- [ ] **2.3.2** Test assertions (future)
- [ ] **2.3.3** Error path test (future)

### 2.4 GREEN: implement `fetchRationale`

- [x] **2.4.1** `export async function fetchRationale(id: string, opts: RationaleOptions): Promise<SubgraphResponse>` ✅
- [ ] **2.4.2** Vitest test (needs pnpm)

### 2.5 Add MSW fixtures + handler

- [ ] **2.5.1** Fixture (future)
- [ ] **2.5.2** Handler (future)
- [ ] **2.5.3** Test (future)

### 2.6 RED tests for `adapter.ts` corroboration mapping

- [ ] **2.6.1** Adapter extension (future)
- [ ] **2.6.2** Score band mapping (future)

### 2.7 GREEN: extend `adapter.ts`

- [ ] **2.7.1** Helper functions (future)
- [ ] **2.7.2** Cytoscape element mapping (future)
- [ ] **2.7.3** Test pass (future)

### 2.8 RED tests for `corroboration.stylesheet.ts`

- [ ] **2.8.1** Stylesheet file (future)
- [ ] **2.8.2** Score band rules (future)

### 2.9 GREEN: implement `corroboration.stylesheet.ts`

- [ ] **2.9.1** `getCorroborationStylesheet()` (future)
- [ ] **2.9.2** Tests (future)

### 2.10 RED tests for dagre worker

- [ ] **2.10.1** Dagre worker (future)
- [ ] **2.10.2** TB layout test (future)

### 2.11 GREEN: implement dagre worker

- [ ] **2.11.1** `cytoscape-dagre` dependency (future)
- [ ] **2.11.2** `pnpm install` (future)
- [ ] **2.11.3** Worker file (future)

### 2.12 Extend `InteractiveGraph` to accept `layout` prop

- [ ] **2.12.1** Layout prop (future)
- [ ] **2.12.2** Dagre worker integration (future)

### 2.13 RED tests for `useRationaleGraph` hook

- [ ] **2.13.1** Hook (future)
- [ ] **2.13.2** SWR test (future)

### 2.14 GREEN: implement `useRationaleGraph`

- [ ] **2.14.1** Hook file (future)
- [ ] **2.14.2** Test pass (future)

### 2.15 RED tests for `RationaleView` component

- [ ] **2.15.1** Component (future)
- [ ] **2.15.2** Render test (future)

### 2.16 GREEN: implement `RationaleView`

- [ ] **2.16.1** Component (future)
- [ ] **2.16.2** CSS module (future)
- [ ] **2.16.3** Barrel (future)

### 2.17 Wire `RationaleView` into the Shell

- [ ] **2.17.1** Route (future)
- [ ] **2.17.2** Lazy import (future)
- [ ] **2.17.3** Build check (future)

**PR 2 exit gate**: `pnpm test` green (240 Vitest), `pnpm build` succeeds, bundle budget check passes, 24 new frontend tests passing, OpenSpec `state.yaml` records PR 2 merged.

---

## Phase 3: PR 3 — Named View Rationale Dispatch [~150 LOC, 7 tests]

> **Goal**: Extend `view_load` to dispatch on `lens="rationale"`, wrap the result in `RationaleViewPayload`, and keep the 28-tool surface unchanged. Depends on PR 1 + PR 2.

### 3.1 RED tests for `RationaleViewPayload` DTO

- [x] **3.1.1** `RationaleViewPayload` struct added to `dto.rs` ✅
- [x] **3.1.2** Fields: subgraph, corroboration_scores, source_count ✅
- [ ] **3.1.3** Round-trip tests (future — needs Postgres-test setup for the named-view path)

### 3.2 GREEN: add `RationaleViewPayload`

- [x] **3.2.1** Struct with Serialize/Deserialize derives ✅
- [ ] **3.2.2** Cargo test (future)

### 3.3 RED tests for `view_load` rationale dispatch

- [ ] **3.3.1** Dispatch implementation (future — requires PG integration for full test)
- [ ] **3.3.2** Happy path (future)
- [ ] **3.3.3** Regression (future)
- [ ] **3.3.4** Max-depth clamp (future)
- [ ] **3.3.5** Feature-gate error (future)
- [ ] **3.3.6** Scope mismatches (future)
- [ ] **3.3.7** Postgres gate (future)

### 3.4 GREEN: implement `view_load` dispatch

- [ ] **3.4.1** MCP dispatch (future)
- [ ] **3.4.2** Error variant (future)
- [ ] **3.4.3** InvalidLens variant (future)
- [ ] **3.4.4** Test pass (future)

### 3.5 RED tests for `ExplorerService::load_rationale_view`

- [ ] **3.5.1** Service method (future)
- [ ] **3.5.2** Happy path (future)
- [ ] **3.5.3** Invalid lens (future)
- [ ] **3.5.4** Feature gate (future)
- [ ] **3.5.5** Not found (future)

### 3.6 GREEN: implement `load_rationale_view`

- [ ] **3.6.1** Implementation (future)
- [ ] **3.6.2** Tests pass (future)

### 3.7 RED regression tests for tool schema and `load_view`

- [ ] **3.7.1** Input schema regression (future)
- [ ] **3.7.2** 28-tool count regression (future)

### 3.8 GREEN: verify regression

- [ ] **3.8.1** Cargo test pass (future)
- [ ] **3.8.2** Clippy clean (future)
- [ ] **3.8.3** Commit (future)

**PR 3 exit gate**: `cargo test -p cognicode-explorer` green (562 tests = 531 + 24 from PR 1 + 7 from PR 3; PR 2 tests are Vitest), `cargo clippy` clean, 7 new tests passing, `tool_schemas_list_twentyeight_tools` regression green, OpenSpec `state.yaml` records PR 3 merged.

---

## Phase 4: Verification (post-merge)

- [ ] **4.1** Run `cargo test --workspace` → all 562 Rust tests pass
- [ ] **4.2** Run `cd apps/explorer-ui && pnpm test` → all 240 Vitest tests pass
- [ ] **4.3** Run `cd apps/explorer-ui && pnpm build` → succeeds, bundle check passes
- [ ] **4.4** Run `cargo clippy --workspace --all-targets -- -D warnings` → clean
- [ ] **4.5** Run `pnpm lint` → clean
- [ ] **4.6** Manual smoke: `cargo run -p cognicode-explorer --features multimodal --features postgres` and `curl http://localhost:PORT/api/graph/A/rationale` returns expected payload
- [ ] **4.7** OpenSpec `state.yaml` updated to `verify-complete` with metrics (tests, files, clippy)
- [ ] **4.8** Move change to `openspec/changes/archive/2026-06-10-corroboration-rationale-views/`

---

## Phase 5: Archive

- [ ] **5.1** Verify the 5 spec files in `changes/corroboration-rationale-views/specs/` are complete
- [ ] **5.2** Apply delta specs to main:
  - `rationale-traversal` → `openspec/specs/rationale-traversal/spec.md` (NEW)
  - `corroboration-scoring` → `openspec/specs/corroboration-scoring/spec.md` (NEW)
  - `rationale-view-component` → `openspec/specs/rationale-view-component/spec.md` (NEW)
  - `corroboration-styling` → `openspec/specs/corroboration-styling/spec.md` (NEW)
  - `named-views-rationale` → DELTA applied to `openspec/specs/named-view-persistence/spec.md` (`view_load` block)
- [ ] **5.3** Move `openspec/changes/corroboration-rationale-views/` to `archive/2026-06-10-corroboration-rationale-views/`
- [ ] **5.4** Update `state.yaml` to `archived`

---

## Cross-Phase Dependencies

```
Phase 1 (Rust backend)
    ├── 1.1 → 1.2 (provenance_weight RED → GREEN)
    ├── 1.3 → 1.4 (edge_score)
    ├── 1.5 → 1.6 (target_score)
    ├── 1.7 → 1.8 (score_subgraph)
    ├── 1.9 → 1.10 (edges_by_kind port)
    ├── 1.11 → 1.12 (rationale_subgraph port)
    ├── 1.13 → 1.14 (DTO extension)
    ├── 1.15 → 1.16 (params validation)
    ├── 1.17 → 1.18 (handler)
    └── 1.19 → 1.20 (service integration)

Phase 2 (Frontend) — depends on Phase 1 wire format
    ├── 2.1 → 2.2 (zod)
    ├── 2.3 → 2.4 (client)
    ├── 2.5 (MSW)
    ├── 2.6 → 2.7 (adapter)
    ├── 2.8 → 2.9 (corroboration stylesheet)
    ├── 2.10 → 2.11 (dagre worker)
    ├── 2.12 (InteractiveGraph layout prop)
    ├── 2.13 → 2.14 (hook)
    └── 2.15 → 2.16 + 2.17 (component + Shell)

Phase 3 (Named view) — depends on Phase 1 + 2
    ├── 3.1 → 3.2 (RationaleViewPayload DTO)
    ├── 3.3 → 3.4 (view_load dispatch)
    ├── 3.5 → 3.6 (load_rationale_view service)
    └── 3.7 → 3.8 (regression)
```

## Out-of-Scope Reminders (locked from spec)

- No cross-space corroboration
- No PDF / Mermaid export
- No `view_load_rationale` separate MCP tool
- No materialized corroboration scores
- No direction filter on rationale endpoint (always bidirectional)
- No per-call `lens` override on `view_load` (lens always from saved row)
- No animation on dagre layout
- No drag-to-rearrange nodes
- No cross-pane linking
