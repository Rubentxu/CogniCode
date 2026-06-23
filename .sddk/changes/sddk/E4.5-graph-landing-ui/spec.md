# Kernel Specs: E4.5 — Backend Explorations LIST + Strip Tests + Doc Sync

## Router Context Used
- Knowledge Coverage: sufficient (roadmap stale; codebase reads confirm `paths` store, trait location, two routers, two test doubles, MSW handler)
- Context Quality: C2 — contradictions: roadmap line 108 says "UI missing" but strip exists since e55c781; `get_exploration` handler calls `load_exploration_session` (not path); dual-model (Path vs Session)
- Taxonomy: missing-backend-endpoint + frontend-untested + stale-docs
- Domain Language: `ExplorationPath` (column-based legacy), `ExplorationSession` (pane-stack, ADR-040), `ExplorationPathStore` (`Mutex<HashMap<String, ExplorationPath>>` at `crates/cognicode-explorer/src/facades/persistence.rs:27`), `PersistenceService` (trait at `crates/cognicode-explorer/src/facades/mod.rs:175`)
- Recommended Effort: verify (done — wiring confirmed in this session)

## Knowledge Provenance
- Scope source: kernel proposal `engram #2686` (`sddk/e4.5-graph-landing-ui/proposal`)
- Invariant source: `crates/cognicode-explorer/src/facades/persistence.rs:27-45`, `:65-106` (in-memory `paths` HashMap keyed by `id`, populated by `save_exploration`); `apps/explorer-ui/src/hooks/useExplorations.ts:98-106` (SWR key `/workspaces/:id/explorations`, schema `explorationPathSchema`); `apps/explorer-ui/src/mocks/handlers.ts:205-214` (MSW contract)
- Memory-only hints excluded from spec truth: none — all invariants sourced from current code reads

## Capability: list-explorations-by-workspace

### Requirement: REQ-E4.5-1 — Backend trait method
The system SHALL expose `PersistenceService::list_explorations(&self, workspace_id: &str) -> ExplorerResult<Vec<ExplorationPath>>` on the trait defined at `crates/cognicode-explorer/src/facades/mod.rs:175-227`, with a concrete implementation on `PersistenceServiceImpl` that reads the in-memory `ExplorationPathStore` (`Mutex<HashMap<String, ExplorationPath>>`, field `paths: Arc<ExplorationPathStore>` at `crates/cognicode-explorer/src/facades/persistence.rs:27,43`) and returns a `Vec` filtered to rows where `path.workspace_id == workspace_id`.

#### Scenario: returns all paths matching the workspace id
Given the in-memory store holds three rows:
- `path_a` with `workspace_id == "ws1"` and `created_at == "2026-06-20T10:00:00Z"`
- `path_b` with `workspace_id == "ws1"` and `created_at == "2026-06-22T11:00:00Z"`
- `path_c` with `workspace_id == "ws2"` and `created_at == "2026-06-21T11:00:00Z"`
When `PersistenceServiceImpl::list_explorations("ws1").await` is called
Then the result is `Ok(vec![path_a, path_b])` and `path_c` is excluded

#### Scenario: returns empty Vec when no paths match
Given the store holds zero rows for workspace `"empty-ws"`
When `list_explorations("empty-ws").await` is called
Then the result is `Ok(vec![])` (NOT `Err`, NOT `None`)

#### Scenario: returns Vec::new when the store itself is empty
Given `PersistenceServiceImpl::new()` was just constructed and `save_exploration` was never called
When `list_explorations("any-id").await` is called
Then the result is `Ok(vec![])` and no allocation of an `Option` or `Result::Err` occurs

#### Scenario: both test doubles implement the new method (compile gate)
Given `crates/cognicode-explorer/src/api_graph_tests.rs:137-199` and `crates/cognicode-explorer/src/api_rationale_tests.rs:200-236` each define a `MockPersistenceService` that implements `PersistenceService`
When `cargo check --workspace` is run
Then compilation succeeds AND both mocks contain a `list_explorations(&self, _workspace_id: &str) -> ExplorerResult<Vec<ExplorationPath>>` stub returning `Err(ExplorerError::FeatureDisabled("mock".into()))` to mirror the existing `list_view_specs` mock (see `api_graph_tests.rs:170-176`)

## Capability: explorations-list-endpoint

