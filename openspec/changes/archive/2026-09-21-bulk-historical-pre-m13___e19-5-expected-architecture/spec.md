# Architecture Drift Governance

## Purpose

Govern C4 architecture drift against `.cognicode/expected-architecture.yaml`. Extends the E6 container-existence checks with **dependency boundary rules** that flag when a container imports another it must not, inferred from workspace manifests (Cargo.toml / package.json).

## Requirements

### Requirement: YAML schema with dependency_rules

The system MUST accept an optional `dependency_rules: Vec<DependencyRule>` field in `.cognicode/expected-architecture.yaml`. Each `DependencyRule` MUST carry `id`, `description`, `from_pattern`, `to_pattern`, and `severity` (one of `error`, `warning`, `info`). Patterns MUST glob-match container names. A missing or empty `dependency_rules` field MUST preserve E6 behavior.

#### Scenario: YAML with one rule parses successfully

- GIVEN `.cognicode/expected-architecture.yaml` containing one `dependency_rules` entry
- WHEN the comparison runs
- THEN the rule loads with id, description, from_pattern, to_pattern, and severity populated

#### Scenario: Rule `apps/* → cognicode-*postgres*` flags explorer-ui

- GIVEN a rule with `from_pattern: "apps/*"`, `to_pattern: "cognicode-*postgres*"`, severity `error`
- AND an inferred `depends_on` edge from `explorer-ui` to `cognicode-postgres`
- WHEN the comparison runs
- THEN a `DriftFinding` of kind `BoundaryViolation` is emitted with severity `error` referencing the rule id

### Requirement: Container-to-container dependency inference

`build_architecture_impl` MUST emit `depends_on` edges between containers inferred from workspace manifests: `[dependencies]` in each member's `Cargo.toml` for Rust crates, `dependencies` in `package.json` for `apps/*`. `[dev-dependencies]` and `[build-dependencies]` MUST be excluded.

#### Scenario: Cargo.toml dependency becomes a depends_on edge in the C4 graph

- GIVEN `apps/explorer-ui/Cargo.toml` declares `[dependencies] cognicode-core = ...`
- WHEN `build_architecture_impl` runs
- THEN the returned C4 subgraph contains a `depends_on` edge from container `explorer-ui` to container `cognicode-core`

### Requirement: Boundary violation findings

`compare_architecture_impl` MUST emit one `DriftFinding{kind: BoundaryViolation, severity}` per inferred `depends_on` edge that matches a rule's `from_pattern` AND `to_pattern`. Each finding MUST carry the violating `expected` (from container) and `actual` (to container) names, the rule's severity, and the rule's id in `detail`. The `DriftReport` MUST expose a `boundary_violations` count.

#### Scenario: Direct dependency violation produces a BoundaryViolation finding

- GIVEN an inferred edge `apps/api → cognicode-postgres`
- AND a rule `{ id: "no-direct-db", from_pattern: "apps/*", to_pattern: "cognicode-postgres", severity: "error" }`
- WHEN the comparison runs
- THEN the report contains a `DriftFinding` with `kind = BoundaryViolation`, `expected = "apps/api"`, `actual = "cognicode-postgres"`, `severity = "error"`, and `detail` mentioning the rule id
- AND `boundary_violations` count is incremented

#### Scenario: Transitive dependencies are NOT reported (MVP)

- GIVEN an inferred direct edge `apps/api → cognicode-core`
- AND `cognicode-core → cognicode-postgres` is inferred as a separate direct edge
- AND no rule covers `apps/api → cognicode-postgres` directly
- WHEN the comparison runs
- THEN no `BoundaryViolation` finding is emitted for `apps/api → cognicode-postgres`
- AND transitive violation detection is explicitly out of scope for MVP

### Requirement: Backward compatibility without rules

When `dependency_rules` is absent or empty, the system MUST produce a `DriftReport` equivalent to E6 (only `MissingContainer`, `ExtraContainer`, `WrongSubKind` findings; `boundary_violations` is zero).

#### Scenario: Missing dependency_rules field yields E6-equivalent report

- GIVEN `.cognicode/expected-architecture.yaml` with no `dependency_rules` field
- WHEN the comparison runs
- THEN no `BoundaryViolation` findings are emitted
- AND `boundary_violations` count equals zero
- AND existing E6 findings (missing/extra/wrong_sub_kind) are unaffected

### Requirement: Frontend boundary-violation overlay

When the user enables the boundary-violation overlay, the C4 graph MUST apply a colored border to each container node that participates in at least one `BoundaryViolation` finding. Border color MUST match severity: red for `error`, amber for `warning`, blue for `info`. The overlay MUST be independent of the existing drift and hotspot overlays (independent toggle, additive CSS classes).

#### Scenario: Severity-colored borders on violating containers

- GIVEN a `DriftReport` with one `BoundaryViolation` of severity `error` on container `explorer-ui`
- AND the user has enabled the boundary-violation overlay
- WHEN the C4 graph renders
- THEN the `explorer-ui` node shows a red border
- AND the boundary-violation toggle persists independently from the drift and hotspot toggles
- AND toggling the overlay off removes the border without affecting other overlays
