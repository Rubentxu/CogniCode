# Decision Trace View Specification

## Purpose

Define the `DecisionTraceExecutor` ViewExecutor contract. This view renders a 2-block composite (Mermaid graph + ADR markdown) for `DecisionArtifact` nodes, tracing the rationale subgraph (Justifies, Cites, Resolves, CorroboratedBy) into a `flowchart LR` diagram.

## Requirements

### Requirement: DecisionTraceExecutor — Registration and Applicability

The system MUST register `DecisionTraceExecutor` in the `ViewRegistry` with the descriptor:

| Field | Value |
|-------|-------|
| `id` | `"decision_trace"` |
| `view_kind` | `ViewKind::DecisionTrace` |
| `renderer_kind` | `RendererKind::Composite` |
| `applies_to` | `[InspectableObjectType::DecisionArtifact]` |

The executor's `applies_to()` SHALL return `true` ONLY for `DecisionArtifact` targets. For any other `InspectableObjectType`, `build()` MUST return `ExplorerError::ViewNotAvailable`.

#### Scenario: Executor applies to DecisionArtifact

- GIVEN a `ViewContext` with `target: InspectionTarget::Decision { id: "ADR-002" }`
- WHEN `DecisionTraceExecutor::build()` is called
- THEN a `ContextualView` is returned with `view_kind: ViewKind::DecisionTrace`
- AND no error is returned

#### Scenario: Executor rejects non-DecisionArtifact

- GIVEN a `ViewContext` with `target: InspectionTarget::Symbol { ... }`
- WHEN `DecisionTraceExecutor::build()` is called
- THEN `ExplorerError::ViewNotAvailable` is returned

### Requirement: DecisionTraceExecutor — Graph Block

When `build()` is called for a valid `DecisionArtifact` target, the executor MUST emit a `ViewBlock` as the first block with:

- `id`: `"decision_trace_graph"`
- `title`: `"Decision Trace"`
- `body`: containing `kind: "mermaid"` and `content` with a valid Mermaid `flowchart LR` diagram

The Mermaid diagram SHALL include the Decision node itself and any artifacts reachable via the 4 rationale edges (Justifies, Cites, Resolves, CorroboratedBy). Edge labels in the diagram MUST match the edge kind (e.g., `Cites`, `Justifies`).

The executor SHALL delegate to `rationale_subgraph` on the `GraphRepository` port to fetch the topology.

#### Scenario: Renders graph block with rationale subgraph

- GIVEN a Decision node `ADR-002` connected to `DOC-001` via `Justifies` and to `FUNC-login` via `Resolves`
- WHEN `DecisionTraceExecutor::build()` runs
- THEN the first `ViewBlock` has `id: "decision_trace_graph"` and `body.kind: "mermaid"`
- AND `body.content` contains `flowchart LR`
- AND the diagram includes nodes for `ADR-002`, `DOC-001`, and `FUNC-login`
- AND edge labels match `Justifies` and `Resolves` respectively

#### Scenario: Empty rationale subgraph

- GIVEN a Decision node with no `Justifies`, `Cites`, `Resolves`, or `CorroboratedBy` edges
- WHEN the graph block is built
- THEN the diagram contains only the Decision node itself
- AND no edges are rendered

#### Scenario: Node count cap at 30

- GIVEN a Decision node whose rationale subgraph contains more than 30 reachable artifacts
- WHEN the graph block is built
- THEN no more than 30 nodes appear in the Mermaid diagram
- AND the truncation is noted (see markdown block scenario)

### Requirement: DecisionTraceExecutor — Markdown Block

The executor MUST emit a second `ViewBlock` with:

- `id`: `"decision_trace_markdown"`
- `title`: the Decision's label (e.g. `"ADR-002: Adopt MoldQL"`)
- `body`: containing `kind: "markdown"`, `title`, `status`, and `decision` text from the ADR artifact

The `status` field SHALL be sourced from `focus_node.properties.status`. The `decision` field SHALL be the ADR's decision section text.

#### Scenario: Renders markdown with ADR excerpt

- GIVEN an ADR Decision node with `properties: { status: "accepted", decision: "We will use PostgreSQL..." }`
- WHEN the markdown block is built
- THEN `body.kind` equals `"markdown"`
- AND `body.title` matches the Decision label
- AND `body.status` equals `"accepted"`
- AND `body.decision` contains the decision excerpt

#### Scenario: Markdown shows truncation note when graph is capped

- GIVEN the graph block was capped at 30 nodes (as in the node count cap scenario)
- WHEN the markdown block is built
- THEN `body.content` includes a truncation notice (e.g., "Showing 30 of N traced artifacts")

#### Scenario: Empty rationale shows placeholder

- GIVEN the Decision node has no rationale edges
- WHEN the markdown block is built
- THEN `body.content` includes "No traced artifacts" message
- AND the ADR title and status are still rendered

### Requirement: Feature Gate — Multimodal

The `DecisionTraceExecutor`'s `build()` method SHALL require the `multimodal` feature flag. When `multimodal` is disabled, calling `build()` MUST return `ExplorerError::FeatureDisabled`.

`ViewDescriptor` methods (`id()`, `title()`, `applies_to()`, `view_kind()`, `renderer_kind()`) MUST be available regardless of the feature flag.

#### Scenario: Disabled without multimodal

- GIVEN the binary is compiled without the `multimodal` feature
- WHEN `DecisionTraceExecutor::build()` is called
- THEN `ExplorerError::FeatureDisabled` is returned

#### Scenario: Descriptor metadata always available

- GIVEN any build configuration
- WHEN `DecisionTraceExecutor::id()` or `title()` is called
- THEN the expected static string is returned

## Coverage

- **Happy paths**: covered (graph + markdown blocks, edge labels, ADR excerpt)
- **Edge cases**: covered (empty subgraph, 30-node cap, truncation note, placeholder message)
- **Error states**: covered (non-DecisionArtifact → ViewNotAvailable, no-multimodal → FeatureDisabled)