### Requirement: REQ-E4.5-2 — Backend route + handler
The system SHALL expose `GET /api/workspaces/:workspace_id/explorations` in BOTH `router_with_state` (currently at `crates/cognicode-explorer/src/api.rs:431-472`) and `router` (currently at `crates/cognicode-explorer/src/api.rs:474-523`), wired to a new handler `list_explorations` that calls `state.persistence.list_explorations(&workspace_id).await` and returns the `Vec<ExplorationPath>` as JSON.

#### Scenario: route is mounted in both routers (compile + runtime gate)
Given `crates/cognicode-explorer/src/api.rs` defines `router_with_state` (line 431) and `router` (line 474)
When the source is read
Then each router contains `.route("/api/workspaces/:workspace_id/explorations", get(list_explorations))` on a distinct line, AND the `useExplorations` SWR key `/workspaces/:id/explorations` + `/api` base resolves to this path

#### Scenario: handler returns 200 + JSON array of ExplorationPath
Given the in-memory store contains two `ExplorationPath` rows for workspace `"test"` with `created_at` `"2026-06-20T10:00:00Z"` and `"2026-06-22T11:00:00Z"`
When `GET /api/workspaces/test/explorations` is invoked against `router(ApiState)`
Then the response status is `200 OK`, `Content-Type: application/json`, body is a JSON array `[{...}, {...}]` where each element matches `explorationPathSchema` (`apps/explorer-ui/src/api/schemas.ts:817-825`) — fields: `id`, `workspace_id`, `columns[]`, `objects[]`, `lens`, `created_at`

#### Scenario: handler returns 200 + `[]` for unknown workspace (NEVER 404)
Given the in-memory store holds zero paths for workspace `"empty"`
When `GET /api/workspaces/empty/explorations` is invoked
Then the response status is `200 OK` and the body is `[]` (a literal empty JSON array, NOT `null`, NOT a 404). This matches the invariant from the proposal: "Empty result → 200 + [], never 404."

#### Scenario: handler returns structured error on PersistenceService failure
Given `PersistenceService::list_explorations` returns `Err(ExplorerError::Anyhow(anyhow::anyhow!("path store poisoned")))`
When `GET /api/workspaces/x/explorations` is invoked
Then the response status is `500 Internal Server Error` and the body is `{"error": "internal: path store poisoned"}` (matching the structured `ApiError` response shape at `api.rs:827-831`)

## Capability: in-memory-store-impl

### Requirement: REQ-E4.5-3 — In-memory store impl
The system SHALL filter the in-memory `ExplorationPathStore` by `workspace_id` and return a `Vec<ExplorationPath>`. Ordering MAY be unspecified at the implementation level because `RecentExplorationsStrip` (`apps/explorer-ui/src/components/GraphLanding/RecentExplorationsStrip.tsx:95-97`) sorts client-side by `created_at` descending before slicing to 5. The impl MUST handle mutex poisoning the same way `save_exploration` does (`persistence.rs:100-103`).

#### Scenario: iterates the HashMap and filters by workspace_id
Given the store contains paths `p1, p2, p3` with `workspace_id` values `"ws1", "ws1", "ws2"`
When the in-memory impl of `list_explorations("ws1")` runs
Then it returns a `Vec` of length 2 containing `p1` and `p2`, and `p3` is excluded. The impl uses the existing `self.paths.lock()` pattern (same as `save_exploration` at `persistence.rs:100-103`) and propagates `Err(ExplorerError::Anyhow(anyhow::anyhow!("exploration path store poisoned")))` on poisoning.

#### Scenario: empty result on empty store
Given `paths: Arc<ExplorationPathStore>` is an empty `HashMap`
When the impl iterates
Then the returned `Vec` is empty (no panic, no error) — client receives `[]` and the strip renders `null` (`RecentExplorationsStrip.tsx:90-92`)

#### Scenario: client-side sort handles ordering regardless of impl order
Given the impl returns paths in arbitrary order (HashMap iteration order) and `RecentExplorationsStrip.tsx:95-97` runs `[...explorations].sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()).slice(0, 5)`
When the strip renders
Then the displayed order is `created_at` DESC regardless of impl order, so the impl is free to skip sorting

## Capability: recent-explorations-strip

### Requirement: REQ-E4.5-4 — RecentExplorationsStrip unit tests
The system SHALL add `apps/explorer-ui/src/components/GraphLanding/__tests__/RecentExplorationsStrip.test.tsx` (or equivalent) with **6 test cases** covering render, loading-null, empty-null, sort-desc, cap@5, and click→onExplorationClick.

