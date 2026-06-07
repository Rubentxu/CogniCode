# CogniCode Explorer MVP

Build the first functional version as a `Symbol Inspector` on top of CogniCode's existing code exploration base. This keeps the full product direction alive while validating the smallest useful vertical slice.

For sequencing and evolutives, use `proposals/cognicode-explorer-roadmap.md` as the controlling roadmap.

## Quick Path

1. Start `cognicode-mcp` against a local repository.
2. Build or load the existing CogniCode call graph into `.cognicode/cognicode.db`.
3. Open the visual app with a known symbol ID.
4. Inspect that symbol through contextual views.
5. Navigate callers/callees as Miller columns.
6. Save the exploration path as a small decision artifact.

Spotter search is Phase 1B. The first technical cut is known-symbol inspection because it validates object identity, contextual views, typed relations, evidence, and Miller Columns without depending on search/index ranking.

## MVP Decision

| Area | Decision |
|------|----------|
| Application boundary | Build `cognicode-explorer` as an independent application in the CogniCode family |
| Implementation location | Add `crates/cognicode-explorer` to the CogniCode Rust workspace |
| Explorer interfaces | Provide both an Explorer API and an Explorer MCP server |
| UI stack | React 19 + Tailwind CSS |
| Evidence source | CogniCode core/db/tools aggregated behind Explorer-owned ports/adapters |
| First inspectable object | `Symbol` |
| First graph source | CogniCode `CallGraph` via `GraphStore` / `SqliteGraphStore` |
| First visual workflow | known Symbol -> contextual views -> callers/callees -> evidence -> report |
| Quality scanning | Optional secondary lens through `cognicode-quality`, not the central MVP |
| Persistent graph engine | Defer Kuzu; keep graph storage behind a portable abstraction |
| Architecture principles | DDD boundaries, SOLID, explicit building blocks, extensible ports/adapters |

## Architecture Principles

The MVP must be small, but it must not be structurally disposable. It should establish the extension shape for later phases.

Rules:

- Model Explorer with DDD language: workspace, scope, inspectable object, relation, evidence, view, lens, path, artifact.
- Follow SOLID: each building block has one responsibility and depends on small interfaces, not concrete backends.
- Make extension points explicit before adding hardcoded features.
- Treat CogniCode MCP, `cognicode-quality`, graph stores, renderers, and future runtime sources as adapters.
- Keep UI/API/MCP contracts in Explorer language, not backend/tool language.

First building blocks:

| Building Block | MVP Responsibility |
|----------------|--------------------|
| Symbol extractor adapter | Feed symbol inspectable objects from CogniCode evidence |
| Call graph relation resolver | Resolve `CALLS` and `CALLED_BY` typed relations |
| Source evidence provider | Produce file/line/source evidence blocks |
| Contextual view registry | Decide which views exist for each object type |
| Artifact generator | Produce canonical JSON replay artifacts |
| Artifact renderer | Render JSON replay into Markdown first, then HTML, queries, tables, diagrams, and other future views |
| Visual renderer | Render Explorer views with React/Tailwind and replaceable visual-thinking libraries |

## UI Stack

The first real Explorer UI uses React 19 and Tailwind CSS.

Rules:

- React/Tailwind belongs to the UI layer only.
- Explorer domain contracts stay framework-agnostic.
- Visual libraries are selected per building block and can be replaced.
- The UI must preserve the Miller Columns workflow from `prototypes/05-moldable-inspector.html`.
- Avoid importing dashboard-style layout assumptions from `cognicode-dashboard`.

Initial library categories to evaluate:

| Category | MVP Use |
|----------|---------|
| TanStack Query | Async loading and cache/freshness for views/evidence |
| TanStack Router | Routeable explorer states, if needed |
| TanStack Virtual | Large lists of relations/evidence |
| React Flow/xyflow | Focused call graph or relation subviews |
| D3/visx | Custom visual encodings where graph libraries are too rigid |
| ELK/Dagre | Automatic graph layout where needed |
| Mermaid renderer | Diagram artifact preview later |

Do not select all libraries upfront. Treat this as a visual-thinking toolkit to pull from as each building block demands it.

## Application Boundary

`cognicode-explorer` should be an independent application in the CogniCode family, not a page, module, or route inside `cognicode-dashboard`.

Reason:

