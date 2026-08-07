# Spec: Block Renderer Registry (E1.4)

## Purpose

Replace the 29-case hand-rolled `switch` in
`apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` with a
lookup against a typed `blockRendererRegistry`. The registry is keyed by
`block.id` (a `ViewBlock["id"]` discriminated union member), resolves to
the existing per-block React component from `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/*`,
and falls back to `UnknownBlockView` for any id outside the union.

This spec is **delta** against `openspec/specs/renderer-registry-frontend/spec.md`
(which introduced the `RendererKind`-keyed registry) and
`openspec/specs/view-spec-domain/spec.md` (which defines `ViewBlock` and
its `id` discriminated union). The block-id namespace is **separate**
from the `RendererKind` namespace: `RendererKind` keys by *visual
strategy* (`graph`, `table`, `tree`, …); `block.id` keys by *domain
content* (`identity`, `call_metrics`, `hotspots`, …). 6 of 29 blocks
happen to map to standard `RendererKind` values; the other 23 do not.

## Domain

`block-renderer-registry` — NEW capability, lives alongside the existing
`RendererKind` registry. Single front-end capability, no backend
contract.

**Phase**: E1 (Renderer Consolidation, sprint E1.4).

---

## Router Context Used

- **Knowledge Coverage**: sufficient (the 29 switch cases are all
  identifiable; the 4 cases that pass `onSelectObject` are identifiable;
  the existing test contracts are identifiable in `ViewBlock.test.tsx`).
- **Context Quality**: C2 — files read, contracts enumerated, no
  external authority consulted beyond ADR-008.
- **Taxonomy**: switch-dispatch entropy (29 cases) + discrimination by
  block-id union.
- **Domain Language**: resolved terms from `CONTEXT.md` (ViewBlock,
  RendererKind, RendererRegistry, ContextualView, ViewKind). Unresolved:
  whether `blockRendererRegistry` lives in the same file as
  `rendererRegistry` or in a sibling file (resolved at proposal time —
  SEPARATE FILE per defaults adopted in AUTO mode).
- **Invariants**: data-testid contract on each block component;
  `onSelectObject` propagation; `UnknownBlockView` fallback for unknown
  ids.
- **Recommended Effort**: deepen (already done in proposal).

---

## ADDED Requirements

### Requirement: REQ-E1.4-1 — Dispatch via registry lookup

`ViewBlock` MUST dispatch each block to its renderer by looking the id
up in `blockRendererRegistry` and rendering the entry's component.
The hand-rolled 29-case `switch` MUST be removed from
`apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx`.

Concretely, the dispatch line in the body of `ViewBlock` collapses to:

```tsx
const entry = blockRendererRegistry.get(block.id);
if (!entry) return <UnknownBlockView block={block as UnknownViewBlock} />;
return <entry.Component {...(entry.props ?? {})} />;
```

The 29 switch cases in `ViewBlock.tsx` lines 94–198 are removed.
`ViewBlock.tsx` reduces from 242 LOC to ≈70 LOC (header comment +
`Blocks` wrapper + dispatch lookup + fallback).

#### Scenario: Known block id routes to its registered component

- **GIVEN** `blockRendererRegistry` has an entry keyed `"identity"`
  whose `Component` is `IdentityView`
- **AND** a `ViewBlock` with `id: "identity"`, `title: "Identity"`,
  `body: { name: "build_overview", kind: "function", file: "src/lib.rs", line: 16 }`
- **WHEN** `ViewBlock` is rendered with that block
- **THEN** the rendered DOM contains `data-testid="view-block-identity"`
  (per `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/shared.tsx:13`)
- **AND** the text `"build_overview"` appears in the rendered tree
- **AND** the rendered tree does **NOT** contain `data-testid="view-block-unknown"`

#### Scenario: Every known block id resolves to a registered entry

- **GIVEN** the union `ViewBlock["id"]` has 29 members (verified by
  `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts:27-57`:
  `KNOWN_IDS` set)
- **WHEN** the test enumerates every id in `KNOWN_IDS` and calls
  `blockRendererRegistry.get(id)`
- **THEN** every lookup returns a non-`undefined` `BlockRendererEntry`
- **AND** every entry's `Component` is a function component (typeof
  entry.Component === 'function' || forwardRef object)

---

### Requirement: REQ-E1.4-2 — Registry is populated at module load

