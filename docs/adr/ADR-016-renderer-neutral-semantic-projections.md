# ADR-016: Renderer-neutral, evidence-grounded semantic projections

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-29
**Deciders**: User, OpenCode orchestrator

## Context

CogniCode has graph views, C4 projections, ViewSpecs, and multiple renderers,
but no common semantic contract connects query results to presentation.
Individual view builders emit ad hoc shapes, and renderers can infer structure
that the graph never established. Parent edges and relationship kinds can be
lost, while sequence-like diagrams can imply ordering without ordered evidence.

ADR-014 deliberately excludes new `GraphTopology` and `FlowTrace` contracts.
A separate decision is therefore required before diagram and Explorer work can
claim semantic fidelity.

## Decision

CogniCode will introduce `SemanticProjection` between typed query results and
all presentation adapters:

```text
GraphPlan -> ResultSet -> SemanticProjection -> Renderer
```

### 1. Structural and ordered projections are distinct

`SemanticProjection` carries one renderer-neutral payload variant:

- `Topology(GraphTopology)` for structural graph and hierarchy facts;
- `Flow(FlowTrace)` for ordered behavior;
- `Table(TypedRows)` for object selection, quality, and analytics rows;
- `Document(DocumentProjection)` for source, code, and markdown content;
- `UnstructuredJson(TypedJson)` only for an explicitly selected JSON view;
- `Composite(Vec<SemanticProjection>)` for intentionally composed views.

`GraphTopology` represents structural facts. It preserves exact node identity,
edge identity, parent edges, edge kinds, provenance, confidence, and truncation.
It does not impose execution order.

`FlowTrace` represents ordered behavior. Every participant, step, and message
must derive from ordered evidence and carry provenance. Missing ordered evidence
produces an explicit unsupported or incomplete capability state, not an
invented sequence.

### 2. Renderers do not own semantics

Graph, Mermaid, table, tree, code, Vega-Lite, JSON, composite, and future
renderers may choose layout and visual styling. They must not synthesize
architecture boundaries, relationships, ordering, identity, confidence, or
capability support.

Diagram languages such as Mermaid and PlantUML remain derived adapters, not the
domain model.

All supported `MoldPlan` result families cross this projection boundary. Raw
JSON is permitted only when a ViewSpec explicitly selects the JSON renderer and
the projection declares `UnstructuredJson(TypedJson)`; it is never an implicit
fallback.

### 3. Every projection reports epistemic state

Each projection declares capability status, confidence, provenance, warnings,
and truncation. Unsupported evidence and limits must remain visible to REST,
MCP, Explorer, and persisted artifacts.

### 4. Semantic models validate before rendering

Type hierarchy, C4, use-case, data-flow, impact, and vertical-slice models
validate their domain invariants before producing `GraphTopology` or
`FlowTrace`. Human overrides are explicit evidence and never silently replace
extracted facts.

## Alternatives considered

### Let each renderer infer its own structure

Rejected. It creates semantic divergence and diagrams that cannot be audited.

### Flatten every view into generic JSON blocks

Rejected. Generic blocks lose structural and ordered invariants.

### Use Mermaid or PlantUML as the domain model

Rejected. Presentation syntax cannot preserve CogniCode identity, provenance,
confidence, and capability semantics without becoming a second domain model.

### Derive sequences from unordered call topology

Rejected. Reachability does not prove runtime order.

## Consequences

### Positive

- Every renderer consumes the same evidence-grounded semantics.
- Diagram output becomes inspectable, testable, and reproducible.
- Missing evidence is explicit instead of hidden by rendering heuristics.

### Negative

- Existing C4, impact, hierarchy, and flow views require adaptation.
- Adds a domain layer between result execution and presentation.
- Ordered views remain unavailable until extraction provides ordered evidence.

### Mitigations

- Keep `GraphTopology` and `FlowTrace` small and renderer-neutral.
- Allow only minimal adapter changes in E29.2; presentation work belongs to
  E29.3.
- Add invariant tests for edge fidelity, provenance, and truncation.

## References

- [E29.2 proposal](../../openspec/changes/e29-2-semantic-projection-kernel/proposal.md)
- [E29.3 proposal](../../openspec/changes/e29-3-moldable-explorer-runtime/proposal.md)
- [ADR-003](./ADR-003-diagram-representations.md)
- [ADR-004](./ADR-004-c4-investigation-model.md)
- [ADR-014](./ADR-014-moldql-pattern-graph-analytics-platform.md)
- [Graph stack assessment](../analysis/cognicode-graph-stack-assessment.md)

## Implementation Log

- **2026-08-10 (E31-C)**: Renderer-neutral semantic projections implemented in e29-2-semantic-projection-kernel. The ProjectionPort trait separates the projection definition from any specific renderer (PaneInspector, GraphView, InteractiveGraph).
