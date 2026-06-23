# Kernel Tasks: sddk/ADR-045-phase1-debt1-debt2

## Router Context Used
- **Knowledge Coverage**: sufficient — proposal (engram #2766), spec (engram #2768, 9 REQs / 25+ scenarios), design (engram #2770, full code snippets), explore (engram #2764, code-verified @ 2f65940). All ~13 + 2 follow-on files pinned.
- **Context Quality**: **C2** (code-verified locations + line ranges; all three findings from #2767 spec+design memo already folded into tasks).
- **Taxonomy**: dead-code removal; dual-model unification (ExplorationPath → ExplorationSession); positive semantic reactivation of `RecentExplorationsStrip` in production.
- **Invariants Driving Tasks**:
  - ADR-039 §1 — pane-stack is the sole navigation model (drives T1.1: `default_navigation_mode` → `default_pane_stack_navigation` returning `"pane-stack"`).
  - ADR-040 Wave 3 — `ExplorationSession` is the unified persistence shape (drives T1.3 generate_artifact migration + T1.2 list_explorations return type + T2.4 strip `panes[0]`).
  - ADR-045 dispositions — Debt 1 (remove orphan route) + Debt 2 (unify model) executed; Debt 3 (Postgres) deferred, must stay ⚠️ Partial in T5.1.
- **Recommended Effort**: **skip** (per proposal entropy-sdd lens; C2-verified, dual shapes never coexist in live call chain).

## Review Budget Forecast
- **Estimated changed lines**: **−290 LOC net** (back-end −155, front-end −100, navigation follow-on ~0, mocks ~0, tests −40, docs +5).
- **400-line budget risk**: **Low** — net reduction across 14 files, no single file grows beyond ~60 LOC, no new connascence introduced.
- **Chained PRs recommended**: **No** — big-bang single PR is safe because the legacy `ExplorationPath` surface has zero production callers (explore #2764 finding #1). The dual shapes never coexist in any live call chain.
- **Decision needed before apply**: **No** — all design forks resolved in proposal §Invariants + design §3. The single ambiguity (`ObjectIdentityEntry` doc rustdoc refs in `domain/object_identity.rs:187,253`) is explicitly out of scope per spec.

## Knowledge Traceability
- **Work item source artifacts**:
  - Proposal: engram #2766 (`sddk/adr-045-phase1-debt1-debt2/proposal`)
  - Spec: engram #2768 (`sddk/adr-045-phase1-debt1-debt2/spec`)
  - Design: engram #2770 (`sddk/adr-045-phase1-debt1-debt2/design`)
  - Spec+design findings memo: engram #2767 (`sddk/adr-045-phase1-debt1-debt2/spec-design-findings`)
- **Ownership source**: explore #2764 (every file + line range pinned); ownership authority = proposal (#2766) + design (#2770).
- **Open knowledge gaps affecting execution**: **None**. All scope items resolved. Pre-existing test failures unrelated to this change are tolerated per instructions.

## Commit Strategy

All sub-tasks land in **one atomic commit** with message:

```
refactor: unify exploration persistence onto ExplorationSession (ADR-045 Phase 1)
```

**Rationale**: The `ExplorationPath` production path is fully dead (explore #2764 finding #1); removing it cannot break any live call chain. Big-bang is safe. The 14 sub-tasks are review-order sub-sections within the single commit — they exist to make the diff navigable to the human reviewer, not to gate the commit.

---

## Tasks

### T1.1: `dto.rs` — remove legacy types + rename default helper
- **Files**: `crates/cognicode-explorer/src/dto.rs`
- **LOC delta**: **−50** (delete `ExplorationPath`, `ExplorationColumn`, `SaveExplorationRequest` structs; delete `default_navigation_mode`; delete `exploration_path_old_format_no_navigation_mode` test; add `default_pane_stack_navigation` returning `"pane-stack"`; update `ExplorationSession.navigation_mode` serde default attribute to point at the new helper; update `ObjectIdentityEntry` doc reference to remove the `ExplorationPath` phrase).
- **Depends on**: none (independent leaf; can be edited first).
- **Verification**:
  ```bash
  grep -n "ExplorationPath\|ExplorationColumn\|SaveExplorationRequest\|default_navigation_mode" \
       crates/cognicode-explorer/src/dto.rs
  # expected: no output, exit code 1
  grep -n "default_pane_stack_navigation" crates/cognicode-explorer/src/dto.rs
  # expected: ≥2 matches (definition + #[serde(default = "...")] attribute)
  cargo check -p cognicode-explorer
  # expected: exit 0 (caller files still reference ExplorationPath; that's expected at this point)
  ```
- **Commit message**: n/a (part of the single atomic commit).
- **Risk**: **Low** — pure deletion + helper rename; compiler will flag any missed caller in subsequent tasks.
- **Rollback**: `git checkout HEAD -- crates/cognicode-explorer/src/dto.rs`.

### T1.2: `facades/mod.rs` — trim `PersistenceService` trait
- **Files**: `crates/cognicode-explorer/src/facades/mod.rs`
- **LOC delta**: **−15** (drop `ExplorationPath` from `use crate::dto::{...}`; remove `save_exploration` method from trait; change `list_explorations` return type from `Vec<ExplorationPath>` → `Vec<ExplorationSession>`; rewrite KNOWN-DEBT doc comment: Debt 1 ✅, Debt 2 ✅, Debt 3 still open).
- **Depends on**: T1.1 (the type must be gone before the trait can drop the method cleanly).
- **Verification**:
  ```bash
  grep -n "ExplorationPath\|save_exploration\b" crates/cognicode-explorer/src/facades/mod.rs
  # expected: no output
  grep -n "list_explorations" crates/cognicode-explorer/src/facades/mod.rs
  # expected: signature shows -> ExplorerResult<Vec<crate::dto::ExplorationSession>>
  cargo check -p cognicode-explorer
  # expected: errors only in persistence.rs (impl) + api_graph_tests.rs + api_rationale_tests.rs — those are resolved by T1.3 + T1.5
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — trait trim; downstream impl/tests must catch up.
- **Rollback**: `git checkout HEAD -- crates/cognicode-explorer/src/facades/mod.rs`.

### T1.3: `facades/persistence.rs` — drop `paths` store + migrate `generate_artifact`
- **Files**: `crates/cognicode-explorer/src/facades/persistence.rs`
- **LOC delta**: **−30 net** (delete `type ExplorationPathStore = Mutex<HashMap<String, ExplorationPath>>`; delete `paths: Arc<ExplorationPathStore>` field + initializer; delete `save_exploration` impl block ~42 LOC; rewrite `generate_artifact` to look up `self.sessions`; rewrite `render_replay_json` + `render_replay_markdown` to consume `&ExplorationSession` and emit `{exploration_id, version, events, panes}` shape with `panes[0] ?? events[0]` first-object fallback; add `render_replay_json_unknown` + `render_replay_markdown_unknown` for missing-id case; rewrite `list_explorations` to read `self.sessions` and filter by `workspace_id`).
- **Depends on**: T1.1 (types), T1.2 (trait signature).
- **Verification**:
  ```bash
  grep -n "ExplorationPathStore\|save_exploration\b\|self.paths\b" \
       crates/cognicode-explorer/src/facades/persistence.rs
  # expected: no output
  grep -n "render_replay_json\|render_replay_markdown" crates/cognicode-explorer/src/facades/persistence.rs
  # expected: 4 functions (json + json_unknown + md + md_unknown), all consume &ExplorationSession
  cargo check -p cognicode-explorer
  # expected: errors only in api.rs (route registrations) + api_graph_tests.rs + api_rationale_tests.rs — resolved by T1.4 + T1.5
  cargo test -p cognicode-explorer --lib facades::persistence
  # expected: exit 0; persistence unit tests green
  ```
- **Commit message**: n/a.
- **Risk**: **Medium** — `generate_artifact` rewrite is the largest behavior change in the change set; defensively coded with explicit unknown fallback.
- **Rollback**: `git checkout HEAD -- crates/cognicode-explorer/src/facades/persistence.rs`.

### T1.4: `api.rs` — unregister orphan routes + delete handlers
- **Files**: `crates/cognicode-explorer/src/api.rs`
- **LOC delta**: **−40** (drop `SaveExplorationRequest` from `use crate::dto::{...}`; delete 2× `.route("/api/explorations", post(save_exploration))` registrations in `router()` and `router_with_state()`; delete 2× `.route("/api/explorations/:exploration_id", get(get_exploration))` registrations; delete `async fn save_exploration(...)` handler ~6 LOC; delete `async fn get_exploration(...)` handler ~13 LOC).
- **Depends on**: T1.1 (type), T1.3 (impl migrated so handler removal doesn't leave a dangling caller).
- **Verification**:
  ```bash
  grep -n "save_exploration\|get_exploration\|SaveExplorationRequest\|/api/explorations" \
       crates/cognicode-explorer/src/api.rs
  # expected: no output for save_exploration/SaveExplorationRequest; the `/api/exploration-sessions` (plural + sessions suffix) routes still appear (those are the live session routes)
  cargo check -p cognicode-explorer
  # expected: exit 0 — backend crate compiles standalone after this task completes (test mocks untouched here; failures deferred to T1.5)
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — pure deletion; the two routes being removed have zero production callers (explore #2764 finding #3).
- **Rollback**: `git checkout HEAD -- crates/cognicode-explorer/src/api.rs`.

### T1.5: backend test mocks — update `MockPersistenceService`
- **Files**:
  - `crates/cognicode-explorer/src/api_graph_tests.rs`
  - `crates/cognicode-explorer/src/api_rationale_tests.rs`
- **LOC delta**: **−20 total** (2 files × ~−10 each: drop `save_exploration` method from both `MockPersistenceService` impls; repoint `list_explorations` return type to `Vec<ExplorationSession>` returning empty vec; remove `ExplorationPath`/`SaveExplorationRequest` imports).
- **Depends on**: T1.2 (trait signature).
- **Verification**:
  ```bash
  grep -n "save_exploration\b\|ExplorationPath\|SaveExplorationRequest" \
       crates/cognicode-explorer/src/api_graph_tests.rs \
       crates/cognicode-explorer/src/api_rationale_tests.rs
  # expected: no output
  cargo test -p cognicode-explorer
  # expected: exit 0; ALL tests in the cognicode-explorer crate pass
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — test-only updates; assertions previously assumed `MockPersistenceService` matched the old trait.
- **Rollback**: `git checkout HEAD -- crates/cognicode-explorer/src/api_graph_tests.rs crates/cognicode-explorer/src/api_rationale_tests.rs`.

### T2.1: `api/schemas.ts` — remove legacy zod schemas
- **Files**: `apps/explorer-ui/src/api/schemas.ts`
- **LOC delta**: **−20** (delete `explorationColumnSchema` + `ExplorationColumn` type alias ~5 LOC; delete `explorationPathSchema` + `ExplorationPath` type alias ~12 LOC; delete `saveExplorationRequestSchema` + `SaveExplorationRequest` type alias ~5 LOC; `explorationSessionSchema` and `saveExplorationSessionRequestSchema` kept untouched).
- **Depends on**: none.
- **Verification**:
  ```bash
  grep -n "explorationColumnSchema\|explorationPathSchema\|saveExplorationRequestSchema\|ExplorationColumn\b\|ExplorationPath\b\|SaveExplorationRequest\b" \
       apps/explorer-ui/src/api/schemas.ts
  # expected: no output
  grep -n "explorationSessionSchema" apps/explorer-ui/src/api/schemas.ts
  # expected: ≥1 match (kept)
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — pure schema deletion; callers updated in T2.2-T2.7.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/api/schemas.ts`.

### T2.2: `api/types.ts` — drop removed re-exports
- **Files**: `apps/explorer-ui/src/api/types.ts`
- **LOC delta**: **−5** (drop `ExplorationColumn`, `ExplorationPath`, `SaveExplorationRequest` from `export type {...}`; drop the same three from `export {...}` of schemas).
- **Depends on**: T2.1 (schemas must be gone first).
- **Verification**:
  ```bash
  grep -n "ExplorationColumn\|ExplorationPath\|SaveExplorationRequest" \
       apps/explorer-ui/src/api/types.ts
  # expected: no output
  cd apps/explorer-ui && npx tsc --noEmit 2>&1 | grep -E "TS2305|Cannot find"
  # expected: errors only in useExplorations.ts / GraphLanding / etc — those resolve in subsequent tasks; this task itself is clean
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — re-export trim.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/api/types.ts`.

### T2.3: `hooks/useExplorations.ts` — drop `saveExploration` + repoint list schema
- **Files**: `apps/explorer-ui/src/hooks/useExplorations.ts`
- **LOC delta**: **−25** (delete `saveExplorationRequestSchema` import; drop `ExplorationPath` type import; repoint `explorationsListSchema = z.array(explorationPathSchema)` → `z.array(explorationSessionSchema)`; delete entire `saveExploration` async function ~20 LOC; update file header doc to remove the save section; keep `useExplorations`, `useSnapshotCache`, `saveExplorationSession`, `generateArtifact`, `useArtifact` unchanged).
- **Depends on**: T2.1 (schemas), T2.2 (types re-exports).
- **Verification**:
  ```bash
  grep -n "saveExploration\b\|ExplorationPath\|explorationPathSchema" \
       apps/explorer-ui/src/hooks/useExplorations.ts
  # expected: no output for saveExploration/ExplorationPath/explorationPathSchema (note: `saveExplorationSession` is the KEEP target — verify it remains)
  grep -n "saveExplorationSession\|explorationSessionSchema" \
       apps/explorer-ui/src/hooks/useExplorations.ts
  # expected: ≥1 match each
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — function deletion + schema repoint.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/hooks/useExplorations.ts`.

### T2.4: `RecentExplorationsStrip.tsx` — consume `panes[0]` instead of `columns[0]`
- **Files**: `apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.tsx`
- **LOC delta**: **−10** (change `import type { ExplorationPath }` → `import type { ExplorationSessionDto }`; change `ExplorationPath` prop type → `ExplorationSessionDto`; replace `exploration.columns[0]?.object_id ?? exploration.id` with `exploration.panes[0]?.object_id ?? exploration.events[0]?.object_id ?? exploration.id`; replace `exploration.columns.length` with `exploration.panes.length`; update pluralization label "drill-down" → "panes").
- **Depends on**: T2.1 (schemas), T2.2 (types), T2.3 (hook repointed).
- **Verification**:
  ```bash
  grep -n "ExplorationPath\|columns\[" apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.tsx
  # expected: no output
  grep -n "ExplorationSessionDto\|panes\[0\]" apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.tsx
  # expected: ≥1 match each
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — semantic mapping is 1:1 (drill-down depth → open pane count) and the prop type is the only consumer signature change.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.tsx`.

### T2.5: `GraphLanding.tsx` — dispatch `SELECT_OBJECT` from `panes[0]`
- **Files**: `apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx`
- **LOC delta**: **−10** (change `onExplorationClick: (exploration: ExplorationPath) => void` → `(exploration: ExplorationSessionDto) => void`; replace `exploration.columns[0].object_id` / `.active_view` reads with `exploration.panes[0].object_id` / `.view_id`; default `kind: "symbol"` since `Pane.kind` is not on the wire schema).
- **Depends on**: T2.4 (strip type).
- **Verification**:
  ```bash
  grep -n "ExplorationPath\|columns\[" apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx
  # expected: no output
  grep -n "ExplorationSessionDto\|panes\[0\]" apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx
  # expected: ≥1 match each
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — click-handler semantic shift; the dispatched action payload shape is unchanged.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/components/GraphLanding/GraphLanding.tsx`.

### T2.6: `state/context.ts` + `state/slices/explorations.ts` + `state/slices/index.ts` — remove dead state slice
- **Files**:
  - `apps/explorer-ui/src/state/context.ts`
  - `apps/explorer-ui/src/state/slices/explorations.ts` (delete file entirely)
  - `apps/explorer-ui/src/state/slices/index.ts`
- **LOC delta**: **−30** (delete `state/slices/explorations.ts` entirely; drop `ExplorationPath` import from `context.ts`; remove `explorations: ExplorationPath[]` field from `AppState`; remove `explorations: []` from `initialState` + `initialStateWithFocus`; remove `{ type: "ADD_EXPLORATION"; payload: ExplorationPath }` from `Action` discriminated union; drop `explorationsReducer` import + call from `state/slices/index.ts`; drop `ExplorationsAction` re-export).
- **Depends on**: T2.1-T2.5 (so no React component still references `state.explorations`).
- **Verification**:
  ```bash
  grep -rn "ExplorationPath\|ADD_EXPLORATION\|ExplorationsAction\|state\.explorations\b" \
       apps/explorer-ui/src/
  # expected: no output
  test -f apps/explorer-ui/src/state/slices/explorations.ts
  # expected: exit 1 (file deleted)
  cd apps/explorer-ui && npx tsc --noEmit
  # expected: exit 0 (all state references gone)
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — removing a never-dispatched reducer (explore #2764 finding #1) cannot break runtime behavior.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/state/context.ts apps/explorer-ui/src/state/slices/explorations.ts apps/explorer-ui/src/state/slices/index.ts`; recreate explorations.ts if deleted.

### T2.7: `state/navigation/types.ts` + `state/navigation/paneStack.ts` — local `ChainEntry` interface
- **Files**:
  - `apps/explorer-ui/src/state/navigation/types.ts`
  - `apps/explorer-ui/src/state/navigation/paneStack.ts`
- **LOC delta**: **+10 / −10 = 0 net** (drop `ExplorationColumn` import from both; add local `export interface ChainEntry { object_id: string; active_view: string | null; kind: string }` in `types.ts`; replace `chain: ExplorationColumn[]` with `chain: ChainEntry[]` in `NavigationState`; rewrite `chainFromActivePane` in `paneStack.ts` to return `ChainEntry[]`).
- **Depends on**: T2.6 (so the state context doesn't still expect the old field shape).
- **Verification**:
  ```bash
  grep -n "ExplorationColumn" apps/explorer-ui/src/state/navigation/types.ts \
                              apps/explorer-ui/src/state/navigation/paneStack.ts
  # expected: no output
  grep -n "ChainEntry" apps/explorer-ui/src/state/navigation/types.ts \
                         apps/explorer-ui/src/state/navigation/paneStack.ts
  # expected: ≥1 match each
  cd apps/explorer-ui && npx tsc --noEmit
  # expected: exit 0
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — internal type rename; the `chain` field is consumed by 1 test only (spec memo #2767).
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/state/navigation/types.ts apps/explorer-ui/src/state/navigation/paneStack.ts`.

### T3.1: `mocks/fixtures.ts` — replace `explorationPathFixture` with `explorationSessionFixture`
- **Files**: `apps/explorer-ui/src/mocks/fixtures.ts`
- **LOC delta**: **+15 / −15 = 0 net** (drop `ExplorationPath` import; delete `explorationPathFixture` ~13 LOC; add `explorationSessionFixture: ExplorationSessionDto` with `events: [3 entries]`, `panes: [3 entries]`, `navigation_mode: "pane-stack"`, `id: "session-001"`, `workspace_id: WORKSPACE_ID`, `created_at`).
- **Depends on**: T2.1 (schemas).
- **Verification**:
  ```bash
  grep -n "explorationPathFixture\|ExplorationPath\b" apps/explorer-ui/src/mocks/fixtures.ts
  # expected: no output
  grep -n "explorationSessionFixture" apps/explorer-ui/src/mocks/fixtures.ts
  # expected: ≥1 match (exported)
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — fixture substitution; the new fixture shape mirrors the wire contract.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/mocks/fixtures.ts`.

### T3.2: `mocks/handlers.ts` — repoint LIST handler + drop POST
- **Files**: `apps/explorer-ui/src/mocks/handlers.ts`
- **LOC delta**: **−10** (drop `explorationPathFixture` import; delete `http.post("/api/explorations", ...)` handler ~10 LOC; rewrite `http.get("/api/workspaces/:workspace_id/explorations", ...)` to return array of `{ ...explorationSessionFixture, workspace_id: <URL param> }`).
- **Depends on**: T3.1 (fixture).
- **Verification**:
  ```bash
  grep -n "explorationPathFixture\|http\.post.*api/explorations\b" \
       apps/explorer-ui/src/mocks/handlers.ts
  # expected: no output
  grep -n "explorationSessionFixture\|http\.get.*explorations" \
       apps/explorer-ui/src/mocks/handlers.ts
  # expected: ≥1 match each
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — MSW handler update; aligns mock response with the now-unified backend.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/mocks/handlers.ts`.

### T4.1: `api/schemas.test.ts` — assert `explorationSessionSchema`
- **Files**: `apps/explorer-ui/src/api/schemas.test.ts`
- **LOC delta**: **−10** (drop `explorationColumnSchema`, `explorationPathSchema`, `saveExplorationRequestSchema` imports; drop `explorationPathFixture` import; add `explorationSessionSchema` import + `explorationSessionFixture` import; delete `describe("explorationColumnSchema", ...)` + `describe("explorationPathSchema", ...)` blocks ~20 LOC; add `describe("explorationSessionSchema", ...)` with 2 tests (accepts with events+panes, requires panes array); delete `saveExplorationRequestSchema` test).
- **Depends on**: T2.1 (schemas), T3.1 (fixture).
- **Verification**:
  ```bash
  grep -n "explorationColumnSchema\|explorationPathSchema\|saveExplorationRequestSchema\|explorationPathFixture" \
       apps/explorer-ui/src/api/schemas.test.ts
  # expected: no output
  cd apps/explorer-ui && npx vitest run src/api/schemas.test.ts
  # expected: exit 0; new explorationSessionSchema describe passes
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — test assertion updates only.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/api/schemas.test.ts`.

### T4.2: `RecentExplorationsStrip.test.tsx` — use `ExplorationSessionDto` + `explorationSessionFixture`
- **Files**: `apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.test.tsx`
- **LOC delta**: **−10** (change `import type { ExplorationPath }` → `import type { ExplorationSessionDto }`; change `explorationPathFixture` → `explorationSessionFixture`; change `makeExploration` factory return type to `ExplorationSessionDto`; change factory test inputs: `columns: [{...}]` → `panes: [{ pane_id, object_id, view_id, scroll_y: 0, viewport: null }]` at the 2 call sites).
- **Depends on**: T2.4 (strip retype), T3.1 (fixture).
- **Verification**:
  ```bash
  grep -n "ExplorationPath\|explorationPathFixture\|columns:" \
       apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.test.tsx
  # expected: no output
  cd apps/explorer-ui && npx vitest run src/components/GraphLanding/RecentExplorationsStrip.test.tsx
  # expected: exit 0; all 4–6 tests pass
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — fixture shape swap; assertions check prop rendering, not internal navigation.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.test.tsx`.

### T4.3: `hooks.test.ts` — drop `saveExploration` test block
- **Files**: `apps/explorer-ui/src/hooks/hooks.test.ts`
- **LOC delta**: **−20** (drop `saveExploration` from `useExplorations` import; drop `explorationPathFixture` import; delete `describe("saveExploration", ...)` block ~20 LOC; keep `useExplorations` + `generateArtifact` describe blocks).
- **Depends on**: T2.3 (hook repoint).
- **Verification**:
  ```bash
  grep -n "saveExploration\b\|explorationPathFixture\|ExplorationPath" \
       apps/explorer-ui/src/hooks/hooks.test.ts
  # expected: no output for `saveExploration\b` (the bare word); `saveExplorationSession` may appear (the KEEP target)
  grep -n "saveExploration\b" apps/explorer-ui/src/hooks/hooks.test.ts
  # expected: no output (removed)
  cd apps/explorer-ui && npx vitest run src/hooks/hooks.test.ts
  # expected: exit 0; remaining tests pass
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — deletion of a describe block targeting a now-removed function.
- **Rollback**: `git checkout HEAD -- apps/explorer-ui/src/hooks/hooks.test.ts`.

### T5.1: `docs/adr/ADR-045-exploration-debts.md` — update debt status
- **Files**: `docs/adr/ADR-045-exploration-debts.md`
- **LOC delta**: **+10 / −5 = +5 net** (in `## Debt 1` section, change status badge from `⚠️ Partial` → `✅ Done` + add one-line resolution note; same for `## Debt 2`; in `## Debt 3` section, keep `⚠️ Partial`; in `## Ordering Constraint` section, update the prose to note that Debt 3 is now unblocked by Phase 1).
- **Depends on**: none (can land in parallel with code changes in the same commit; status doc reflects post-apply truth).
- **Verification**:
  ```bash
  grep -B1 "⚠️ Partial\|✅ Done" docs/adr/ADR-045-exploration-debts.md | grep -E "Debt [123]"
  # expected: Debt 1 → ✅ Done; Debt 2 → ✅ Done; Debt 3 → ⚠️ Partial
  grep -A2 "Ordering Constraint" docs/adr/ADR-045-exploration-debts.md
  # expected: prose mentions Phase 1 completion unblocking Debt 3
  ```
- **Commit message**: n/a.
- **Risk**: **Low** — doc-only edit; ADR stays the canonical debt-disposition document.
- **Rollback**: `git checkout HEAD -- docs/adr/ADR-045-exploration-debts.md`.

---

## Cumulative Verification (run after all sub-tasks applied)

Run **after** every T1.*–T5.* task lands but **before** committing the atomic commit:

```bash
# --- Backend ---
cd /var/home/rubentxu/Proyectos/rust/CogniCode
cargo test -p cognicode-explorer
# expected: exit 0; ALL tests pass (dto, persistence, api, api_graph_tests, api_rationale_tests)
cargo check --workspace
# expected: exit 0; no warnings about ExplorationPath, ExplorationColumn,
# SaveExplorationRequest, or default_navigation_mode

# --- Grep invariants (zero matches) ---
grep -rn "ExplorationPath\|ExplorationColumn\|SaveExplorationRequest\|default_navigation_mode" \
     crates/ 2>/dev/null
# expected: no output, exit code 1
grep -rn "save_exploration\b" crates/ 2>/dev/null
# expected: no output, exit code 1
grep -n "get_exploration\b" crates/cognicode-explorer/src/api.rs
# expected: no output, exit code 1

# --- Frontend ---
cd apps/explorer-ui
npx tsc --noEmit
# expected: exit 0; no type errors
npm run test -- --run
# expected: exit 0; schemas.test.ts, RecentExplorationsStrip.test.tsx, hooks.test.ts all pass
grep -rn "ExplorationPath\|ExplorationColumn\|saveExploration\b" src/ 2>/dev/null
# expected: no output (note: `saveExplorationSession` is the KEEP target, not searched)

# --- Docs ---
cd /var/home/rubentxu/Proyectos/rust/CogniCode
grep -B1 "Done\|Partial" docs/adr/ADR-045-exploration-debts.md | grep -E "Debt [123]"
# expected: Debt 1 → ✅ Done; Debt 2 → ✅ Done; Debt 3 → ⚠️ Partial
```

**Tolerated pre-existing failures** (NOT in scope per instructions): any test failures unrelated to `ExplorationPath`/`ExplorationSession`/`saveExploration` references observed in `main @ 2f65940` baseline are pre-existing and out of scope.

## Rollback Notes

Because all sub-tasks land in **one atomic commit**, rollback is a single `git revert <commit-sha>` of the entire PR. No intermediate state is observable. The in-memory `sessions` HashMap in `PersistenceServiceImpl` is process-lifetime only (no schema migration, no Postgres data to roll back), so revert is clean and lossless.

Per-sub-task rollbacks are listed above for **pre-commit** scenarios only (e.g., the developer notices a problem in T1.3 before committing and wants to undo just that file). Once committed, rollback = revert the whole PR.

---

## Summary

- **Total tasks**: **14** (T1.1, T1.2, T1.3, T1.4, T1.5, T2.1, T2.2, T2.3, T2.4, T2.5, T2.6, T2.7, T3.1, T3.2, T4.1, T4.2, T4.3, T5.1) — **18 atomic sub-tasks** within **1 atomic commit**.
- **Estimated total LOC delta**: **~−290 LOC net** (back-end ~−155, front-end ~−100, navigation follow-on 0, mocks 0, tests ~−40, docs +5).
- **Estimated commits**: **1** (single PR, big-bang migration).
- **400-line review budget**: **Low risk** — largest single-file delta is `dto.rs` at ~−50 LOC.
- **Unresolved blockers**: **None**.
