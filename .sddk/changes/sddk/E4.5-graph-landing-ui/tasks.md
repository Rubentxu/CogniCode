# Kernel Tasks: sddk/E4.5-graph-landing-ui

## Router Context Used
- **Knowledge Coverage**: sufficient (explore #2685, proposal #2686, spec #2688, design #2687 all in engram)
- **Context Quality**: C2 — additive change, no re-exploration needed
- **Taxonomy**: dominant axis = missing-API-contract (additive); secondary = test-coverage gap (strip has 0 tests)
- **Domain Language**: ExplorationPath (legacy columns), ExplorationSession (pane-stack per ADR-040); LIST returns `Vec<ExplorationPath>` per frontend `explorationsListSchema`
- **Invariants Driving Tasks**:
  - Trait method MUST be `async fn` (trait is `#[async_trait]`)
  - Route MUST be in BOTH `router()` (api.rs:474) AND `router_with_state()` (api.rs:431) — unconditional
  - Handler MUST use `State<ApiState>` + `state.persistence` (peer handlers at api.rs:753, 760, 783)
  - Empty result MUST be `200 + []` (NEVER 404)
  - Test doubles MUST be updated in the SAME COMMIT as the trait method (compile gate)
- **Recommended Effort**: verify → small additive change, sized to ~150 LOC across 4 atomic commits

## Design Corrections Incorporated (MUST follow)
1. **Trait method is `async fn`** — not plain `fn`. Trait is `#[async_trait]`, every peer is async.
2. **Handler shape**: `State<ApiState>` + `state.persistence` + `Result<Response, ApiError>` + `.into_response()`. NOT `State<Arc<ExplorerServices>>`.
3. **Test doubles in SAME COMMIT** as trait method — both `api_rationale_tests.rs` and `api_graph_tests.rs` MUST add `list_explorations` or `cargo build` dies.
4. **Route in BOTH routers** unconditionally — no `#[cfg(feature = ...)]` gate.

## Open Question Resolved
- `columns` is NOT vestigial: `save_exploration` (persistence.rs:90-98) assigns `columns: request.columns` and validates non-empty (line 69). Strip's pane-count label (line 74) is correct. **No UX debt.**

## Known Debt (flagged in `KNOWN-DEBT` rustdoc — NOT fixed here)
- `get_exploration` (api.rs:768) calls `load_exploration_session` despite doc saying "path" — pre-existing mis-wire
- `ExplorationPath` vs `ExplorationSession` dual model — unification is future ADR
- In-memory store lifetime (lost on restart) — backend limitation, requires Postgres backing

## Review Budget Forecast
- **Estimated changed lines**: ~165 (Rust ~95, TS ~60, MD ~10)
- **400-line budget risk**: Low
- **Chained PRs recommended**: No
- **Decision needed before apply**: No

## Knowledge Traceability
- **Work item source artifacts**:
  - explore: engram #2685 (3 contradictions, 6 gaps)
  - proposal: engram #2686 (verified LIST returns `Vec<ExplorationPath>`; deeper mis-wire flagged)
  - spec: engram #2688 (7 REQs, 24 scenarios)
  - design: engram #2687 (2 corrections to spec — async trait, test doubles same commit; resolved columns question)
- **Ownership source**:
  - Backend trait + impl: `crates/cognicode-explorer` (no other owner)
  - Frontend strip: `apps/explorer-ui` (no other owner)
  - Docs: `docs/explorer-roadmap.md`, `docs/adr/ADR-039` (no owner conflicts)
- **Open knowledge gaps affecting execution**: None

## Pre-existing State to Tolerate (NOT fixed in this change)
- 5 pre-existing unit test failures (RationaleView.test.tsx)
- 38 pre-existing lint errors
- `cargo check` 2 warnings
- TypeScript build was fixed in v0.11.3 — current build must stay green

---

## ⚠️ LOAD-BEARING COMMIT — Commit 1 is the hinge

Without Commit 1, the build is BROKEN. The trait method addition forces every implementor (impl + 2 test doubles) to add it in the SAME commit. A worktree split between trait and doubles will fail `cargo build`.

---

## Tasks

### Commit 1: Backend — trait + impl + route + handler + test doubles (HINGE)

#### T1.1: Add `list_explorations` to `PersistenceService` trait
- **Files**: `crates/cognicode-explorer/src/facades/mod.rs` (insert after line 192, before `generate_artifact`)
- **LOC delta**: +7 (trait method signature + doc comment)
- **Depends on**: none
- **Implementation**:
  ```rust
  /// List saved exploration paths for a workspace, filtered by `workspace_id`.
  ///
  /// Returns the legacy column-based `ExplorationPath` records (NOT
  /// `ExplorationSession`). Used by `GET /api/workspaces/:workspace_id/explorations`
  /// to feed the Graph Landing recent-explorations strip.
  ///
  /// KNOWN-DEBT (out of scope for this change):
  /// - `get_exploration` (api.rs:768) mis-wires the GET-by-id path to the
  ///   session store; only the LIST path is correct here.
  /// - `ExplorationPath` (legacy columns) vs `ExplorationSession` (pane-stack
  ///   per ADR-040) are SEPARATE stores; unification is a future ADR.
  /// - This is an in-memory store — paths do not survive a restart.
  async fn list_explorations(
      &self,
      workspace_id: &str,
  ) -> ExplorerResult<Vec<ExplorationPath>>;
  ```
- **Verification**:
  ```bash
  cargo check -p cognicode-explorer 2>&1 | grep -E "error\[" 
  # EXPECTED: empty (impl + 2 doubles must add the method in same commit, see T1.2, T1.3)
  ```
- **Commit message**: `feat(cognicode-explorer): add list_explorations trait method`
- **Risk**: Medium — trait surface change forces all implementors to update
- **Rollback**: Remove the trait method block; impl + doubles still compile (no caller)

#### T1.2: Implement `list_explorations` in `PersistenceServiceImpl`
- **Files**: `crates/cognicode-explorer/src/facades/persistence.rs` (insert after `save_exploration_session` impl)
- **LOC delta**: +15 (impl block + rustdoc)
- **Depends on**: T1.1
- **Implementation**:
  ```rust
  async fn list_explorations(
      &self,
      workspace_id: &str,
  ) -> ExplorerResult<Vec<ExplorationPath>> {
      let paths = self.paths
          .lock()
          .map_err(|_| ExplorerError::Anyhow(anyhow::anyhow!("exploration path store poisoned")))?;
      Ok(paths
          .values()
          .filter(|p| p.workspace_id == workspace_id)
          .cloned()
          .collect())
  }
  ```
- **Verification**:
  ```bash
  cargo check -p cognicode-explorer 2>&1 | grep -E "error\[" 
  # EXPECTED: empty (errors are in test doubles, see T1.3)
  ```
- **Commit message**: `feat(cognicode-explorer): implement list_explorations over in-memory paths store`
- **Risk**: Low — pure read, mirrors `list_view_specs` pattern
- **Rollback**: Remove the impl block; trait method orphans but compiles

#### T1.3: Update BOTH test doubles to implement `list_explorations` (COMPILE GATE)
- **Files**:
  - `crates/cognicode-explorer/src/api_rationale_tests.rs` (insert after `load_exploration_session` impl, ~line 235)
  - `crates/cognicode-explorer/src/api_graph_tests.rs` (insert after `load_exploration_session` impl, ~line 198)
- **LOC delta**: +16 (8 per file × 2 files)
- **Depends on**: T1.1
- **Implementation** (identical for both files):
  ```rust
  async fn list_explorations(
      &self,
      _workspace_id: &str,
  ) -> crate::ExplorerResult<Vec<crate::dto::ExplorationPath>> {
      Err(crate::error::ExplorerError::FeatureDisabled("mock".into()))
  }
  ```
- **Verification**:
  ```bash
  cargo check -p cognicode-explorer --tests 2>&1 | grep -E "error\[" 
  # EXPECTED: empty (both doubles now implement the method)
  cargo build -p cognicode-explorer 2>&1 | tail -5
  # EXPECTED: "Finished ... target ..." with no errors
  ```
- **Commit message**: `test(cognicode-explorer): implement list_explorations in mock doubles`
- **Risk**: High if MISSED — `cargo build` dies the moment the trait method is added without these
- **Rollback**: Remove the new impls; trait method becomes unused but compiles
- **CRITICAL**: This MUST land in the SAME commit as T1.1. Splitting breaks the build.

#### T1.4: Add `GET /api/workspaces/:workspace_id/explorations` route to BOTH routers
- **Files**:
  - `crates/cognicode-explorer/src/api.rs` line ~460 (in `router_with_state()`, after `exploration-sessions` block)
  - `crates/cognicode-explorer/src/api.rs` line ~503 (in `router()`, after `exploration-sessions` block)
- **LOC delta**: +4 (2 lines × 2 routers)
- **Depends on**: T1.1, T1.3 (handler must exist for `get(list_explorations)` to compile)
- **Implementation** (insert in both routers at same logical position — after `/api/exploration-sessions/:session_id`):
  ```rust
  .route(
      "/api/workspaces/:workspace_id/explorations",
      get(list_explorations),
  )
  ```
- **Verification**:
  ```bash
  cargo build -p cognicode-explorer 2>&1 | grep -E "error\[" 
  # EXPECTED: empty (handler exists by this point — T1.5 same commit)
  grep -n "/api/workspaces/:workspace_id/explorations" crates/cognicode-explorer/src/api.rs
  # EXPECTED: 2 occurrences (one in each router)
  ```
- **Commit message**: `feat(cognicode-explorer): route GET /api/workspaces/:id/explorations`
- **Risk**: Medium — wrong order breaks build (handler must be defined before route)
- **Rollback**: Remove the `.route(...)` lines from both routers

#### T1.5: Add `list_explorations` handler using `State<ApiState>` + `state.persistence`
- **Files**: `crates/cognicode-explorer/src/api.rs` (insert after `save_exploration_session` handler, ~line 788)
- **LOC delta**: +12 (handler + rustdoc)
- **Depends on**: T1.1, T1.2
- **Implementation**:
  ```rust
  /// GET /api/workspaces/:workspace_id/explorations — list saved
  /// exploration paths for a workspace. Returns `200` + `Vec<ExplorationPath>`
  /// (empty array if no paths), NEVER 404.
  ///
  /// KNOWN-DEBT: the sibling `get_exploration` (api.rs:768) is mis-wired
  /// to the session store; this LIST path is correct. See ADR-045 (future).
  async fn list_explorations(
      State(state): State<ApiState>,
      Path(workspace_id): Path<String>,
  ) -> Result<Response, ApiError> {
      Ok(Json(state.persistence.list_explorations(&workspace_id).await?).into_response())
  }
  ```
- **Verification**:
  ```bash
  cargo build -p cognicode-explorer 2>&1 | grep -E "error\[" 
  # EXPECTED: empty
  cargo clippy -p cognicode-explorer --no-deps 2>&1 | grep -E "warning.*list_explorations"
  # EXPECTED: empty
  ```
- **Commit message**: `feat(cognicode-explorer): add list_explorations handler`
- **Risk**: Low — mirrors `save_exploration` (line 753) and `generate_artifact` (line 760) pattern
- **Rollback**: Remove the handler block; route becomes dangling (cargo warns, not errors)

#### T1.6: Add `KNOWN-DEBT` rustdoc on `list_explorations` trait method
- **Files**: `crates/cognicode-explorer/src/facades/mod.rs` (already covered in T1.1 rustdoc)
- **LOC delta**: 0 (rustdoc inline in T1.1)
- **Depends on**: T1.1
- **Verification**:
  ```bash
  cargo doc -p cognicode-explorer --no-deps 2>&1 | grep -E "warning.*list_explorations"
  # EXPECTED: empty
  grep -A 10 "list_explorations" crates/cognicode-explorer/src/facades/mod.rs | head -15
  # EXPECTED: rustdoc block with KNOWN-DEBT listing 3 debts
  ```
- **Commit message**: `docs(cognicode-explorer): document known-debt on list_explorations`
- **Risk**: Low — documentation only
- **Rollback**: Remove the rustdoc lines
- **Note**: T1.6 is rolled into T1.1 commit (rustdoc lives with the method). Listed here for traceability.

#### T1.7: Add backend unit test for `list_explorations` (filter by workspace_id)
- **Files**: `crates/cognicode-explorer/src/facades/persistence.rs` (append `#[cfg(test)] mod tests` block at end of file)
- **LOC delta**: +50 (test module + 3 test cases)
- **Depends on**: T1.2
- **Implementation sketch**:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::dto::{ExplorationColumn, NavigationMode, SaveExplorationRequest};

      fn make_request(workspace_id: &str, object_id: &str) -> SaveExplorationRequest {
          SaveExplorationRequest {
              workspace_id: workspace_id.into(),
              columns: vec![ExplorationColumn {
                  object_id: object_id.into(),
                  lens: None,
              }],
              lens: None,
              navigation_mode: NavigationMode::PaneStack,
          }
      }

      #[tokio::test]
      async fn list_explorations_filters_by_workspace_id() {
          let svc = PersistenceServiceImpl::new(None);
          svc.save_exploration(make_request("ws-A", "obj-1")).await.unwrap();
          svc.save_exploration(make_request("ws-B", "obj-2")).await.unwrap();

          let a = svc.list_explorations("ws-A").await.unwrap();
          let b = svc.list_explorations("ws-B").await.unwrap();
          assert_eq!(a.len(), 1);
          assert_eq!(a[0].workspace_id, "ws-A");
          assert_eq!(b.len(), 1);
          assert_eq!(b[0].workspace_id, "ws-B");
      }

      #[tokio::test]
      async fn list_explorations_empty_workspace_returns_empty_vec() {
          let svc = PersistenceServiceImpl::new(None);
          let result = svc.list_explorations("ws-empty").await.unwrap();
          assert!(result.is_empty());
      }
  }
  ```
  **NOTE**: Exact field names depend on `dto.rs`; verifier should read `SaveExplorationRequest` and `ExplorationColumn` definitions and adjust. The 2 test cases (filter + empty) are the contract.
- **Verification**:
  ```bash
  cargo test -p cognicode-explorer --lib facades::persistence 2>&1 | tail -20
  # EXPECTED: 2 tests pass
  ```
- **Commit message**: `test(cognicode-explorer): add list_explorations unit tests`
- **Risk**: Low — test-only, no production code change
- **Rollback**: Remove the `#[cfg(test)] mod tests` block

