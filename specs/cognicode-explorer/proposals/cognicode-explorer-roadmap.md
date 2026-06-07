# CogniCode Explorer Roadmap

CogniCode Explorer is an independent CogniCode-family application for moldable code exploration. The roadmap starts with a narrow `Symbol Inspector` MVP and evolves toward richer scopes, lenses, agents, runtime evidence, MoldQL, and backend portability.

## Current Decisions

| Area | Decision |
|------|----------|
| Product name | `CogniCode Explorer` |
| Product shape | Independent application in the CogniCode family |
| Not inside | `cognicode-dashboard` |
| MVP object | `Symbol` |
| MVP workflow | Spotter -> Symbol -> contextual views -> relation navigation -> evidence -> artifact |
| UI contract | Explorer API |
| Agent contract | Explorer MCP |
| UI stack | React 19 + Tailwind CSS |
| Evidence source | CogniCode core/db/tools, aggregated behind Explorer contracts |
| First storage | Existing CogniCode graph/cache, especially `CallGraph` and `SqliteGraphStore` |
| Later storage | Kuzu or another graph backend only behind a portable abstraction |
| Architecture principles | DDD boundaries, SOLID, explicit extension points, replaceable adapters |

## Product Spine

Every phase should preserve this interaction spine:

```text
Workspace
  -> Scope
  -> Inspectable Object
  -> Contextual View
  -> Typed Relation
  -> Evidence Block
  -> Exploration Path
  -> Decision Artifact
```

If a feature does not strengthen this spine, it is probably dashboard drift.

## Architecture Principles

CogniCode Explorer must be designed as a domain-oriented, extensible system. The goal is not only to build the first Symbol Inspector, but to create a product architecture where future building blocks can increase the application's power without breaking the core workflow.

### DDD Boundary Rule

Explorer concepts must be modeled in the product language:

- `Workspace`
- `Scope`
- `Inspectable Object`
- `Property`
- `Typed Relation`
- `Evidence Block`
- `Contextual View`
- `Lens`
- `Exploration Path`
- `Decision Artifact`

Implementation details such as CogniCode MCP calls, SQLite rows, Kuzu queries, quality scanner outputs, and runtime traces must enter through adapters. They should not leak into the UI or agent contracts as the primary language.

### SOLID Rule

Each building block should have one reason to change:

| Principle | Explorer Interpretation |
|-----------|-------------------------|
| Single Responsibility | Extractors extract, lenses interpret, views present, artifact generators generate, adapters integrate |
| Open/Closed | Add new lenses/views/adapters without rewriting the Symbol Inspector workflow |
| Liskov Substitution | A new graph/evidence adapter must satisfy the same Explorer contract as the old one |
| Interface Segregation | UI, MCP, graph storage, evidence, rendering, and artifact ports stay small and task-shaped |
| Dependency Inversion | Explorer domain services depend on ports, not concrete backends/tools |

### Building Block Catalog

These building blocks should be explicit from the start, even if the first implementation is minimal:

| Building Block | Responsibility | Example Implementations |
|----------------|----------------|-------------------------|
| Extractor | Turn raw artifacts into Explorer evidence | CogniCode symbol extractor, quality issue extractor, runtime trace extractor |
| Evidence Source | Provide evidence blocks with provenance | source lines, tool/query traces, quality findings, runtime traces |
| Lens | Ask a specific question over evidence | call graph, quality, connascence, SOLID, runtime, ownership |
| Contextual View | Present one lens/object combination | overview, call graph, source, evidence, module dependencies |
| Relation Resolver | Navigate typed relations | callers/callees, defined-in, imports, tests, violates |
| Renderer | Render a view or artifact | Miller column view, Markdown, HTML, Mermaid, JSON replay |
| Artifact Generator | Convert exploration paths into durable outputs | JSON replay, Markdown report, ADR, PR comment |
| Agent Workflow | Let agents operate in Explorer language | explain evidence, suggest next view, generate artifact |
| Graph Store Adapter | Back Explorer graph needs | existing CogniCode cache, in-memory, Kuzu, future Rust-native store |

### Extension Point Rule

Every phase should identify which building block it adds or extends. If a feature cannot be described as a building block behind an extension point, it needs more design before implementation.

## Phase 0: Documentation Lock

Status: Accepted.

Goal: make the product language and implementation boundaries unambiguous before more code is written.

### Scope

- Canonical terms in `CONTEXT.md`.
- ADRs corrected and aligned with the current decision chain.
- MVP proposal reviewed against roadmap.
- Rejected prototypes clearly marked as rejected.
- Implementation scaffold explicitly treated as a seed, not as the source of truth.

