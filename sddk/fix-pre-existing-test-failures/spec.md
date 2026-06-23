# Kernel Specs: Fix Pre-Existing Test Failures F1–F4 + Shared Cytoscape Mock

## Router Context Used
- Knowledge Coverage: sufficient (ADR-039 C4 contract, ADR-045 debts ledger, engram #2745 explore, engram #2747 proposal)
- Context Quality: C1 (line-level verified against `graph.rs:398`, `stylesheet.test.ts:206`, 5 mock sites)
- Taxonomy: code-bug · stale-count · mock-divergence (CoA)
- Domain Language: C4 levels (`system` / `container` / `component` / `code`), `style_class` = `node-{level}`, `kind` = `{level}` per ADR-039, GraphNode, CyMock, `__cyInstances`
- Recommended Effort: deepen (verified)

## Knowledge Provenance
- Scope source: engram #2747 (proposal), engram #2745 (explore), ADR-039 (C4 kind/style_class contract)
- Invariant source: ADR-039 ("C4 nodes use `kind == {level}` AND `style_class == node-{level}`")
- Memory-only hints excluded from spec truth: none (proposal aligns with durable ADR)

## Capability: shared-cytoscape-mock

### Requirement: REQ-PETFIX-4 — Single API-faithful cytoscape mock is importable
The system SHALL provide a shared cytoscape mock factory that all graph-view tests import instead of redefining inline.

#### Scenario: shared mock module exists with superset API
Given `apps/explorer-ui/src/test/cytoscapeMock.ts` is created
When any test file imports `vi.mock("cytoscape", () => createCytoscapeMock())`
Then the mock exports a `Cy` class whose instances expose `nodes()` (function), `edges()` (function), `destroy()`, `fit()`, `on()`, `off()`, `layout()`, `add()`, `remove()`, `mount()`, `json()`, `elements()`, `style()`, `width()`, `height()`, `container()`, `zoom()`, `pan()`, `center()`, `resizable()`
And `nodes()` returns a `CyCollection` with `.length`, `.map`, `.filter`, `.forEach`, `.addClass`, `.removeClass`, `.subtract`, `.each`
And each `CyNode` exposes `.id()`, `.data()`, `.classes()`, `.addClass()`, `.removeClass()`, `.hasClass()`, `.on()`, `.off()`, `.emit("tap")`

#### Scenario: shared mock preserves per-test hooks
Given a test file imports the shared mock
When the test reads `globalThis.__cyInstances`
Then it contains every `Cy` instance constructed during the test
And each instance exposes `destroyed: boolean`, `clickNode(id: string)`, `destroy()` that flips `destroyed = true`
And the file can override or extend the mock without rewriting the base (factory composition pattern)

#### Scenario: RationaleView previously-broken `cy.nodes()` call now resolves
Given `RationaleView.test.tsx` migrates to the shared mock
When the test renders `<RationaleView />` and `waitFor`s the layout effect in `InteractiveGraph.tsx:222`
Then `cy.nodes is not a function` is NOT thrown
And the test reaches the assertion phase (currently fails at line ~78)

## Capability: build-architecture C4-code invariant

### Requirement: REQ-PETFIX-1 — C4 code nodes emit `kind: "code"` per ADR-039
The system SHALL emit C4 code-level nodes with `kind == "code"` and `style_class == "node-code"`, matching the convention used by sibling levels (`system`, `container`, `component`).

#### Scenario: source assigns literal `"code"` to kind
Given `crates/cognicode-explorer/src/facades/graph.rs:398`
When the file is read
Then line 398 contains exactly: `kind: "code".to_string(),`
And the `format!("{:?}", symbol.kind).to_lowercase()` expression is removed

#### Scenario: existing cap test becomes green
Given the fix at `graph.rs:398`
When `cargo test -p cognicode-explorer --lib facades::graph::tests::build_architecture_caps_code_nodes_at_200` runs
Then the filter `n.kind == "code"` at `graph.rs:777` matches exactly `200` nodes
And the test asserts `assert_eq!(code_nodes.len(), 200)` passes

#### Scenario: C4 invariant is consistent across all four levels
Given a workspace with one C4 architecture (system + container + component + code)
When `build_architecture` is invoked
Then for every emitted node: `node.kind == node.style_class.trim_start_matches("node-")`
And the four observed level pairs are: `("system","node-system")`, `("container","node-container")`, `("component","node-component")`, `("code","node-code")`

### Requirement: REQ-PETFIX-2 — Independent regression test guards the C4 code convention
The system SHALL provide a Rust test that asserts `kind == "code"` AND `style_class == "node-code"` independently of the cap-count test, so a future regression that reintroduces the dynamic format string is caught even if the cap test is deleted.

#### Scenario: new test exists and passes
Given `crates/cognicode-explorer/src/facades/graph.rs` test module
When the new test `build_architecture_code_nodes_use_literal_kind` runs
Then it builds a small fixture (≥1 symbol inside a C3 component)
And asserts: for every emitted node whose `id` starts with `"code:"`, `node.kind == "code"` AND `node.style_class == "node-code"`
And the assertion passes

#### Scenario: regression is caught independently of cap test
Given a future change reverts `graph.rs:398` to `kind: format!("{:?}", symbol.kind).to_lowercase()` while leaving `build_architecture_caps_code_nodes_at_200` deleted
When the new test runs
Then it fails with `assertion failed: node.kind == "code"`
And the failure is unambiguous (no cap-count noise)

## Capability: stylesheet node-class registry parity

### Requirement: REQ-PETFIX-3 — Stylesheet test count matches the 4-C4-level registry
The system SHALL expect `KNOWN_NODE_CLASSES.size === 14` to reflect that C4 contributes FOUR node classes (`node-component`, `node-container`, `node-system`, `node-code`).

#### Scenario: test expects 14
Given `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.test.ts:206`
When the file is read
Then line 206 contains exactly: `expect(KNOWN_NODE_CLASSES.size).toBe(14);`
And the `it(...)` description on line 205 reads `"total node class count is 14 (3 legacy + 4 multimodal + 4 C4 + 3 landing E4)"`

#### Scenario: C4 node-classes describe block enumerates 4 entries
Given the test file
When the `for (const c of [...])` loop at line 200 is read
Then it iterates over `["node-component", "node-container", "node-system", "node-code"]`
And each is asserted to be in `KNOWN_NODE_CLASSES`

## Capability: cytoscape-mock consolidation

### Requirement: REQ-PETFIX-5 — Five test files migrate to the shared cytoscape mock
The system SHALL eliminate the 5 inline `vi.mock("cytoscape")` factories and replace them with imports from `apps/explorer-ui/src/test/cytoscapeMock.ts`, preserving per-test hooks.

#### Scenario: each of 5 files imports the shared mock
Given the migration is complete
When each of the following files is read at its first non-import lines:
- `apps/explorer-ui/src/components/Shell.test.tsx`
- `apps/explorer-ui/src/components/RationaleView/RationaleView.test.tsx`
- `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.test.tsx`
- `apps/explorer-ui/src/components/ContextualPanel/NeighborMinigraph.test.tsx`
- `apps/explorer-ui/src/components/ContextualPanel/ContextualPanel.test.tsx`

Then each contains exactly one `vi.mock("cytoscape", ...)` call delegating to the shared factory
And no file redeclares `CyNode`, `CyEdge`, `CyCollection`, or `Cy` classes inline
And each file retains its existing test logic and assertions (no test was deleted or weakened)

#### Scenario: per-test hooks survive migration
Given the migrated test files
When `beforeEach` blocks run
Then `resetCyMock()` clears `globalThis.__cyInstances` (preserved from `NeighborMinigraph.test-helpers.ts`)
And `getCyInstances()` returns the constructed instances for assertions
And the `clickNode(id)` helper and `destroyed: boolean` flag remain accessible on each instance
And any test that previously asserted `destroyed === true` after unmount still passes

## Capability: pre-merge safety net

### Requirement: REQ-PETFIX-6 — No frontend consumer keys off raw symbol `kind` for C4 code nodes
The system SHALL be verified before merge to ensure no UI consumer depends on `kind === "function"` (or any other dynamic symbol-kind value) for architecture-rendered nodes.

#### Scenario: grep audit returns no offending consumers
Given the pre-merge check
When `grep -rn 'kind ===' apps/explorer-ui/src --include='*.ts' --include='*.tsx'` runs
Then no match is on `GraphNode.kind` semantics for C4 code nodes
And the only matches on `kind ===` are either: (a) ViewKind discriminators (`call_graph`, `dependency_graph`, etc.), (b) data-source / strategy / transform discriminators (`moldql`, `jsonata`, `registry`), or (c) fixture schema validation that the string is non-empty

## Capability: verification

### Requirement: REQ-PETFIX-7 — All checks pass with zero regressions
The system SHALL satisfy the verification gates enumerated below.

#### Scenario: Rust gate passes
Given the Rust fix at `graph.rs:398` and the new regression test
When `cargo test -p cognicode-explorer --lib facades::graph::tests` runs from repo root
Then every test in that module passes (including `build_architecture_caps_code_nodes_at_200` and `build_architecture_code_nodes_use_literal_kind`)
And `cargo build -p cognicode-explorer` exits 0

#### Scenario: UI gates pass
Given the stylesheet test fix and the 5-file mock migration
When the following run from `apps/explorer-ui/`:
- `npm run test -- --run -- RationaleView.test`
- `npm run test -- --run -- stylesheet.test`
- `npm run test -- --run -- Shell.test`
- `npm run test -- --run -- InteractiveGraph.test`
- `npm run test -- --run -- NeighborMinigraph.test`
- `npm run test -- --run -- ContextualPanel.test`

Then each exits 0
And `npx tsc --noEmit` exits 0

#### Scenario: no new failures across the whole suite
Given the change is applied
When the full UI test suite runs (`npm run test -- --run`)
Then the number of failing tests equals the number of failing tests on `main @ 7439a4f` minus 4 (F1, F2, F3, F4) and plus 0
And no test file outside the 5 migrated files shows a status change

## Invariants Covered
- **ADR-039 C4 contract** (`kind == level` AND `style_class == node-{level}`) — REQ-PETFIX-1 (Scenario: C4 invariant is consistent across all four levels), REQ-PETFIX-2 (Scenario: new test exists and passes)
- **5-file mock divergence → 1** (Connascence of Algorithm) — REQ-PETFIX-4 (Scenario: shared mock module exists with superset API), REQ-PETFIX-5 (Scenario: each of 5 files imports the shared mock)
- **Per-test hook preservation** (`__cyInstances`, `clickNode`, `destroyed`) — REQ-PETFIX-4 (Scenario: shared mock preserves per-test hooks), REQ-PETFIX-5 (Scenario: per-test hooks survive migration)
- **Stylesheet registry parity** (14 = 3 legacy + 4 multimodal + 4 C4 + 3 landing) — REQ-PETFIX-3 (Scenario: test expects 14)

## Out of Scope
- New ADR for the C4 invariant (ADR-039 already encodes it; F4 is a violation, not new knowledge)
- Frontend product-code changes beyond mock migration
- Type-safety refactor of the mock (the superset API is `any`-tolerant)
- Bench-renderer mocks (`apps/explorer-ui/src/bench/renderers/cytoscape-canvas.test.ts`, `cytoscape-webgl.test.ts`) — separate concern with different registry pattern
- Adding new C4 levels or changing the convention itself

## Verification Commands
```bash
# Rust
cargo test -p cognicode-explorer --lib facades::graph::tests
cargo build -p cognicode-explorer

# UI (run from apps/explorer-ui/)
npm run test -- --run -- RationaleView.test
npm run test -- --run -- stylesheet.test
npm run test -- --run -- Shell.test
npm run test -- --run -- InteractiveGraph.test
npm run test -- --run -- NeighborMinigraph.test
npm run test -- --run -- ContextualPanel.test
npx tsc --noEmit

# Pre-merge audit
grep -rn 'kind ===' apps/explorer-ui/src --include='*.ts' --include='*.tsx'
```

## Open Questions
- None. The regression test for REQ-PETFIX-2 was confirmed in proposal (recommend yes). All file paths and line numbers verified against `main @ 7439a4f`.