- `cognicode-dashboard` is naturally oriented around scans, issues, metrics, and quality status.
- `cognicode-explorer` is naturally oriented around object inspection, Miller Columns, contextual views, evidence, and exploration paths.
- Reusing the dashboard shell too early risks pulling the product back into the dashboard/cockpit shape that was already rejected.
- Keeping it independent makes the product boundary clear while still allowing shared crates, APIs, styles, and infrastructure.

The boundary should look like this:

```text
cognicode-core
  -> code graph, symbols, parsing, call graph

cognicode-db
  -> persisted graph/evidence cache

cognicode-mcp
  -> tool/API surface for agents and external clients

cognicode-quality
  -> SonarQube-style scan and quality analysis

cognicode-dashboard
  -> metrics, scans, issues, quality status

cognicode-explorer
  -> moldable code exploration and decision workflow
```

`cognicode-explorer` may reuse shared components, API clients, crates, design tokens, and deployment conventions later, but it should own its information architecture from day one.

Implementation starts inside the CogniCode Rust workspace:

```text
/home/rubentxu/Proyectos/rust/CogniCode/
  crates/cognicode-explorer/
    src/lib.rs
    src/dto.rs
    src/service.rs
    src/api.rs
    src/mcp.rs
    src/bin/api.rs
    src/bin/mcp.rs
```

This keeps Explorer close to `cognicode-core`, `cognicode-db`, and other family crates while preserving product independence from `cognicode-dashboard`.

## Interface Boundary

`cognicode-explorer` should own two public interfaces:

| Interface | Primary Consumer | Purpose |
|-----------|------------------|---------|
| Explorer API | Visual UI | Low-latency interactive exploration, Spotter, column state, contextual views, evidence retrieval, report creation |
| Explorer MCP | Agents and external tools | Agent-driven exploration, guided inspection, evidence-backed explanations, decision artifact generation |

Reason:

- The visual UI needs endpoints shaped around interaction, not generic tool calls.
- Agents need MCP tools shaped around exploration workflows, not only raw code intelligence.
- `cognicode-mcp` remains a valuable upstream source for code intelligence, but `cognicode-explorer` exposes a different product language.
- `cognicode-quality` remains a separate scan/quality product and can feed Explorer as a later lens.

The intended flow is:

```text
cognicode-core / cognicode-db / cognicode-mcp / cognicode-quality
  -> Explorer indexing and evidence model
  -> Explorer API for UI
  -> Explorer MCP for agents
  -> Decision artifacts
```

This means Explorer can reuse existing CogniCode data extraction while still owning its own app-specific contracts.

## Symbol Inspector Contracts

Explorer should not expose raw CogniCode tool calls directly to the UI. It should aggregate CogniCode capabilities into product-level contracts shaped around inspectable objects, contextual views, evidence, and exploration paths.

Existing CogniCode capabilities that can feed the first contracts:

| Existing Capability | Source | Explorer Use |
|---------------------|--------|--------------|
| `semantic_search` | `cognicode-mcp` | Spotter symbol search |
| `query_symbol_index` | `cognicode-mcp` | Fast exact/partial symbol lookup |
| `get_symbol_code` | `cognicode-mcp` | Source contextual view |
| `get_call_hierarchy` | `cognicode-mcp` | Call Graph contextual view |
| `find_usages_with_context` | `cognicode-mcp` | Evidence and usages view |
| `analyze_impact` | `cognicode-mcp` | Later Impact contextual view |
| `CallGraph` | `cognicode-core` | Symbol relations and traversal |
| `SqliteGraphStore` | `cognicode-db` | Persisted graph/evidence cache |

### Explorer API v0

These endpoints are UI-first. They should return stable Explorer DTOs, not raw upstream MCP responses.