### Acceptance Criteria

- [x] `CONTEXT.md` uses `CogniCode Explorer` consistently.
- [x] ADR 0002 clearly defines Explorer as an independent application.
- [x] `proposals/cognicode-explorer-mvp.md` links to this roadmap.
- [x] DDD/SOLID/building-block principles are accepted as architecture constraints.
- [x] Roadmap phases are accepted before implementing additional functionality.

### Cut Line

Phase 0 is accepted. Product implementation may continue by following Phase 1A: known-symbol inspection.

## Phase 1: MVP Symbol Inspector

Goal: prove the core moldable workflow on concrete symbol evidence.

Phase 1 is split deliberately:

| Subphase | Decision | Why |
|----------|----------|-----|
| Phase 1A | Known-symbol inspection | Validates the moldable core without depending on search/index ranking |
| Phase 1B | Spotter search | Adds the natural entry point after object inspection works |

### User Journey

```text
Open workspace
  -> open known Symbol column
  -> switch Overview / Call Graph / Source / Evidence
  -> click caller or callee
  -> open related Symbol column to the right
  -> save exploration path
  -> generate artifact
```

### Inspectable Objects

- `Workspace`
- `Symbol`

### Contextual Views

| View | Required Evidence |
|------|-------------------|
| Overview | symbol name, kind, file, line, fan-in, fan-out |
| Call Graph | callers, callees, relation direction, relation evidence |
| Source | file path, line range, source snippet |
| Evidence | tool/query provenance, freshness, source references |

### Explorer API

- `POST /api/workspaces/open`
- `POST /api/workspaces/{workspace_id}/index`
- `GET /api/workspaces/{workspace_id}/spotter`
- `GET /api/objects/{object_id}`
- `GET /api/objects/{object_id}/views`
- `GET /api/objects/{object_id}/views/overview`
- `GET /api/objects/{object_id}/views/call-graph`
- `GET /api/objects/{object_id}/views/source`
- `GET /api/objects/{object_id}/views/evidence`
- `POST /api/explorations`
- `POST /api/explorations/{exploration_id}/artifacts`

### Explorer MCP

- `explorer_open_workspace`
- `explorer_spotter_search`
- `explorer_inspect_object`
- `explorer_get_view`
- `explorer_follow_relation`
- `explorer_explain_evidence`
- `explorer_save_path`
- `explorer_generate_artifact`

### Acceptance Criteria

- [ ] A user can open a known symbol as an inspectable object from a stable MVP object ID.
- [ ] A user can switch contextual views without losing column state.
- [ ] A user can navigate callers/callees as Miller Columns.
- [ ] Every claim shown in the UI has an evidence block.
- [ ] The exploration path can be saved.
- [ ] A minimal artifact can be generated from the saved path.
- [ ] The UI does not call low-level CogniCode MCP tools directly.
- [ ] The implementation keeps domain services behind ports/adapters instead of hardcoding concrete backends into UI/API handlers.

Phase 1B adds:

- [ ] A user can search symbols from the UI through Spotter.
- [ ] Spotter results can disambiguate same-name symbols by file, kind, and location.
- [ ] Opening a Spotter result uses the same inspectable object flow as known-symbol inspection.

### Non-Goals

- No module inspector yet.
- No quality dashboard.
- No generic graph canvas.
- No chat-first navigation.
- No Kuzu requirement.
- No MoldQL language implementation.

## Phase 2: File And Module Scopes

Goal: move from symbol-level exploration to larger code structures without becoming a file explorer.

`Module` starts as a derived scope, not as a real inspectable object. A folder/package is not automatically a module.

### New Inspectable Objects

- `File`
- `Scope`

### Derived Scopes

- `ModuleCandidate`

### Module Promotion Rule

A `ModuleCandidate` becomes a real `Module` inspectable object only when it has:

- Stable ID.
- Explicit boundary rule.
- Member symbols.
- Member files.
- Incoming typed relations.
- Outgoing typed relations.
- Evidence blocks for membership and relations.

Rule:

```text
Folder != Module
Package != Module automatically
Module = boundary with evidence
```

### New Contextual Views

- File Overview
- File Symbols
- Module Overview
- Module Dependencies
- Module Hotspots

### Acceptance Criteria

- [ ] A file can be opened as an inspectable object.
- [ ] A module/folder-like scope can be derived from repository structure.
- [ ] A `ModuleCandidate` can show grouped files, grouped symbols, and dependency summary.
- [ ] A `ModuleCandidate` can be promoted to `Module` only when boundary/evidence rules are satisfied.
- [ ] Clicking a file/module relation opens a new column.
- [ ] Symbol Inspector remains the deepest reliable drill-down.

