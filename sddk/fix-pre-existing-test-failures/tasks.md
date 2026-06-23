# Kernel Tasks: fix-pre-existing-test-failures (F1–F4)

## Router Context Used
- **Knowledge Coverage**: sufficient — ADR-039 (C4 contract), ADR-045 (debt ledger), explore #2745, proposal #2747, spec #2748, design #2749 all present
- **Context Quality**: C2 — both suites executed, Rust bug instrumented, root causes pinned
- **Taxonomy**: code-bug (F4), stale-count (F3), mock-divergence / Connascence-of-Algorithm (F1/F2)
- **Invariants Driving Tasks**: ADR-039 (`kind == level` ∧ `style_class == node-{level}`); shared mock superset API; per-test hook preservation (`__cyInstances`, `clickNode`, `destroyed`)
- **Recommended Effort**: deepen — design depth, not breadth; C2 proven

## Review Budget Forecast
- **Estimated changed lines**: ~180 (PR1 ~24, PR2 ~155)
- **400-line budget risk**: **Low** (each PR well under 400; PR1 ~24, PR2 ~155)
- **Chained PRs recommended**: **Yes** — design explicitly recommends 2 PRs (correctness + test-infra)
- **Decision needed before apply**: **No** — design is decisive, no open questions

## Knowledge Traceability
- **Work item source artifacts**: proposal #2747, spec #2748 (7 REQs, 14 scenarios), design #2749
- **Ownership source**: design #2749 (architecture decisions table)
- **Open knowledge gaps affecting execution**: None. All file paths and line numbers verified against `main @ 7439a4f`.

---

## PR 1 — Correctness (F3 + F4)

> Goal: Align Rust C4 code node with ADR-039 + fix stale stylesheet count. F1/F2 remain red after this PR (pre-existing, not regression).

### T1.1: Fix C4 code node `kind` in graph.rs
- **Files**: `crates/cognicode-explorer/src/facades/graph.rs` (line 398)
- **LOC delta**: +1, -1 (alignment preserved)
- **Depends on**: —
- **Verification**:
  ```bash
  # Confirm change is in place
  sed -n '395,402p' crates/cognicode-explorer/src/facades/graph.rs
  # Expected: kind: "code".to_string(), at line 398
  cargo build -p cognicode-explorer
  # Expected: exit 0
  ```
- **Commit message**: `fix(explorer): align C4 code node kind with ADR-039`
- **Risk**: **Low** — 1-line change, scoped to C4 code-level emission; sibling levels (system/container/component) already follow the convention
- **Rollback**: `git revert <commit>` — single-line revert, F4 reintroduced but no cascade

### T1.2: Add Rust regression test for C4 code invariant
- **Files**: `crates/cognicode-explorer/src/facades/graph.rs` (tests module, near existing `build_architecture_caps_code_nodes_at_200`)
- **LOC delta**: ~+20 (new test function)
- **Depends on**: T1.1
- **Verification**:
  ```bash
  cargo test -p cognicode-explorer --lib facades::graph::tests
  # Expected: ALL tests pass, including new test
  # New test name: build_architecture_emits_code_kind_for_c4_code_nodes
  # Asserts: every node with id starting "code:" has kind=="code" AND style_class=="node-code"
  ```
- **Commit message**: `test(explorer): guard C4 code node kind invariant (ADR-039)`
- **Risk**: **Low** — independent guard; uses existing test fixture pattern; mirrors `build_architecture_caps_code_nodes_at_200` style
- **Rollback**: `git revert <commit>` — invariant unguarded but no behavior change

