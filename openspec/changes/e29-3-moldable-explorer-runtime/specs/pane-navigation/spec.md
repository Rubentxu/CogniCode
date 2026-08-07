# Delta for Pane Navigation

This delta extends `pane-navigation` so pane dispatch is keyed by `renderer_kind` (not `view_kind`), every pane surfaces loading / empty / error / truncation / revision-change states, and the Inspector is responsive (320 / 768 / 1280) and WCAG AA contrast-compliant. **`renderer_kind` is the SOLE normal dispatch path**; `runtime_renderer_dispatch` defaults `on`. Setting it `off` is a temporary, explicit emergency rollback action. The legacy `isGraphViewKind` path is rollback-only and MUST be removed once the `rendererRegistry` is stable.

> **No MODIFIED Requirements:** the canonical `pane-navigation` spec (`openspec/specs/pane-navigation/spec.md`, requirements 1–6) is preserved unchanged by this change. All new behaviour below is recorded under `## ADDED Requirements`.

## ADDED Requirements

### Requirement: Pane dispatch keyed by `renderer_kind`

When the feature flag `runtime_renderer_dispatch` is `true`, opening a new pane MUST be driven by `ContextualView.renderer_kind` (not `view_kind`). The dispatch MUST consult `rendererRegistry` first and MUST fall back to `UnsupportedRendererState` when no renderer matches. When the flag is `false`, the legacy `isGraphViewKind` path remains mounted **as an emergency rollback path only**; it MUST NOT be presented as the normal dispatch behaviour in any UI copy, log, or diagnostic.

#### Scenario: Flag on routes by renderer_kind (normal path)

- GIVEN `runtime_renderer_dispatch = true` and a `ContextualView` with `renderer_kind = "graph"`
- WHEN the user drills into the underlying object
- THEN a new pane opens and `rendererRegistry.get("graph")` renders it
- AND no `isGraphViewKind` fallback runs

#### Scenario: Flag off uses legacy path (emergency rollback)

- GIVEN `runtime_renderer_dispatch = false` (operational rollback in effect) and the same `ContextualView`
- WHEN the user drills into the underlying object
- THEN a new pane opens via the legacy `isGraphViewKind` path
- AND a `tracing::warn!` is logged naming `runtime_renderer_dispatch=false` so operators can see the rollback is active

### Requirement: Pane states cover loading / empty / error / truncation / revision-change

Every active pane MUST render one `PaneExecutionState`. `empty`, `truncated`,
`error`, and `ready` map from `ContextualView.status`; `loading` is client-owned
while execute is in flight; `revision_change` is client-owned by comparing the
pane's retained pin with the latest workspace head:

| `status` | Pane rendering |
|----------|---------------|
| `loading` | Skeleton + spinner; no blocks visible |
| `empty` | "No data" message + the execute query (helpful for debugging) |
| `truncated` | Block list + visible truncation banner naming `truncation_reason` |
| `error` | Error card with `error.code` and `error.message`; no raw stack trace |
| `revision_change` | Banner showing `previous_revision_id → current_revision_id` with a "Reload" action |
| `ready` | Blocks rendered normally |

States MUST NOT silently collapse into each other; transitions MUST be observable in tests.

#### Scenario: Loading skeleton is observable

- GIVEN an in-flight execute call
- WHEN the pane polls
- THEN `data-testid = "pane-loading"` is visible
- AND no block content is rendered

#### Scenario: Revision-change banner surfaces both ids

- GIVEN the pane was rendered at `revision_id = R1`
- WHEN a new ingest creates `R2` and the pane re-renders
- THEN `data-testid = "pane-revision-change"` is visible
- AND it shows `R1 → R2` and a "Reload" button

#### Scenario: Error card hides stack trace

- GIVEN execute returns `status = "error"` with `error.code = "graph_executor_failure"`
- WHEN the pane renders
- THEN `data-testid = "pane-error"` is visible with `error.code` and `error.message`
- AND no stack trace, file path, or row id is shown

### Requirement: Responsive Inspector at 320 / 768 / 1280

The Inspector Pane Stack MUST be usable at viewports 320, 768, and 1280 px wide without horizontal scroll or clipped controls. Layout MUST be:
- 320 px: panes stack vertically; tabs collapse into an overflow menu; breadcrumb wraps to two lines.
- 768 px: two panes visible side-by-side; third pane pushes to overflow.
- 1280 px: up to four panes visible side-by-side; breadcrumb on one line.

#### Scenario: 320 px layout has no horizontal scroll

- GIVEN a 320 × 800 viewport
- WHEN the Inspector renders with one pane
- THEN `document.documentElement.scrollWidth <= 320`
- AND the tabs overflow menu opens without clipping

#### Scenario: 1280 px layout fits four panes

- GIVEN a 1280 × 800 viewport
- WHEN four panes are open
- THEN each pane is visible without horizontal scroll
- AND the breadcrumb renders on one line

### Requirement: WCAG AA contrast across pane states

Every pane state (loading, empty, error, truncation, revision-change, ready) MUST meet WCAG 2.2 AA contrast: ≥ 4.5:1 for normal text, ≥ 3:1 for large text and UI components. This applies to all renderer backgrounds shipped in `renderer-registry-frontend`. Automated axe-core audits MUST be green at the three target viewports.

#### Scenario: Pane text meets 4.5:1

- GIVEN the Inspector at 320 / 768 / 1280 with each pane state
- WHEN `axe-core.run()` runs
- THEN zero `color-contrast` violations are reported

#### Scenario: Error card meets 3:1 against background

- GIVEN `pane-error` background `#FFFFFF` and foreground `#B00020`
- WHEN contrast is computed
- THEN the ratio is ≥ 4.5:1 (large text uses ≥ 3:1)

#### Scenario: Truncation banner passes

- GIVEN the truncation banner background and foreground
- WHEN axe-core audits the pane
- THEN the banner reports zero contrast violations
