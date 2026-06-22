# ADR-008: Moldable View Runtime

**Fecha:** 2026-06-12  
**Estado:** PROPOSED (implementation partly complete; promotion blocked on Validation gap)  
**Decisión:** Hybrid backend/frontend runtime for moldable view discovery and custom view authoring  
**Fuente:** grill-with-docs + gtoolkit research + repository fit analysis  
**Confianza:** alta  
**Última revisión de implementación:** 2026-06-22 — 4/6 Validation checkboxes satisfied, 1 partial, 1 gap (MCP ViewSpec tooling missing). See **Implementation status** section below.

---

## Context

CogniCode wants to recreate the useful parts of Glamorous Toolkit's moldable UI: navigability, auto-discovery, many views per object, custom views, hierarchy exploration, entry points, and extensibility.

In Glamorous Toolkit, a contextual view is defined as a method annotated with `<gtView>`. The inspector discovers these methods dynamically through the Smalltalk runtime. This makes views cheap to create and immediately visible while exploring a domain.

Rust does not provide equivalent runtime reflection. A pure Rust trait-based registry gives compile-time safety, but it does not support instant user-defined views without recompilation. The Explorer UI is the primary consumer of these custom views; MCP consumers can receive degraded non-visual access.

## Decision

Adopt a **Moldable View Runtime** composed of four layers:

1. **Built-in ViewRegistry**
   - Rust-defined core views.
   - Discovered through `linkme`/distributed slices or a LensRegistry-style trait-object registry.
   - Used for stable views such as overview, call graph, source, evidence, quality, dependencies, and hotspots.

2. **Runtime ViewSpec Store**
   - User-created views are stored as declarative JSON data, not Rust code.
   - A ViewSpec separates semantic intent (`ViewKind`) from visual rendering (`RendererKind`).
   - A ViewSpec describes what concept the user is exploring, what data to fetch, how to transform it, and which renderer should display it.
   - Runtime ViewSpecs appear in the Explorer immediately without recompiling the backend.

3. **Frontend RendererRegistry**
   - The Explorer owns visual rendering.
   - Renderer ids such as `graph`, `table`, `tree`, `code`, `vega-lite`, and `raw-json` map to React components.
   - Backend sends ViewSpecs and data; frontend chooses the concrete renderer.
   - The Explorer provides the primary authoring workflow for runtime ViewSpecs.

4. **Advanced Extension Host**
   - Future/pro tier for remote renderer components or plugin-based custom visualizations.
   - Inspired by VS Code contribution points, custom editors, and webviews.
   - Explicitly out of scope for v1.

v1 supports only built-in renderers plus declarative ViewSpecs. External
plugins, remote React renderers, Module Federation runtime remotes, WASM view
plugins, and embedded scripting runtimes are not part of v1.

The minimal ViewSpec shape is:

```json
{
  "id": "hot-symbols",
  "title": "Hot Symbols",
  "applies_to": "Scope",
  "view_kind": "quality_hotspots",
  "data_source": {
    "kind": "moldql",
    "query": "symbols where fan_out > 5"
  },
  "transform": {
    "language": "jsonata",
    "expression": "nodes[fan_out > 5]"
  },
  "renderer_kind": "table",
  "props": {
    "columns": ["label", "kind", "fan_out"]
  }
}
```

The first-class `ViewKind` values include, at minimum:

- `vertical_slice`
- `call_graph`
- `seam_map`
- `c4_context`
- `c4_container`
- `c4_component`
- `c4_code`
- `dependency_graph`
- `source_view`
- `quality_hotspots`
- `evidence_view`
- `decision_graph`
- `diff_view`
- `data_flow`
- `impact_radius`

The full first-class catalog also reserves architecture, development, and
living-documentation views. These names are part of the domain vocabulary even
when implementation is phased:

**Architecture views**