`blockRendererRegistry` MUST be populated synchronously at module load
from the existing 29 components in
`apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/*`.
The 29 entries correspond 1:1 to the 29 switch cases being removed
(`identity`, `call_metrics`, `signature`, `callers`, `callees`,
`source_slice`, `symbol_quality_identity`, `symbol_quality_issues`,
`file_quality_identity`, `file_quality_issues`, `file_quality_gate`,
`scope_quality_identity`, `scope_quality_gate`, `scope_quality_issues`,
`issue_identity`, `issue_location`, `issue_message`, `rule_identity`,
`rule_related`, `file_identity`, `kinds`, `symbols`, `scope_identity`,
`scope_kinds`, `scope_files`, `cross_scope`, `hotspots`,
`quality_summary`, `quality_issue_detail`).

The registry MUST live in a NEW file
`apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.ts`
(co-located with `ViewBlock.tsx` so the namespace is co-located with
its consumer — separate file from `rendererRegistry.tsx` per the
proposal decision).

```ts
// blockRendererRegistry.ts (sketch)
import {
  IdentityView, CallListView, CallMetricsView, SignatureView, SourceView,
  FileIdentityView, FileSymbolsView, KindsView,
  FileQualityIdentityView, QualityGateView, QualityIssueDetailView,
  QualitySummaryView, IssueIdentityView, IssueLocationView,
  IssueMessageView, IssuesListView, RuleIdentityView,
  ScopeQualityIdentityView, SymbolQualityIdentityView,
  CrossScopeView, ScopeFilesView, ScopeIdentityView,
  HotspotsView, UnknownBlockView,
  type CallListProps,
} from "./ViewBlocks";

export interface BlockRendererEntry {
  /** Which props the entry needs from `extra` (see REQ-E1.4-6). */
  readonly accepts: "block-only" | "block+onSelectObject";
  readonly Component: React.ComponentType<{ block: ViewBlock; onSelectObject?: (id: string) => void }>;
  /** If true, the entry's typed `block.id` discriminant. Used by the
   *  registration-time exhaustiveness assertion (REQ-E1.4-4). */
  readonly id: ViewBlock["id"];
}

class BlockRendererRegistry { /* ... */ }

export const blockRendererRegistry = new BlockRendererRegistry();
```

#### Scenario: Registry size equals 29 known ids

- **GIVEN** `blockRendererRegistry` is imported as a singleton
- **WHEN** `blockRendererRegistry.size()` is called
- **THEN** the result is `29` (matches `KNOWN_IDS.size`)

#### Scenario: Module-load timing — entries are present before first render

- **GIVEN** `import { blockRendererRegistry } from "./blockRendererRegistry"`
  is evaluated at the top of `ViewBlock.tsx`
- **WHEN** the very first `<ViewBlock block={…} />` mounts
- **THEN** `blockRendererRegistry.get(block.id)` is non-`undefined` for
  every id in `KNOWN_IDS` (no async loading, no `useEffect`, no
  `Suspense`)
- **AND** there is no flash of `UnknownBlockView` before the registry
  hydrates

---

### Requirement: REQ-E1.4-3 — Unknown block IDs fall back to `UnknownBlockView`

When `blockRendererRegistry.get(block.id)` returns `undefined`, the
dispatch MUST render `UnknownBlockView` with the same JSON body, the
same `data-testid="view-block-unknown"` marker, and the same
`data-testid="view-block-unknown-json"` child marker as the current
`ViewBlock.tsx` lines 88–90 and lines 287–300 of
`apps/explorer-ui/src/components/ObjectInspector/ViewBlock.test.tsx`.

The pre-check currently at line 87 (`const known = isKnownBlockId(id)`)
is replaced by the registry's own lookup — no separate `KNOWN_IDS` set
is needed in `ViewBlock.tsx`. The `KNOWN_IDS` set in
`apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts:27-57`
remains in place because `isKnownBlockId` is still consumed by
`ObjectInspector.tsx` consumers and by tests.

#### Scenario: Unknown id renders UnknownBlockView

- **GIVEN** a `UnknownViewBlock` with
  `id: "future_block_2027"`, `title: "Future"`, `body: { novel_field: 42 }`
- **WHEN** `ViewBlock` is rendered with that block
- **THEN** `data-testid="view-block-unknown"` is in the DOM
- **AND** `data-testid="view-block-unknown-json"` is in the DOM and
  contains the text `'"novel_field": 42'`

#### Scenario: An empty registry returns UnknownBlockView (forced fallback)