#### Scenario (a): renders cards when data is loaded
Given `useExplorations(workspaceId)` resolves to `{ data: [pathA, pathB], isLoading: false }` (mocked via `vi.mock("../../../hooks/useExplorations")`)
When `<RecentExplorationsStrip workspaceId="ws1" onExplorationClick={vi.fn()} />` is rendered
Then the DOM contains `[data-testid="recent-explorations-strip"]` AND `[data-testid="recent-exploration-${pathA.id}"]` AND `[data-testid="recent-exploration-${pathB.id}"]`. Asserted via `screen.getByTestId(...)`.

#### Scenario (b): renders null when data is empty
Given `useExplorations(workspaceId)` resolves to `{ data: [], isLoading: false }`
When the strip is rendered
Then the DOM does NOT contain `[data-testid="recent-explorations-strip"]` (the component returns `null` per `RecentExplorationsStrip.tsx:90-92`). Asserted via `screen.queryByTestId("recent-explorations-strip")` returning `null`.

#### Scenario (c): sorts by created_at descending
Given `useExplorations(workspaceId)` resolves to 3 paths with `created_at` values `"2026-06-20T10:00:00Z"`, `"2026-06-22T11:00:00Z"`, `"2026-06-21T11:00:00Z"` (oldest, newest, middle)
When the strip is rendered
Then the rendered card order in the DOM is `[newest, middle, oldest]` — verified by reading the test-ids in DOM order: `recent-exploration-${newest.id}` first, then `recent-exploration-${middle.id}`, then `recent-exploration-${oldest.id}`.

#### Scenario (d): caps at 5 items
Given `useExplorations(workspaceId)` resolves to 7 paths with distinct `created_at` values (one per day over 7 days)
When the strip is rendered
Then the DOM contains exactly 5 `[data-testid^="recent-exploration-"]` elements AND the two oldest paths are not rendered (asserted via `screen.getAllByTestId(/^recent-exploration-/)` returning length 5).

#### Scenario (e): loading state renders null (graceful degradation)
Given `useExplorations(workspaceId)` resolves to `{ data: undefined, isLoading: true }`
When the strip is rendered
Then the DOM does NOT contain `[data-testid="recent-explorations-strip"]` AND does NOT throw — verified by `screen.queryByTestId("recent-explorations-strip") === null` and no `act()` warnings. This is the pre-fix production behavior; the test pins it as the contract.

#### Scenario (f): click dispatches `SELECT_OBJECT` via onExplorationClick
Given a single rendered path `pathA` with `columns[0].object_id == "symbol:user_service"` and a `vi.fn()` spy passed as `onExplorationClick`
When the user clicks `[data-testid="recent-exploration-${pathA.id}"]`
Then the spy is invoked exactly once AND the first argument is the `pathA` object reference (NOT `pathA.id`, NOT `pathA.columns[0].object_id`). Note: `RecentExplorationsStrip.tsx:122-126` passes the whole `ExplorationPath` to `onExplorationClick`; the parent `GraphLanding.tsx:198-209` is responsible for translating that into a `SELECT_OBJECT { objectId: exploration.columns[0].object_id, viewId: "overview" }` dispatch. The test asserts the prop contract only — not the downstream dispatch (that belongs to GraphLanding.test.tsx).

## Capability: doc-sync

### Requirement: REQ-E4.5-5 — Doc sync
The system SHALL update `docs/explorer-roadmap.md:108` (E4.5 status row) and `docs/adr/ADR-039-explorer-navigation-model.md:131` (Sprint E4 row) to reflect that E4.5 is complete, with a reference to the commits that close the gap.

#### Scenario: explorer-roadmap.md E4.5 row updated
Given `docs/explorer-roadmap.md:108` currently reads `| E4.5 | Add recent explorations strip (from useExplorations) | ⚠️ Hook done, UI missing | ...`
When the doc is updated
Then the row reads `| E4.5 | Add recent explorations strip (from useExplorations) | ✅ Done (v0.11.4) | `RecentExplorationsStrip.tsx` exists (e55c781); `GET /api/workspaces/:id/explorations` endpoint closed in <merge commit>; 6 unit tests in `RecentExplorationsStrip.test.tsx` |` — replacing the stale "UI missing" evidence.