| Endpoint | Purpose | First Backing Source |
|----------|---------|----------------------|
| `POST /api/workspaces/open` | Open a local workspace and return graph/index status | local config + `.cognicode` metadata |
| `POST /api/workspaces/{workspace_id}/index` | Build or refresh the symbol/call graph index | `build_graph`, `build_lightweight_index` |
| `GET /api/workspaces/{workspace_id}/spotter?q=...&kind=...` | Search inspectable objects for Spotter | `semantic_search`, `query_symbol_index` |
| `GET /api/objects/{object_id}` | Load an inspectable object identity and properties | `CallGraph`, symbol index |
| `GET /api/objects/{object_id}/views` | List contextual views available for that object | Explorer view registry |
| `GET /api/objects/{object_id}/views/overview` | Return symbol overview properties | `Symbol`, `CallGraph` |
| `GET /api/objects/{object_id}/views/call-graph?depth=1` | Return callers/callees as typed relations | `get_call_hierarchy`, `CallGraph` |
| `GET /api/objects/{object_id}/views/source` | Return source snippet and location evidence | `get_symbol_code` |
| `GET /api/objects/{object_id}/views/evidence` | Return evidence blocks backing current claims | upstream tool/query traces |
| `POST /api/explorations` | Create or update a Miller Columns exploration path | Explorer state store |
| `POST /api/explorations/{exploration_id}/artifacts` | Generate a decision artifact from the path | Explorer report generator |

### Explorer MCP v0

These tools are agent-first. They should describe workflows in Explorer language instead of leaking raw UI endpoints.

| Tool | Purpose | First Backing Source |
|------|---------|----------------------|
| `explorer_open_workspace` | Open/index a repository for exploration | Explorer API + CogniCode indexers |
| `explorer_spotter_search` | Search inspectable objects using product language | Explorer Spotter API |
| `explorer_inspect_object` | Open an inspectable object and return summary + available views | Explorer object API |
| `explorer_get_view` | Fetch one contextual view for an object | Explorer view API |
| `explorer_follow_relation` | Navigate a typed relation and return the target object | Explorer graph relation API |
| `explorer_explain_evidence` | Explain what evidence supports a claim or view | Explorer evidence model |
| `explorer_save_path` | Save a Miller Columns exploration path | Explorer state store |
| `explorer_generate_artifact` | Generate Markdown/HTML/JSON artifact from a saved path | Explorer report generator |

### Minimal DTOs

The DTO names matter because they preserve the product language.

```text
WorkspaceSummary
  id
  root_path
  graph_status
  indexed_at
  symbol_count
  relation_count

InspectableObjectSummary
  id
  type
  label
  subtitle
  properties[]
  available_views[]

Property
  key
  value
  value_type
  source

TypedRelation
  type
  direction
  target_object_id
  target_label
  evidence_ids[]

EvidenceBlock
  id
  kind
  claim_id
  source_kind
  source_ref
  object_ids[]
  title
  file
  line_range
  tool_or_query
  observed_at
  freshness
  confidence

ContextualView
  object_id
  view_id
  title
  blocks[]
  relations[]
  evidence[]

ExplorationPath
  id
  workspace_id
  columns[]
  lens
  created_at
```

### Symbol Object ID

The first implementation can derive symbol object IDs from CogniCode's existing symbol identity:

```text
symbol:{file}:{name}:{line}
```

This is an internal MVP ID, not a durable identity.

Persisted exploration paths and JSON replay artifacts must use versioned `ObjectIdentity` records:

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

If later indexing produces stronger IDs, Explorer maps old MVP IDs through `ObjectIdentity` instead of breaking saved paths.

### First Implementation Rule

The UI talks to Explorer API. Agents talk to Explorer MCP. Explorer may call existing `cognicode-mcp` tools or shared CogniCode crates internally.

Do not make the UI orchestrate low-level MCP tool calls itself.

## Core Primitives

| Primitive | MVP Meaning | Evolves Into |
|-----------|-------------|--------------|
| Workspace | Local repo path plus `.cognicode` artifacts | Multi-repo workspace, branch snapshots, remote indexes |
| Scope | Repository or folder filter | Module, bounded context, PR, runtime trace, feature slice |
| Inspectable Object | Symbol first | Repository, module, file, route, test, ADR, MCP tool, agent run |
| Property | Symbol kind, file, line, fan-in, fan-out | ownership, churn, risk, confidence, freshness, coverage |
| Typed Relation | `CALLS`, `CALLED_BY`, `DEFINED_IN` | imports, tests, violates, owns, mentions, generated-by |
| Evidence Block | source location, tool result, query result | runtime trace, test result, quality issue, agent explanation |
| Contextual View | Overview, Call Graph, Source, Evidence | Connascence, SOLID, Tests, Runtime, Architecture, Security |
| Lens | Call Graph first | architecture, quality, connascence, test impact, ownership |
| Exploration Path | Open Miller columns | Saved notebook narrative, replayable investigation |
| Decision Artifact | Mini report | ADR, PR review, refactor proposal, test plan, diagram export |