- **GIVEN** a test-only renderer that mutates the registry to empty
  (e.g., `blockRendererRegistry.clear()` for the test's duration)
- **AND** a `ViewBlock` with `id: "identity"`
- **WHEN** rendered
- **THEN** `data-testid="view-block-unknown"` is in the DOM (NOT
  `view-block-identity`) — proving the dispatch truly falls through on
  lookup miss and does not cache or short-circuit

---

### Requirement: REQ-E1.4-4 — Registration-time exhaustiveness assertion

The current `switch` relies on a TypeScript `never` exhaustiveness
check (`ViewBlock.tsx` line 194: `const _exhaustive: never = id;`) that
fires **at compile time** when a new id is added to `ViewBlock["id"]`.

When the switch becomes a runtime registry, that compile-time signal
is lost. This requirement restores it as a **registration-time
assertion** that runs when the registry's constructor populates the 29
entries, plus a **test** that asserts the registry has an entry for
every id in `KNOWN_IDS`.

The assertion MUST run inside the registry's constructor (synchronous,
top-level `import` side-effect) and MUST throw a descriptive `Error`
naming the missing id(s). The assertion MUST iterate the union type
`ViewBlock["id"]` (not the `KNOWN_IDS` set) so that adding a new id to
the union breaks the assertion even before `KNOWN_IDS` is updated.

```ts
// Inside the registry constructor:
const REGISTERED: ReadonlySet<ViewBlock["id"]> = /* ids added above */;
const UNION: ReadonlyArray<ViewBlock["id"]> = [
  "identity", "call_metrics", "signature", "callers", "callees",
  /* ... full union, 29 entries ... */
];
for (const id of UNION) {
  if (!REGISTERED.has(id)) {
    throw new Error(`[blockRendererRegistry] Missing renderer for block id "${id}"`);
  }
}
```

#### Scenario: Missing renderer fails fast at module load

- **GIVEN** a developer adds `"new_block_2027"` to the
  `ViewBlock["id"]` union in
  `apps/explorer-ui/src/api/types` and to `KNOWN_IDS` in
  `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts`
  but **forgets** to register a renderer in `blockRendererRegistry`
- **WHEN** `import { blockRendererRegistry } from "./blockRendererRegistry"` is
  evaluated
- **THEN** the JavaScript runtime throws
  `Error: [blockRendererRegistry] Missing renderer for block id "new_block_2027"`
- **AND** the application fails to start (no silent fallback to
  `UnknownBlockView` at runtime)

#### Scenario: Test asserts every known id has a registered entry

- **GIVEN** a test in `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.test.ts`
- **WHEN** the test iterates every id in `KNOWN_IDS`
- **THEN** `expect(blockRendererRegistry.get(id)).toBeDefined()` passes
  for every id
- **AND** the test fails loudly if any entry is removed or any new id is
  added without a corresponding `register()` call

---

### Requirement: REQ-E1.4-5 — data-testid contract preserved

Every block component currently emits
`data-testid="view-block-{block_id}"` via
`apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/shared.tsx:13`
(`testId ?? \`view-block-${id}\``). The contract is asserted across
`apps/explorer-ui/src/components/ObjectInspector/ViewBlock.test.tsx`
lines 52, 64, 83, 103, 129, 154, 170, 182, 202, 241, 270, 295 (and
indirectly at line 214 via the `getAllByTestId(/^view-block-/)` regex).

The registry refactor MUST NOT change the `data-testid` contract. Every
testid asserted in `ViewBlock.test.tsx` MUST continue to resolve to the
same element with the same shape. Per-component testids embedded in
specific components — e.g.
`view-block-issue-{id}` (line 130),
`view-block-hotspot-button-{object_id}` (line 158),
`quality-issue-detail-location` (line 271),
`quality-summary-rating` (line 242) — are owned by the component
implementations and are NOT changed by this refactor.

#### Scenario: Per-id testids resolve through the registry path

- **GIVEN** the registry has been populated
- **WHEN** the existing test suite in `ViewBlock.test.tsx` runs
- **THEN** every existing assertion of the form
  `screen.getByTestId("view-block-{id}")` passes
- **AND** no testid is renamed, removed, or prefixed

#### Scenario: All-fixture render guard

- **GIVEN** `contextualViewFixture` (29 blocks, see
  `apps/explorer-ui/src/mocks/fixtures.ts:86`)
- **WHEN** rendered via `Blocks` (which iterates `view.blocks` and
  hands each to `ViewBlock`)