#### T1.8: Add backend integration test for the route (200 + [] / 200 + list)
- **Files**: `crates/cognicode-explorer/src/api_graph_tests.rs` (append new `#[tokio::test]` at end of file)
- **LOC delta**: +40 (2 test cases using existing `router()` helper)
- **Depends on**: T1.4, T1.5
- **Implementation sketch**:
  ```rust
  #[tokio::test]
  async fn list_explorations_route_returns_empty_array_for_empty_workspace() {
      use axum::http::StatusCode;
      let app = router(ApiState::for_tests()); // use the existing test AppState constructor
      let response = app
          .oneshot(
              axum::http::Request::builder()
                  .uri("/api/workspaces/ws-empty/explorations")
                  .body(axum::body::Body::empty())
                  .unwrap(),
          )
          .await
          .unwrap();
      assert_eq!(response.status(), StatusCode::OK);
      let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
      assert_eq!(&body[..], b"[]");
  }
  ```
  **NOTE**: Adapt to existing test infrastructure in `api_graph_tests.rs` (look at how other route tests construct state). The contract: `200` + `[]` for empty workspace, `200` + array after `save_exploration`.
- **Verification**:
  ```bash
  cargo test -p cognicode-explorer --test '*' list_explorations_route 2>&1 | tail -15
  # EXPECTED: at least 2 route tests pass
  ```