### T1.3: Fix stylesheet test count + comment
- **Files**: `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.test.ts` (lines 205-206)
- **LOC delta**: ~+2, -2
- **Depends on**: T1.1 (logically — count rises because `node-code` is now referenced as a class, but the stylesheet itself is unchanged; this can be merged independently)
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- stylesheet.test
  # Expected: 2 tests pass (C4 includes test + total count test)
  ```
- **Commit message**: `test(stylesheet): bump KNOWN_NODE_CLASSES count 13→14 (+ node-code)`
- **Risk**: **Low** — assertion update + comment clarification; no logic change
- **Rollback**: `git revert <commit>` — count reverts, test fails again, no cascade

### T1.4: Pre-merge downstream parity check
- **Files**: none (grep only); document finding in PR1 description
- **LOC delta**: 0
- **Depends on**: T1.1, T1.2, T1.3
- **Verification**:
  ```bash
  grep -rn 'kind ===' apps/explorer-ui/src --include='*.ts' --include='*.tsx'
  # Expected: NO matches on GraphNode.kind for C4 code level.
  # Verified at design time: existing matches are ALL_KINDS, ds.kind, transform.kind,
  # strategy.kind, fixture-schema validation only.
  ```
- **Commit message**: _(no commit — gate step before PR1 merge)_
- **Risk**: **Low** — read-only check; design already verified clean
- **Rollback**: N/A (no code change)

**PR1 commit (squash or 3 commits + gate)**: `fix: align C4 code node kind with ADR-039 + bump stylesheet count`

---

## PR 2 — Test-Infra (F1 + F2 + systemic CoA reduction)

> Goal: Extract shared cytoscape mock, migrate 5 inline mocks → shared. F1/F2 become green as a side-effect of RationaleView migration.

### T2.1: Create shared cytoscape mock
- **Files**: `apps/explorer-ui/src/test/cytoscapeMock.ts` (NEW)
- **LOC delta**: ~+100
- **Depends on**: —
- **Verification**:
  ```bash
  cd apps/explorer-ui && npx tsc --noEmit
  # Expected: exit 0
  ls apps/explorer-ui/src/test/cytoscapeMock.ts
  # Expected: file exists
  ```
  - Exports `createCytoscapeMock(opts?: MockOptions): MockedCytoScape`
  - `MockOptions`: `{ destroyTracking?: boolean; clickNodeHelper?: boolean }`
  - `MockedCytoScape` superset: `nodes()`, `edges()`, `destroy()`, `fit()`, `on()`, `off()`, `layout()`, `add()`, `remove()`, `getElementById()`, `mount()`, `json()`, `elements()`, `style()`, `width()`, `height()`, `container()`, `zoom()`, `pan()`, `center()`, `resizable()`
  - `CyCollection`: `.length`, `.map`, `.filter`, `.forEach`, `.addClass`, `.removeClass`, `.subtract`, `.each`
  - `CyNode`: `.id()`, `.data()`, `.classes()`, `.addClass()`, `.removeClass()`, `.hasClass()`, `.on()`, `.off()`, `.emit("tap")`
  - Global side-effects: `globalThis.__cyInstances`, `resetCyMock()`, `getCyInstances()` — must match existing inline-mock contract
- **Commit message**: `test(infra): extract shared API-faithful cytoscape mock`
- **Risk**: **Medium** — superset API must cover all 5 current inline mocks; per-test hook contract must be preserved or migrations break
- **Rollback**: `git revert <commit>` — file gone, downstream T2.2–T2.6 cannot apply independently (they depend on this file existing)

### T2.2: Migrate `Shell.test.tsx` to shared mock
- **Files**: `apps/explorer-ui/src/components/Shell.test.tsx`
- **LOC delta**: ~-20, +5
- **Depends on**: T2.1
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- Shell.test
  # Expected: all tests pass
  ```
  - Remove inline `vi.mock("cytoscape", () => {...})` block
  - Import `createCytoscapeMock` from `../../test/cytoscapeMock`
  - Preserve `destroyed: boolean` tracking (was in inline mock)
- **Commit message**: `test(Shell): use shared cytoscape mock`
- **Risk**: **Low** — Shell test does not have specific F1/F2 failures; mechanical migration
- **Rollback**: `git revert <commit>` — Shell test reverts to inline mock; isolated

### T2.3: Migrate `RationaleView.test.tsx` to shared mock
- **Files**: `apps/explorer-ui/src/components/RationaleView/RationaleView.test.tsx`
- **LOC delta**: ~-25, +5
- **Depends on**: T2.1
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- RationaleView.test
  # Expected: ALL tests pass — this fixes F1 and F2 (the only 2 failing tests in scope)
  ```
  - Remove inline `vi.mock("cytoscape", ...)` block
  - Import shared mock
  - Preserve `clickNode(id)` helper
- **Commit message**: `test(RationaleView): use shared cytoscape mock (fixes F1, F2)`
- **Risk**: **Low** — fixes are isolated; failure was missing `nodes()` API in inline mock, shared mock is superset
- **Rollback**: `git revert <commit>` — F1/F2 re-emerge; isolated

### T2.4: Migrate `InteractiveGraph.test.tsx` to shared mock
- **Files**: `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx`
- **LOC delta**: ~-25, +5
- **Depends on**: T2.1
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- InteractiveGraph.test
  # Expected: all tests pass
  ```
  - Remove inline `vi.mock("cytoscape", ...)` block
  - Import shared mock
  - Preserve `destroyed: boolean` tracking