- `architecture_rationale` — explains why a structure exists using ADRs, decisions, evidence, and related code.
- `architecture_drift` — shows where code diverges from ADRs, expected C4 structure, or documented boundaries.
- `boundary_map` — shows boundaries between modules, crates, layers, bounded contexts, or components.
- `dependency_pressure` — highlights modules with excessive incoming or outgoing dependencies.
- `change_impact_story` — explains what a change affects, who depends on it, and which tests/docs should move with it.
- `ownership_map` — shows ownership of crates, modules, ADRs, issues, components, or slices.
- `risk_map` — combines hotspots, churn, complexity, debt, and criticality.
- `decision_trace` — connects ADRs → code → tests → docs → issues.

**Development views**

- `test_slice` — connects an entry point to the tests that cover that flow.
- `debug_slice` — connects an error, crash, or log to probable execution paths and relevant symbols.
- `refactor_plan` — shows what to move or change, affected dependencies, and a safe order of operations.
- `callers_and_implementors` — shows callers, callees, trait implementors, and related usage.
- `usage_examples` — shows real usages of a function, type, module, API, or ViewSpec.
- `api_surface` — shows public API of a crate/module plus stability and consumers.
- `dead_code_candidates` — shows symbols with no callers or no observable use.
- `semantic_search_results` — treats search results as a moldable collection rather than a flat list.

**Living documentation views**

- `doc_code_alignment` — compares docs/ADRs/concepts with the code that implements them.
- `example_object` — executable or reproducible example that materializes a concept.
- `composed_narrative` — navigable story made of objects, views, evidence, and explanations.
- `project_diary` — technical diary for decisions, experiments, snippets, and linked artifacts.
- `concept_map` — map of domain terms and their relationships to code, ADRs, issues, and evidence.
- `evidence_pack` — bundle of evidence used to justify a decision, change, or review outcome.

`project_diary` and `composed_narrative` are the v1 living-documentation
equivalent of Lepiter. They use markdown narrative plus embedded ViewSpecs,
linked objects, evidence packs, and decision traces. Executable snippets are a
future capability and are not required for v1.

The first-class `RendererKind` values include, at minimum:

- `graph`
- `table`
- `tree`
- `code`
- `markdown`
- `vega-lite`
- `json`
- `composite`

The first-class `HierarchyKind` values for v1 include:

- `file_tree` — workspace → directories → files
- `module_tree` — crate → module → items
- `type_hierarchy` — traits, impls, inheritance-like relations, and implementors
- `call_hierarchy` — callers and callees
- `package_graph` — crates, packages, and dependency relationships
- `c4_hierarchy` — system → container → component → code

Hierarchy kinds are not renderers. They are structural data projections used by
ViewSpecs and normally rendered with `tree`, `graph`, or `composite` renderers.

Runtime ViewSpecs are authored primarily through the Explorer:

1. Inspect an object.
2. Select **Create custom view**.
3. Choose a `ViewKind`.
4. Choose a `RendererKind`.
5. Select a data source.
6. Adjust a JSONata transform.
7. Preview the view live.
8. Save the ViewSpec.

Raw JSON editing is allowed as an advanced/debug path, but it is not the
primary authoring flow.

`MoldQL` is the query language for ViewSpec data sources. It selects objects,
graph relations, docs, evidence, and architecture artifacts. It does not
describe visual layout. Layout remains the responsibility of `RendererKind`,
renderer props, and the frontend `RendererRegistry`.

Examples:

```text
symbols where kind = "function" and fan_out > 5
calls from "UserService::create_user" depth 3
docs citing adr "ADR-008"
```

Entry points resolve into views through a common pipeline:

```text
User input
  ↓
EntryPointResolver
  ↓
ResolvedEntryPoint
  ↓
Default ViewKind selection
  ↓
ViewSpec or built-in view
  ↓
RendererRegistry
```

Each `ResolvedEntryPoint` kind has a default `ViewKind`, but the Explorer lets
the user switch to any applicable view after resolution:

