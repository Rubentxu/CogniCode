# Context — CogniCode

## Project Mission
CogniCode is a code intelligence platform that provides graph-based exploration, vertical slice tracing, and AI-augmented analysis of software systems.

## Core Concepts

### Graph Model
- **GraphNode**: Any code entity (symbol, file, module, decision, doc, issue, evidence, component, container, system)
- **GraphEdge**: Relationships between nodes (dependency, calls, cites, resolves, etc.)
- **NodeKind**: Taxonomy of node types — Symbol (22 kinds), Decision, Doc, Issue, Evidence, Component, Container, System
- **EdgeKind**: Taxonomy of relationships — Dependency (8 kinds), Cites, Justifies, Resolves, CorroboratedBy, PartOf, DeployedAs, InSystem
- **SymbolRepository**: Read-only port for symbol identity resolution — `resolve`, `find_symbols_by_name`, `find_symbols_by_file`, `all_symbols`, `graph_stats`, `module_list`. Does NOT navigate the call graph.
- **GraphQueryPort**: Read-only port for structural graph navigation — neighbors with metadata (`provenance`, `confidence`), multi-hop traversals (`traverse`, `subgraph`). MoldQL compiles its queries to operations of this port. Replaces the deprecated `MetadataAwareRepository` + `as_metadata_aware()` escape hatch.
- **ToolHandler**: ISP-segregated trait for MCP tool dispatch. Each tool family (graph, views, search, sessions) registers its handlers via registry. The MCP handler dispatches by tool name — no central match arms. Same pattern as `ViewExecutor`.

### Visualization
- **ContextualView**: A graph view with focus node + surrounding context (parent/children/same_level)
- **Vertical Slice**: Trace from ANY entry point through entire call graph + data flow — the core pain point this project solves
- **Entry Point**: Any valid starting point — HTTP route, CLI command, event handler, use case name, or any symbol
- **Moldable View Runtime**: Hybrid backend/frontend system for discovering, defining, rendering, and persisting custom views
- **ViewSpec**: Declarative runtime view definition stored as data — `{ id, title, applies_to, view_kind, data_source, transform, renderer_kind, props }`
- **ViewKind**: Semantic view intent — what the user wants to understand, such as vertical slice, call graph, seam map, C4 view, impact radius, or source view.
- **RendererKind**: Visual rendering strategy — how a ViewSpec is displayed, such as graph, table, tree, code, markdown, Vega-Lite, JSON, or composite.
- **HierarchyKind**: Navigable structural projection used by views and data sources, such as file tree, module tree, type hierarchy, call hierarchy, package graph, or C4 hierarchy.
- **RendererRegistry**: Frontend registry that maps renderer ids (`graph`, `table`, `tree`, `code`, `vega-lite`) to React components
- **MoldQL**: Query language for selecting code objects, graph relations, docs, evidence, and architecture artifacts. MoldQL is a data-source/query language, not a visual layout DSL.

### Entry Points
All of the following are valid starting points for exploration:
- HTTP route path (e.g., `POST /api/users`)
- CLI command name (e.g., `cognicode analyze`)
- Event name (e.g., `UserCreated`)
- Use case name (e.g., `CreateUser`)
- Any symbol (function, type, variable)
- Search result from Spotter/Search
- Saved exploration
- ViewSpec
- ADR, decision, doc, issue, or evidence object

Entry points resolve through a common pipeline:

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

Each entry point type has a default `ViewKind`, but the user can switch views
from the Explorer after resolution. Examples:

- `POST /api/users` → `ResolvedEntryPoint::HttpRoute` → `vertical_slice`
- `cognicode analyze` → `ResolvedEntryPoint::CliCommand` → `vertical_slice`
- `UserCreated` → `ResolvedEntryPoint::Event` → `data_flow`
- `CreateUser` → `ResolvedEntryPoint::UseCase` → `vertical_slice`
- `UserRepository::save` → `ResolvedEntryPoint::Symbol` → `call_graph`

Search/Spotter is a universal entry point. It searches symbols, files, modules,
entry points, ViewSpecs, ADRs, decisions, docs, issues, evidence, and saved
explorations. Search results are not a flat list; they are represented as the
`semantic_search_results` ViewKind so the result set can be filtered, grouped,
rendered differently, saved as a ViewSpec, or opened as inspector panes.

### Architecture Principles
- **Clean/Hexagonal Architecture**: Vertical slices cross layers (HTTP → UseCase → Domain → Repository → DB)
- **No WASM in browser**: Never duplicate backend logic in the frontend
- **Layout split**:
  - Backend produces deterministic tree layout (useful for API/MCP consumers)
  - Frontend applies force-directed / interactive layouts
