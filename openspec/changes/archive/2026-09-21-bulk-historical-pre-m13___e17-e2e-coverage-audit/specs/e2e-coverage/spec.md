# Spec: E2E Coverage Audit — Moldable Parity via Playwright

## ADDED Requirements

### R1 — Spotter multi-family coverage

The Spotter E2E suite MUST assert user-visible behavior for all 6 wired families.

#### Scenario R1.1 — Symbol family
- **WHEN** user opens Spotter (Cmd+K) and types a substring matching a known symbol name in MSW fixtures
- **THEN** the result list shows ≥1 result tagged `data-family="Symbol"`
- **AND** selecting the result closes the palette and opens an inspector for the symbol

#### Scenario R1.2 — File family
- **WHEN** user types a substring matching a known file path
- **THEN** the result list shows ≥1 result tagged `data-family="File"`

#### Scenario R1.3 — ViewSpec family
- **WHEN** user types a substring matching a saved ViewSpec title
- **THEN** the result list shows ≥1 result tagged `data-family="ViewSpec"`

#### Scenario R1.4 — SavedExploration family
- **WHEN** user types a substring matching a saved exploration id or label
- **THEN** the result list shows ≥1 result tagged `data-family="SavedExploration"`

#### Scenario R1.5 — QualityIssue family
- **WHEN** user types a substring matching a known quality issue rule_id or file_path
- **THEN** the result list shows ≥1 result tagged `data-family="QualityIssue"`

#### Scenario R1.6 — Rule family
- **WHEN** user types a substring matching a known rule id
- **THEN** the result list shows ≥1 result tagged `data-family="Rule"`

#### Scenario R1.7 — Cross-family isolation
- **WHEN** user types a query that matches multiple families
- **THEN** results from all matching families are shown, grouped by family

### R2 — View executor coverage (15 executors)

For each of the 15 wired executors in `crates/cognicode-explorer/src/registry.rs:335-413`:

#### Scenario R2.<i> — <executor-name>
- **WHEN** user inspects an object whose `applies_to` matches `<executor-name>`
- **AND** clicks the view tab labeled `<view-id>`
- **THEN** the renderer mounts and produces a non-empty DOM
- **AND** the renderer matches the documented `RendererKind` (`Table | Graph | Tree | Code | Markdown | Composite`)
- **AND** a screenshot is captured to `apps/explorer-ui/e2e/view-tabs-coverage.spec.ts-snapshots/<executor-name>.png`

Executors to cover: `overview`, `call-graph`, `source`, `quality`, `evidence`, `symbols`, `dependencies`, `hotspots`, `architecture-drift`, `usage-examples`, `api-surface`, `test-slice`, `debug-slice`, `change-impact-story`, `ownership-map`.

### R3 — Pane stack drill-down (GtPager parity)

#### Scenario R3.1 — Drill into callee opens new pane
- **WHEN** user is inspecting a function symbol with the call-graph view active
- **AND** clicks a callee node
- **THEN** a new pane opens to the right of the current pane
- **AND** the original pane remains visible and unmodified

#### Scenario R3.2 — Three-level drill preserves history
- **WHEN** user drills from `Symbol A` → `Symbol B` → `Symbol C` via three clicks
- **THEN** three panes are visible side-by-side
- **AND** the leftmost pane still shows `Symbol A`

#### Scenario R3.3 — Close pane via ✕
- **WHEN** user clicks the `✕` button on the active (rightmost) pane
- **THEN** the pane is removed
- **AND** the previous pane becomes active

#### Scenario R3.4 — Dedup on re-select
- **WHEN** user drills into `Symbol A`
- **AND** later selects `Symbol A` again from a different context
- **THEN** no new pane is created
- **AND** the existing `Symbol A` pane becomes active

### R4 — ViewSpecWizard full flow

#### Scenario R4.1 — Complete wizard and save
- **WHEN** user opens the wizard from the inspector
- **AND** picks `ViewKind = call-graph`
- **AND** picks `RendererKind = Graph`
- **AND** picks data source (graph query for symbol X)
- **AND** edits the JSONata transform
- **AND** clicks Save
- **THEN** the ViewSpec is persisted to the MSW-backed `ViewSpecStore`
- **AND** the new view appears in the inspector's available views for symbol X
- **AND** opening it renders the configured view

#### Scenario R4.2 — Wizard preview updates as user types
- **WHEN** user is in the JSONata step
- **AND** types a valid JSONata expression
- **THEN** the live preview pane updates within 500ms

### R5 — Landing real-data + virtualization

#### Scenario R5.1 — Landing shows real entry points
- **WHEN** user opens the app
- **THEN** the landing payload includes `entry_points`, `hot_paths`, `god_nodes`
- **AND** `entry_points.length ≥ 1` when MSW fixtures provide at least one

#### Scenario R5.2 — Virtualization activates at 200+ nodes
- **WHEN** MSW fixtures provide a workspace with ≥200 nodes
- **THEN** the visible node-list window contains exactly the viewport-height number of rows
- **AND** scrolling reveals more rows (no DOM blow-up)

#### Scenario R5.3 — Truncation banner when capped
- **WHEN** the landing payload has `truncated: true`
- **THEN** the truncation banner is visible with `truncated_reason` text