#### Scenario: explorer-roadmap.md Sprint E4 status banner updated
Given `docs/explorer-roadmap.md:100` currently reads `**Status:** ~70% — E4.1/E4.3/E4.4 done; E4.2 renamed; E4.5 UI missing`
When the doc is updated
Then the line reads `**Status:** ✅ Complete — E4.1/E4.3/E4.4/E4.5 done; E4.2 renamed`

#### Scenario: explorer-roadmap.md "Next step" callout removed
Given `docs/explorer-roadmap.md:113-114` currently reads `**Next step:** Add RecentExplorationsStrip component to GraphLanding consuming useExplorations().`
When the doc is updated
Then the callout is removed (the work it pointed to is now done)

#### Scenario: ADR-039 Sprint E4 row updated
Given `docs/adr/ADR-039-explorer-navigation-model.md:131` currently reads `| E4 — Graph Landing Page | ~70% | E4.1✅ E4.2⚠️(hook renamed) E4.3✅ E4.4✅ E4.5⚠️(hook exists, UI strip missing) |`
When the doc is updated
Then the row reads `| E4 — Graph Landing Page | ✅ Complete | E4.1✅ E4.2✅(renamed to useLanding/useArchitecture) E4.3✅ E4.4✅ E4.5✅(strip + endpoint + tests landed) |` — replacing the stale "UI strip missing" note.

## Capability: verification-gate

### Requirement: REQ-E4.5-6 — Verification
The change is mergeable only when ALL of the following commands exit 0 from the workspace root `/var/home/rubentxu/Proyectos/rust/CogniCode`.

#### Scenario: cargo check --workspace
When `cargo check --workspace` runs
Then exit code is `0` AND stderr contains no `error[E0599]` or `error[E0277]` referencing `PersistenceService::list_explorations` missing from any implementor (compile-gates REQ-E4.5-1 against both test doubles)

#### Scenario: cargo test -p cognicode-explorer
When `cargo test -p cognicode-explorer` runs
Then exit code is `0` AND the test summary reports zero `FAILED` cases AND no test reports `panicked at ... FeatureDisabled("mock")` (the test doubles must remain wired)

#### Scenario: frontend type-check
When `cd apps/explorer-ui && npx tsc --noEmit` runs
Then exit code is `0` (gates the new `.test.tsx` against the TS types of `useExplorations` and `ExplorationPath`)

#### Scenario: frontend unit tests for the strip
When `cd apps/explorer-ui && npm run test -- --run -- RecentExplorationsStrip` runs
Then exit code is `0` AND the summary shows 6 tests passed for `RecentExplorationsStrip.test.tsx` matching scenarios (a)-(f) of REQ-E4.5-4

#### Scenario: end-to-end curl smoke
Given the explorer binary is running on `localhost:8080` (or the configured `COGNICODE_EXPLORER_ADDR`)
When `curl -sS -o /tmp/expls.json -w "%{http_code}" http://localhost:8080/api/workspaces/test/explorations` runs
Then the printed status code is `200` AND `/tmp/expls.json` parses as a JSON array (empty `[]` is acceptable for a fresh store) AND every element validates against `explorationPathSchema` (no extra/missing fields). This is the manual smoke confirming the route works outside the test harness.

## Capability: known-debt-documentation

### Requirement: REQ-E4.5-7 — Known-debt documentation in code comment
The system SHALL document the dual-model situation in a doc-comment at the top of the in-memory impl of `PersistenceService::list_explorations` (inside `PersistenceServiceImpl` in `crates/cognicode-explorer/src/facades/persistence.rs`), so future readers understand why the strip consumes `ExplorationPath[]` (not `ExplorationSession[]`).

#### Scenario: code comment names the three known debts
Given the impl function body of `list_explorations` in `crates/cognicode-explorer/src/facades/persistence.rs`
When the source is read
Then the rustdoc `///` block at the top of the function mentions all three debts:
1. `get_exploration` (handler at `api.rs:768-780`) is mis-wired — it calls `load_exploration_session` instead of a path loader; flagged for a future ADR (NOT fixed in this change).
2. The strip (`RecentExplorationsStrip.tsx`) uses `ExplorationPath` for the LIST response, not `ExplorationSession`; full model unification is future work.
3. The `paths` HashMap is process-lifetime only (per the comment at `persistence.rs:26`); a server restart loses all rows.