- **THEN** `screen.getAllByTestId(/^view-block-/)` returns at least 29
  elements (29 blocks + the per-item `view-block-issue-7` +
  `view-block-hotspot-button-*` if those items exist in the fixture)
- **AND** none of those elements is `view-block-unknown`

---

### Requirement: REQ-E1.4-6 — Runtime props flow through the registry

Four of the 29 block components receive `onSelectObject` as a prop:
`callers`, `callees` (`CallListView`),
`hotspots` (`HotspotsView`),
`quality_issue_detail` (`QualityIssueDetailView`). These are the 4
"interactive" block types — clicking a row in any of them dispatches
`onSelectObject(objectId)`. The other 25 components are non-interactive
and do not consume the callback.

The registry's entry type MUST express the prop needs so that:

1. `blockRendererRegistry.get("callers").accepts === "block+onSelectObject"`
2. `blockRendererRegistry.get("identity").accepts === "block-only"`
3. `ViewBlock` passes `onSelectObject` to the entry's `Component` only
   when `entry.accepts === "block+onSelectObject"`

The typed shape is:

```ts
export interface BlockRendererEntry {
  readonly id: ViewBlock["id"];
  readonly accepts: "block-only" | "block+onSelectObject";
  readonly Component: React.ComponentType<{
    block: ViewBlock;
    onSelectObject?: (objectId: string) => void;
  }>;
}
```

`ViewBlock` invokes `entry.Component({ block, onSelectObject })` —
TypeScript's structural typing accepts both prop shapes because
non-interactive entries simply ignore the extra `onSelectObject`.

> **Verification note (open question from proposal):** The proposal
> mentions "6 block types" receiving `onSelectObject`. Verified code
> shows **4** — `callers`, `callees`, `hotspots`, `quality_issue_detail`.
> `issue_location` is **not** wired with `onSelectObject` in the
> current switch (`ViewBlock.tsx:148`). If a 5th or 6th block is
> intended, it must be added to this spec before implementation.

#### Scenario: Interactive block receives onSelectObject

- **GIVEN** `onSelectObject` is provided to `ViewBlock` as a callback
- **AND** the block is `id: "hotspots"` with a row whose
  `object_id: "sym-1"`
- **WHEN** the user clicks `data-testid="view-block-hotspot-button-sym-1"`
  (the inner `<button>` per the accessibility note in
  `ViewBlock.test.tsx:155-159`)
- **THEN** `onSelectObject` is invoked once with the argument `"sym-1"`
  (asserted at `ViewBlock.test.tsx:160`)

#### Scenario: Non-interactive block does not require onSelectObject

- **GIVEN** `onSelectObject` is **omitted** (or `undefined`)
- **AND** the block is `id: "call_metrics"` with `body: { fan_in: 3, fan_out: 4 }`
- **WHEN** `ViewBlock` is rendered
- **THEN** `data-testid="view-block-call_metrics"` is in the DOM
- **AND** the values `3` and `4` render (asserted at
  `ViewBlock.test.tsx:65-66`)
- **AND** no console warning is emitted about missing
  `onSelectObject` for non-interactive blocks

#### Scenario: Registry lookup with `extra` payload (future-proofing)

- **GIVEN** the registry exposes a `get(id, extra?)` overload that
  accepts the runtime context object as a second argument
- **AND** an entry declares `accepts: "block+onSelectObject"`
- **WHEN** `blockRendererRegistry.get("callers", { onSelectObject })` is called
- **THEN** the returned entry's `Component` is invoked with both
  `block` and `onSelectObject` bound from the extra payload
- **AND** the typed shape of `extra` is `Partial<RuntimeContext>` where
  `RuntimeContext` is the shared type introduced by E1.5
  (`{ dispatch, objectId, paneId, viewId, onClose, onSelectObject }`).

> **Note**: REQ-E1.4-6 makes `RuntimeContext` available to *block*
> entries so that interactive blocks can dispatch (via `onSelectObject`)
> without each block needing its own `useAppDispatch()` hook. E1.5 makes
> `RuntimeContext` available to the `graph` *renderer*. The two are
> intentionally compatible — the `extra` shape is the same object
> threaded through both registries.

---

## Invariants Covered