- **Two intelligence sources**: MCP tools (structured) + LLM (unstructured reasoning)
- **Explorer-first moldability**: The Explorer UI is the primary consumer of custom runtime views; MCP may expose degraded non-visual access to ViewSpecs
- **Hybrid navigation**: Frontend owns rich visual navigation state; backend persists semantic exploration state for restore, sharing, and MCP
- **Composition root**: `cognicode-runtime` crate is the single place where concrete implementations are selected and dependencies assembled. Binaries are thin: parse args → bootstrap → serve. PostgreSQL is the only persistence backend.

### LLM Integration (gt4llm patterns)
- **Chat Registry**: Named chat sessions with world-view scope
- **Playground**: Scratchpad for exploratory chat
- **Assistant vs Direct**: Two chat models — Assistant (structured JSON schema) vs Direct (unstructured)
- **Lepiter**: Pages with evaluable snippets, linkable to code objects
- **RAG**: Vector DB + chunking strategy for code-aware retrieval

### View Discovery
- **ViewRegistry**: Compile-time registry for built-in views, preferably linkme/distributed-slice or the existing LensRegistry-style trait-object pattern
- **InspectableObjectType**: Enum of object types that can have views (Symbol, File, Scope, Issue, Rule, Component, Container, System, Decision, Doc, Evidence)
- **ViewDescriptor**: ISP-segregated trait providing metadata-only access to a view's identity (`id`, `title`, `applies_to`, `view_kind`, `renderer_kind`). Consumers that only list views depend on this trait and know nothing about `build()`.
- **ViewExecutor**: Trait extending `ViewDescriptor` with `async build(ctx: &ViewContext) -> ContextualView`. The capability that constructs a view from a pre-resolved target. Registry stores `dyn ViewExecutor` — no downcast needed.
- **InspectionTarget**: Enum carrying pre-resolved object data (`Symbol(ResolvedSymbol)`, `File { path, symbols }`, `Scope { path, files, symbols }`, `Issue(QualityIssue)`, `Rule { rule_id }`) passed to view capabilities.
- **ViewContext**: Struct carrying `&InspectionTarget` + ports (`&dyn SymbolRepository`, `&dyn SourceReader`, `Option<&dyn QualityRepository>`) passed to `ViewExecutor::build()`. The service resolves identity and prepares this context; capabilities only build.
- **ContextualView**: The rendered view — `{ object_id, view_id, blocks: Vec<ViewBlock>, relations, evidence, findings }`
- **ViewBlock**: Generic JSON block `{ id, title, body: serde_json::Value }` — render-type agnostic
- **2-phase construction**: View descriptor listing → view instantiation with target (matches gtoolkit pattern)
- **GtPager**: Tab-based inspector with paging

### Moldable View Runtime
CogniCode view discovery has four layers:

1. **Built-in views** — Rust-defined views discovered by ViewRegistry; type-safe, compiled, used for core views such as overview, call graph, source, quality.
2. **Runtime ViewSpecs** — user-defined declarative views persisted as data; appear immediately in the Explorer without recompiling.
3. **Frontend RendererRegistry** — React-side renderer catalog; maps declarative renderer ids to concrete components such as Cytoscape, table, tree, code, Vega-Lite, and raw JSON.
4. **Advanced extension host** — future/pro tier for remote renderers or plugin-based custom components; explicitly out of scope for v1.

v1 supports only built-in renderers plus declarative ViewSpecs. External
plugins, remote React renderers, Module Federation runtime remotes, WASM view
plugins, and embedded scripting runtimes are not part of v1.

ViewSpecs separate semantic intent from visual representation:

- **ViewKind** answers: "what system concept is this view explaining?"
- **RendererKind** answers: "which visual component renders it?"

Examples:

| ViewKind | Typical RendererKind |
|----------|----------------------|
| `vertical_slice` | `composite` (`graph` + `tree` + `code` + `table`) |
| `call_graph` | `graph` |
| `seam_map` | `graph` + `table` |
| `c4_context` | `graph` or `tree` |
| `c4_container` | `graph` or `tree` |
| `c4_component` | `graph` or `tree` |
| `c4_code` | `tree` + `code` |
| `dependency_graph` | `graph` |
| `source_view` | `code` |
| `quality_hotspots` | `table` + `vega-lite` |
| `evidence_view` | `markdown` + `table` |
| `decision_graph` | `graph` + `markdown` |
| `diff_view` | `code` |
| `data_flow` | `graph` |
| `impact_radius` | `graph` + `table` |

First-class ViewKind catalog:

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
equivalent of Lepiter. They are based on markdown narrative plus embedded
ViewSpecs, linked objects, evidence packs, and decision traces. Executable
snippets are a future capability, not required for v1.