### Dependency

Phase 1 must be stable enough that symbols can be reached from file/module views.

## Phase 3: Quality Lens

Goal: incorporate `cognicode-quality` as a lens, not as the product center.

### New Inspectable Objects

- `QualityIssue`
- `Rule`
- `QualityGateResult`

### New Contextual Views

- Quality Issues
- Rule Evidence
- Debt Impact
- Quality Gate

### Acceptance Criteria

- [ ] Quality issues appear as evidence-backed objects.
- [ ] A quality issue can link to symbols/files.
- [ ] A symbol/file/module can show related quality issues.
- [ ] The UI does not become a SonarQube clone.

### Dependency

Phase 2 should exist so quality can attach to file/module/symbol scopes.

## Phase 4: Design Lenses

Goal: add decision-oriented design lenses such as connascence and SOLID.

### Lenses

- Connascence
- SOLID
- Complexity
- Boundary / architecture rules

### New Contextual Views

- Connascence Hotspots
- SOLID Findings
- Boundary Violations
- Refactor Candidates

### Acceptance Criteria

- [ ] Design findings are framed as hypotheses, not verdicts.
- [ ] Each finding links to evidence blocks.
- [ ] The user can navigate from a design finding to affected symbols/files/modules.
- [ ] The app can generate a refactor proposal artifact from a design finding.

### Dependency

Quality and graph evidence must be available enough to support non-trivial claims.

## Phase 5: Runtime Evidence

Goal: compare static code graph evidence with observed runtime behavior.

### Evidence Sources

- Traces
- Test execution
- Runtime call paths
- Performance hotspots
- Failures/crashes if Chronos-style traces are available

### New Contextual Views

- Runtime Flows
- Static vs Runtime Difference
- Hot Paths
- Failure Evidence

### Acceptance Criteria

- [ ] Runtime evidence can be attached to symbols/files/modules.
- [ ] Static-only claims are visually distinguished from runtime-backed claims.
- [ ] Runtime evidence includes freshness/provenance.
- [ ] A runtime path can be saved as part of an exploration path.

### Dependency

Evidence model must be mature enough to handle timestamps, runs, and provenance.

## Phase 6: Agent Workflows Through Explorer MCP

Goal: make agents use Explorer's product language rather than raw code intelligence calls.

### Agent Workflows

- Explain current view.
- Suggest next relation to follow.
- Summarize evidence.
- Generate decision artifact.
- Ask missing-context questions.
- Compare alternatives.

### Acceptance Criteria

- [ ] Agent responses cite Explorer evidence blocks.
- [ ] Agents can open and traverse inspectable objects through Explorer MCP.
- [ ] Agents do not become the primary navigation surface.
- [ ] The UI can show what the agent used as evidence.

### Dependency

Explorer MCP v0 must be implemented over stable Explorer API/service contracts.

## Phase 7: MoldQL And Contextual Playground

Goal: provide a product-level query/playground layer without exposing backend graph syntax as the primary UX.

### Capabilities

- Query current object.
- Query current scope.
- Generate a contextual view.
- Save reusable view definition.
- Compile to backend graph/query/rule operations.

### Acceptance Criteria

- [ ] MoldQL uses product concepts: scope, lens, view, evidence, decision.
- [ ] Raw Cypher/Kuzu is only advanced/debug mode.
- [ ] Queries can be replayed as part of decision artifacts.
- [ ] Playground output can become a contextual view.

### Dependency

Enough stable Explorer contracts must exist to avoid designing MoldQL against an unstable schema.

## Phase 8: Graph Backend Evolution

Goal: evolve from existing graph/cache sources to a richer graph backend if the product needs it.

### Candidate Backends

- Existing CogniCode graph/cache.
- In-memory backend for tests/prototypes.
- Kuzu backend for property graph traversal.
- Future Rust-native graph backend.

### Acceptance Criteria

- [ ] Explorer contracts do not expose backend-specific graph syntax.
- [ ] The same Explorer query/view can be backed by multiple stores when possible.
- [ ] Kuzu is introduced only when it solves a proven query/performance/product need.
- [ ] Backend migration does not break saved exploration paths without an explicit migration plan.

### Dependency

Phase 1-4 must reveal real graph-query pressure before adding backend complexity.

## Cross-Cutting Requirements

### Evidence Provenance

Every non-trivial claim must expose:

