# Pane Navigation Specification

## Purpose

Defines the lateral Inspector Pane Stack anchoring the Explorer
workbench zone: drill-down narrative, view-tab representation
switching, duplicate dedup, causal breadcrumb origin. ContextRail
content is out of scope (E27.3); this slice only marks the
boundary.

## Requirements

### Requirement: Lateral Pane Stack Persists the Exploration Narrative

Drilling into a different object MUST open a NEW pane and keep
prior panes as tabs. Closing the active pane MUST move focus to a
neighbour.

#### Scenario: Drill into a different object appends a pane

- GIVEN one pane is open showing object A
- WHEN the user selects a different object B
- THEN the pane stack contains two panes
- AND pane B is active
- AND pane A remains open as a non-active tab

#### Scenario: Closing the active pane moves focus to a neighbour

- GIVEN two panes are open with B active
- WHEN the user clicks ✕ on pane B
- THEN pane B is removed
- AND pane A becomes the active pane

### Requirement: Shell Anchors Stay Stable While Panes Deepen

While panes open, close, or switch, the StartRail and TopBar MUST
remain mounted; the pane stack MUST occupy `shell-zone-center`
reserved by E27.1.

#### Scenario: StartRail and TopBar persist across pane depth

- GIVEN the Explorer is in active-pane mode with one pane
- WHEN the user drills three times into different objects
- THEN `data-testid="start-rail"` stays in the DOM at every depth
- AND the TopBar Spotter trigger stays reachable

### Requirement: View Tabs Switch Representation of the Same Object

Selecting a view tab MUST change the rendered representation of the
active pane's object without creating a new pane, the URL, or
`fromObjectId`/`viaViewKind`.

#### Scenario: Tab change updates activeViewId without adding a pane

- GIVEN pane B is active with view `call_graph`
- WHEN the user clicks the `source_view` tab
- THEN the pane count is unchanged
- AND pane B's `activeViewId` becomes `source_view`
- AND pane B's `objectId`, `fromObjectId`, `viaViewKind` are unchanged

### Requirement: Duplicate Object Selection Dedups to the Existing Pane

Re-selecting an already-open object MUST activate that pane without
pushing a duplicate.

#### Scenario: Reselecting the same object activates the existing pane

- GIVEN pane A shows `Symbol:Foo` and pane B shows `Symbol:Bar`
- WHEN the user selects `Symbol:Foo` again
- THEN the pane count is unchanged
- AND pane A is now active

#### Scenario: Selecting from a breadcrumb From label dedups

- GIVEN pane B is active with breadcrumb From=`Symbol:Foo`
- WHEN the user clicks the From label
- THEN pane A becomes the active pane

### Requirement: Causal Breadcrumb Explains Pane Origin

When a pane has a `fromObjectId` the inspector MUST render
`From <label> · Via <view-title>`. Clicking From MUST navigate
back to the origin object. When `fromObjectId` is absent the
breadcrumb MUST NOT render. It MUST NOT contain a hardcoded
shortcut hint.

#### Scenario: Breadcrumb renders From and Via when origin is set

- GIVEN pane B was drilled from pane A via `call_graph`
- WHEN pane B is active
- THEN `data-testid="pane-breadcrumb"` is visible
- AND shows pane A's label after `From`
- AND shows the title of `call_graph` after `Via`

#### Scenario: Breadcrumb hidden when origin is absent

- GIVEN pane A is the only pane and has no `fromObjectId`
- WHEN pane A is active
- THEN no `data-testid="pane-breadcrumb"` exists in the DOM

### Requirement: ContextRail Knowledge Section Marked E27.3-Pending

The ContextRail's Knowledge section MUST render only its structural
placeholder, marked `// E27.3-pending` in source. It MUST NOT
render ADR, evidence, decision, or artifact titles not backed by
persisted data.

#### Scenario: ContextRail exposes no fabricated knowledge list

- GIVEN no investigation is active and no ADR linked to the active object
- WHEN the ContextRail renders at desktop viewport
- THEN no list of ADR/evidence/decision titles is shown
- AND the Knowledge section displays only the structural placeholder