- **Commit message**: `test(cognicode-explorer): add list_explorations route integration tests`
- **Risk**: Medium — route tests often surface state wiring issues; budget time to debug
- **Rollback**: Remove the new `#[tokio::test]` functions

---

### Commit 2: Frontend — 6 unit tests for `RecentExplorationsStrip`

#### T2.1: Test "renders cards when data is loaded"
- **Files**: `apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.test.tsx` (new file)
- **LOC delta**: +20
- **Depends on**: none (test stub lives next to component)
- **Implementation sketch**:
  ```ts
  it("renders one card per exploration when data is loaded", async () => {
    vi.mocked(useExplorations).mockReturnValue({
      data: [
        { id: "exp-1", workspace_id: "ws-1", columns: [{ object_id: "obj-A", lens: null }], objects: [], lens: null, created_at: new Date().toISOString(), navigation_mode: "pane-stack" },
      ],
      isLoading: false,
    } as any);
    render(<RecentExplorationsStrip workspaceId="ws-1" onExplorationClick={vi.fn()} />);
    expect(await screen.findByTestId("recent-exploration-exp-1")).toBeInTheDocument();
  });
  ```
- **Verification**:
  ```bash
  pnpm --filter explorer-ui test RecentExplorationsStrip 2>&1 | tail -10
  # EXPECTED: 6 tests pass
  ```