- Source object IDs.
- File and line references when applicable.
- Tool/query used.
- Timestamp or graph freshness.
- Confidence when available.
- Whether the evidence is static, quality-derived, agent-derived, or runtime-derived.

Phase 1 minimum `EvidenceBlock` contract:

```text
EvidenceBlock
  id
  kind
  claim_id
  source_kind
  source_ref
  object_ids[]
  file
  line_range
  tool_or_query
  observed_at
  freshness
  confidence
```

Allowed `source_kind` values for Phase 1A:

- `source_file`
- `call_graph`
- `tool_result`
- `cached_graph`

Allowed `freshness` values for Phase 1A:

- `fresh`
- `stale`
- `unknown`

Rule: no UI claim may be rendered without at least one `evidence_id`. Direct source and call graph evidence can use `confidence = 1.0`; inferred evidence must use a lower confidence and explain the inference.

### Extensibility

New capabilities must be added as building block implementations behind extension points. The default extension model is:

```text
Explorer domain concept
  -> small port/interface
  -> one or more adapters/implementations
  -> contextual view or agent workflow consumes the port
```

This applies to extractors, evidence sources, lenses, views, renderers, artifact generators, agent workflows, and graph stores.

### Identity Durability

MVP symbol IDs may start as:

```text
symbol:{file}:{name}:{line}
```

This is an MVP object ID only. Persisted exploration paths and decision artifacts must use a versioned `ObjectIdentity` layer so they can survive file moves, line changes, and renamed symbols.

Minimum durable identity shape:

```text
ObjectIdentity
  id
  object_type
  version
  natural_key
  fingerprints[]
  first_seen
  last_seen
  supersedes[]
```

Rule: UI/API calls can accept MVP `object_id` values during Phase 1A, but JSON replay artifacts must store `ObjectIdentity` entries for every persisted object reference.

### Artifact Strategy

Accepted artifact split:

| Format | Role |
|--------|------|
| JSON replay | Canonical reproducible artifact |
| Markdown | Human-readable report generated from JSON replay |
| HTML | Later rich/shareable report generated from JSON replay |
| Query | Later replayable query/playground artifact generated from JSON replay |
| Table | Later tabular evidence/report view generated from JSON replay |
| Diagram | Later Mermaid/PlantUML/C4/graph view generated from JSON replay |

Rule: `Decision Artifact` persistence starts from JSON replay. Markdown, HTML, queries, tables, and diagrams are renderers over that replayable artifact, not separate sources of truth.

### UI Direction

The preferred UI reference is `prototypes/05-moldable-inspector.html`.

The initial real UI stack is:

```text
React 19
Tailwind CSS
visual-thinking libraries as replaceable building blocks
```

React/Tailwind is an implementation decision for Explorer UI, not a domain dependency. The domain model, Explorer API, Explorer MCP, evidence contracts, lenses, artifact model, and graph/storage ports must remain framework-agnostic.

Candidate visual-thinking library categories:

| Category | Use |
|----------|-----|
| Graph/canvas | Call graph, relation maps, dependency views |
| Layout engines | DAG/layered layout, auto-arrangement, graph readability |
| Tables/data grids | Evidence tables, properties, quality findings |
| Virtualization | Large symbol/evidence lists |
| State/query orchestration | Column state, async view loading, cache/freshness |
| Diagrams/renderers | Mermaid/PlantUML/C4 output and preview |
| Whiteboard/spatial tools | Future exploratory canvases, annotations, visual thinking |

Concrete libraries should be selected per building block, not baked into the domain. Examples to evaluate include React Flow/xyflow, D3/visx, Cytoscape, ELK/Dagre, TanStack Query/Table/Virtual/Router, tldraw, Mermaid renderers, and Vega/Observable Plot-style renderers.

The rejected prototypes remain useful only as negative references:

- `01-inspector-workbench.html`
- `02-pr-review-cockpit.html`
- `03-architecture-communities.html`
- `04-agent-exploration-flow.html`

## Open Grill Questions

Phase 0 grill decisions are closed and the roadmap is accepted as the controlling implementation sequence.

## Review Checklist

- [ ] Product name is consistent.
- [ ] MVP cut is narrow enough.
- [ ] Evolutives are sequenced and not mixed into MVP.
- [ ] Public contracts are Explorer-owned.
- [ ] Backend implementation remains replaceable.
- [ ] UI implementation remains framework-contained and does not leak into domain contracts.
- [ ] Building blocks are explicit and extensible.
- [ ] DDD/SOLID boundaries are preserved.
- [ ] Agent support does not make the product chat-first.
- [ ] Quality support does not make the product dashboard-first.
- [ ] Graph support does not make the product graph-first.
