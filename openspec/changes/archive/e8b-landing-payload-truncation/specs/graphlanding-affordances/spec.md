# Delta for GraphLanding Affordances

This delta modifies `openspec/specs/graphlanding-affordances/spec.md`
(added by cycle `e8-graphlanding-affordances`, v0.24.1) to reflect the
backend contract added by cycle `e8b-landing-payload-truncation`
(v0.24.2).

## MODIFIED Requirements

### Requirement: 2. Landing Payload Schema Accepts Truncation Fields

The `landingPayloadSchema` (Zod) MUST accept two additional fields:
`truncated: z.boolean()` and `truncated_reason: z.string().nullable()`.
The backend MUST produce these fields starting from v0.24.2. The fields
remain backwards-compatible for v0.24.1 servers (which omits them) by
parsing the zod schema with `.optional()` semantics for clients that need
to interoperate with pre-v0.24.2 backends.
(Previously: "Both fields MUST be optional so that older backends that do
not return them continue to parse correctly.")

#### Scenario: Parsing succeeds when fields are absent (legacy v0.24.1 server)

- GIVEN a JSON body from a pre-v0.24.2 backend that omits both
  `truncated` and `truncated_reason`
- WHEN `landingPayloadSchema.parse(json)` is called by a client that
  uses `.optional()` semantics for backwards compatibility
- THEN the result is a valid `LandingPayload` object

#### Scenario: Parsing succeeds when fields are present (v0.24.2+ server)

- GIVEN a JSON body from a v0.24.2+ backend that contains
  `truncated: true` and `truncated_reason: "node_cap"`
- WHEN `landingPayloadSchema.parse(json)` is called
- THEN the result reflects both fields

#### Scenario: Parsing fails strict mode without fields

- GIVEN a JSON body that omits both fields
- WHEN `landingPayloadSchema.strict().parse(json)` is called
- THEN parsing fails with a clear zod error indicating both fields
  are missing

## ADDED Requirements

### Requirement: 9. Backend `LandingPayload` Truncation Contract

The backend `landing_handler` at `GET /api/workspaces/:id/landing`
MUST produce a `LandingPayload` JSON body that includes:

| Field | Type | When present | Value |
|---|---|---|---|
| `truncated` | `bool` | Always (v0.24.2+) | `false` if `entry_points.len() <= LANDING_NODE_CAP`; `true` otherwise |
| `truncated_reason` | `string \| null` | Always (v0.24.2+) | `Some("node_cap")` when `truncated === true`; `null` otherwise |

The constant `LANDING_NODE_CAP` is defined in
`crates/cognicode-explorer/src/dto.rs` with value `50`. The pure helper
`apply_landing_cap(total: usize) -> (bool, Option<String>)` in
`crates/cognicode-explorer/src/api.rs` MUST implement:

| Input `total` | Return `(truncated, truncated_reason)` |
|---|---|
| `total <= LANDING_NODE_CAP` | `(false, None)` |
| `total > LANDING_NODE_CAP` | `(true, Some("node_cap"))` |

The helper MUST be a pure function with no I/O. It MUST be the single
source of truth for the truncation policy — the handler MUST NOT
re-implement the comparison inline.

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

#### Scenario: Backend omits fields when graph is missing

- GIVEN the workspace has no ingested graph
- WHEN the client calls `GET /api/workspaces/ws-empty/landing`
- THEN response status is 200 (NOT 503)
- AND `graph_status === "missing"`
- AND `truncated === false`
- AND `truncated_reason === null`
- AND `nodes === []`, `entry_points === []`, `hot_paths === []`,
  `god_nodes === []`, `edges === []`

(Last scenario explicitly documents the v0.24.2 transitional state:
the handler still returns empty stubs because the data wiring is
deferred to `e10-landing-real-data`. The truncation contract is closed
even though the data is empty.)

## REMOVED Requirements

None.
