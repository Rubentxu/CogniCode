# Spec: interactive-graph

> New capability. Companion to proposal `sdd/visualization-stack/proposal`.
> Replaces the read-only `SvgGraph` for graphs with >50 nodes; `SvgGraph` remains
> the fallback and is NOT removed by this change.

## Purpose

An interactive React component (`InteractiveGraph`) that wraps Cytoscape.js and
renders large call / dependency / impact graphs with pan, zoom, node selection,
and style-class driven styling. The component is the visual surface for
`graph-data-endpoint` data; it consumes `elements` (Cytoscape node/edge JSON)
and dispatches `onSelectObject(id)` to the parent on node selection. Layout is
delegated to `elkjs-layout` running in a Web Worker — `InteractiveGraph` does
NOT compute positions inline.

## Requirements

### Requirement 1: Component surface

`InteractiveGraph` MUST be a default-exported React component at
`apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx` with
the following props (TypeScript):

| Prop          | Type                                              | Required | Meaning                                              |
|---------------|---------------------------------------------------|----------|------------------------------------------------------|
| `elements`    | `Cytoscape.ElementDefinition[]`                   | yes      | Nodes + edges to render (from `graph-data-endpoint`) |
| `selectedId`  | `string \| null`                                  | no       | Currently selected node id (controlled)              |
| `onSelectObject` | `(id: string) => void`                         | no       | Dispatched on node click / keyboard activation       |
| `layout`      | `"layered" \| "force" \| "radial"`                | no       | Layout algorithm (default `"layered"`)               |
| `ariaLabel`   | `string`                                          | no       | Accessible name for the graph region                 |
| `className`   | `string`                                          | no       | Class passthrough                                    |

The component MUST be wrapped with `React.lazy` when imported from
`Shell.tsx` (Cytoscape.js is ~200KB and must not block initial paint).
The component MUST accept `null`/`undefined` `elements` by rendering an empty
`<div data-testid="interactive-graph-empty">` with a visible "No graph data"
message — it MUST NOT throw.

#### Scenario: Renders an empty-state when elements is null

- GIVEN `<InteractiveGraph elements={null} />`
- WHEN mounted
- THEN `getByTestId("interactive-graph-empty")` is in the document AND no
  `<canvas>` (Cytoscape mount target) is present

#### Scenario: Mounts a Cytoscape canvas when elements are provided

- GIVEN `<InteractiveGraph elements={[{data:{id:"a"}},{data:{id:"b"}},{data:{id:"a",source:"a",target:"b"}}] } />`
- WHEN mounted
- THEN `getByTestId("interactive-graph-canvas")` is in the document AND
  the cytoscape instance has `nodes().length === 2` AND `edges().length === 1`

#### Scenario: Lazy-load boundary is in place

- GIVEN `Shell.tsx`
- WHEN the `InteractiveGraph` import is inspected
- THEN it is wrapped in `React.lazy(...)` AND a `Suspense` fallback exists
  in `Shell.tsx` (verified by a test asserting `lazy` is used)

### Requirement 2: Node/edge rendering and style classes

Every node in the rendered graph MUST carry a `style_class` field (from the
`graph-data-endpoint` DTO) mapped to a Cytoscape selector of the form
`node.style_class_<value>`. Edge classes MUST follow the same convention
(`edge.style_class_<value>`). The component MUST register a Cytoscape stylesheet
that, at minimum, distinguishes:

| style_class | Visual                                          |
|-------------|-------------------------------------------------|
| `function`  | Solid blue rectangle, label = node `label`      |
| `module`    | Orange rounded rectangle                        |
| `external`  | Grey dashed border, italic label                |
| `edge.calls` | Solid arrow, weight 1.0                        |
| `edge.implements` | Dashed arrow, weight 1.0                  |
| `edge.uses`  | Dotted arrow, weight 0.8                       |
| `selected`   | Highlight ring (yellow, 3px) — applied via class on the selected node, NOT a separate style_class |

If an element has an unknown `style_class`, the component MUST fall back to
the `function` visual AND log a `console.warn` with the unknown class name.