| Invariant | Source | Scenario |
|-----------|--------|----------|
| `data-testid="view-block-{block_id}"` on every known block | `ViewBlocks/shared.tsx:13`, asserted in `ViewBlock.test.tsx` | REQ-E1.4-5 (per-id + fixture guard) |
| `onSelectObject` propagates to interactive blocks (callers, callees, hotspots, quality_issue_detail) | `ViewBlock.tsx:104-114, 173, 186`; asserted at `ViewBlock.test.tsx:160, 279` | REQ-E1.4-6 |
| Unknown block id falls back to `UnknownBlockView` with `view-block-unknown` + `view-block-unknown-json` markers | `ViewBlock.tsx:88-90`; asserted at `ViewBlock.test.tsx:295-298` | REQ-E1.4-3 |
| Compile-time exhaustiveness on new block ids | `ViewBlock.tsx:194` (`const _exhaustive: never = id`) | REQ-E1.4-4 (registration-time assertion + test) |
| `Blocks` empty-state contract | `ViewBlock.tsx:218-227`; asserted at `ViewBlock.test.tsx:223` | (covered by `Blocks` wrapper, unchanged by E1.4) |

---

## Affected Files

| File | LOC | Change |
|------|-----|--------|
| `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` | 242 | Lines 94–198 (29-case switch) removed; dispatch replaced with `blockRendererRegistry.get(block.id)`; file reduces to ≈70 LOC |
| `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.ts` | NEW (~80 LOC) | Defines `BlockRendererEntry`, `BlockRendererRegistry`, populates 29 entries from `ViewBlocks/*`, runs registration-time exhaustiveness assertion |
| `apps/explorer-ui/src/components/ObjectInspector/blockRendererRegistry.test.ts` | NEW (~60 LOC) | Asserts every id in `KNOWN_IDS` resolves; asserts registration-time assertion fires on missing entries |
| `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/types.ts` | 77 | `KNOWN_IDS` set retained at lines 27-57; `isKnownBlockId` retained for consumers |
| `apps/explorer-ui/src/components/ObjectInspector/ViewBlocks/*.tsx` (8 files) | unchanged | All per-block components retain their `data-testid` and `onSelectObject` props |

---

## Out of Scope

- New `ViewBlock["id"]` members or `RendererKind` variants.
- Replacing `RendererKind`-keyed `rendererRegistry` (E1.5).
- Removing the `KNOWN_IDS` set (kept for `isKnownBlockId` consumers).
- Refactoring `Blocks` wrapper (lines 200-242 of `ViewBlock.tsx`).
- Renaming any `data-testid` markers.
- The graph-renderer short-circuit in `PaneInspector.tsx:238` — that
  is E1.5.
- Vega-Lite wiring (Phase 4).
- MCP/ViewSpec integration (separate change).

---

## Open Questions

1. **4 vs 6 interactive blocks** — proposal says 6, code shows 4. If
   `issue_location` (and possibly one other) should be interactive,
   this spec must be amended before implementation. (Verification
   note in REQ-E1.4-6.)
2. **`blockRendererRegistry` exports shape** — proposal defaults
   to a class singleton (`new BlockRendererRegistry()`). An alternative
   is a frozen `Record<ViewBlock["id"], BlockRendererEntry>`. The
   class form allows future runtime registration (e.g., a ViewSpec
   wizard registering custom blocks); the frozen form prevents that
   but loses the registration-time assertion trigger. Recommend the
   class form (default).

---

## Acceptance Criteria (Given/When/Then)

These are the executable checks that gate E1.4 acceptance:

1. **Given** `import { ViewBlock } from "./ViewBlock"` resolves,
   **When** `ViewBlock.tsx` is read,
   **Then** no `switch (id)` statement remains (regex `/^\s*switch\s*\(\s*id\s*\)/`
   has zero matches).
2. **Given** the existing 486 LOC `ViewBlock.test.tsx`,
   **When** run,
   **Then** every assertion passes unchanged (no test edits).
3. **Given** a developer adds `"future_block_x"` to `ViewBlock["id"]`
   without registering a renderer,
   **When** the registry module loads,
   **Then** the runtime throws a descriptive `Error`.
4. **Given** the registry is module-loaded,
   **When** `blockRendererRegistry.get(id)` is called for every id in
   `KNOWN_IDS`,
   **Then** all 29 lookups return a defined entry.
5. **Given** a `hotspots` block with one interactive row,
   **When** the row's inner button is clicked,
   **Then** `onSelectObject("sym-1")` is invoked (asserted at
   `ViewBlock.test.tsx:160`).
6. **Given** a `future_block_2027` block (not in the union),
   **When** rendered,
   **Then** `view-block-unknown` and `view-block-unknown-json` markers
   appear (asserted at `ViewBlock.test.tsx:295-298`).