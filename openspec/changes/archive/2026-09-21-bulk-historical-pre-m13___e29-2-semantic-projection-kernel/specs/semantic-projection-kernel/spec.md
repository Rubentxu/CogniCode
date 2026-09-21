# Semantic Projection Kernel Specification

## Purpose
Define renderer-neutral, evidence-grounded projections over every supported `MoldPlan` result family without changing plan contracts. The kernel introduces `SemanticProjection`, `GraphTopology`, `FlowTrace`, `TypedRows`, and `DocumentProjection` value contracts so every view produces honest data before any renderer draws.

## ADDED Requirements

### Requirement: Projection envelope
Every projection MUST declare capability status, confidence in `[0,1]`, provenance, and truncation state. Unsupported or incomplete capabilities MUST be explicit and MUST NOT be represented as successful empty structure.

#### Scenario: Supported projection
- GIVEN a valid graph result with source evidence
- WHEN it is projected
- THEN the envelope reports supported status, bounded confidence, provenance, and `truncated=false`

#### Scenario: Unsupported projection
- GIVEN a requested relation or projection is not supported by available evidence
- WHEN projection is requested
- THEN the result reports unsupported status and MUST NOT synthesize nodes or edges

### Requirement: Projection payload covers supported result families

`SemanticProjection` MUST contain exactly one typed payload variant:
`Topology(GraphTopology)`, `Flow(FlowTrace)`, `Table(TypedRows)`,
`Document(DocumentProjection)`, `UnstructuredJson(TypedJson)`, or
`Composite(Vec<SemanticProjection>)`. Graph-selecting `MoldPlan` operations MUST
produce topology or flow where applicable. Object selection, quality, lens, and
analytics rows MUST produce `Table`; source and markdown results MUST produce
`Document`. A supported result MUST NOT bypass the projection envelope or be
coerced into graph topology.

#### Scenario: Quality rows produce a table projection

- GIVEN a quality operation returns typed hotspot rows
- WHEN the result is projected
- THEN the payload is `Table(TypedRows)` with field types and row order preserved
- AND no synthetic graph nodes are created

#### Scenario: Source result produces a document projection

- GIVEN a source view returns code lines and language metadata
- WHEN the result is projected
- THEN the payload is `Document(DocumentProjection)` with both preserved

#### Scenario: Explicit JSON view uses typed unstructured payload

- GIVEN a ViewSpec explicitly selects `RendererKind::Json`
- WHEN its supported result cannot use a more specific projection variant
- THEN the payload is `UnstructuredJson(TypedJson)`
- AND unknown renderer kinds MUST NOT use this variant as a fallback

### Requirement: Exact graph topology fidelity
A `GraphTopology` projection MUST preserve every selected node, edge endpoint, parent-edge relationship, and edge kind exactly as present in the evidence. It MUST NOT reinterpret, merge, reorder, or invent structural relations.

#### Scenario: Mixed edge kinds
- GIVEN nodes A and B are connected by `Calls`, and A has parent edge `LivesIn` to file F
- WHEN topology is projected
- THEN both edges retain exact endpoints and kinds, including the parent edge

#### Scenario: Missing parent
- GIVEN node A has no parent edge in evidence
- WHEN topology is projected
- THEN parent is absent and no inferred file or hierarchy edge is emitted

### Requirement: Structural view projections
Call, dependency, impact, use-case, and data-flow views MUST emit semantic projections whose relations correspond only to their named evidence; renderers MUST receive the projection before presentation.

#### Scenario: Call projection
- GIVEN A calls B and B calls C
- WHEN a call projection is requested
- THEN it contains exactly those call edges and no dependency-only edge

#### Scenario: Empty impact evidence
- GIVEN no evidenced dependent reaches target T
- WHEN impact is projected
- THEN the result is supported with an empty topology, not fabricated dependents