#### Scenario: Known style_class produces the documented visual

- GIVEN an element `{data:{id:"f", style_class:"module"}}`
- WHEN the cytoscape stylesheet is applied
- THEN the rendered node has the `module` shape/colour (asserted via
  `cy.$('#f').style('shape')` returning `'round-rectangle'` in test)

#### Scenario: Unknown style_class falls back to function visual

- GIVEN an element `{data:{id:"x", style_class:"alien-thing"}}`
- WHEN the component mounts
- THEN a `console.warn` was called with the string `"alien-thing"` AND
  `cy.$('#x').style('shape')` is the default `function` shape (rectangle)

#### Scenario: Edge style classes distinguish calls / implements / uses

- GIVEN three edges with `style_class` `"edge.calls"`, `"edge.implements"`,
  `"edge.uses"`
- WHEN the stylesheet is applied
- THEN `cy.$edges()[i].style('line-style')` is respectively `"solid"`,
  `"dashed"`, `"dotted"` AND `cy.$edges()[i].style('width')` is 1.0 / 1.0 / 0.8

### Requirement 3: Selection and edge highlighting

Clicking a node MUST dispatch `onSelectObject(id)` exactly once with the
node's `data.id`. Pressing `Enter` or `Space` on a focused node MUST also
dispatch `onSelectObject(id)`. When `selectedId` is provided, the matching
node MUST receive the `selected` style class AND every edge incident to it
MUST receive a `highlighted` class; non-incident edges MUST receive a
`dimmed` class. When `selectedId` is `null` or changes, all `highlighted` /
`dimmed` classes MUST be removed.

#### Scenario: Click dispatches onSelectObject once with the node id

- GIVEN a rendered graph with node `n1`
- WHEN the user clicks `n1`
- THEN a single `onSelectObject` call is observed with argument `"n1"`

#### Scenario: Keyboard activation dispatches onSelectObject

- GIVEN a rendered graph with node `n2` focused
- WHEN the user presses `Enter`
- THEN `onSelectObject` is called once with `"n2"`

#### Scenario: Selected node and its incident edges are highlighted

- GIVEN `<InteractiveGraph elements={...} selectedId="A" />` with edges
  `A→B`, `A→C`, `D→E`
- WHEN the cytoscape classes are inspected
- THEN `cy.$('#A').hasClass('selected')` is `true` AND edges `A→B` and
  `A→C` have class `highlighted` AND edge `D→E` has class `dimmed` AND
  nodes `B`, `C`, `D`, `E` have class `dimmed`

#### Scenario: Clearing selectedId removes all highlight/dim classes

- GIVEN the previous scenario
- WHEN `selectedId` changes to `null`
- THEN no node has class `selected` AND no edge has class `highlighted` or
  `dimmed` AND no node has class `dimmed`

### Requirement 4: Accessibility

The component MUST expose the graph region with `role="application"` (Cytoscape
canvas) AND a sibling `role="complementary"` region that contains an
off-screen `<table>` listing every node id and label (screen-reader fallback,
same pattern as `SvgGraph`). The region MUST have an `aria-label` derived from
the `ariaLabel` prop or the default string `"Interactive graph"`. Every node
MUST be reachable by `Tab` and MUST be activatable by `Enter` / `Space`.

#### Scenario: Graph region has role=application and an accessible name

- GIVEN `<InteractiveGraph ariaLabel="Call graph of foo" elements={...} />`
- WHEN mounted
- THEN a region with `role="application"` and `aria-label="Call graph of foo"`
  is in the document AND a `role="complementary"` fallback with the same
  accessible name is in the document

#### Scenario: Off-screen table lists every node

- GIVEN a graph with 3 nodes labelled `"a"`, `"b"`, `"c"`
- WHEN the fallback table is queried
- THEN a row exists for each label `a`, `b`, `c` (by `getByText`)

#### Scenario: Nodes are keyboard reachable