### R6 — Exploration sharing

#### Scenario R6.1 — Share button produces URL
- **WHEN** user inspects an object and clicks `ShareExplorationButton`
- **THEN** the URL bar updates to include `?exploration=<id>`

#### Scenario R6.2 — Opening share URL restores state
- **WHEN** user opens a URL with `?exploration=<id>`
- **THEN** the pane stack is restored to the saved state
- **AND** the saved exploration is shown in the explorer

### R7 — Scan progress

#### Scenario R7.1 — ScanBar appears during scan
- **WHEN** user triggers a scan
- **THEN** `ScanBar` mounts with progress text
- **AND** progress increments from 0% to 100%
- **AND** on completion, an ingest notification is shown

### R8 — Lens panel (with known-debt flag)

#### Scenario R8.1 — LensPanel opens
- **WHEN** user toggles `LensSidebarToggle`
- **THEN** `LensPanel` mounts
- **AND** shows the treemap and sunburst visualizations

#### Scenario R8.2 — Debt: treemap uses mock fixture
- **WHEN** the treemap is rendered
- **THEN** the test asserts the data comes from `HOTSPOT_TREEMAP_FIXTURE` (documented as known-debt in test comment)
- **AND** the test fails the cycle IF the fixture is removed (forcing real-data wiring)

### R9 — Perspective toggle

#### Scenario R9.1 — Toggle to C4 perspective
- **WHEN** user toggles perspective from Default to C4
- **THEN** the landing payload switches to the C4 view
- **AND** the explorer layout adapts (sidebar tree visible)

#### Scenario R9.2 — Toggle back to Default
- **WHEN** user toggles back
- **THEN** the explorer returns to the default layout

### R10 — Responsive layout

For viewports 320×568, 768×1024, 1280×800, 1920×1080:

#### Scenario R10.<viewport>
- **WHEN** the app is rendered at `<viewport>`
- **THEN** no horizontal scroll appears
- **AND** all interactive controls are reachable
- **AND** no overlapping elements

### R11 — Error states

#### Scenario R11.1 — Network failure shows actionable fallback
- **WHEN** the API returns 500
- **THEN** the affected panel shows an error boundary with retry button

#### Scenario R11.2 — Unknown view kind falls back to UnknownBlockView
- **WHEN** the backend returns a `ViewKind` not in the registered set
- **THEN** the renderer falls back to `UnknownBlockView` showing JSON
- **AND** no white screen of death

### R12 — A11y (axe-core) coverage

#### Scenario R12.1 — All views pass axe-core
- **WHEN** axe-core runs against every wired view tab
- **THEN** zero violations of `wcag2a` or `wcag2aa` rules

### R13 — Call-graph interaction

#### Scenario R13.1 — Pan / zoom on call graph
- **WHEN** user mouse-drags on the SVG canvas
- **THEN** the viewport transforms

#### Scenario R13.2 — Node click drills into callee
- **WHEN** user clicks a node
- **THEN** a new pane opens for that symbol

### R14 — Visual regression baselines

For each new spec, capture at least one canonical screenshot and commit as baseline.

#### Scenario R14.1 — Baseline committed
- **WHEN** a new spec is added
- **THEN** at least one PNG exists under `apps/explorer-ui/e2e/<spec>.spec.ts-snapshots/`
- **AND** the PNG matches the page in the spec's "happy path" scenario

### R15 — MSW fixture consistency

#### Scenario R15.1 — Every handler is hit
- **WHEN** the suite completes
- **THEN** every handler in `apps/explorer-ui/src/mocks/handlers.ts` has at least one matching request in the run log
- **AND** unused handlers are flagged as debt

### R16 — Flake mitigation

#### Scenario R16.1 — No `waitForTimeout` for non-network waits
- **WHEN** any spec waits for a UI element
- **THEN** it uses `expect(...).toBeVisible()` with timeout, not `waitForTimeout`

#### Scenario R16.2 — Animations disabled in screenshots
- **WHEN** a screenshot is captured
- **THEN** `animations: "disabled"` is set in `toHaveScreenshot` options

#### Scenario R16.3 — Deterministic time
- **WHEN** the app displays time-dependent UI
- **THEN** MSW freezes `Date.now()` to a fixed timestamp

## MODIFIED Requirements

None. This cycle adds coverage; it does not change existing behavior.

## REMOVED Requirements

None.

## Cross-cutting

### R17 — Coverage matrix maintained

A coverage matrix at `docs/inventory/e17-coverage-matrix.md` MUST list every cataloged feature with its test name, screenshot path, and GToolkit equivalent. The matrix MUST be updated with each spec completion. The matrix is the source of truth for "did we cover everything".

### R18 — No production code changes

The cycle is test + docs only. Any production bug found is recorded in `docs/inventory/e17-deferred-bugs.md` and proposed as a follow-up cycle.

### R19 — Local-only docs

All coverage matrices, gap reports, ADRs, and updated ROADMAP live in `docs/`, `plans/`, `openspec/` — never pushed to the remote per user policy (2026-06-24).

### R20 — Conventional commits

Commit messages follow the conventional commit format: `test(e2e): <spec-name>`. No `Co-Authored-By` or AI attribution.
