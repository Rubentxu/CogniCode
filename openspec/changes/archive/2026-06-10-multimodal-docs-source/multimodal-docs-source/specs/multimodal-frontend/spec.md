# multimodal-frontend Specification (NEW)

## Purpose

The frontend gains 4 new Cytoscape node shapes and 4 new edge visual styles so multimodal nodes (Decision, Doc, Issue, Evidence) are visually distinguishable from code symbols in the InteractiveGraph component. The ObjectInspector learns to render multimodal fields. Style classes are kept as a strict Zod enum, and unknown classes fall back to the existing `console.warn` behavior.

## Files

| File | Change |
|------|--------|
| `apps/explorer-ui/src/api/schemas.ts` | Extend `GraphNodeStyleClass` and `GraphEdgeStyleClass` Zod enums |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Add 4 node styles + 4 edge styles |
| `apps/explorer-ui/src/components/ObjectInspector/ObjectInspector.tsx` | Render multimodal `kind` fields |
| `apps/explorer-ui/src/lib/styleClass.ts` | Add the new style class mappings |

## Requirements

### Requirement: GraphNodeStyleClass Extension

The Zod enum `GraphNodeStyleClass` in `apps/explorer-ui/src/api/schemas.ts` MUST be extended from `z.enum(["function", "module", "external"])` to `z.enum(["function", "module", "external", "decision", "doc", "issue", "evidence"])`. The 3 existing variants MUST remain (backward compatibility). The new variants MUST map to Cytoscape shapes and colors per the table below.

| Style Class | Shape | Background Color | Border Color | Border Width | Size |
|-------------|-------|------------------|--------------|--------------|------|
| `function` | round-rectangle | `#5B8FF9` | `#344A6E` | 1 | 40 |
| `module` | rectangle | `#F6BD16` | `#A06A00` | 1 | 60 |
| `external` | round-pentagon | `#E8684A` | `#9D3B22` | 1 | 40 |
| `decision` | diamond | `#A078FF` | `#5C3FB8` | 2 | 50 |
| `doc` | round-octagon | `#5AD8A6` | `#2A8C66` | 1 | 45 |
| `issue` | triangle | `#F4664A` | `#A82D14` | 1 | 40 |
| `evidence` | ellipse | `#9FE5D8` | `#4A9B8E` | 1 | 35 |

#### Scenario: All 7 style classes parse
- GIVEN the new Zod enum definition
- WHEN `GraphNodeStyleClass.parse("decision")`, `... .parse("doc")`, etc. are called for all 7 values
- THEN each MUST return `Ok`
- AND an unknown value (e.g., `"wibble"`) MUST return `ZodError`

#### Scenario: Existing style classes still validate
- GIVEN the extended enum
- WHEN a legacy node DTO arrives with `style_class: "function"`
- THEN it MUST parse cleanly (no breaking change)

### Requirement: GraphEdgeStyleClass Extension

The Zod enum `GraphEdgeStyleClass` in `apps/explorer-ui/src/api/schemas.ts` MUST be extended from `z.enum(["edge.calls", "edge.implements", "edge.uses"])` to `z.enum(["edge.calls", "edge.implements", "edge.uses", "edge.cites", "edge.justifies", "edge.resolves", "edge.corroborated_by"])`. The 3 existing variants MUST remain. New edge styles MUST use distinct line patterns.

| Style Class | Line Style | Color | Width | Curve Style | Target Arrow Shape |
|-------------|-----------|-------|-------|-------------|-------------------|
| `edge.calls` | solid | `#5B8FF9` | 1.5 | bezier | triangle |
| `edge.implements` | solid | `#F6BD16` | 2 | bezier | triangle |
| `edge.uses` | dashed | `#E8684A` | 1 | bezier | triangle |
| `edge.cites` | dotted | `#5AD8A6` | 1 | bezier | triangle |
| `edge.justifies` | solid | `#A078FF` | 2.5 | bezier | diamond |
| `edge.resolves` | solid | `#4A9B8E` | 2 | bezier | triangle-backcurve |
| `edge.corroborated_by` | dashed | `#9FE5D8` | 1 | bezier | none (bidirectional via source arrow) |

#### Scenario: All 7 edge classes parse
- GIVEN the extended enum
- WHEN each of the 7 values is parsed
- THEN each MUST return `Ok`
- AND unknown values MUST return `ZodError`

#### Scenario: Edge stylesheet entries exist
- GIVEN the stylesheet.ts
- WHEN searched for entries with `selector: 'edge[style_class = "edge.cites"]'`
- THEN exactly one style block exists with `style: "line-style": "dotted"`

### Requirement: Cytoscape Stylesheet Completeness

For each of the 4 new node and 4 new edge style classes, `stylesheet.ts` MUST contain exactly one matching style block. Unknown style classes MUST continue to produce a single `console.warn` per build (existing behavior — do not change).

#### Scenario: Every new node style has a stylesheet block
- GIVEN the 4 new style classes (decision, doc, issue, evidence)
- WHEN the stylesheet is loaded
- THEN a regex search for `'node[style_class = "decision"]'`, `'node[style_class = "doc"]'`, etc. returns 1 match each

#### Scenario: Unknown class logs warning
- GIVEN a node arrives with `style_class: "wibble"`
- WHEN the graph renders
- THEN `console.warn` is called once with `"unknown style class: wibble"` (or similar) and the node falls back to the default `function` style

### Requirement: ObjectInspector Multimodal Fields

