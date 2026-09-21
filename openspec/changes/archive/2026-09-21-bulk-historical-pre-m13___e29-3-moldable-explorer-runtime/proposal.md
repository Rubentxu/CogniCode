# Proposal: E29.3 — Moldable Explorer Runtime

> Change: `e29-3-moldable-explorer-runtime` · Branch: `feat/e29-3-moldable-explorer-runtime` · Depends: E29.1, E28.2 runtime closure, and E29.2

## Intent
The moldable view runtime has domain vocabulary (Ph0), an authoring wizard (Ph4), a renderer registry (Ph3), and a `PostgresViewSpecStore` — but they are **disconnected**. `execute_view_spec` returns `FeatureDisabled`; `PaneInspector` dispatches by `view_kind`, not `renderer_kind`; unknown renderers silently degrade to JSON; saved specs never execute. E29.3 closes the loop: persist → search → register → execute → render, end-to-end.

## Scope

### In Scope
- Wire `PostgresViewSpecStore` through persistence/search/registry/execution
- REST CRUD + `POST /api/viewspecs/:id/execute`
- `renderer_kind` controls EVERY `PaneInspector` dispatch
- Complete renderer states: graph, table, tree, code, markdown, vega-lite, JSON, composite, **mermaid**
- Explicit unsupported-renderer state (no silent JSON fallback)
- Discoverable intent entry points; responsive 320/768/1280; WCAG AA incl. contrast
- Loading/empty/error/truncation/revision-change E2E
- Persisted JSONata transform parity between authoring preview and frontend runtime rendering

### Out of Scope
- Implementing MoldQL Pattern Profile v1 (E28.3) or Analytics Registry Cohort 1
  (E28.4); those changes add independent integration-conformance gates after
  their own surfaces ship
- JSONata execution in Rust (frontend preview and runtime use the same sandboxed evaluator); remote/plugin renderers (out of v1)

## Capabilities

> CONTRACT with sddk-spec.

### New Capabilities
- `viewspec-runtime-execution`: execute pipeline `ViewSpec → MoldQL → ContextualView`, pinned to a graph revision; replaces the `execute_view_spec not implemented` stub. **The premature canonical `openspec/specs/viewspec-runtime-execution/spec.md` is deleted in this change; the capability is introduced as a new spec inside the change, including runtime registration / applicability discovery requirements.**
- `viewspec-rest-api`: REST `POST/GET/PUT/DELETE /api/viewspecs` + execute wired to `PostgresViewSpecStore`. This uses a dedicated capability namespace rather than colliding with the existing `explorer/diagram-snapshot-export` spec.

### Modified Capabilities
- `view-spec-domain`: add `mermaid` to `RendererKind` enum (Rust + TS).
- `viewspec-authoring-flow`: wizard saves round-trip through the real store AND execute; status feedback.
- `renderer-registry-frontend`: dispatch keyed by `renderer_kind`; add `mermaid`; explicit unsupported-renderer state replacing silent JSON fallback.
- `pane-navigation`: dispatch path moves `view_kind` → `renderer_kind`; add loading/empty/error/truncation/revision-change states.

### GraphPlan Reconciliation

The runtime execute pipeline lowers `ViewSpec.data_source` MoldQL through
`MoldqlAstLowerer` to `MoldPlan`. Graph-selecting variants carry a pinned
`GraphPlan` and execute through `GraphExecutor`; object selection, quality,
lens, and view execution retain their typed operations. The pipeline consumes
the shipped plan contracts; **no changes to
`crates/cognicode-core/src/domain/plan/graph_plan.rs` are required by E29.3**.

## Approach
The execute entry lowers `ViewSpec.data_source` MoldQL to `MoldPlan`, dispatches
the applicable typed read-only operation, then applies E29.2 semantic
projections and materialises a `ContextualView` carrying `renderer_kind`.
`MoldPlan::Graph` uses the pinned E28.2 `GraphExecutor` path. `PaneInspector`
reads `renderer_kind` and dispatches via `rendererRegistry`; a missing renderer
yields `UnsupportedRendererState`, never silent JSON.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `crates/cognicode-explorer/src/facades/view.rs` | Modified | implement `execute_view_spec` |
| `crates/cognicode-explorer/src/api.rs` | Modified | REST CRUD + execute routes |
| `crates/cognicode-explorer/src/registry.rs` | Modified | wire runtime `spec_store` path |
| `crates/cognicode-explorer/src/dto.rs` | Modified | `RendererKind::Mermaid` |
| `apps/explorer-ui/src/components/ObjectInspector/PaneInspector.tsx` | Modified | `renderer_kind` dispatch |
| `apps/explorer-ui/src/components/rendererRegistry.ts` | Modified | mermaid + unsupported state |
| `apps/explorer-ui/e2e/` | New | state + responsive E2E |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `renderer_kind` dispatch breaks existing graph routing | High | parity E2E plus an explicit, observable emergency rollback flag |
| Stale-revision execution returns wrong graph | Med | pin `RevisionId`; revision-change E2E |
| Entropy creep across renderers | Med | connascence budget gate in verify |

## Rollback Plan
`renderer_kind` is the sole normal dispatch path. During stabilization, an
explicit `runtime_renderer_dispatch` emergency rollback flag may temporarily
restore the old `isGraphViewKind` path. The flag defaults `true`; rollback mode
is therefore disabled by default, observable in diagnostics, and removed after
parity evidence is retained.
Store/execute are additive; no schema migration. The shipped
`crates/cognicode-core/src/domain/plan/graph_plan.rs` contract remains untouched.

## Dependencies
- E29.1 (truthful revision pinning)
- `e28-2-runtime-closure` (production `GraphExecutor` composition)
- E29.2 (renderer-neutral semantic projections)

## Integration Gates

- E28.3 MUST prove that Pattern Profile ViewSpecs execute through this runtime
  before E28.3 is called user-visible.
- E28.4 MUST prove that analytics-backed ViewSpecs execute through this runtime
  before E28.4 is called user-visible.
- These gates do not block the core E29.3 runtime closure.

## Success Criteria
- [ ] `execute_view_spec` returns a real `ContextualView`, not `FeatureDisabled`
- [ ] `renderer_kind` drives every `PaneInspector` dispatch
- [ ] Unknown renderer → explicit unsupported state, not silent JSON
- [ ] E2E green: loading/empty/error/truncation/revision-change at 320/768/1280
- [ ] WCAG AA contrast passes across all renderer states
- [ ] Entropy budget held: connascence Δ low, no new coupling seams introduced
