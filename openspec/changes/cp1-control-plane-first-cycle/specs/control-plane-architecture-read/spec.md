# Spec — Control Plane architecture read (CP1.0)

## Requirement: control-plane-architecture-read (CP1.0)

The system MUST expose a read-only HTTP endpoint under the `control-plane`
namespace that returns the current architecture state of a workspace.

### Scenarios

#### Scenario: no service wired → incomplete, never clean

- **GIVEN** the explorer is started without `with_control_query`
- **WHEN** `GET /control-plane/workspaces/:workspace_id/architecture` is called
- **THEN** response `200` with body `{ "status": "incomplete", ... }`
- **AND** `status` is NEVER `"clean"` or `"evaluated"`

#### Scenario: empty admission set → incomplete

- **GIVEN** the architecture registry has an empty admitted-constraints set
- **WHEN** the endpoint is called
- **THEN** response `200` with `{ "status": "incomplete" }`

#### Scenario: real evaluation → evaluated + projection

- **GIVEN** the architecture registry has ≥1 admitted constraint
- **WHEN** the endpoint is called
- **THEN** response `200` with `{ "status": "evaluated", "constraints": [...], "violations": [...] }`
- **AND** `constraints` items contain `ConstraintRef { id, .. }` only (no payload)

#### Scenario: violation projection is reference-only

- **GIVEN** there is at least one violation in the read model
- **WHEN** the response is inspected
- **THEN** every `violations[*]` is `ViolationRef { id, .. }` — never a truth payload

#### Scenario: endpoint path is read-only (mutating verbs 405/404)

- **GIVEN** the route is mounted
- **WHEN** `POST`/`PUT`/`DELETE` is sent to `/control-plane/workspaces/:id/architecture`
- **THEN** the explorer returns `405` or `404` — no state mutation is reachable
  through this path
