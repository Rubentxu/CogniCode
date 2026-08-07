# Proposal: E29.2 — Semantic Projection Kernel

> Change: `e29-2-semantic-projection-kernel` · Branch: `feat/e29-2-semantic-projection-kernel` · Mode: A-lite · Strict TDD: ACTIVE

## Intent
The graph stack assessment (§1.1) shows no contract linking extraction → projection → rendering: structural views emit ad-hoc shapes, C4 is a heuristic directory toggle (ADR-004), and sequence/dynamic views don't exist. Renderers invent architecture the model never declared. E29.2 introduces a renderer-neutral **SemanticProjection** contract so every view produces honest, evidence-grounded data *before* any renderer draws.

## Scope

### In Scope
- Renderer-neutral `SemanticProjection` contract across graph, row, document, and composite result families
- `GraphTopology`: exact nodes / edges / **parent-edge** / **edge-kind** preserved
- `FlowTrace`: ordered participants / messages + provenance
- Typed inheritance / implements / member relations → `TypeHierarchyModel`
- Honest capability status / confidence / truncation on every projection
- Rebuild call, dependency, impact, C4, type-hierarchy, use-case, data-flow projections
- Sequence derived **only** from ordered evidence

### Out of Scope
- Renderer implementation (Cytoscape / Mermaid / ELK adapters — E29.3)
- Speculative UML generation without factual edges
- PlantUML as model (adapter only — assessment §14.5)

## Capabilities

> CONTRACT with sddk-spec. The shipped E28.1/E28.2 `graph_plan.rs` contract stays FROZEN — projections consume `GraphPlan`/`ResultSet`, never modify it.

### New Capabilities
- `semantic-projection-kernel`: `SemanticProjection` trait + `GraphTopology`, `FlowTrace`, `TypedRows`, `DocumentProjection`, and composite payload contracts; capability/status/confidence ∈ [0,1]/truncation envelope.
- `flow-trace`: ordered-step execution model for vertical-slice / sequence / data-flow; every step MUST carry provenance, never synthesized.
- `type-hierarchy-projection`: inheritance/implements/member relations as a validated `TypeHierarchyModel` before topology projection.
- `c4-semantic-projection`: Context/Container/Component/Code from manifests+IaC+routes (ADR-004), human overrides, **no silent 200 cap**.
- `visualization-stack`: establishes the renderer-neutrality rule — renderers consume projections, never synthesize structure. **No canonical base exists**; this slice introduces the capability as a new spec.

### Modified Capabilities
- `contextual-views`: contextual endpoint emits `GraphTopology`, preserving parent-edge + edge-kind (currently drops both).

### Scope Reconciliation (Renderer Adapters)

E29.2 MAY adapt existing renderer adapters *only enough* to consume
`SemanticProjection` values (i.e., read the projection envelope and
render the typed `GraphTopology`/`FlowTrace` it carries). E29.2
MUST NOT redesign UI presentation, introduce new visual layouts,
or change how renderers map projections to pixels — those concerns
are owned by E29.3 (`moldable-explorer-runtime`). Renderer-adapter
edits in E29.2 are strictly bounded to the minimum changes needed
to keep pixels honest with the projection envelope
(capability status, confidence, provenance, truncation).

## Approach
Projections sit above typed `MoldPlan` results. Graph operations map
`GraphPlan → ResultSet` into `GraphTopology` or `FlowTrace`; object selection,
quality, lens, source, markdown, explicit JSON, and composed results map to
`TypedRows`, `DocumentProjection`, `UnstructuredJson`, or `Composite`. Each
model validates its invariants before projecting.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-core/src/domain/projection/` | New | Complete `SemanticProjection` payload algebra |
| `crates/cognicode-explorer/src/views/` | Modified | Graph, flow, row, document, JSON, and composite view adapters |
| `crates/cognicode-core/src/domain/plan/graph_plan.rs` | **Preserved** | Shipped E28.1/E28.2 contract, untouched |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Topology drops parent-edge/kind (regression) | Med | invariant tests assert exact edge fidelity |
| Sequence synthesized without evidence | High | FlowTrace step MUST carry provenance; no-provenance ⇒ reject |
| Entropy budget blowup on projection fan-out | Med | budget ≤ +0.08 DQS; gate per projection |

## Entropy Budget
One indirection layer risks name-coupling to `GraphPlan`. Budget: **DQS delta ≤ +0.08**; connascence ≤ `Name` (no `Algorithmic`/`Position`); breach ⇒ refactor, never ship.

## Rollback Plan
Pure additive: no `graph_plan.rs` / E28.1 / E28.2 change and no schema migration. Existing renderer adapters may receive only the minimal compatibility edits defined in Scope Reconciliation; `git revert` removes those adapters with the projection layer. No canonical graph data is affected.

## Dependencies
- E29.1 (temporal graph history and atomic ingest)
- `e28-2-runtime-closure` (E28.2 executors wired into runtime)
- Graph stack assessment §15; ADR-003 (Mermaid derived), ADR-004 (C4 model)

## Success Criteria
- [ ] Every structural view emits `GraphTopology` with exact node/edge/parent-edge/kind
- [ ] `FlowTrace` steps carry provenance; none synthesized
- [ ] C4 declares capability honestly; no silent 200-cap
- [ ] `graph_plan.rs` byte-identical to E28.1
- [ ] Entropy budget respected (DQS delta ≤ +0.08)