- **Commit message**: `test(explorer-ui): add RecentExplorationsStrip tests (render + null cases)`
- **Risk**: Low — pure component test
- **Rollback**: Delete the new test file

#### T2.2: Test "renders null when data is empty"
- **Files**: same test file
- **LOC delta**: +8
- **Depends on**: T2.1
- **Implementation**: mock `useExplorations` to return `{ data: [], isLoading: false }`, assert `queryByTestId("recent-explorations-strip")` is null
- **Verification**: included in T2.1's command
- **Commit message**: rolled into T2.1 commit (single test file)
- **Risk**: Low
- **Rollback**: Delete the test function

#### T2.3: Test "sorts by created_at descending"
- **Files**: same test file
- **LOC delta**: +15
- **Depends on**: T2.1
- **Implementation**: mock 3 explorations with ascending `created_at`, assert the rendered order is descending (query by index `0`/`1`/`2` or by stable test-id)
- **Verification**: included in T2.1's command
- **Commit message**: rolled into T2.1 commit
- **Risk**: Low
- **Rollback**: Delete the test function

#### T2.4: Test "caps at 5 items"
- **Files**: same test file
- **LOC delta**: +10
- **Depends on**: T2.1
- **Implementation**: mock 7 explorations, assert exactly 5 cards rendered
- **Verification**: included in T2.1's command
- **Commit message**: rolled into T2.1 commit
- **Risk**: Low
- **Rollback**: Delete the test function

