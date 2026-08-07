# Spec: RendererRegistry Frontend Skeleton

## Purpose

Introduce a `RendererRegistry` on the React side: a typed map from
`RendererKind` to a component that knows how to render a
`ContextualView`'s payload. The registry wraps the existing
`ViewBlock` switch in `ObjectInspector/ViewBlock.tsx` so that
**known renderers are dispatched via lookup** and **unknown
renderers fall back to raw JSON** (matching ADR-008 §Validation).
v1 ships the skeleton + a small built-in renderer set; the
authoring flow that lets users register renderers for their
ViewSpecs is Phase 4 (`viewspec-authoring-flow`).

## Domain

`renderer-registry-frontend` — NEW capability. No existing spec
to delta against; this is a full spec.

**Phase**: 3 (skeleton).

---

## ADDED Requirements

### Requirement: 1. `RendererRegistry` map

The system MUST expose a `RendererRegistry` module under
`apps/explorer-ui/src/renderer/registry.ts` with a single
`rendererRegistry: Map<RendererKind, ViewRenderer>` and a
`registerRenderer(kind, component)` function.

`ViewRenderer` is the contract:

```ts
export type ViewRenderer = (props: {
  view: ContextualView;
  onSelectObject?: (objectId: string) => void;
}) => React.ReactElement;
```

`registerRenderer` MUST be idempotent: a second call with the same
`RendererKind` MUST replace the previous registration and MUST
log a `console.warn` with the previous renderer name. The
registry is a plain `Map`; React context is **not** required.

#### Scenario: Built-in renderers register on module import

- GIVEN `apps/explorer-ui/src/renderer/builtin.ts` imports
  `registerRenderer` once per built-in kind
- WHEN the module is evaluated
- THEN `rendererRegistry.has(RendererKind.Graph)` is `true` and
  the same holds for `Table`, `Tree`, `Code`, `Markdown`,
  `Composite`, and `Json`
- AND no key is registered twice

#### Scenario: Re-register replaces the previous renderer

- GIVEN a renderer `Foo` is registered for `RendererKind.Table`
- WHEN a second `registerRenderer(Table, Bar)` runs
- THEN `rendererRegistry.get(RendererKind.Table) === Bar`
- AND `console.warn` was called with the previous name

### Requirement: 2. Built-in renderer set (Phase 3)

The Phase 3 built-in renderers MUST cover every `RendererKind`
variant shipped in Phase 0. The first-class mapping:

| `RendererKind` | Source today | Renderer behavior in Phase 3 |
|----------------|--------------|------------------------------|
| `graph` | `useContextualGraph` hook | Wraps `ContextualPanel` (already in main, must adapt to `ViewRenderer` signature) |
| `table` | (new in Phase 3) | Renders the first `ViewBlock.body` that looks like `{ count, items[] }` as a table; columns inferred from item keys |
| `tree` | (new in Phase 3) | Renders the first `ViewBlock.body` that looks like `{ scope, files[] }` as a flat list; full tree composition is Phase 4+ |
| `code` | (new in Phase 3) | Renders the first `ViewBlock.body` of shape `{ lines: [{ line, text }] }` using the existing `SourceView` |
| `markdown` | (new in Phase 3) | Renders `ViewBlock.body.markdown` as a `<Markdown />` string (no html sanitisation library yet — wrap text in `<pre>` for safety) |
| `vega_lite` | (new in Phase 3) | Renders `ViewBlock.body.spec` via a `vega-embed` call wrapped in `useEffect`; degrades to raw JSON if `vega-embed` is not installed |
| `json` | existing `UnknownBlockView` | Renamed to `JsonRenderer`; same JSON-stringify behavior |
| `composite` | (new in Phase 3) | Renders the first three blocks vertically, falling back to `json` for unknown shapes |

The `graph` and `code` renderers MUST reuse the existing
`ContextualPanel` / `SourceView` components — no behaviour change
for the user. New renderers (`table`, `tree`, `markdown`, `composite`,
`vega_lite`) ship as small wrappers, not full rewrites.

#### Scenario: Table renderer uses first table-shaped block

- GIVEN a `ContextualView` whose first block has
  `body = { count: 2, items: [{ name: "a" }, { name: "b" }] }`
- WHEN the `table` renderer runs
- THEN a `<table>` element with two rows and a `name` column
  appears in the output

#### Scenario: Composite renderer shows top three blocks

- GIVEN a `ContextualView` with 5 blocks
- WHEN the `composite` renderer runs
- THEN the first 3 blocks are rendered; blocks 4 and 5 are
  ignored (not rendered as raw JSON, not appended)

#### Scenario: Vega-lite renderer degrades gracefully

