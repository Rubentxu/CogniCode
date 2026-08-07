# Delta for GraphLanding Affordances

## MODIFIED Requirements

### Requirement: 1. Truncation Banner on Landing

The system MUST render a visible truncation banner above the cytoscape
canvas when `LandingPayload.truncated === true`. The backend now produces
real `entry_points`, so the banner is no longer a dormant contract: when the
workspace has more than `LANDING_NODE_CAP` entry points, the handler MUST set
`truncated === true` and `truncated_reason === "node_cap"`.
(Previously: the banner contract existed, but the handler still returned empty
stubs and therefore `truncated` was always `false`.)

#### Scenario: Banner activates on a wide workspace

- GIVEN a workspace whose graph has 75 root symbols
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN the response status is 200
- AND `truncated === true`
- AND `truncated_reason === "node_cap"`

#### Scenario: Banner stays hidden on a small workspace

- GIVEN a workspace whose graph has 8 root symbols
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN the response status is 200
- AND `truncated === false`
- AND `truncated_reason === null`

### Requirement: 4. Node List Fallback for Canvas-Unreachable Users

The system MUST render a `<div data-testid="graph-landing-node-list">`
directly below the cytoscape canvas. The fallback MUST contain one
`<button data-testid="graph-node-{id}">` per node in `LandingPayload.nodes`.
These nodes are no longer empty stubs; they are the union of the selected
landing `entry_points`, `hot_paths`, and `god_nodes`, deduplicated by symbol id.
Clicking a button MUST dispatch `SELECT_OBJECT` with the node's `id` and
`viewId: "overview"`, identical to clicking the corresponding node on the
canvas.
(Previously: the fallback contract existed, but `nodes` could remain empty
because the backend returned only stubs.)

#### Scenario: Landing nodes are real backend symbols

- GIVEN a workspace with 3 entry points and 2 hot paths
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN `nodes` contains the deduplicated union of those symbols
- AND every `graph-node-{id}` fallback button corresponds to a real backend symbol

## ADDED Requirements

### Requirement: 10. Landing endpoint returns real semantic workspace seeds

The backend `landing_handler` MUST return a first real semantic overview of the
workspace graph. The payload shape stays the same, but these collections now
have semantics:

| Field | Meaning | Source |
|---|---|---|
| `entry_points` | Symbols with no incoming edges (`fan_in == 0`) | mirrors `AnalysisService::get_entry_points()` |
| `hot_paths` | Symbols with highest `fan_in`, `fan_in > 0`, limited and filtered | mirrors `CallGraphAnalyzer::find_hot_paths()` |
| `god_nodes` | Highly depended-upon symbols with a `score: f64` | MVP backend approximation is acceptable |
| `nodes` | Deduplicated union of landing symbols | backend union logic |
| `edges` | Only relations among the selected landing nodes | `GraphQueryPort::callees()` filtered to selected set |

The endpoint MUST continue to return 200 when the graph is missing or still
building. In that case the semantic collections MAY be empty, but the shape and
`graph_status` contract remain unchanged.

#### Scenario: Entry points mirror root symbols

- GIVEN a call graph with roots `[A, B, C]`
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN `entry_points` contains `A`, `B`, `C`
- AND every entry point is returned as `InspectableObjectSummary`

#### Scenario: Hot paths sorted by fan-in descending

- GIVEN symbols `X (fan_in=10)`, `Y (fan_in=4)`, `Z (fan_in=1)`
- WHEN the client calls `GET /api/workspaces/ws-1/landing`
- THEN `hot_paths` is ordered `[X, Y, Z]`
- AND symbols with `fan_in == 0` are excluded from `hot_paths`

#### Scenario: Edges contain no dangling endpoints

- GIVEN the landing node union contains ids `[A, B, C]`
- WHEN the backend returns `edges`
- THEN every edge `source` is one of `[A, B, C]`
- AND every edge `target` is one of `[A, B, C]`

#### Scenario: Empty graph still returns shape

- GIVEN the workspace has no ingested graph
- WHEN the client calls `GET /api/workspaces/ws-empty/landing`
- THEN response status is 200
- AND `graph_status === "missing"`
- AND `entry_points === []`, `hot_paths === []`, `god_nodes === []`, `nodes === []`, `edges === []`
