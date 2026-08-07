# corroboration-styling Specification (NEW)

## Purpose

Cytoscape style rules that visually encode **corroboration** for edges and nodes in the rationale sub-graph. Edge thickness is proportional to the score, color intensity increases with the count of independent sources, and a `confidence-band` class is applied to the focus node based on the average confidence of incident edges. These styles are **additive** to the existing `multimodal-frontend` stylesheet — they do not modify any of the 7 existing node or 7 existing edge style classes. The styles are loaded only by the `RationaleView` (lazy-loaded with the component) and do not affect other graph views.

## Domain

| Term | Definition |
|------|------------|
| Score band | A discrete bucket for the `0.0..=1.0` score: `weak (0.0..0.4)`, `medium (0.4..0.7)`, `strong (0.7..1.0]`. |
| Source-count band | Discrete bucket for the count of distinct `provenance` values incident to a target: `solo (1)`, `paired (2)`, `corroborated (3+)`. |
| Edge width formula | `width = lerp(1, 6, score)` rounded to 1 decimal place. Width 1 for `score = 0`, width 6 for `score = 1.0`. |
| Edge color intensity | Edges with `source-count >= 3` use the bright variant; `2` use the regular variant; `1` use the dim variant. |
| Focus confidence band | Average `confidence` of all edges incident to the focus, bucketed as `low (0.0..0.4)`, `med (0.4..0.7)`, `high (0.7..1.0]`. Applied as a class on the focus node itself. |

## Files

| File | Change |
|------|--------|
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.ts` | Add 3 edge score-band classes + 3 source-count classes + 3 focus confidence-band classes + 1 helper function |
| `apps/explorer-ui/src/components/InteractiveGraph/stylesheet.test.ts` | +6 RED tests for the new classes |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.ts` | Emit `data.score`, `data.source_count`, `data.confidence_band` on edges and focus node |
| `apps/explorer-ui/src/components/InteractiveGraph/adapter.test.ts` | +4 RED tests for the new data fields |

## Requirements

### Requirement: Edge width proportional to score

`stylesheet.ts` MUST contain three Cytoscape style rules keyed by `[style_class = "edge.justifies"][score_band = "..."]`:

| `score_band` | Width | Color override |
|--------------|-------|----------------|
| `score_weak` | 1.0 | inherit base `edge.justifies` color |
| `score_medium` | 2.5 | inherit base color |
| `score_strong` | 5.0 | inherit base color |

The width MUST be exact (no interpolation at the style layer — the adapter pre-computes the band from the score and emits the discrete class). A score of `0.0` MUST match `score_weak` and yield width `1.0`. A score of `1.0` MUST match `score_strong` and yield width `5.0`. The width MUST override the base `edge.justifies` width (`2.5`) when the band selector matches.

#### Scenario: Score 0.0 → score_weak → width 1.0

- GIVEN a style block matching `edge.justifies` with `score_band = "score_weak"`
- WHEN the cytoscape style is loaded
- THEN a rule exists with `width: 1.0` AND it is listed AFTER the base `edge.justifies` rule (override order)

#### Scenario: Score 0.5 → score_medium → width 2.5

- GIVEN a style block with `score_band = "score_medium"`
- WHEN the cytoscape style is loaded
- THEN a rule exists with `width: 2.5`

#### Scenario: Score 1.0 → score_strong → width 5.0

- GIVEN a style block with `score_band = "score_strong"`
- WHEN the cytoscape style is loaded
- THEN a rule exists with `width: 5.0`

#### Scenario: Override order

- GIVEN the stylesheet
- WHEN rules are enumerated in array order
- THEN base `edge.justifies` rule precedes the 3 score-band rules AND the score-band rules precede any `edge.corroborated_by` rules

### Requirement: Source-count color intensity

`stylesheet.ts` MUST contain three Cytoscape style rules keyed by `[source_count_band = "..."]`:

| `source_count_band` | Line color (hex) |
|---------------------|------------------|
| `sources_solo` | base color × 0.6 (dim) |
| `sources_paired` | base color × 0.85 (regular) |
| `sources_corroborated` | base color (bright, full saturation) |

The color MUST be set via Cytoscape's `line-color` property, not via inline `style`. The bands MUST stack with the score-band selectors: an edge with `score_band = "score_strong"` AND `source_count_band = "sources_corroborated"` MUST show the bright color and the strong width.

#### Scenario: sources_solo yields dim color

- GIVEN a style block with `source_count_band = "sources_solo"`
- WHEN the cytoscape style is loaded
- THEN `line-color` is set to a dim variant (alpha < 1.0 OR hex with reduced brightness vs base)

#### Scenario: sources_corroborated yields full color

- GIVEN a style block with `source_count_band = "sources_corroborated"`
- WHEN the cytoscape style is loaded
- THEN `line-color` is set to the full base color (no dim)

#### Scenario: Bands stack with score bands

- GIVEN an edge with both `score_band = "score_strong"` and `source_count_band = "sources_corroborated"`
- WHEN cytoscape resolves the style
- THEN the width is `5.0` AND the color is bright (multi-class selector match)

### Requirement: Focus confidence-band on the focus node

`stylesheet.ts` MUST contain three Cytoscape style rules keyed by `node[confidence_band = "..."]`:

| `confidence_band` | Border color (hex) | Border width |
|-------------------|--------------------|--------------|
| `confidence_low` | `#A82D14` (red-ish) | 2 |
| `confidence_medium` | `#F6BD16` (amber) | 2 |
| `confidence_high` | `#2A8C66` (teal) | 3 |

