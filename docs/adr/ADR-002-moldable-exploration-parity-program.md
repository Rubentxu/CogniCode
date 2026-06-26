# ADR-002: Moldable exploration parity program

**Status**: PROPOSED  
**Date**: 2026-06-25  
**Deciders**: User, OpenCode orchestrator session 2026-06-25

## Context

CogniCode already implements a substantial moldable-development substrate:

- `ViewRegistry` + `ViewDescriptorProvider` + `ViewExecutor` in
  `crates/cognicode-explorer/src/{registry.rs,domain/views.rs}`.
- `PostgresViewSpecStore` in
  `crates/cognicode-explorer/src/view_spec_store.rs`.
- `EntryPoint` + `ResolvedEntryPoint` in
  `crates/cognicode-explorer/src/domain/entry_point.rs`.
- `MoldQL` execution in
  `crates/cognicode-explorer/src/facades/moldql.rs` and
  `crates/cognicode-explorer/src/moldql/*`.
- Frontend `RendererRegistry`, `PaneStackView`, `PaneInspector`,
  `ViewSpecWizard`, and `Spotter` in `apps/explorer-ui/src/components/*`.
- Graph analytics in `cognicode-core` and `cognicode-graph-algos`, with
  selective WASM exports in `cognicode-graph-wasm`.

However, the system does **not** yet offer feature parity with GToolkit's
full moldable development experience.

The investigation performed on 2026-06-25 showed these concrete gaps:

1. **Catalog > implementation gap**.
   `ViewRegistry.known_view_kinds()` lists a broad catalog including
   `ComposedNarrative`, `ProjectDiary`, `ExampleObject`, `ConceptMap`,
   `EvidencePack`, `UsageExamples`, `ApiSurface`, `DocCodeAlignment`,
   `TestSlice`, `DebugSlice`, and others, but only 9 real executors are
   wired today in `registry.rs:336-382`:
   `overview`, `call-graph`, `source`, `quality`, `evidence`, `symbols`,
   `dependencies`, `hotspots`, `architecture-drift`.

2. **No Lepiter-equivalent runtime**.
   `project_diary`, `composed_narrative`, and `example_object` exist as
   `ViewKind`s and appear in `ViewSpecWizard`, but there is no executable
   notebook or programmable-document runtime behind them.

3. **Spotter is not universal**.
   `SpotterSearchResult` currently supports only `Symbol` and `ViewSpec`
   (`crates/cognicode-explorer/src/dto.rs:53-60`). It does not search ADRs,
   docs, evidence, issues, narratives, examples, or runtime objects as
   first-class results.

4. **No contextual editor**.
   The current UX includes a JSONata textarea (`TransformStep.tsx`) and live
   preview, but there is no real contextual code editor with references,
   completion, peek, or graph-aware edit operations.

5. **Core capabilities are not fully surfaced to exploration UX**.
   `cognicode-core` already exposes `get_entry_points()` and
   `get_hot_paths()` (`workspace_session.rs:1198,1443`), and
   `cognicode-graph-algos` already exposes graph computations such as
   `god_nodes`, `page_rank`, `communities`, `feedback_arc_set`,
   `surprising_connections`, and `transitive_reduction`. These are not yet
   systematically turned into contextual views, narratives, or Spotter
   surfaces.

This means we have the **substrate** of moldable exploration, but not yet the
**density of contextual tools**, the **narrative runtime**, or the
**self-discoverable object experience** that define GToolkit's practical power.

## Decision

We will pursue **functional parity in moldable exploration workflows** with
GToolkit through a phased program that preserves the current Rust/TypeScript
architecture and deepens it incrementally.

This decision has five parts:

### 1. Scope the parity target honestly

The target is **not** to recreate the full Smalltalk/Pharo reflective image.
We are **not** promising live image editing or 1:1 implementation parity with
GToolkit internals.

We **are** targeting functional parity for the exploration experience around:

- **Objects** — every relevant software/domain artifact can be inspected.
- **Representations** — each object can have multiple contextual views.
- **Navigability** — views open laterally, preserve narrative, and support
  drill-down without losing the path.
- **Self-discovery** — users and AI can discover relevant views, narratives,
  and related objects from context, not from documentation alone.
- **Narrative explainability** — durable knowledge can be built as executable
  or object-backed narratives.

### 2. Keep the current crate responsibilities and sharpen them

We will not collapse the system into one giant crate. We will deepen the
existing architecture:

#### `cognicode-explorer`

Owns the **moldable shell**:

- object inspection
- view execution
- view registries
- renderer registries
- Spotter UX
- pane-stack navigation
- runtime ViewSpec authoring
- narratives/diaries/examples UX
- contextual editor UX
- MCP exposure of exploration capabilities

#### `cognicode-core`

Owns the **semantic object and query layer**:

- object identity and domain vocabulary
- workspace session and query orchestration
- entry points, hot paths, ownership, test slices, impact slices
- narrative/document domain objects when they are more than UI state
- cross-cutting explainability services that should not depend on React/UI

