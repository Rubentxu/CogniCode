# Spec: GraphLanding Affordances

## Purpose

Define the behavior of `GraphLanding` (the Explorer UI's landing page) when
the cytoscape renderer is unable to communicate the full landing graph to
the user — whether because the backend truncated the payload, because the
user navigates by keyboard or screen reader, or because the artifact
endpoint contract has drifted between frontend and backend.

This spec covers:
- Truncation banner rendering driven by `LandingPayload.truncated`.
- Canvas accessibility (`role`, `aria-label`, `tabIndex`) plus a fallback
  node list rendered as buttons.
- Frontend alignment with the backend's
  `/api/exploration-sessions/:id/artifacts/:format` contract (ADR-040 Wave 3).
- E2E test harness compatibility with MSW (Mock Service Worker).

---

## ADDED Requirements

### Requirement: 1. Truncation Banner on Landing

The system MUST render a visible truncation banner above the cytoscape
canvas when `LandingPayload.truncated === true`. The banner MUST display
the value of `LandingPayload.truncated_reason` when present, or omit the
parenthetical reason when `null` / absent. The banner MUST use the
`data-testid="graph-landing-warning"` testid.

#### Scenario: Banner renders when payload is truncated

- GIVEN the `LandingPayload` returned by `GET /api/workspaces/:id/landing`
  contains `truncated: true` and `truncated_reason: "node_cap"`
- WHEN `GraphLanding` renders the landing page
- THEN the banner with `data-testid="graph-landing-warning"` is visible
- AND the banner text contains `"Showing a truncated landing graph (node_cap)"`
- AND the banner text contains `"Refine the focus"`

#### Scenario: Banner is absent when payload is not truncated

- GIVEN the `LandingPayload` contains `truncated: false` or omits the field
- WHEN `GraphLanding` renders the landing page
- THEN no element with `data-testid="graph-landing-warning"` exists in the DOM

#### Scenario: Banner survives missing reason

- GIVEN the `LandingPayload` contains `truncated: true` and no
  `truncated_reason`
- WHEN `GraphLanding` renders the landing page
- THEN the banner is visible
- AND the banner text contains `"Showing a truncated landing graph"` without a
  trailing parenthetical

### Requirement: 2. Landing Payload Truncation Fields (frontend + backend contract)

The `LandingPayload` JSON payload MUST include two fields related to
truncation:

| Field | Type | Required? (v0.24.2+) | Required? (legacy ≤ v0.24.1) |
|---|---|---|---|
| `truncated` | `bool` | **Required** | Optional (clients parse as `.optional()`) |
| `truncated_reason` | `string \| null` | **Required** | Optional (clients parse as `.nullable().optional()`) |

The frontend `landingPayloadSchema` (Zod) MUST declare both fields.
Starting from v0.24.2, the backend MUST produce both fields; older clients
MUST continue to parse payloads that omit them by using `.optional()` /
`.nullable().optional()` semantics.

(Previously: "Both fields MUST be optional so that older backends that do
not return them continue to parse correctly." The contract was relaxed in
cycle `e8b-landing-payload-truncation` (v0.24.2) once the backend began
producing the fields.)

#### Scenario: Backend (v0.24.2+) produces both fields

- GIVEN a v0.24.2+ backend response
- WHEN the client parses the JSON
- THEN the payload contains `truncated: false` (or `true`) and
  `truncated_reason` either `null` or `"node_cap"`

#### Scenario: Legacy backend (≤ v0.24.1) omits both fields

- GIVEN a pre-v0.24.2 backend response that omits both fields
- WHEN the client parses with `.optional()` / `.nullable().optional()`
  semantics
- THEN the result is a valid `LandingPayload` object (the missing fields
  default to `undefined` on the client and the banner renders nothing)

#### Scenario: Strict-mode client fails clearly on legacy server

- GIVEN a pre-v0.24.2 backend response
- WHEN the client parses with `landingPayloadSchema.strict()`
- THEN parsing fails with a clear zod error listing both missing fields

### Requirement: 3. Cytoscape Canvas Accessibility

The cytoscape container `<div data-testid="graph-landing-canvas">` MUST
expose the following accessibility attributes:
- `role="application"`
- `aria-label` describing the graph (e.g., `"Workspace landing graph"` or
  `"Architecture landing graph"` depending on perspective)
- `tabIndex={0}` so the element receives keyboard focus

#### Scenario: Canvas exposes application role and label

- GIVEN `GraphLanding` is rendered with perspective `"graph"`
- WHEN the canvas div is inspected
- THEN it has `role="application"`
- AND it has `aria-label="Workspace landing graph"`
- AND it has `tabIndex="0"`

#### Scenario: Canvas label reflects C4 perspective

- GIVEN `GraphLanding` is rendered with perspective `"c4"`
- WHEN the canvas div is inspected
- THEN it has `aria-label="Architecture landing graph"`

### Requirement: 4. Node List Fallback for Canvas-Unreachable Users

The system MUST render a `<div data-testid="graph-landing-node-list">`
directly below the cytoscape canvas. The fallback MUST contain one
`<button data-testid="graph-node-{id}">` per node in `LandingPayload.nodes`.
Clicking a button MUST dispatch `SELECT_OBJECT` with the node's `id` and
`viewId: "overview"`, identical to clicking the corresponding node on the
canvas.

#### Scenario: Fallback renders one button per node

- GIVEN `LandingPayload` contains 3 nodes `["a", "b", "c"]`
- WHEN `GraphLanding` renders
- THEN the `graph-landing-node-list` element exists
- AND it contains exactly 3 buttons with testids
  `graph-node-a`, `graph-node-b`, `graph-node-c`

#### Scenario: Fallback button click opens the pane stack

- GIVEN `GraphLanding` is rendered with workspace data
- WHEN a user clicks the button with `data-testid="graph-node-b"`
- THEN `SELECT_OBJECT` is dispatched with
  `{ objectId: "b", viewId: "overview" }`
- AND the pane stack shows a pane for object `"b"`

### Requirement: 5. `selectObject` Memoization

`GraphLanding` MUST expose `selectObject` as a stable callback via
`useCallback` with `[dispatch]` as its dependency array. The cytoscape
mount effect MUST depend on `selectObject` (not the raw `dispatch` function)
so the cytoscape instance is not destroyed and re-created on every
dispatch.

#### Scenario: selectObject is referentially stable

- GIVEN `GraphLanding` re-renders without a change in `dispatch`
- WHEN the function reference is compared across renders
- THEN the `selectObject` reference is identical

### Requirement: 6. Artifact Endpoint Path Contract

The frontend `useArtifact` hook (and the equivalent artifact generation
path) MUST call
`POST /api/exploration-sessions/:exploration_id/artifacts/:format` (and
`GET .../artifacts/:format` for status reads). The MSW mock handler MUST
match the wildcard `*/api/exploration-sessions/:exploration_id/artifacts`
to satisfy both the SWR-key-style call (without the `/api` prefix) and
the explicit-URL call.

#### Scenario: Artifact fetch uses the exploration-sessions path

- GIVEN `useArtifact("session-123", "json")` is invoked
- WHEN the SWR request is dispatched
- THEN the request URL is
  `/api/exploration-sessions/session-123/artifacts/json`

#### Scenario: MSW mock matches the wildcard

- GIVEN MSW is registered with the handler
  `*/api/exploration-sessions/:exploration_id/artifacts`
- WHEN any code path issues a request whose URL ends in
  `/api/exploration-sessions/{id}/artifacts`
- THEN the mock handler responds

### Requirement: 7. Quality Summary Mock Endpoint

The MSW mock layer MUST expose a `GET /api/workspaces/:workspace_id/quality-summary`
endpoint that returns a JSON body shaped as
`{ summary: { scope, rating, total_issues, debt_minutes, by_severity, last_run }, issues: [...] }`.
This endpoint MUST be documented as dev-only and MUST NOT have a
matching real-backend implementation in this change.

#### Scenario: Mock returns the expected shape

- GIVEN MSW handlers are registered in the test harness
- WHEN a test issues `GET /api/workspaces/ws-1/quality-summary`
- THEN the response is 200
- AND the body has `summary.rating` equal to `"B"`
- AND `summary.by_severity` includes `critical`, `major`, `minor` keys

### Requirement: 8. E2E Test Harness Compatibility with MSW

E2E tests that need to override a specific Explorer UI request MUST use
`page.addInitScript` to override `window.fetch` directly. The override
MUST run before MSW bootstraps so the test's response wins. The
`page.route(...)` Playwright API MUST NOT be used for Explorer UI request
overrides because MSW intercepts requests inside the page context before
Playwright's network layer observes them.

#### Scenario: Override survives MSW bootstrap

- GIVEN a test that registers a fetch override via `addInitScript` for
  `/api/workspaces/*/landing*` returning a 500 response
- AND MSW handlers are registered with the default 200 responses
- WHEN the user navigates to the landing page
- THEN the user sees the error state
- AND the 500 from the override is the response actually consumed by the app

#### Scenario: openSpotterResult helper supports index selection

- GIVEN the test helper `openSpotterResult(page, index = 0)` exists
- WHEN a test calls `openSpotterResult(page, 1)`
- THEN the second Spotter result (zero-indexed) is selected
- AND the pane stack reflects the new selection

### Requirement: 9. Backend `LandingPayload` Truncation Contract

The backend `landing_handler` at `GET /api/workspaces/:id/landing` MUST
produce a `LandingPayload` JSON body that includes the `truncated` and
`truncated_reason` fields per Requirement 2. The truncation policy MUST be
applied through the pure helper `apply_landing_cap` in
`crates/cognicode-explorer/src/api.rs`:

| Input `total` | Return `(truncated, truncated_reason)` |
|---|---|
| `total <= LANDING_NODE_CAP` (50) | `(false, None)` |
| `total > LANDING_NODE_CAP` (50) | `(true, Some("node_cap"))` |

`LANDING_NODE_CAP` is a `pub const` in
`crates/cognicode-explorer/src/dto.rs` with value `50`. The handler MUST
NOT re-implement the comparison inline — `apply_landing_cap` is the single
source of truth.

#### Scenario: Handler returns truncated=false when entry points fit

- GIVEN a workspace whose `entry_points` count is 30
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN response status is 200
- AND `truncated === false`
- AND `truncated_reason === null`

#### Scenario: Handler returns truncated=true when entry points exceed cap

- GIVEN a workspace whose `entry_points` count is 75 (above
  `LANDING_NODE_CAP = 50`)
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN response status is 200
- AND `truncated === true`
- AND `truncated_reason === "node_cap"`

#### Scenario: apply_landing_cap pure helper boundary at cap

- GIVEN `LANDING_NODE_CAP = 50`
- WHEN `apply_landing_cap(49)` is called
- THEN it returns `(false, None)`
- WHEN `apply_landing_cap(50)` is called
- THEN it returns `(false, None)` (at cap, not over)
- WHEN `apply_landing_cap(51)` is called
- THEN it returns `(true, Some("node_cap"))`

#### Scenario: Backend produces the fields even when the graph is missing

- GIVEN the workspace has no ingested graph
- WHEN the client calls `GET /api/workspaces/ws-empty/landing`
- THEN response status is 200 (NOT 503)
- AND `graph_status === "missing"`
- AND `truncated === false`
- AND `truncated_reason === null`
- AND `nodes === []`, `entry_points === []`, `hot_paths === []`,
  `god_nodes === []`, `edges === []`

(The last scenario documents the v0.24.2 transitional state: the handler
still returns empty stubs because the data wiring is deferred to
`e10-landing-real-data`. The truncation contract is closed even though
the data is empty.)