#### Scenario: comment uses `// KNOWN-DEBT:` markers for grep-ability
Given the doc-comment
When read
Then it contains the literal token `KNOWN-DEBT:` (UPPERCASE, colon-terminated) at least 3 times so that `grep -rn "KNOWN-DEBT:" crates/` surfaces every debt marker in CI

## Invariants Covered
- **LIST returns ExplorationPath, not ExplorationSession** — covered by REQ-E4.5-1 Scenario "returns all paths matching the workspace id" (asserts return type) + REQ-E4.5-7 Scenario "code comment names the three known debts" (documents the choice)
- **Route path is `/api/workspaces/:workspace_id/explorations`** — covered by REQ-E4.5-2 Scenario "route is mounted in both routers" + REQ-E4.5-6 Scenario "end-to-end curl smoke" (asserts `200`)
- **Empty result → 200 + [], never 404** — covered by REQ-E4.5-2 Scenario "handler returns 200 + [] for unknown workspace"
- **Both test doubles implement the new method** — covered by REQ-E4.5-1 Scenario "both test doubles implement the new method (compile gate)" + REQ-E4.5-6 Scenario "cargo check --workspace"
- **`get_exploration` mis-wire is flagged, not fixed** — covered by REQ-E4.5-7 (doc comment) — explicitly listed in "Out of scope" below
- **Sort by `created_at` descending is the contract** — covered by REQ-E4.5-4 Scenario (c) "sorts by created_at descending" (asserts DOM order)
- **Cap at 5 items is the contract** — covered by REQ-E4.5-4 Scenario (d) "caps at 5 items"
- **Loading state renders null** — covered by REQ-E4.5-4 Scenario (e) — pre-fix production behavior pinned as contract

## Open Questions
- **Does `save_exploration` still populate `columns`?** The strip at `RecentExplorationsStrip.tsx:74` reads `exploration.columns.length` to render a "N panes" label, and line 23-24 falls back to `exploration.id` as the card title when `columns[0]` is missing. If `columns` is vestigial (populated only on save and never updated post-pane-stack cut), the count label reads "0 panes" and the title is the exploration id. The DESIGN phase should confirm: (a) `save_exploration` still emits non-empty `columns`, OR (b) the strip should be reworked to read from `objects` or `panes`. REQ-E4.5-3 assumes (a) without asserting it. This is a soft invariant — flagged for design.
- **GraphLanding `onExplorationClick` translation**: REQ-E4.5-4 Scenario (f) tests the prop contract only (`onExplorationClick(exploration)` is invoked). It does NOT test the downstream `SELECT_OBJECT { objectId: exploration.columns[0].object_id, viewId: "overview" }` dispatch that lives in `GraphLanding.tsx:198-209`. If that parent wiring is wrong, the strip click does nothing visible. The change is still complete per REQ-E4.5-4 — verifying the parent belongs to a separate test surface.

## Out of Scope (explicit non-goals from proposal)
- Unifying `ExplorationPath` ↔ `ExplorationSession` models — separate change
- Fixing `get_exploration` mis-wire (handler at `api.rs:768-780` calls `load_exploration_session` not path loader) — flagged for future ADR
- Implementing full multi-pane restore on click (strip click currently only opens the first column's object) — separate change
- Backend pagination (the cap@5 is client-side only at `RecentExplorationsStrip.tsx:97`) — separate change
- Creating the dual-model ADR — flagged for post-merge work
- Wiring `useSnapshotCache` (`useExplorations.ts:49-86`) into the strip or pane-stack — out of scope
- Postgres-backed persistence for explorations (currently in-memory only per `persistence.rs:26`) — separate change

## Verification Commands
Run from workspace root `/var/home/rubentxu/Proyectos/rust/CogniCode`:

```bash
# 1. Backend compile gate (catches missing trait method in test doubles)
cargo check --workspace

# 2. Backend unit + integration tests
cargo test -p cognicode-explorer

# 3. Frontend type-check
cd apps/explorer-ui && npx tsc --noEmit

# 4. Frontend unit tests for the strip (6 new tests)
cd apps/explorer-ui && npm run test -- --run -- RecentExplorationsStrip

# 5. Manual smoke (requires running binary)
cargo run -p cognicode-explorer &
sleep 2
curl -sS -w "\n%{http_code}\n" http://localhost:8080/api/workspaces/test/explorations
# Expected: "[]" followed by "200"

# 6. Doc grep gate (KNOWN-DEBT markers must exist)
grep -rn "KNOWN-DEBT:" crates/ | wc -l   # Expected: >= 3
```