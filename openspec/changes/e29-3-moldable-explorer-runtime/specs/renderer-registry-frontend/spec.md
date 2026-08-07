# Delta for RendererRegistry Frontend

This delta rewires `renderer-registry-frontend` so every `PaneInspector` dispatch is keyed by `renderer_kind`, adds the `mermaid` built-in renderer, and **replaces the silent JSON fallback with an explicit `UnsupportedRendererState`** — there is no silent JSON fallback in this change, including for `vega_lite`. **`renderer_kind` is the SOLE normal dispatch path**; `runtime_renderer_dispatch` defaults `on`. Setting it `off` is a temporary, explicit emergency rollback action and MUST emit diagnostics.

## MODIFIED Requirements

### Requirement: 1. `RendererRegistry` map

The system MUST expose a `RendererRegistry` module under `apps/explorer-ui/src/renderer/registry.ts` with a single `rendererRegistry: Map<RendererKind, ViewRenderer>` and a `registerRenderer(kind, component)` function. The registry is the **sole normal source** of renderer dispatch — the legacy `view_kind` switch MUST NOT run when `rendererRegistry.get(view.renderer_kind)` resolves to a renderer.

`ViewRenderer` is the contract:

```ts
export type ViewRenderer = (props: {
  view: ContextualView;
  onSelectObject?: (objectId: string) => void;
}) => React.ReactElement;
```

`registerRenderer` MUST be idempotent: a second call with the same `RendererKind` MUST replace the previous registration and MUST log a `console.warn` with the previous renderer name. The registry is a plain `Map`; React context is **not** required.

(Previously: a hand-rolled `view_kind` switch in `PaneInspector` was the normal dispatch path; the registry existed but was not consulted on the hot path. The registry is now the sole normal source.)

#### Scenario: Built-in renderers register on module import

- GIVEN `apps/explorer-ui/src/renderer/builtin.ts` imports `registerRenderer` once per built-in kind
- WHEN the module is evaluated
- THEN `rendererRegistry.has(RendererKind.Graph)` is `true` and the same holds for `Table`, `Tree`, `Code`, `Markdown`, `Composite`, `Json`, and **`Mermaid`**
- AND no key is registered twice

#### Scenario: Re-register replaces the previous renderer

- GIVEN a renderer `Foo` is registered for `RendererKind.Table`
- WHEN a second `registerRenderer(Table, Bar)` runs
- THEN `rendererRegistry.get(RendererKind.Table) === Bar`
- AND `console.warn` was called with the previous name

### Requirement: 2. Built-in renderer set (Phase 3)

The Phase 3 built-in renderers MUST cover every `RendererKind` variant shipped in Phase 0. The first-class mapping:

| `RendererKind` | Source today | Renderer behavior in Phase 3 |
|----------------|--------------|------------------------------|
| `graph` | `useContextualGraph` hook | Wraps `ContextualPanel` (already in main, must adapt to `ViewRenderer` signature) |
| `table` | (new in Phase 3) | Renders the first `ViewBlock.body` that looks like `{ count, items[] }` as a table; columns inferred from item keys |
| `tree` | (new in Phase 3) | Renders the first `ViewBlock.body` that looks like `{ scope, files[] }` as a flat list; full tree composition is Phase 4+ |
| `code` | (new in Phase 3) | Renders the first `ViewBlock.body` of shape `{ lines: [{ line, text }] }` using the existing `SourceView` |
| `markdown` | (new in Phase 3) | Renders `ViewBlock.body.markdown` as a `<Markdown />` string (no html sanitisation library yet — wrap text in `<pre>` for safety) |
| `vega_lite` | (new in Phase 3) | Renders `ViewBlock.body.spec` via a `vega-embed` call wrapped in `useEffect`. **If `vega-embed` is not installed OR the spec is unsupported, the renderer MUST return `<UnsupportedRendererState rendererKind="vega_lite" reason="vega_embed_unavailable" />`. The renderer MUST NOT silently fall back to raw JSON serialisation.** |
| `json` | existing `UnknownBlockView` | Renamed to `JsonRenderer`; same JSON-stringify behavior. Used only when an author explicitly selects `RendererKind::Json`; never as a silent fallback for unknown kinds. |
| `composite` | (new in Phase 3) | Renders the first three blocks vertically, falling back to `UnsupportedRendererState` for unknown shapes within the composite — never silent JSON |
| **`mermaid`** | **(new in E29.3)** | Renders `ViewBlock.body.mermaid` via `mermaid.render`; degrades to `UnsupportedRendererState` if `mermaid` package is not installed |