#### `cognicode-graph-algos`

Owns the **graph-computation substrate**:

- clustering
- communities
- path discovery
- graph summarisation
- graph compression / ranking
- structural risk algorithms
- future "view-ready summaries" that views can render deterministically

`cognicode-graph-wasm` remains the optional browser acceleration layer for
algorithms already defined in `cognicode-graph-algos`.

### 3. Treat catalog-only capabilities as debt, not as shipped functionality

Any `ViewKind` that appears in `ViewRegistry.known_view_kinds()` or the
`ViewSpecWizard` but lacks a real `ViewExecutor` (or equivalent runtime
implementation) is considered **capability debt**.

This includes, at minimum:

- `ComposedNarrative`
- `ProjectDiary`
- `ExampleObject`
- `ConceptMap`
- `EvidencePack`
- `UsageExamples`
- `ApiSurface`
- `DocCodeAlignment`
- `TestSlice`
- `DebugSlice`
- `OwnershipMap`
- `RiskMap`
- `ChangeImpactStory`
- `DecisionTrace`

These view kinds stay in the catalog because the vocabulary is strategically
correct, but we will stop speaking about them as if they were already
implemented.

### 4. Execute the parity program in explicit phases

The roadmap program is:

#### Phase 0 — Foundation closures (completed)