All catalogued ViewKinds are first-class domain vocabulary. Implementation can
be phased, but the names are reserved so future work does not lose the intended
capabilities.

First-class hierarchy kinds for v1:

- `file_tree` — workspace → directories → files
- `module_tree` — crate → module → items
- `type_hierarchy` — traits, impls, inheritance-like relations, and implementors
- `call_hierarchy` — callers and callees
- `package_graph` — crates, packages, and dependency relationships
- `c4_hierarchy` — system → container → component → code

Backend responsibilities:
- Resolve objects and entry points
- Provide graph/code/data sources
- Execute MoldQL queries as ViewSpec data sources
- Persist and validate ViewSpecs
- List applicable built-in and custom views

Frontend responsibilities:
- Render ViewSpecs through RendererRegistry
- Provide Explorer-first authoring UX for custom views
- Provide live preview while users create or edit ViewSpecs
- Run safe client-side transforms for visual exploration, with JSONata preferred for JSON reshaping and aggregation

MCP responsibilities:
- List, read, and execute ViewSpecs when possible
- Degrade gracefully for views that require browser-only renderers

Custom ViewSpec authoring is Explorer-first:

1. User inspects an object.
2. User selects **Create custom view**.
3. User chooses a `ViewKind`.
4. User chooses a `RendererKind`.
5. User selects a data source.
6. User adjusts a JSONata transform.
7. Explorer shows live preview.
8. User saves the result as a persisted `ViewSpec`.

Editing raw JSON is supported as an advanced/debug path, but it is not the
primary authoring workflow.

MoldQL is used inside ViewSpecs to select data, not to describe layout. Visual
layout remains the responsibility of `RendererKind`, renderer props, and the
frontend `RendererRegistry`.

Examples:

```text
symbols where kind = "function" and fan_out > 5
calls from "UserService::create_user" depth 3
docs citing adr "ADR-008"
```

### Navigation
- **Explorer Navigation State**: Frontend-owned visual state — open panes, active tabs, selected nodes, scroll, split layout, and visual breadcrumbs.
- **ExplorationSession**: Backend-owned semantic state — ordered navigation events `{ object_id, view_id, query, timestamp }` for restore, sharing, and MCP.
- **Navigation Stack**: User-facing drill-down history derived from Explorer state and periodically synchronized to the backend.
- **Inspector Pane Stack**: GtPager-like lateral stack of object inspections. Each pane owns `{ object_id, active_view_id, available_views, local_state, outgoing_links }`.
- **Shareable Exploration**: A persisted semantic exploration path that can be restored in the Explorer or inspected by MCP with degraded non-visual fidelity.

Explorer inspection is pane-based, not replacement-based. Clicking a related
object opens a new pane to the right instead of replacing the current object.
This preserves the exploration narrative:

```text
[Entry Point] → [Symbol] → [Repository] → [Decision] → [Test]
```

### Ingest Pipeline
- **Ingest**: The process of scanning a workspace's source files, extracting structural information, and persisting it as a queryable graph in PostgreSQL. The canonical trigger for graph creation and updates.
_Avoid_: index, build, scan (scan is a phase within ingest, not the whole process)

- **Scan**: The first phase of ingest — walking the filesystem, classifying files by type, computing content hashes, and detecting changes against the `scan_manifest` table. Output: a set of file changes (New, Changed, Deleted, Unchanged).
_Avoid_: crawl, walk

- **Extraction**: The phase of ingest where each changed source file is parsed via tree-sitter and its structural elements (symbols, calls, imports, type references, inheritance) are emitted as `GraphNode` + `GraphEdge` pairs. Extraction is deterministic and language-specific.
_Avoid_: parse, analyze

- **LanguageConfig**: A data-driven configuration object describing how to extract structural information from a specific programming language. Contains tree-sitter node-type mappings (function, class, import, call), extension list, and optional import/call handlers. One config per language; the generic extractor is shared.
_Avoid_: parser config, language definition

- **Infrastructure-as-Code (IaC) Extraction**: The extraction of Terraform (`.tf`/`.hcl`) and Ansible (`.yml`/`.yaml`) files as first-class nodes in the graph. Terraform resources, data sources, variables, and modules become `GraphNode`s with `References` edges between them. Ansible plays, tasks, and modules become `GraphNode`s with `Calls` (task → module) and `Imports` (playbook → playbook) edges.
_Avoid_: config parsing, deployment analysis

- **GraphQuery**: A natural-language query over the graph's topology. The agent asks "what connects X to Y?" and the tool returns a subgraph grounded in real edges with provenance. The query is deterministic (keyword extraction + IDF matching + BFS expansion), not LLM-based — the AI agent calling the tool provides the intelligence.
_Avoid_: semantic search (which is keyword-based, not topology-based), graph_search (too generic)