The `graph` and `code` renderers MUST reuse the existing `ContextualPanel` / `SourceView` components — no behaviour change for the user. New renderers (`table`, `tree`, `markdown`, `composite`, `vega_lite`, `mermaid`) ship as small wrappers, not full rewrites.

(Previously: unknown renderers silently fell back to `UnknownBlockView` (raw JSON); the `mermaid` variant was absent. The E29.3 modification makes the silent-JSON fallback behaviour impossible — every missing renderer surfaces `UnsupportedRendererState`.)

#### Scenario: Table renderer uses first table-shaped block

- GIVEN a `ContextualView` whose first block has `body = { count: 2, items: [{ name: "a" }, { name: "b" }] }`
- WHEN the `table` renderer runs
- THEN a `<table>` element with two rows and a `name` column appears in the output

#### Scenario: Composite renderer shows top three blocks

- GIVEN a `ContextualView` with 5 blocks
- WHEN the `composite` renderer runs
- THEN the first 3 blocks are rendered; blocks 4 and 5 are ignored (not rendered as raw JSON, not appended)
- AND any block whose shape is unknown to its renderer surfaces `UnsupportedRendererState` rather than silently serialising JSON

#### Scenario: Vega-lite renderer surfaces UnsupportedRendererState (no silent JSON)

- GIVEN the `vega-embed` package is not installed OR the spec is unsupported
- WHEN the `vega_lite` renderer runs
- THEN `UnsupportedRendererState` renders with `rendererKind = "vega_lite"` and `reason = "vega_embed_unavailable"`
- AND no silent JSON serialisation occurs
- AND the renderer MUST NOT degrade to raw JSON

### Requirement: 3. ViewSpecRenderable — the `RendererRegistry`

This is the registry entry point used by `ViewBlock`.

`ViewBlock` MUST change from a hand-rolled `switch` to a registry lookup:

```tsx
const renderer = rendererRegistry.get(view.renderer_kind);
if (renderer) {
  return <>{renderer({ view, onSelectObject })}</>;
}
// Fallback: render an explicit UnsupportedRendererState instead of
// silently serialising JSON. The UnknownBlockView component is
// preserved bit-for-bit for the Phase 3 test suite, but the
// registry fallback path now renders the explicit state.
return <UnsupportedRendererState rendererKind={view.renderer_kind} />;
```

The lookup runs once per block, not once per render: the registry MUST be read inside the function component (no `useMemo` is required — `Map.get` is O(1)). `UnknownBlockView` MUST be preserved bit-for-bit so the existing test suite (`ViewBlock.test.tsx`) keeps passing; the explicit `UnsupportedRendererState` is a new component living alongside it.

(Previously: unknown renderers silently fell back to `UnknownBlockView` (raw JSON). The explicit `UnsupportedRendererState` replaces that behaviour on the registry path.)

#### Scenario: Known renderer dispatched via registry

- GIVEN a block with `id = "callers"` and `view_kind = CallGraph` (the existing callgraph builder is wired to the `graph` renderer)
- WHEN `ViewBlock` renders
- THEN the `graph` renderer is called and the cytoscape panel appears

#### Scenario: Unknown renderer surfaces UnsupportedRendererState

- GIVEN a `ViewBlock` whose `view.renderer_kind` is a `Custom("future_ai_view")` and the registry has no entry
- WHEN `ViewBlock` renders
- THEN an `UnsupportedRendererState` component renders with the offending `renderer_kind` shown in the message
- AND no silent JSON serialisation occurs on the registry path

#### Scenario: Existing test suite passes

- GIVEN the test file `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.test.tsx`
- WHEN `vitest run ViewBlock` runs after the migration
- THEN every existing test passes; the only permitted test signature change is adding a new test for the explicit unsupported state

### Requirement: 4. `RendererKind` is carried on the wire