| Entry point | Resolved kind | Default ViewKind |
|-------------|---------------|------------------|
| `POST /api/users` | `HttpRoute` | `vertical_slice` |
| `cognicode analyze` | `CliCommand` | `vertical_slice` |
| `UserCreated` | `Event` | `data_flow` |
| `CreateUser` | `UseCase` | `vertical_slice` |
| `UserRepository::save` | `Symbol` | `call_graph` |

Search/Spotter is a universal entry point. It searches symbols, files, modules,
entry points, ViewSpecs, ADRs, decisions, docs, issues, evidence, and saved
explorations. Results are represented as `semantic_search_results`, not as a
flat list, so the result set can be filtered, grouped, rendered differently,
saved as a ViewSpec, or opened as inspector panes.

## Rationale

- **Rust registry alone is not moldable enough.** It is safe and extensible for core developers, but it cannot provide instant user-defined views.
- **ViewSpec as data restores dynamism.** Users can create, edit, preview, and persist views from the Explorer without recompilation.
- **Semantic intent and rendering must not be conflated.** A vertical slice is not a renderer; it is a composite semantic view that may use graph, tree, code, and table renderers together.
- **Explorer-first authoring mirrors moldable development.** Users should turn an exploration path into a contextual view from the place where the discovery happens.
- **Entry point defaults reduce friction without removing choice.** CogniCode can open the most useful view automatically while keeping all applicable views available.
- **MoldQL should not become a second rendering language.** Keeping querying separate from rendering prevents a confused DSL that mixes data selection, transformation, and UI layout.
- **Frontend owns visual variety.** React already has the component ecosystem needed for graph, table, tree, code, and declarative chart renderers.
- **Backend owns semantic correctness.** Object resolution, entry point resolution, source data, graph data, and authorization remain server-side.
- **MCP can degrade.** MCP tools can list and inspect ViewSpecs, but they do not need to fully render browser-only visualizations.
- **JSONata is preferred for v1 transforms.** It is stronger than JSONLogic for reshaping, aggregation, grouping, and tabular projections.
- **Vega-Lite is preferred for declarative charts.** It provides a mature JSON grammar for interactive visualizations.

## Alternatives Considered

- **Pure trait-based ViewRegistry:** rejected as the only mechanism because every new custom view requires Rust code and recompilation.
- **Attribute macro only:** useful for built-in ergonomics, but still compile-time and not enough for runtime moldability.
- **YAML/TOML only:** weaker authoring and validation story than JSON ViewSpecs stored and validated by the backend.
- **Embedded JS runtime in backend:** postponed. QuickJS/Deno-style scripting adds security and operational complexity before there is proof that JSONata is insufficient.
- **WASM plugins for all custom views:** postponed. Powerful but too heavy for everyday view authoring; keep as future/pro extension tier.
- **Frontend-only custom views:** rejected because backend must remain the source of truth for object resolution, graph data, and source analysis.

## Consequences

- `ContextualView` and `ViewBlock` remain useful as stable transport types.
- `NamedView` should evolve from saved graph projection parameters into a more general persisted ViewSpec mechanism.
- The frontend `ViewBlock` switch should evolve toward a `RendererRegistry` map.
- Graph rendering should become embeddable inside the block/view pipeline, not isolated only in the right-side graph panel.
- Custom runtime views become Explorer-first; MCP support is best-effort.
- v1 scope stays intentionally bounded: no external plugin host, no remote React renderer loading, no WASM view plugins, and no embedded JS runtime.

## Validation

- [x] **Built-in views are discoverable without central hardcoded match arms.**
  Satisfied in commit `b1cb450` (Sprint E1.4 — block renderer registry).
  `apps/explorer-ui/src/components/ObjectInspector/ViewBlock.tsx` no longer
  contains a `case "..."` switch; dispatch goes through
  `blockRendererRegistry.get(block.id)`. 29 block renderers are registered
  at module load.

- [x] **A user can create a ViewSpec in the Explorer and see it immediately without backend recompilation.**
  `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx`
  exists with a 5-step authoring flow backed by `useWizardDraft.ts` and
  `TransformStep.tsx` (JSONata preview via `useJsonataPreview`).