- **GraphReport**: An auto-generated summary of the graph's key structural properties, produced at the end of each ingest. Contains community clusters, god nodes (high PageRank), surprising cross-community connections, and dead code candidates. Cached in PostgreSQL for temporal diffing.
_Avoid_: analysis output, metrics report

- **Job**: An asynchronous unit of work in the Explorer API. Scan and analyze operations run as jobs with progress tracking (`scanned`, `total`, `stage`) and status polling via `GET /api/jobs/:id`. Jobs use `tokio::spawn_blocking` internally.
_Avoid_: task, operation, request

- **ScanManifest**: A PostgreSQL table (`scan_manifest`) that tracks `{file_path, content_hash, language, scanned_at}` per file. Serves as both the change-detection manifest and the extraction cache invalidation key. Replaces file-based JSON manifests.
_Avoid_: cache file, manifest.json

The ingest pipeline is a streaming, PG-native sequence:

```text
Scan ──▶ Extract ──▶ PgUpsert ──▶ Resolve ──▶ Cluster ──▶ Analyze ──▶ Report ──▶ Refresh ──▶ Notify
```

Each stage communicates through typed channels. Extraction (CPU-bound, `rayon`)
and PgUpsert (I/O-bound, `sqlx`) overlap via a `tokio::sync::mpsc` channel.
PostgreSQL is the sole persistence layer: graph store, manifest, and report
cache. No intermediate files.

## Terminology (vs gtoolkit)
| CogniCode | gtoolkit | Notes |
|-----------|----------|-------|
| Vertical Slice | (no equivalent) | Trace from entry point through call graph + data flow |
| ContextualView | GtPhlowView | Graph view with focus + surrounding context |
| Entry Point | Spotter/Chat | Any valid starting point for exploration |
| EntryPointResolver | Spotter resolution | Converts user input into a typed ResolvedEntryPoint |
| semantic_search_results | Spotter results | Search results as a moldable collection, not a flat list |
| ViewRegistry | gtView pragma | Built-in compile-time discovery for Rust-defined views |
| ViewSpec | gtView method body | Declarative runtime view definition stored as data |
| ViewKind | View intent | Semantic concept being explained by a view |
| RendererKind | Phlow view type | Visual strategy used to render a view |
| HierarchyKind | Tree/navigation view | Structural projection for hierarchical exploration |
| RendererRegistry | Phlow view renderer | Frontend mapping from renderer id to concrete UI component |
| MoldQL | Mondrian query preparation | Data-source/query language for selecting objects and relations, not visual layout |
| RendererKind + RendererRegistry | Mondrian rendering/layout | Visual rendering and layout responsibility |
| Layout: tree (backend) | Layout: force | Deterministic tree vs interactive force-directed |
| Inspector Pane Stack | GtPager | Lateral stack of object inspections |
| ExplorationSession | GtPager navigation history | Semantic backend trace of user navigation |

## Terminology (vs Graphify)
| CogniCode | Graphify | Notes |
|-----------|----------|-------|
| Ingest Pipeline | detect → extract → build → cluster → analyze → report | PG-native streaming, no JSON files |
| Scan | detect() | Walk FS + classify + hash + diff vs `scan_manifest` |
| Extraction | extract() | tree-sitter AST, 36+ languages via LanguageConfig |
| LanguageConfig | LanguageConfig dataclass | Data-driven per-language tree-sitter config |
| GraphReport | GRAPH_REPORT.md | Auto-generated insights, cached in PG |
| ScanManifest | manifest.json | PG table, not a file |
| GenericGraph | nx.Graph | petgraph + PG, not NetworkX + JSON |
| CallGraph | (no equivalent) | Code-only projection of GenericGraph |
| Dual-write projection | (no equivalent) | GenericGraph → CallGraph via PG dual-write |
| ArcSwap GraphCache | (no equivalent) | Lock-free serving with broadcast events |
| SolidLens | (no equivalent) | SOLID principle analysis as a Lens on the graph |
| PG NOTIFY/LISTEN | (no equivalent) | Real-time graph update notifications to Explorer |

## Open Questions
- [ ] How to auto-generate MCP tool schemas from handler signatures
- [ ] How to implement ReAct agent loop with tool calling
- [ ] RAG + LLM port from gt4llm patterns
- [x] ~~Epoch-based source cache for incremental updates~~ → Resolved: PG-native `scan_manifest` table with SHA256 content hash (ADR-017)
- [ ] C4 level extraction (component/container/system from code)
- [ ] Multimodal extraction (docs, PDFs, images via LLM) — Phase 2
- [ ] File watcher integration (notify crate) — Phase 1.5