- **Commit message**: `test(InteractiveGraph): use shared cytoscape mock`
- **Risk**: **Low** — already had superset-like API; mechanical migration
- **Rollback**: `git revert <commit>` — isolated

### T2.5: Migrate `NeighborMinigraph.test.tsx` to shared mock
- **Files**: `apps/explorer-ui/src/components/ContextualPanel/NeighborMinigraph.test.tsx`
- **LOC delta**: ~-25, +5
- **Depends on**: T2.1
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- NeighborMinigraph.test
  # Expected: all tests pass
  ```
  - Remove inline `vi.mock("cytoscape", ...)` block
  - Import shared mock
  - **Reuse** `NeighborMinigraph.test-helpers.ts` (already exists) for any per-test hook utilities — do NOT duplicate into shared mock
- **Commit message**: `test(NeighborMinigraph): use shared cytoscape mock`
- **Risk**: **Low** — pre-existing `test-helpers.ts` already factors some utilities; just rewires the mock source
- **Rollback**: `git revert <commit>` — isolated

### T2.6: Migrate `ContextualPanel.test.tsx` to shared mock
- **Files**: `apps/explorer-ui/src/components/ContextualPanel/ContextualPanel.test.tsx`
- **LOC delta**: ~-25, +5
- **Depends on**: T2.1
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run -- ContextualPanel.test
  # Expected: all tests pass
  ```
  - Remove inline `vi.mock("cytoscape", ...)` block
  - Import shared mock
- **Commit message**: `test(ContextualPanel): use shared cytoscape mock`
- **Risk**: **Low** — mechanical migration
- **Rollback**: `git revert <commit>` — isolated

### T2.7: Full UI suite regression gate
- **Files**: none (verification only)
- **LOC delta**: 0
- **Depends on**: T2.1, T2.2, T2.3, T2.4, T2.5, T2.6
- **Verification**:
  ```bash
  cd apps/explorer-ui && npm run test -- --run
  # Expected: exit 0, failure count = main-baseline - 4 (F1, F2, F3, F4 now green)
  cd apps/explorer-ui && npx tsc --noEmit
  # Expected: exit 0
  ```
- **Commit message**: _(no commit — gate step before PR2 merge)_
- **Risk**: **Low** — read-only verification
- **Rollback**: N/A

**PR2 commit (1 base + 5 file migrations or 1 squash)**: `test: extract shared cytoscape mock, migrate 5 test files (eliminate CoA)`

---

## Verification (overall, end-to-end)

```bash
# PR1 verification
cargo build -p cognicode-explorer
cargo test -p cognicode-explorer --lib facades::graph::tests
cd apps/explorer-ui && npm run test -- --run -- stylesheet.test
grep -rn 'kind ===' apps/explorer-ui/src --include='*.ts' --include='*.tsx'

# PR2 verification
cd apps/explorer-ui && npx tsc --noEmit
cd apps/explorer-ui && npm run test -- --run -- Shell.test
cd apps/explorer-ui && npm run test -- --run -- RationaleView.test
cd apps/explorer-ui && npm run test -- --run -- InteractiveGraph.test
cd apps/explorer-ui && npm run test -- --run -- NeighborMinigraph.test
cd apps/explorer-ui && npm run test -- --run -- ContextualPanel.test
cd apps/explorer-ui && npm run test -- --run   # full suite
```

**Expected**: All 4 originally-failing tests (F1, F2, F3, F4) green. No new failures. No TS errors.

## Rollback Notes

- **PR1 rollback**: `git revert <PR1-merge-commit>` — F3, F4 reintroduced; F1, F2 unaffected (still red pre-PR2). Safe to revert at any time.
- **PR2 rollback**: `git revert <PR2-merge-commit>` — shared mock removed; F1, F2 reintroduced; no other behavior change. Safe to revert independently of PR1.
- **Cross-PR independence**: PR1 does not import anything from PR2; PR2 does not modify any file from PR1. Order of merge is PR1 → PR2.

## Out of Scope (tolerated, not addressed by this change)

- Bench-renderer mocks (`cytoscape-canvas.test.ts`, `cytoscape-webgl.test.ts`) — different registry, not in `vi.mock("cytoscape")` chain, excluded by design
- New ADR (ADR-039 already encodes the C4 contract; F4 was a violation, not new knowledge)
- Frontend product-code changes beyond mock migration
- Type-safety refactor of the mock