#### T2.5: Test "loading state renders null (graceful degradation)"
- **Files**: same test file
- **LOC delta**: +8
- **Depends on**: T2.1
- **Implementation**: mock `{ data: undefined, isLoading: true }`, assert strip is null
- **Verification**: included in T2.1's command
- **Commit message**: rolled into T2.1 commit
- **Risk**: Low
- **Rollback**: Delete the test function

#### T2.6: Test "click dispatches onExplorationClick with exploration"
- **Files**: same test file
- **LOC delta**: +10
- **Depends on**: T2.1
- **Implementation**: render card, click it, assert `onExplorationClick` called with the exploration object
- **Verification**: included in T2.1's command
- **Commit message**: rolled into T2.1 commit
- **Risk**: Low
- **Rollback**: Delete the test function

**Note on T2.x**: All 6 tests share ONE file (`RecentExplorationsStrip.test.tsx`) and ONE commit. They are independent cases; failure of one does not block the others. `vi.mock("../../hooks/useExplorations")` at the top of the file mocks the SWR hook.

---

### Commit 3: Frontend — index.ts export

#### T3.1: Export `RecentExplorationsStrip` from GraphLanding barrel
- **Files**: `apps/explorer-ui/src/components/GraphLanding/index.ts`
- **LOC delta**: +1
- **Depends on**: none
- **Implementation**:
  ```ts
  export { RecentExplorationsStrip } from "./RecentExplorationsStrip";
  ```
  Insert as line 2 (after `GraphLanding`).
