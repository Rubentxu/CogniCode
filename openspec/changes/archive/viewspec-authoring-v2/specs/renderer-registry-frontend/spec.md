# Delta for RendererRegistry Frontend

This delta extends the `renderer-registry-frontend` skeleton so saved runtime `ViewSpec`s appear in `ViewTabs` and can be opened in the Explorer via the existing `executeViewSpec` flow.

## ADDED Requirements

### Requirement: 6. Runtime views in ViewTabs

`useAvailableViews` MUST merge runtime `ViewSpec`s from `listViewSpecs()` into the available-views list. Runtime views MUST appear after built-ins (alphabetical-by-title), and MUST render with a `data-source="runtime"` and a "custom" badge in `ViewTabs`. The badge is permanent — not dismissible — so the user always knows the view is user-defined, not compiled-in.

#### Scenario: Runtime view appears in ViewTabs

- GIVEN a saved `ViewSpec` `{ id: "V", title: "Hot Symbols", view_kind: "quality_hotspots", applies_to: "Symbol", owner: "alice" }`
- WHEN the user inspects a `Symbol` object
- THEN `ViewTabs` shows the 4 built-in views plus a 5th tab titled "Hot Symbols" with a "custom" badge

#### Scenario: Runtime view opens in a new pane

- GIVEN the user is on a `Symbol` pane with 4 built-in tabs
- WHEN they click the "Hot Symbols" runtime tab
- THEN a new pane opens to the right (not replacing the current one) with the spec's view rendered via `executeViewSpec`

### Requirement: 7. Runtime `renderer_kind` resolution

When `ViewBlock` looks up a renderer for a `ViewSpec` whose `renderer_kind` is a built-in (e.g. `table`, `graph`, `code`), the existing `RendererRegistry` entry is used. When the `renderer_kind` is `Custom("...")` or unknown, the runtime falls back to the `json` renderer (the existing `UnknownBlockView` behaviour from Phase 3).

#### Scenario: Built-in renderer reused for runtime view

- GIVEN a runtime `ViewSpec` with `renderer_kind: "table"`
- WHEN the spec executes and returns blocks
- THEN the `table` renderer from `RendererRegistry` renders the blocks

#### Scenario: Unknown runtime renderer falls back to JSON

- GIVEN a runtime `ViewSpec` with `renderer_kind: { kind: "custom", value: "future_chart" }`
- WHEN the spec executes and returns blocks
- THEN the `json` fallback renderer (former `UnknownBlockView`) renders the block body

## MODIFIED Requirements

### Requirement: 5. Hook update merges runtime views

`useAvailableViews` (in `apps/explorer-ui/src/hooks/useViews.ts`) MUST return a `ViewList = ViewDescriptor[]` that is the union of built-in descriptors and runtime `ViewSpec`s. Runtime entries MUST carry `is_builtin: false` and a stable `source: "runtime"` discriminator. The hook signature stays `ViewList` — no new fields are added to the public type; the new fields live on the runtime entries only.

(Previously: the hook returned built-in descriptors only; runtime views were invisible to `ViewTabs`.)

#### Scenario: useAvailableViews returns built-ins + runtime

- GIVEN a symbol `S` with 4 built-in views and 2 runtime `ViewSpec`s for `Symbol`
- WHEN `useAvailableViews(S).data` is read
- THEN the array has 6 entries; the first 4 are built-ins (id-ordered); the last 2 are runtime entries with `source: "runtime"` and `is_builtin: false`

#### Scenario: SWR revalidation refreshes runtime views

- GIVEN a runtime view was just saved via the wizard
- WHEN SWR revalidates `listViewSpecs()`
- THEN the new view appears in `ViewTabs` for the matching `applies_to` object without a page reload
