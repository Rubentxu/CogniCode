# Kernel Specs: Exploration Snapshot

## Router Context Used
- Knowledge Coverage: sufficient (ADR-040 §6-§9, roadmap §6-§9, UX wireframes §MOLDABLE-VIEW-UX-WORKFLOW, CONTEXT.md Navigation section)
- Context Quality: C2
- Taxonomy: persistence, viewport-state, pane-snapshot, breaking-change
- Domain Language: Exploration Snapshot, PaneSnapshot, ViewportState, ExplorationSession, PaneStack (all resolved)
- Recommended Effort: deepen

## Knowledge Provenance
- Scope source: `openspec/changes/moldable-view-call-graph/proposal.md` (Capabilities, Approach, Affected Areas)
- Invariant source: `docs/adr/ADR-040-graph-view-renderer.md` (§8 NO backward compatibility), `docs/roadmap/MOLDABLE-VIEW-PANE-STATE-2026.md` (Task 5.1-5.5), grill session Decision 13
- Memory-only hints excluded from spec truth: None — all persistence/breaking-change decisions are ADR-backed

## Capability: exploration-snapshot

### Requirement: PaneState includes ViewportState
The frontend `Pane` type SHALL include an optional `viewport: ViewportState` field.

#### Scenario: SvgGraph captures viewport on pan/zoom
- GIVEN a user pans or zooms the SvgGraph
- WHEN the interaction completes (pointerup / wheelend)
- THEN `dispatch({ type: "UPDATE_PANE_VIEWPORT", payload: { paneId, viewport } })` SHALL fire
- AND the reducer SHALL update the active pane's `viewport` field

#### Scenario: Pane restored from snapshot
- GIVEN a saved exploration with `panes[i].viewport`
- WHEN the explorer loads the exploration
- THEN each pane SHALL be hydrated with its `viewport` value
- AND SvgGraph SHALL render with the saved zoom and pan position

### Requirement: ExplorationSession schema includes panes
The Rust `ExplorationSession` struct SHALL include `panes: Vec<PaneSnapshot>` field with NO default value.

#### Scenario: save exploration captures all panes
- GIVEN a user clicks "Save exploration snapshot" with 3 panes open
- WHEN `POST /api/explorations/session` is called
- THEN the request body SHALL include `panes` array with 3 entries
- AND each entry SHALL contain `{ pane_id, object_id, view_id, scroll_y, viewport }`

#### Scenario: load exploration restores panes
- GIVEN a saved exploration with 3 panes
- WHEN `GET /api/explorations/session/{id}` returns the session
- THEN the frontend SHALL hydrate a PaneStack with 3 panes in order
- AND the last pane in the array SHALL be active

### Requirement: Hybrid trigger for snapshot persistence
The snapshot persistence SHALL use both localStorage cache AND server save.

#### Scenario: localStorage cache on every change
- GIVEN any navigation event (open pane, close pane, switch view, pan/zoom)
- WHEN the action completes
- THEN `localStorage[cognicode.exploration.snapshot.{workspaceId}.{sessionId}]` SHALL be updated

#### Scenario: manual save to server
- GIVEN a user clicks "Save exploration snapshot"
- WHEN the request completes successfully
- THEN `POST /api/explorations/session` SHALL be called
- AND a toast SHALL show "Saved! Share URL: /explore/{sessionId}"

### Requirement: No backward compatibility for ExplorationSession
Sessions saved before this change SHALL be invalid after deployment.

#### Scenario: legacy session deserialization fails
- GIVEN a session JSON without `panes` field (legacy)
- WHEN the Rust server deserializes it
- THEN deserialization SHALL fail with a clear error
- AND the error message SHALL indicate "missing field: panes"

#### Scenario: frontend ignores legacy localStorage
- GIVEN a localStorage entry without `panes` field (legacy)
- WHEN the explorer loads on startup
- THEN the legacy entry SHALL be ignored silently
- AND the user SHALL see a fresh empty PaneStack

## Invariants Covered
- `ExplorationSession.panes` has NO `#[serde(default)]` — verified by `legacy session deserialization fails` (regression test in API)
- `MAX_PANES = 8` cap — verified by `save exploration captures all panes` (panes array length bounded)
- `SELECT_OBJECT` deduplicates by `objectId` — verified by `load exploration restores panes` (no duplicates introduced)
- localStorage key namespace `cognicode.exploration.snapshot.{workspaceId}.{sessionId}` — verified by `localStorage cache on every change`

## Open Questions
- None. The breaking-change policy is intentional (Decision 13) and the localStorage fallback is the user-facing mitigation per proposal §"Risks".