- **Verification**:
  ```bash
  grep -n "RecentExplorationsStrip" apps/explorer-ui/src/components/GraphLanding/index.ts
  # EXPECTED: 1 line containing the export
  pnpm --filter explorer-ui typecheck 2>&1 | tail -5
  # EXPECTED: 0 errors
  ```
- **Commit message**: `feat(explorer-ui): export RecentExplorationsStrip from GraphLanding barrel`
- **Risk**: Low — barrel export only, no runtime impact
- **Rollback**: Remove the added line

---

### Commit 4: Doc sync

#### T4.1: Update `docs/explorer-roadmap.md` — E4.5 status
- **Files**: `docs/explorer-roadmap.md` (lines 100, 108, 113)
- **LOC delta**: +5 / -5 (net 0, content swap)
- **Depends on**: T1, T2, T3 complete
- **Edits**:
  - **Line 100** — change status banner:
    - From: `**Status:** ~70% — E4.1/E4.3/E4.4 done; E4.2 renamed; E4.5 UI missing`
    - To: `**Status:** 100% — E4.1/E4.3/E4.4/E4.5 done; E4.2 renamed`
  - **Line 108** — change E4.5 row:
    - From: `⚠️ Hook done, UI missing | ...`
    - To: `✅ Done | Component built (e55c781), backend `GET /api/workspaces/:id/explorations` wired (sddk/E4.5), 6 unit tests pass`
  - **Lines 113-114** — change Next step callout:
    - From: `**Next step:** Add `RecentExplorationsStrip` component to `GraphLanding` consuming `useExplorations()`.`
    - To: `**Next step:** Sprint E4 closed. Next sprint: E5 — wire perspective toggle into `InteractiveGraphPanel` (per ADR-039 gap).`
- **Verification**:
  ```bash
  grep -n "E4.5" docs/explorer-roadmap.md
  # EXPECTED: line 100 no longer says "UI missing"; line 108 shows "Done"
  ```
- **Commit message**: `docs(explorer-roadmap): mark E4.5 complete; close Sprint E4`
- **Risk**: Low — documentation only
- **Rollback**: `git checkout -- docs/explorer-roadmap.md`

#### T4.2: Update `docs/adr/ADR-039-explorer-navigation-model.md` — Sprint E4 status
- **Files**: `docs/adr/ADR-039-explorer-navigation-model.md` (line 131)
- **LOC delta**: +3 / -3 (net 0)
- **Depends on**: T4.1 (keep doc syncs together)
- **Edits**:
  - **Line 131** — change E4 row:
    - From: `| E4 — Graph Landing Page | ~70% | E4.1✅ E4.2⚠️(hook renamed) E4.3✅ E4.4✅ E4.5⚠️(hook exists, UI strip missing) |`
    - To: `| E4 — Graph Landing Page | ✅ Complete | E4.1✅ E4.2⚠️(hook renamed) E4.3✅ E4.4✅ E4.5✅(strip + backend LIST endpoint, sddk/E4.5) |`
  - **Optional**: add line 131a to flag the E4.5 dual-model debt deferred to ADR-045.