The `ContextualView` DTO MUST carry a required `renderer_kind: RendererKind`
field in the E29.3 wire version. A versioned `LegacyRendererAdapter` MAY accept
pre-E29.3 payloads that omit the field and MUST derive it only from the explicit
legacy `view_id` mapping (`overview` → `json`, `call-graph` → `graph`, `source`
→ `code`, quality → `table`). An unversioned payload, or a legacy payload with
an unknown `view_id`, MUST produce `UnsupportedRendererState`; it MUST NOT
default to JSON. New E29.3 responses MUST NOT use `#[serde(default)]` for this
field.

#### Scenario: Phase 1 service maps view_id → renderer_kind

- GIVEN `build_callgraph` returns a `ContextualView` with `view_id = "call-graph"`
- WHEN the service serialises the response
- THEN `renderer_kind` is `"graph"`
- AND no existing field is removed from the wire

#### Scenario: Known legacy payload uses the versioned adapter

- GIVEN a versioned legacy payload with `view_id = "call-graph"` and no `renderer_kind`
- WHEN `LegacyRendererAdapter` handles it
- THEN the adapted value carries `renderer_kind = "graph"`

#### Scenario: Unknown missing renderer_kind is explicit

- GIVEN an unversioned payload or unknown legacy `view_id` with no `renderer_kind`
- WHEN renderer dispatch validates it
- THEN `UnsupportedRendererState` is returned
- AND the payload is not rendered as JSON

### Requirement: 5. Hook update is additive

`useAvailableViews` (in `apps/explorer-ui/src/hooks/useViews.ts`) MUST keep its existing return type `ViewList = ViewDescriptor[]`. The hook MUST NOT expose `view_kind` or `renderer_kind` on the wire until the follow-up spec. This keeps the Phase 1 UI surface byte-compatible with today's `ViewTabs` and `ObjectInspector`.

#### Scenario: Existing useAvailableViews contract

- GIVEN a symbol `S`
- WHEN `useAvailableViews(S).data` is read after Phase 1
- THEN the array contains 4 entries with the same `{ id, title }` shape as before; the hook signature is unchanged

## ADDED Requirements

### Requirement: Renderer registry is the sole normal dispatch path

`PaneInspector` MUST dispatch panes using `rendererRegistry.get(view.renderer_kind)` and MUST NOT consult `view_kind` on the normal path. The legacy `view_kind` switch (the `isGraphViewKind` predicate) MUST be removed once the `runtime_renderer_dispatch` flag has been `true` in production for at least one release. While the flag exists, the legacy path is **emergency rollback only** and MUST NOT silently preserve itself as the normal behaviour. Diagnostic output MUST identify which dispatch path is active.

#### Scenario: Registry path is the normal dispatch

- GIVEN `runtime_renderer_dispatch = true` (the normal production state)
- WHEN `PaneInspector` receives a `ContextualView` with `renderer_kind = "table"`
- THEN `rendererRegistry.get("table")` runs and renders the pane
- AND no `isGraphViewKind` fallback runs

#### Scenario: Diagnostics report active dispatch path

- GIVEN `runtime_renderer_dispatch = true`
- WHEN `GET /api/diagnostics/runtime_flags` runs (or its equivalent)
- THEN the response names the active dispatch path as `renderer_kind`
- AND identifies the `runtime_renderer_dispatch` flag value

## Out of Scope (Phase 3 — explicit non-requirements)

- Registering renderers at runtime for user-defined `ViewSpec.renderer_kind`s — Phase 4
- Composing multiple renderers side-by-side in a single view — Phase 4
- JSONata execution in Rust — out of E29.3; persisted transforms run in the sandboxed frontend before registry dispatch
- Re-binding a `RendererKind` to a remote / Module Federation component — out of v1 scope (ADR-008 §Alternatives)
- A second hand-rolled renderer switch — forbidden; every shipped built-in MUST register before sole dispatch is enabled
- Silent JSON fallback for unknown / missing renderers — explicitly forbidden; `UnsupportedRendererState` is the only allowed response

## Coverage

- **Happy paths**: covered (registry lookup, all built-in kinds register, renderer_kind wire field, ViewBlock switch replaced)
- **Edge cases**: covered (unknown renderer state, versioned legacy adapter, duplicate register, no `vega-embed`, no `mermaid` package)
- **Error states**: covered (no panic on unknown kind, console warn on duplicate, test suite unchanged, no silent JSON fallback)