- [x] **The same object lists both built-in views and persisted runtime ViewSpecs.**
  `apps/explorer-ui/src/api/schemas.ts:141` defines `available_views:
  z.array(viewDescriptorSchema)` on object summaries. `ViewTabs.tsx`
  consumes this list directly. Built-in descriptors come from the
  backend's `ViewDescriptor::is_builtin = true` flag
  (`crates/cognicode-explorer/src/dto.rs`); runtime ViewSpecs come from
  the persistence layer. The Inspector merges them.

- [x] **Unknown renderer ids degrade to raw JSON or a clear unsupported-renderer message.**
  `apps/explorer-ui/src/components/rendererRegistry.tsx:127` exposes
  `getOrJson(id)` which falls back to `#getJsonRenderer()` (label
  "JSON (fallback)") for any unknown renderer id. The registry's
  `render(id, body)` (line 141) is a convenience wrapper that always
  uses `getOrJson` internally.

- [ ] **MCP can list/read ViewSpecs even when it cannot render them visually.** ⚠️ **GAP**
  Current MCP surface (see
  `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs`):
  `handle_smart_search`, `handle_graph_analyze`, `handle_project_overview`,
  `handle_compare_graph`, `handle_codebase_map`, `handle_project_insights`,
  `handle_review_pr`, `handle_iac_query`, `handle_graph_diff`,
  `handle_ingest`, `handle_graph_timeline`, `handle_graph_checkpoint`.
  **No `list_view_specs` / `read_view_spec` tool exists yet.** This is
  the load-bearing gap blocking promotion of this ADR to ACCEPTED.
  Follow-up: implement MCP ViewSpec tooling as a separate SDDK cycle.

- [x] **JSONata transforms are sandboxed and bounded by execution limits.**
  `apps/explorer-ui/src/workers/jsonata.worker.ts:8` documents the
  enforcement: "100ms evaluation timeout via `setTimeout` +
  `worker.terminate()`". `useJsonataPreview.ts:51,62,76,87,95` wires
  the timeout into the preview hook with structured error reporting.
  The worker is lazy-loaded (no bundle impact until used).

## Implementation status

As of 2026-06-22 (post-v0.11.0):

| Validation checkbox | Status | Evidence |
|---------------------|--------|----------|
| Built-in views discoverable without match arms | ✅ Satisfied | Sprint E1.4 (v0.10.0) |
| ViewSpec creation in Explorer | ✅ Satisfied | `ViewSpecWizard.tsx`, `TransformStep.tsx` |
| Same listing for built-in + runtime | ✅ Satisfied | `available_views` schema, `ViewTabs.tsx` |
| Unknown renderer fallback | ✅ Satisfied | `getOrJson` in `rendererRegistry.tsx` |
| MCP ViewSpec tooling | ❌ Gap | No `list_view_specs` / `read_view_spec` handler |
| JSONata sandbox + limits | ✅ Satisfied | 100ms timeout via worker termination |

**5 of 6 satisfied.** The single gap (MCP ViewSpec tooling) blocks
promotion to ACCEPTED. This is intentional and documented as a
follow-up.

## Promotion criteria

This ADR will be promoted from PROPOSED to ACCEPTED when:

1. `list_view_specs` MCP tool exists and lists both built-in and
   persisted runtime ViewSpecs.
2. `read_view_spec` MCP tool exists and returns the full ViewSpec
   JSON (id, title, applies_to, view_kind, data_source, transform,
   renderer_kind, props) for any spec by id.
3. MCP tool schemas are registered in `rmcp_adapter.rs` alongside
   the other consolidated handlers.
4. Integration tests cover: list returns ≥1 built-in, list returns
   runtime spec after creation, read returns valid spec JSON.

A separate SDDK cycle (proposed name: `sddk/MCP-view-spec-tools`)
should implement these four items. Estimated effort: M (3-5 commits,
single PR).
