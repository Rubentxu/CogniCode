# Visualization Stack Specification

## Purpose
Define the renderer boundary for semantic, evidence-grounded projections. Renderers consume projections; they MUST NOT synthesize structure absent from the projection.

> **Domain status:** `visualization-stack` has no canonical base in `openspec/specs/visualization-stack/` (the directory exists but is empty). This change introduces the capability as a **new spec**, recorded entirely under `## ADDED Requirements`.

## ADDED Requirements

### Requirement: Renderers consume semantic projections
Every renderer MUST consume a validated semantic projection, including topology, flow, type hierarchy, and C4 projections. A renderer MUST NOT synthesize nodes, edges, parent relations, hierarchy levels, ordering, or architecture absent from the projection.

#### Scenario: Graph renderer preserves model
- GIVEN a topology contains nodes A and B and one `Calls` edge A→B
- WHEN a graph renderer displays it
- THEN it displays exactly A, B, and that edge with its kind
- AND it does not add layout-implied nodes or relations

#### Scenario: Sequence renderer lacks order
- GIVEN a flow projection has related participants but no ordered evidence
- WHEN a sequence renderer receives it
- THEN it displays an unsupported or unavailable state and no invented message order

### Requirement: Renderer-neutral capability states
Renderers MUST expose projection status, confidence, provenance, and truncation without converting unsupported or truncated results into complete-looking visualizations.

#### Scenario: Truncated visualization
- GIVEN a projection reports truncation with reason R
- WHEN it is rendered
- THEN the visualization indicates truncation and R
- AND retained content remains limited to projected evidence

#### Scenario: Unsupported C4 level
- GIVEN a C4 projection reports that components are unsupported
- WHEN a C4 renderer displays it
- THEN it shows the unsupported state and MUST NOT draw component boxes

### Requirement: View-family projection selection
Call, dependency, impact, use-case, data-flow, type-hierarchy, and C4 renderings MUST use the projection matching the selected view family; a renderer MUST NOT change relation semantics to fit a visual format.

#### Scenario: Type hierarchy renderer
- GIVEN a model contains `implements`, `inherits`, and `member` relations
- WHEN it is rendered
- THEN each relation remains distinguishable and connects the exact identities