- `e10-landing-real-data` ✅ — wire real entry points/hot paths into landing.
- `e9-landing-perf` ✅ (v0.26.0, PR #61) — virtualise node-list fallback (windowed list > 200 nodes).
- `e11-context-response-field-naming` ✅ (v0.26.1, PR #62) — harmonise truncation field names.
- `e12a-usage-examples` ✅ (v0.27.0, PR #63) — UsageExamplesExecutor wired as 10th executor (Phase 1).

These are not the parity program itself, but they remove local defects and
prepare exploration surfaces for Phase 1.

#### Phase 1 — View coverage realization (e12a–e12h)

Goal: convert high-value catalogued `ViewKind`s into real executors/renderers.

**e12a-usage-examples** ✅ (completed 2026-06-26, v0.27.0, PR #63): `UsageExamplesExecutor`
wired as the 10th real executor. `build_usage_examples` returns callers + callees as
Table blocks. `renderer_kind: RendererKind::Table`. Frontend already has Table renderer.
Gracefully degrades when `graph_query` is None (empty blocks). 4 new tests in
`views.rs`. `registry.rs` entry added.

**e12b-api-surface** ✅ (completed 2026-06-26, v0.27.1, PR #64): `ApiSurfaceExecutor`
wired as the 11th real executor. `build_api_surface` returns all scope symbols sorted
by name as Table blocks. Columns: name, kind, file, line. V1 pragmatic: shows all
symbols (no visibility filter — `ResolvedSymbol` has no visibility field). 4 new tests.
Registry entry + static instance added.

Wave 1 remaining (reordered 2026-06-26):

- `e12c-test-slice` — TestSlice as 12th executor. Shows test functions that call a given symbol. Uses `GraphQueryPort.callers()` + test-path heuristics (files named `_test.rs` or paths containing `/tests/`). DocCodeAlignment is deferred: `EntryPoint::Doc` is not wired to any `InspectionTarget`, and there is no DocService or `ObjectIdentity::Doc` — implementing it now would return `ViewNotAvailable` always.
- `OwnershipMap`
- `DebugSlice`
- `DocCodeAlignment` — deferred to after Doc infrastructure is built (Phase 2+)

Success criterion: the number of real executors grows materially beyond the
current 9, and every newly exposed view kind is reachable from the inspector.

#### Phase 2 — Universal Spotter (`e13-*`)

Goal: make Spotter a genuine universal discovery surface.

Expand `SpotterSearchResult` and `SearchServiceImpl` to search and return:

- docs
- ADRs / decisions
- evidence packs
- issues
- saved explorations
- narratives / diaries
- examples
- ViewSpecs
- code symbols/files/scopes

Success criterion: a user can start from one search box and discover most
important object families without knowing internal ids or menu locations.

#### Phase 3 — Narrative runtime (`e14-*`)

Goal: add a real equivalent of GToolkit's narrative/document layer.

This phase introduces working implementations of:

- `ComposedNarrative`
- `ProjectDiary`
- `ExampleObject`

and decides whether the runtime is:

- markdown + embedded views only,
- or markdown + embedded views + executable snippets,
- or object-backed narrative blocks persisted in Postgres.

Success criterion: architectural explanations and tutorials become first-class
runtime artifacts, not just markdown files checked into git.

#### Phase 4 — Contextual editor (`e15-*`)

Goal: close the gap between exploration and intervention.

Introduce a real contextual editor surface that understands:

- object identity
- graph navigation
- references / callers / callees
- view-driven edits
- symbol-aware completion / peek workflows

Success criterion: users can move from contextual exploration to targeted
change without dropping to a generic editor context for every operation.

#### Phase 5 — Federated runtime objects and explainable agent context (`e16-*`)

Goal: make more runtime/domain objects explorable and passable to agents as
first-class structured objects.

This phase deepens:

- federation surfaces
- saved explorations as navigable stories
- runtime objects in MCP
- explainable agent memory built from contextual views

Success criterion: AI and humans share the same structured exploration
surfaces, and the environment can explain what an agent saw and why.

### 5. Require proof before claiming parity

We will only claim "similar moldable exploration functionality" when all of
the following are true:

1. **View coverage**: high-value catalogued view kinds have real executors.
2. **Universal discovery**: Spotter searches the main object families.
3. **Narratives**: there is a working `ComposedNarrative` / `ProjectDiary`
   runtime, not just enum values.
4. **Examples**: `ExampleObject` is executable or object-backed, not just a
   planned view kind.
5. **Editor**: there is at least one contextual editor experience beyond a
   textarea-based transform step.

Until then, the honest language is: **"substantial moldable substrate, not
yet GToolkit-equivalent."**

## Alternatives considered

### A. Claim parity now because the substrate exists

Rejected.

Reason: the investigation showed that key capabilities exist only as catalog
entries or wizard options, not as executable exploration tools.

### B. Rewrite around a notebook-first platform

Rejected.

Reason: it would discard working exploration infrastructure already present in
`cognicode-explorer`, `cognicode-core`, and `cognicode-graph-algos`.

### C. Continue opportunistically without a named parity program

Rejected.

Reason: that would perpetuate the current drift between vocabulary and
implementation. A phased program makes the gaps explicit and reviewable.

### D. Aim for exact Smalltalk/Pharo parity

Rejected.

Reason: that is architecturally inappropriate for the current stack. We are
targeting **functional exploration parity**, not reflective image-level parity.

## Consequences

### Positive

- Gives us an honest public narrative about current capability.
- Converts the existing "planned but unimplemented" catalog into an explicit
  execution program.
- Reuses the architecture we already invested in.
- Aligns `cognicode-explorer`, `cognicode-core`, and `cognicode-graph-algos`
  around clear responsibilities.
- Makes future claims measurable.

### Negative

- Commits us to a multi-cycle program rather than a one-PR feature.
- Forces us to maintain documentation discipline: the catalog and the runtime
  implementation can no longer drift silently.
- Some phases (especially narratives and contextual editor) are materially
  larger than a patch-level iteration.

### Mitigations

- Keep each phase split into reviewable cycles.
- Update the roadmap after every phase with real counts (executors shipped,
  Spotter result families, narratives enabled, etc.).
- Prefer proof-driven language in docs and PRs.

## e11 Field Naming (PATCH — completed 2026-06-26)

`ContextualGraphResponse` used `truncation_reason` (extra 'i') while
`LandingPayload` and `SubgraphResponse` used `truncated_reason` (no 'i').
Both carried the same semantic meaning.

Resolution: renamed `ContextualGraphResponse.truncation_reason` →
`truncated_reason`. Wire format: `truncationReason` → `truncatedReason`.
The old `truncationReason` camelCase alias is accepted on deserialisation
(serde `alias = "truncationReason"`) for wire-compatible migration.
Remove the alias in the next MAJOR release.

Files changed:
- `crates/cognicode-explorer/src/dto.rs` — renamed field + alias
- `crates/cognicode-explorer/src/facades/view.rs` — variable rename
- `crates/cognicode-explorer/src/dto_tests.rs` — updated + backwards-compat test
- `crates/cognicode-explorer/src/api_graph_tests.rs` — updated
- `apps/explorer-ui/src/api/schemas.ts` — Zod field rename
- `apps/explorer-ui/src/components/ContextualPanel/index.tsx` — property access
- `apps/explorer-ui/src/components/ContextualPanel/ContextualPanel.test.tsx` — test data
- `apps/explorer-ui/src/mocks/handlers.ts` — mock response update

## References

- `CONTEXT.md:326-344` — current terminology mapping vs GToolkit.
- `crates/cognicode-explorer/src/registry.rs:336-382` — currently wired real
  executors.
- `apps/explorer-ui/src/components/ObjectInspector/ViewSpecWizard.tsx:157-220`
  — catalog of view kinds exposed to users.
- `crates/cognicode-explorer/src/dto.rs:53-60` — `SpotterSearchResult` currently
  supports only `Symbol` and `ViewSpec`.
- `crates/cognicode-core/src/application/workspace_session.rs:1198,1443` —
  `get_entry_points()` and `get_hot_paths()` already exist in core.
- `crates/cognicode-graph-algos/src/algorithms/*` — graph algorithms available
  for future contextual tools.
- Investigation memory: `Verified GToolkit parity gaps with code evidence`
  (`architecture/gtoolkit-parity-gap`).
