# c4-mermaid-export Specification (NEW)

## Purpose

Pure text generation of canonical Mermaid C4 diagrams (C1 Context, C2 Container, C3 Component) from an inferred C4 `SubgraphResponse`. Exposed via REST `GET /api/workspaces/:id/architecture/mermaid` and MCP tool `export_c4_mermaid`, gated behind the `multimodal` Cargo feature. SVG rendering is out of scope for v1 (text-only per ADR-003).

## Requirements

### Requirement: Context Level (C1 + C2)

The export function MUST produce a valid Mermaid `C4Context` diagram when invoked with the Context level. The output MUST contain the single System node and every Container node enclosed inside a `Boundary(c1, ...)` block scoped to the System. Component and Code nodes MUST NOT appear.

#### Scenario: Context level emits only System and Containers

- GIVEN a SubgraphResponse with 1 System, 3 Containers, 5 Components
- WHEN the Context level is exported
- THEN the first non-comment line is `C4Context`
- AND the output contains 1 `System(...)` and 3 `Container(...)` declarations
- AND zero `Component(...)` declarations appear

#### Scenario: Context level wraps Containers in System boundary

- GIVEN a System labeled "CogniCode" with 2 Container children
- WHEN the Context level is exported
- THEN a `Boundary(c1, "CogniCode", "System") { ... }` block wraps both Container lines

### Requirement: Container Level (C1 + C2 + C3)

The export function MUST produce a valid `C4Container` diagram when invoked with the Container level. The output MUST contain the System, all Containers, and every Component enclosed inside a `Boundary(c2, ...)` block scoped to its parent Container. Code-level nodes MUST NOT appear.

#### Scenario: Container level emits System, Containers, and Components

- GIVEN a SubgraphResponse with 1 System, 2 Containers, 4 Components
- WHEN the Container level is exported
- THEN the first non-comment line is `C4Container`
- AND the output contains 1 System, 2 Container, and 4 Component declarations
- AND each Component is nested inside its parent Container's `Boundary` block

### Requirement: Component Level (Full C1+C2+C3+Code)

The export function MUST produce a valid `C4Component` diagram when invoked with the Component level. The output MUST contain the System, all Containers, all Components, and every Code node enclosed inside a `Boundary(c3, ...)` block scoped to its parent Component.

#### Scenario: Component level emits all four C4 levels

- GIVEN a SubgraphResponse with 1 System, 2 Containers, 3 Components, 10 Code nodes
- WHEN the Component level is exported
- THEN the first non-comment line is `C4Component`
- AND the output contains 1 System, 2 Container, 3 Component, and 10 Code declarations
- AND each Code declaration is nested inside its parent Component's `Boundary` block

### Requirement: Mermaid-Safe Node ID Sanitization

The export function MUST sanitize node ids before emitting them. Characters conflicting with Mermaid syntax (`:`, `/`, `(`, `)`, `<`, `>`, `{`, `}`) MUST be replaced with `_`. The same sanitized id MUST be used in both the declaration and any referencing edge. Collisions MUST be resolved by appending `_2`, `_3`, ... so every emitted id is unique.

#### Scenario: Colon, slash, and generic punctuation are replaced

- GIVEN a node id `container:crates/cognicode-explorer` and an edge from it
- WHEN the node and edge are emitted
- THEN the declaration and edge both use the same `_`-separated sanitized id

#### Scenario: Sanitization collisions receive numeric suffix

- GIVEN two distinct node ids `a/b` and `a:b`
- WHEN both are emitted
- THEN one declaration uses `a_b` and the other uses `a_b_2`
- AND no two declarations share the same Mermaid id

### Requirement: depends_on Edges Rendered As Labeled Rel Lines

Every `depends_on` edge MUST be rendered as a `Rel(source, target, "label", "technology")` line using `GraphEdge.relation` as the label. Self-loop edges (source == target after sanitization) MUST be omitted.

#### Scenario: depends_on edge appears as a labeled Rel line

- GIVEN an edge with source `container:a`, target `container:b`, relation `depends_on`
- WHEN the edge is rendered
- THEN the output contains a line matching `Rel\(.*container_a.*container_b.*depends_on.*\)`

#### Scenario: Self-loop edges are omitted

- GIVEN an edge whose source and target both reference the same node id
- WHEN the edge is rendered
- THEN the output MUST NOT contain a `Rel(...)` line for that edge

### Requirement: C4 Keyword Selection Matches Requested Level

The export function MUST emit `C4Context`, `C4Container`, or `C4Component` as the first non-comment line of the output, matching the requested level with exact casing.

#### Scenario: Header keyword matches each of the three levels

- GIVEN the three levels Context, Container, Component
- WHEN the export function is called for each
- THEN the first non-comment line is exactly `C4Context`, `C4Container`, and `C4Component` respectively

### Requirement: Empty Level Returns Placeholder Comment

When no nodes match the requested level, the export function MUST still emit a syntactically valid Mermaid block: the correct C4 header followed by a `%%` comment indicating the level is empty (e.g., `%% <level> level is empty for this workspace`). No `System(...)`, `Container(...)`, or `Component(...)` declarations are emitted.

#### Scenario: Empty level returns header + placeholder comment

- GIVEN a SubgraphResponse with zero nodes
- WHEN any of the three levels is exported
- THEN the output starts with the matching C4 keyword
- AND the output contains a `%%` comment stating the level is empty
- AND no `System(...)`, `Container(...)`, or `Component(...)` declarations appear

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Node label contains a newline | Replace newlines with a single space before emitting the label |
| `depends_on` edge with empty relation string | Emit `Rel(...)` with an empty quoted label rather than omitting the edge |
| Two nodes sanitize to the same id AND share a label | Append numeric suffix to disambiguate ids; the label is preserved verbatim |
| A Container has zero Components in Container level | Render the Container with an empty `Boundary(c2, ...)` body (no inner declarations) |
| Empty SubgraphResponse (no architecture inferred) | All three levels return their header + placeholder comment |
| Edge references an id not in the node set | Skip the edge; guards against dangling edges after truncation |

## Out of Scope

- SVG / PNG rendering (deferred per ADR-003)
- Mermaid → graph round-trip / editing
- CLI command for export (follow-up change)
- Layout hints, themes, or styling directives (default Mermaid theme only)
- ViewExecutor wiring for C4 ViewKinds (already exist; this change is export-only)

## Risks

- Mermaid C4 keyword drift between renderers — mitigated by emitting canonical keywords and not promising SVG output in v1
- ID collision after sanitization for `:`-heavy FQNs — mitigated by the dedup pass with numeric suffix