The `ObjectInspector` component MUST render multimodal node kinds distinctly. When the inspected object has `kind: "decision" | "doc" | "issue" | "evidence"`, the inspector MUST:

- Display a colored badge matching the node style color.
- Show a "Citations" section listing outbound `Cites` edges.
- Show a "Provenance" section with the edge's `(provenance, confidence)` pairs.
- Render metadata as a key-value table for `Decision` nodes (status, date) and `Doc` nodes (section, line).

For `Symbol` nodes, the existing behavior is preserved.

#### Scenario: Decision node inspector
- GIVEN a node `{ kind: "decision", label: "ADR-0001", metadata: { status: "accepted", date: "2026-01-15" } }`
- WHEN inspected
- THEN a purple badge with "Decision" is rendered
- AND the metadata table shows `status: accepted` and `date: 2026-01-15`

#### Scenario: Doc node inspector with citations
- GIVEN a Doc node with 3 outgoing `edge.cites` edges
- WHEN inspected
- THEN a "Citations" section lists 3 entries with target labels and `confidence` values

#### Scenario: Symbol node inspector unchanged
- GIVEN a Symbol node `{ kind: "symbol" }` (the existing kind)
- WHEN inspected
- THEN the legacy symbol view renders (no badge, no metadata table)

### Requirement: Backend `style_class_for` and `edge_style_class_for` Mapping

`crates/cognicode-explorer/src/api.rs` MUST extend the `style_class_for` and `edge_style_class_for` functions to map the new `NodeKind` and `EdgeKind` variants. The new mappings MUST match the frontend enum strings.

| Rust variant | Frontend style class |
|--------------|----------------------|
| `NodeKind::Decision` | `decision` |
| `NodeKind::Doc` | `doc` |
| `NodeKind::Issue` | `issue` |
| `NodeKind::Evidence` | `evidence` |
| `EdgeKind::Cites` | `edge.cites` |
| `EdgeKind::Justifies` | `edge.justifies` |
| `EdgeKind::Resolves` | `edge.resolves` |
| `EdgeKind::CorroboratedBy` | `edge.corroborated_by` |
| `EdgeKind::Dependency(DependencyType::Calls)` | `edge.calls` |
| `EdgeKind::Dependency(DependencyType::Inherits)` | `edge.implements` |
| `EdgeKind::Dependency(_)` (other 6 variants) | `edge.uses` |

#### Scenario: New NodeKind variants map correctly
- GIVEN a `GraphNode` with `kind: NodeKind::Doc`
- WHEN `style_class_for(&node)` is called
- THEN the returned string MUST be `"doc"` (matches the frontend enum)

#### Scenario: New EdgeKind variants map correctly
- GIVEN a `GraphEdge` with `kind: EdgeKind::Justifies`
- WHEN `edge_style_class_for(&edge)` is called
- THEN the returned string MUST be `"edge.justifies"`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| A node has `kind: NodeKind::Symbol(SymbolKind::Function)` AND an unrecognized `SymbolKind` sub-variant | Falls through to the default symbol mapping (`function`); no panic |
| A `Doc` node has 100+ outgoing `Cites` edges | ObjectInspector paginates after 50; shows "load more" |
| Two adjacent nodes share the same style class | Visual collision; Cytoscape handles overlap via its built-in layout engine (no special handling) |
| A node's `metadata` is malformed JSON (e.g., wrong type for a field) | ObjectInspector renders the raw value; no crash |
| A node's `style_class` field is empty string | `console.warn` + fallback to `function` style (existing behavior) |
| Cytoscape.js fails to load the stylesheet | Frontend surfaces a global "graph render error" toast; no infinite re-render |
| A SymbolKind variant is added in a future change but no frontend style exists | Backend maps to `function`; frontend `console.warn` once per render; no schema break |
| A multimodal node is rendered before the Cytoscape stylesheet is loaded | Deferred render — component waits for the stylesheet to be ready (no flash of unstyled content) |

## Out of Scope

- New interactive graph features (clustering, minimap, timeline scrubber)
- Dark mode adjustments to the new color palette (handled in a follow-up)
- Internationalization of the badge labels (English only for now)
- Animation/transition effects on new node kinds
- Tooltip customization for multimodal nodes (uses the existing tooltip renderer)
- Re-skinning the existing `function`/`module`/`external` styles

## TDD RED Gate

Before any implementation, the following tests MUST exist and be RED:

1. `GraphNodeStyleClass.parse` — 7 OK cases + 1 error case (8 tests)
2. `GraphEdgeStyleClass.parse` — 7 OK cases + 1 error case (8 tests)
3. `styleClassFor` (backend) — one test per new `NodeKind` variant (4 tests)
4. `edgeStyleClassFor` (backend) — one test per new `EdgeKind` variant (4 tests)
5. `ObjectInspector` snapshot test — 4 fixtures (Decision, Doc, Issue, Evidence) with Playwright
6. Cytoscape stylesheet loader test — confirm 7+7 style blocks are registered
7. `console.warn` regression test — unknown class still warns once

## Dependencies

- `generic-graph-model` (provides `NodeKind`, `EdgeKind` variants the frontend maps)
- `docs-source-adapter` (drives Doc/Decision node creation; frontend just renders)
- `explorerql-targets` (consumers of `TargetType::Decisions` etc. land in the graph view; the frontend must render them)
- Existing Zod schema pattern (no new validation library)
- Existing `console.warn` fallback pattern (do not change)