- **Verification**:
  ```bash
  grep -n "E4 — Graph Landing" docs/adr/ADR-039-explorer-navigation-model.md
  # EXPECTED: status shows "✅ Complete"
  ```
- **Commit message**: `docs(adr-039): mark Sprint E4 complete in implementation status`
- **Risk**: Low — documentation only
- **Rollback**: `git checkout -- docs/adr/ADR-039-explorer-navigation-model.md`

---

## Verification (full suite, run after each commit)

```bash
# Backend (after Commit 1)
cargo check -p cognicode-explorer 2>&1 | tail -3
cargo test -p cognicode-explorer --lib 2>&1 | tail -5
cargo build -p cognicode-explorer 2>&1 | tail -3

# Frontend (after Commits 2 & 3)
pnpm --filter explorer-ui test RecentExplorationsStrip 2>&1 | tail -10
pnpm --filter explorer-ui typecheck 2>&1 | tail -3
pnpm --filter explorer-ui build 2>&1 | tail -3

# Full regression (after all commits)
cargo build --workspace 2>&1 | tail -3
pnpm --filter explorer-ui test 2>&1 | tail -10
```

**Acceptance gates:**
- `cargo build -p cognicode-explorer` succeeds (no trait-mismatch errors)
- All 6 frontend strip tests pass
- No new TypeScript errors (must remain at 0 after fix-pre-existing-ts-build-failure)
- `pnpm --filter explorer-ui build` succeeds
- 5 pre-existing test failures and 38 pre-existing lint errors MUST remain unchanged (no regression)

---

## Rollback Notes

**Per-slice rollback** (each commit is independently revertible):
- **Commit 1 (backend)**: `git revert <sha>` — removes trait method, impl, route, handler, test doubles. The strip still degrades to `null` (current prod behavior). Compiles because trait surface is restored.
- **Commit 2 (frontend tests)**: delete `RecentExplorationsStrip.test.tsx`. No production code change.
- **Commit 3 (frontend export)**: remove the one-line export. No runtime impact unless another file imports it (currently nothing does).
- **Commit 4 (docs)**: `git checkout -- docs/explorer-roadmap.md docs/adr/ADR-039-explorer-navigation-model.md`.

**Full rollback**: `git revert <merge-sha>` — single PR strategy makes this a one-command operation.

**Pre-merge**: if Commit 1 fails to compile, do NOT split into separate commits for trait/doubles. Keep them together or the build dies.

---

## Execution Order (Hinge)

```
Commit 1 (HINGE: trait + impl + route + handler + 2 test doubles + KNOWN-DEBT rustdoc + 2 tests)
   ↓
Commit 2 (frontend: 6 strip tests)
   ↓
Commit 3 (frontend: index export)
   ↓
Commit 4 (docs: roadmap + ADR-039 sync)
```

Commit 1 is the load-bearing commit. Commits 2-4 can land in any order after Commit 1, but Commit 4 must be last to reflect the actual state of the merged change.

---

## Summary

- **Status**: success
- **Engram ID**: #2689 (to be saved with topic_key `sddk/E4.5-graph-landing-ui/tasks`, `capture_prompt: false`)
- **tasks.md**: this file
- **Total task count**: 14 atomic tasks (T1.1–T1.8 = 8 backend; T2.1–T2.6 = 6 frontend tests; T3.1 = 1 export; T4.1–T4.2 = 2 doc edits)
- **Estimated LOC**: ~165 total (Rust ~95, TS ~60, MD ~10)
- **Estimated commits**: 4 atomic commits in a single PR
- **Unresolved blockers**: none