- GIVEN a graph with 2 nodes
- WHEN the user presses `Tab` repeatedly
- THEN focus eventually reaches the first node (asserted via
  `getByRole('button', {name: /Label of node 1/})` or a `data-testid`
  marker on the cy container's tabbable children)

## Acceptance Criteria

| #   | Criterion                                                              | Verifies |
| --- | ---------------------------------------------------------------------- | -------- |
| AC1 | `InteractiveGraph.tsx` exists, is `React.lazy`-imported from `Shell`   | R1       |
| AC2 | Renders ≥200 nodes without exceeding 30fps panning (Playwright trace)   | R1, R2   |
| AC3 | Unknown `style_class` falls back to `function` visual + warn           | R2       |
| AC4 | Selection propagates `selected` / `highlighted` / `dimmed` classes     | R3       |
| AC5 | `ariaLabel` prop + fallback table mirror `SvgGraph` accessibility      | R4       |
| AC6 | Empty-state renders without throwing for `null` / `undefined` elements | R1       |

## Edge Cases (exhaustive — all MUST have ≥1 test)

| ID  | Case                                  | Expected behavior                                  |
| --- | ------------------------------------- | -------------------------------------------------- |
| E1  | `elements === null`                   | Empty-state, no canvas, no throw                   |
| E2  | `elements === []`                     | Empty-state, no canvas                             |
| E3  | Single node, no edges                 | Render one node, no error, layout does not crash   |
| E4  | Self-loop edge                        | Rendered; one node + one edge; selection highlights both endpoints |
| E5  | Duplicate node ids in input           | Cytoscape dedupes; warn emitted with the id        |
| E6  | Edge referencing unknown source/target | Edge is dropped; `console.warn` with both ids      |
| E7  | Unknown `style_class`                 | Fallback visual + warn                             |
| E8  | `selectedId` does not match any node  | No `selected` class is applied; no error           |
| E9  | Rapid `selectedId` change (5 in 100ms) | Final state matches final value; no stale classes  |
| E10 | Component unmount during layout       | Cytoscape instance `.destroy()` is called (no leak warning) |
| E11 | `ariaLabel` prop omitted              | Region uses default `"Interactive graph"`          |
| E12 | Web Worker layout fails (worker error)| Component renders un-laid-out graph + `console.error`; does not crash |

## TDD RED Gate

Before implementation is considered started, the following tests MUST exist
and FAIL (RED). Each test MUST be mapped to a numbered requirement.

| Test file                                                        | Requirement | Status   |
|------------------------------------------------------------------|-------------|----------|
| `InteractiveGraph.test.tsx::empty state`                        | R1          | RED      |
| `InteractiveGraph.test.tsx::mounts cytoscape canvas`             | R1          | RED      |
| `InteractiveGraph.test.tsx::lazy boundary in Shell`              | R1          | RED      |
| `InteractiveGraph.test.tsx::known style_class shapes`            | R2          | RED      |
| `InteractiveGraph.test.tsx::unknown style_class falls back`      | R2          | RED      |
| `InteractiveGraph.test.tsx::edge style classes`                  | R2          | RED      |
| `InteractiveGraph.test.tsx::click dispatches onSelectObject`     | R3          | RED      |
| `InteractiveGraph.test.tsx::keyboard activation`                 | R3          | RED      |
| `InteractiveGraph.test.tsx::selection highlights edges`          | R3          | RED      |
| `InteractiveGraph.test.tsx::clearing selectedId removes classes` | R3          | RED      |
| `InteractiveGraph.test.tsx::region aria-label`                   | R4          | RED      |
| `InteractiveGraph.test.tsx::fallback table lists nodes`          | R4          | RED      |

## Out of Scope (locked)

- D3.js analytics overlays (heatmaps, histograms) — deferred to a later change
- Server-side layout (computation lives in elkjs Web Worker)
- Named views, ExplorerQL autocomplete, C4 projections
- Mermaid export of the rendered graph
- Persistence of pan/zoom state across reloads
- Right-click context menus for nodes (deferred to UX iteration)
- Touch / gesture support beyond what Cytoscape provides out of the box