- GIVEN the `vega-embed` package is not installed
- WHEN the `vega_lite` renderer runs
- THEN the block body is rendered as raw JSON and a console
  message reads `"vega-embed not installed; showing raw spec"`

### Requirement: 3. ViewSpecRenderable — the `RendererRegistry`
entry point used by `ViewBlock`

`ViewBlock` MUST change from a hand-rolled `switch` to a registry
lookup:

```tsx
const renderer = rendererRegistry.get(view.renderer_kind);
if (renderer) {
  return <>{renderer({ view, onSelectObject })}</>;
}
// Fallback: keep the existing UnknownBlockView behavior. Reuse
// the same component so tests stay green.
return <UnknownBlockView block={firstUnknownBlock(view)} />;
```

The lookup runs once per block, not once per render: the registry
MUST be read inside the function component (no `useMemo` is
required — `Map.get` is O(1)). The `UnknownBlockView` component
MUST be preserved bit-for-bit so the existing test suite
(`ViewBlock.test.tsx`) keeps passing.

#### Scenario: Known renderer dispatched via registry

- GIVEN a block with `id = "callers"` and `view_kind =
  CallGraph` (the existing callgraph builder is wired to the
  `graph` renderer)
- WHEN `ViewBlock` renders
- THEN the `graph` renderer is called and the cytoscape panel
  appears

#### Scenario: Unknown renderer falls back to UnknownBlockView

- GIVEN a `ViewBlock` whose `view.renderer_kind` is a
  `Custom("future_ai_view")` and the registry has no entry
- WHEN `ViewBlock` renders
- THEN the `UnknownBlockView` markup is rendered with the
  `view-block-unknown` testid and the body serialised as JSON

#### Scenario: Existing test suite passes

- GIVEN the test file
  `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.test.tsx`
- WHEN `vitest run ViewBlock` runs after the migration
- THEN every existing test passes; the only permitted test
  signature change is adding a new test for the registry path

### Requirement: 4. `RendererKind` is carried on the wire

The `ContextualView` DTO MUST gain a `renderer_kind: RendererKind`
field, defaulting to `RendererKind::Json` (TS: `"json"`) when
absent. The Rust struct MUST use `#[serde(default)]` so Phase 1
responses (no `renderer_kind`) deserialise cleanly with
`RendererKind::Json`. The Phase 1 service layer MUST populate
`renderer_kind` from the existing view id → renderer mapping
(`overview` → `json`, `call-graph` → `graph`, `source` → `code`,
quality → `table`).

#### Scenario: Phase 1 service maps view_id → renderer_kind

- GIVEN `build_callgraph` returns a `ContextualView` with
  `view_id = "call-graph"`
- WHEN the service serialises the response
- THEN `renderer_kind` is `"graph"`
- AND no existing field is removed from the wire

#### Scenario: Missing renderer_kind defaults to json

- GIVEN a JSON payload `{"view_id": "old", "blocks": [...], ...}`
  with no `renderer_kind`
- WHEN deserialised by `viewRendererKindSchema`
- THEN the value is `RendererKind::Json` / `"json"`

### Requirement: 5. Hook update is additive

`useAvailableViews` (in
`apps/explorer-ui/src/hooks/useViews.ts`) MUST keep its existing
return type `ViewList = ViewDescriptor[]`. The hook MUST NOT
expose `view_kind` or `renderer_kind` on the wire until the
follow-up spec. This keeps the Phase 1 UI surface byte-compatible
with today's `ViewTabs` and `ObjectInspector`.

#### Scenario: Existing useAvailableViews contract

- GIVEN a symbol `S`
- WHEN `useAvailableViews(S).data` is read after Phase 1
- THEN the array contains 4 entries with the same `{ id, title }`
  shape as before; the hook signature is unchanged

## Out of Scope (Phase 3 — explicit non-requirements)

- Registering renderers at runtime for user-defined
  `ViewSpec.renderer_kind`s — Phase 4
- Composing multiple renderers side-by-side in a single view —
  Phase 4
- JSONata-driven transform of block bodies before rendering —
  Phase 4
- Re-binding a `RendererKind` to a remote / Module Federation
  component — out of v1 scope (ADR-008 §Alternatives)
- Migrating `ViewBlock`'s 27 hand-rolled cases into the registry
  en masse — Phase 4 (Phase 3 only adds the `graph`, `code`,
  `json` paths through the registry; the rest keep their
  switch-fallback)

## Coverage

- **Happy paths**: covered (registry lookup, all built-in kinds
  register, renderer_kind wire field, ViewBlock switch replaced)
- **Edge cases**: covered (unknown renderer fallback, missing
  `renderer_kind` defaults, duplicate register, no `vega-embed`)
- **Error states**: covered (no panic on unknown kind, console
  warn on duplicate, test suite unchanged)