The border MUST override the focus node's base `style_class` border (e.g. `decision` has purple border `#5C3FB8` width 2). The focus band is set ONLY on the focus node by the adapter — other nodes never carry `confidence_band`.

#### Scenario: confidence_high yields teal border width 3

- GIVEN a style block with `confidence_band = "confidence_high"`
- WHEN the cytoscape style is loaded
- THEN `border-color: #2A8C66` AND `border-width: 3`

#### Scenario: confidence_low yields red border

- GIVEN a style block with `confidence_band = "confidence_low"`
- WHEN the cytoscape style is loaded
- THEN `border-color: #A82D14` AND `border-width: 2`

#### Scenario: Other nodes never carry confidence_band

- GIVEN a rationale sub-graph
- WHEN the adapter emits cytoscape elements
- THEN ONLY the focus node has `data.confidence_band` set AND every other node has `data.confidence_band === undefined`

### Requirement: Adapter emits score / source-count / confidence-band data

`toCytoscapeElements` in `adapter.ts` MUST, for each edge, compute the `score_band` (from the `corroboration_scores[edge.id]`) and the `source_count_band` (from the number of distinct `provenance` values on edges incident to the edge's target node) and set them as `data.score_band` and `data.source_count_band`. For the focus node (the first node in `nodes`), the adapter MUST compute `confidence_band` from the average `confidence` of incident edges and set it as `data.confidence_band`. The adapter MUST be a pure function — no cytoscape imports, no side effects.

#### Scenario: Edge score_band bucketed

- GIVEN an edge with `score = 0.2`
- WHEN `toCytoscapeElements` runs
- THEN `data.score_band === "score_weak"`

- GIVEN an edge with `score = 0.55`
- THEN `data.score_band === "score_medium"`

- GIVEN an edge with `score = 0.85`
- THEN `data.score_band === "score_strong"`

#### Scenario: source_count_band bucketed

- GIVEN a target with 1 incident edge (1 distinct prov)
- WHEN `toCytoscapeElements` runs
- THEN `data.source_count_band === "sources_solo"`

- GIVEN a target with 3 incident edges from 3 distinct prov values
- THEN `data.source_count_band === "sources_corroborated"`

#### Scenario: Focus confidence_band bucketed

- GIVEN the focus with 2 incident edges of confidence `0.5` and `0.7` (avg `0.6`)
- WHEN `toCytoscapeElements` runs
- THEN `data.confidence_band === "confidence_medium"`

#### Scenario: Non-focus nodes have no confidence_band

- GIVEN the focus has `confidence_band = "confidence_high"`
- WHEN `toCytoscapeElements` runs
- THEN every non-focus node has `data.confidence_band === undefined` (or absent)

### Requirement: Styles are lazy-loaded with `RationaleView`

The corroboration stylesheet MUST NOT be merged into the global `InteractiveGraph` stylesheet. They MUST live in a separate file (`corroboration.stylesheet.ts`) and be merged into the cytoscape instance at mount time, only when the `RationaleView` is rendered. When unmounted, the styles MUST be removed from the cytoscape instance. This ensures the bundle for other views (`ContextualPanel`, `SvgGraph`) is unaffected.

#### Scenario: Styles applied on mount

- GIVEN `RationaleView` mounts with a rationale sub-graph
- WHEN the cytoscape instance is initialized
- THEN `cy.style().fromJson(...).length >= 3` AND the corroboration classes are present in the active style

#### Scenario: Styles removed on unmount

- GIVEN `RationaleView` unmounts
- WHEN the cytoscape instance is torn down
- THEN the corroboration style entries are no longer in `cy.style()`

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `stylesheet_has_score_weak_rule` | `stylesheet.test.ts` | Width 1.0 |
| `stylesheet_has_score_medium_rule` | `stylesheet.test.ts` | Width 2.5 |
| `stylesheet_has_score_strong_rule` | `stylesheet.test.ts` | Width 5.0 |
| `stylesheet_override_order` | `stylesheet.test.ts` | Base before band rules |
| `stylesheet_has_sources_solo_dim_color` | `stylesheet.test.ts` | `line-color` is dim |
| `stylesheet_has_sources_corroborated_bright_color` | `stylesheet.test.ts` | `line-color` is full |
| `stylesheet_has_confidence_high_teal_border` | `stylesheet.test.ts` | `#2A8C66` width 3 |
| `adapter_emits_score_band_per_edge` | `adapter.test.ts` | `score_weak` / `_medium` / `_strong` |
| `adapter_emits_source_count_band` | `adapter.test.ts` | `sources_solo` / `_paired` / `_corroborated` |
| `adapter_emits_confidence_band_on_focus` | `adapter.test.ts` | Focus gets band, others don't |
| `rationale_view_applies_corroboration_styles` | `RationaleView.test.tsx` | `cy.style()` has corroboration rules |
| `rationale_view_removes_corroboration_styles_on_unmount` | `RationaleView.test.tsx` | Style rules absent after unmount |

## Out of Scope (locked)

- Animated edge width transitions (deferred — instant change is fine)
- Heatmap-style color scale (e.g., viridis) — discrete bands only in v1
- Per-provenance color coding (one base color per edge `style_class`)
- Score tooltip on hover (deferred — `ObjectInspector` shows raw score)
- Confidence band on non-focus nodes
- A11y fallback for color intensity (the focus band uses both color AND border width; the score band uses width only — this is intentional and locked)