## First Vertical Slice

```text
Open known object id: symbol:src/foo.rs:calculate_total:42
  -> Column 1: Symbol calculate_total
  -> View: Overview
  -> View: Call Graph
  -> Click caller OrderService::checkout
  -> Column 2: Symbol OrderService::checkout
  -> View: Source
  -> View: Evidence
  -> Save: mini report with symbols, relations, file locations, and query/tool evidence
```

## Minimal Contextual Views

### Overview

Shows the symbol identity and its most useful properties.

Required fields:

- Symbol name
- Symbol kind
- File path
- Line and column
- Fan-in
- Fan-out

### Call Graph

Shows related symbols as navigable typed relations.

Required relations:

- `CALLS`
- `CALLED_BY`
- `DEFINED_IN`

### Source

Shows the source location and nearby code.

Required evidence:

- File path
- Line range
- Source snippet

### Evidence

Shows why the app can make each claim.

Required evidence:

- MCP tool or graph query used
- Result timestamp or graph freshness
- Source nodes and relation types
- Confidence when available

Phase 1 evidence rule:

- Every claim rendered in the UI must reference `evidence_ids[]`.
- Direct source and call graph claims can use `confidence = 1.0`.
- Inferred claims must use lower confidence and expose what made them inferred.
- `source_kind` starts with `source_file`, `call_graph`, `tool_result`, and `cached_graph`.

## What We Keep For Later

These ideas stay documented, but they are not blockers for the first vertical slice.

| Later Capability | Why It Matters | First Hook In The MVP |
|------------------|----------------|------------------------|
| Module inspector | Developers reason above symbol level | derive module-like scopes from file/folder structure |
| Quality lens | SonarQube-style risk and debt view | optional `cognicode-quality` adapter |
| Connascence lens | Better design-decision support | add as a contextual view once relation evidence exists |
| SOLID lens | Explain design rule violations | use quality issues as evidence blocks |
| Architecture lens | Move from code graph to system understanding | aggregate symbols/files into scopes |
| Runtime lens | Compare static graph with observed behavior | future Chronos/trace evidence block |
| MCP tool inspector | Understand agent/tool capabilities | treat MCP tools as inspectable objects |
| Agent explanation | Guide without becoming chat-first | evidence-aware contextual view, not main UI |
| MoldQL | Human/product-level query language | playground can start with canned commands |
| Kuzu backend | Richer graph querying | implement as another `GraphStore` adapter later |

## Non-Goals For MVP

- Do not build a generic graph viewer.
- Do not make chat the main navigation surface.
- Do not require Kuzu before validating the UI workflow.
- Do not make `cognicode-quality` the product center.
- Do not infer deep architectural boundaries before symbol navigation works.

## Module Rule

`Module` is not part of the MVP. In Phase 2, module-like groupings begin as `ModuleCandidate` derived scopes.

A `ModuleCandidate` becomes a real `Module` inspectable object only when it has stable identity, explicit boundary rule, member symbols/files, typed incoming/outgoing relations, and evidence blocks.

Rule:

```text
Folder != Module
Package != Module automatically
Module = boundary with evidence
```

## Artifact Strategy

The MVP persists `Decision Artifact` as canonical JSON replay. Human-readable output is generated from that replay.

MVP:

- JSON replay as source of truth.
- Markdown renderer for human review.

Future renderers:

- HTML reports.
- Replayable queries.
- Evidence tables.
- Mermaid/PlantUML/C4 diagrams.
- Other renderer implementations added behind the artifact renderer extension point.

## Acceptance Checklist

- [ ] A user can open a known symbol as an inspectable object.
- [ ] A user can switch between Overview, Call Graph, Source, and Evidence views.
- [ ] A user can click a caller or callee and open it as a new column to the right.
- [ ] A user can see the file/line evidence behind each claim.
- [ ] A user can save the current exploration path as JSON replay.
- [ ] A user can render the JSON replay as Markdown.
- [ ] The data source is CogniCode's existing exploration/call graph base, not a hand-built mock.

Spotter acceptance is deferred to Phase 1B:

- [ ] A user can search for a symbol.
- [ ] A user can open a Spotter result through the same inspectable object flow.

## Remaining Review Questions

- Is the roadmap accepted as the controlling implementation sequence?
