# Current State Audit

This file inventories what CogniCode already has across the three relevant
crates, what it can do for users today, and where the seams are starting to
show. It is the ground truth for the rest of the documentation set. Nothing
here is aspirational.

The stack the product is moving toward is documented in
`stack-recommendation.md` (storage and engine), `query-language-decision.md`
(query language), and `visualization-stack.md` (frontend libraries). This
file is the gap claim against that target.

## Scope of the Audit

Three surfaces are covered:

- `cognicode-core` - the analytical engine and graph data model.
- `cognicode-explorer` - the interactive UI built on top of core.
- `cognicode-mcp` - the MCP server that exposes core capabilities to agents.

The audit focuses on graph-shaped capabilities. Linting rule catalogs, language
adapters, and CLI surface are out of scope unless they touch the graph.

## `cognicode-core`: Graph Primitives

`cognicode-core` is where the symbol graph and its derived views live. The
following table is the inventory of what is implemented and reachable from
the public API.

| Primitive                          | Status      | Notes                                                   |
| ---------------------------------- | ----------- | ------------------------------------------------------- |
| `Symbol` node                      | Implemented | Includes kind, location, signature, visibility.         |
| `File` node                        | Implemented | File-level aggregation, language, size, line count.     |
| `Scope` node                       | Implemented | Module, namespace, crate, and similar containers.       |
| Call edge (`calls`)                | Implemented | Resolved, with location span.                           |
| Reverse call edge (`called_by`)    | Implemented | Same edge, traversed in reverse.                        |
| Fan-in                             | Implemented | Count of distinct callers.                              |
| Fan-out                            | Implemented | Count of distinct callees.                              |
| Callers / callees listing          | Implemented | Filtered, paginated, sorted.                            |
| Path tracing between two symbols   | Implemented | Bounded by depth, returns shortest paths first.         |
| Cycle detection                    | Implemented | SCC over the call graph.                                |
| Dead code surface                  | Implemented | Zero fan-in, entry-point aware.                         |
| Hot paths (top fan-in)             | Implemented | Used for hotspot ranking.                               |
| Impact analysis (blast radius)     | Implemented | Forward and reverse, depth bounded.                     |
| Complexity metrics (cyclomatic,    | Implemented | Per-function.                                           |
| cognitive, nesting)                |             |                                                         |
| Mermaid export                     | Implemented | Subgraph, full, and per-symbol views.                   |
| `SymbolRepository` trait           | Implemented | The contract that the explorer depends on.              |
| Persistent graph snapshot          | Partial     | Some caching; not yet a first-class, versioned artifact. |
|                                   |             | The target is a single canonical store in PostgreSQL;   |
|                                   |             | the JSON snapshot is an export and debugging artifact.  |
| Edge provenance                    | Partial     | Locations stored; source-of-truth and confidence weak.  |
| Edge confidence                    | Missing     | No first-class confidence field on edges.               |
| Multi-level projections            | Missing     | No component, container, or system nodes.               |
| Community / cluster detection      | Missing     | No Leiden or similar algorithm exposed.                 |
| Multimodal nodes (doc, decision)   | Missing     | No nodes for non-code entities.                         |
| Cross-source federation            | Missing     | No model for joining multiple repositories or brains.   |
| Graph signal / corroboration       | Missing     | No ranking signal that mixes paths, hotspots, evidence. |

## `cognicode-explorer`: Navigation Capabilities

The explorer turns the core primitives into interactive views. The current
surface is functional but narrow.

| Capability                         | Status      | Notes                                                   |
| ---------------------------------- | ----------- | ------------------------------------------------------- |
| Object Inspector                   | Implemented | Detail panel for the currently focused object.          |
| Caller / callee navigation         | Implemented | One level at a time, direct only.                       |
| Lens registry                      | Implemented | Multiple lenses, selectable per view.                   |
| MoldQL query field                 | Partial     | Limited grammar; not promoted to a first-class surface. |
| Exploration path history           | Implemented | Back / forward stack; no sharable paths.                |
| Evidence blocks                    | Implemented | Shows sources backing the current view.                 |
| Mermaid viewer                     | Implemented | Renders exported Mermaid diagrams.                      |
| Quality cards                      | Implemented | Per-symbol complexity and smell summaries.              |
| Cross-level climb-up               | Missing     | No way to go from symbol to file to scope to component.  |
| Cross-level drill-down             | Partial     | Only via direct "open file" or "open scope" actions.    |
| "What connects X and Y"            | Missing     | No first-class verb for the question.                   |
| "Why is this here"                 | Missing     | No explanation view that surfaces rationale.            |
| Community / cluster map            | Missing     | No community-level visualization.                      |
| Architecture projection            | Missing     | No C4-style container or system view.                   |
| Federated search                   | Missing     | Search is local to the analyzed project.                |
| Persistent named views             | Missing     | No saveable, named, shareable views.                    |
| Contextual help                    | Missing     | No in-app guidance tied to the focused object.          |
| Suggested questions per object      | Missing     | No prompt surface per object kind.                      |

## `cognicode-mcp`: Exposed Surface

The MCP server is a toolbox of single-purpose tools. Each tool is well-defined
and easy to call, but the model is "list of tools" rather than "graph OS".

| Tool shape             | Status      | Notes                                                  |
| ---------------------- | ----------- | ------------------------------------------------------ |
| Single-purpose tools   | Implemented | Each tool answers one focused question.                |
| Tool composition       | Manual      | Agents must orchestrate; no graph-aware orchestrator.  |
| Path / neighbors /     | Implemented | Lower-level graph tools are present.                   |
| subgraph primitives    |             |                                                        |
| `explain` verb         | Missing     | No tool that explains the evidence for a result.       |
| `watch` verb           | Missing     | No tool for change-driven exploration.                  |
| `update` verb          | Missing     | No tool to add notes, links, or annotations.           |
| Question routing       | Missing     | No "ask anything about this graph" entry point.        |
| Persistent session     | Missing     | No long-lived graph session the agent can attach to.   |

## Current Limits and Gaps

The limits below are not bugs; they are the edges of a v1 model. The roadmap
in `roadmap.md` is structured around them.

| Gap                                           | Why it matters                                                |
| --------------------------------------------- | ------------------------------------------------------------- |
| No multi-level architecture projection        | Users cannot climb from a symbol to a system view in one move. |
| Weak provenance model on edges                | Users cannot tell why an edge exists or how strong it is.     |
| No confidence field on edges                  | Inferred and ambiguous links look identical to extracted ones.|
| No multimodal graph                           | Docs, ADRs, decisions, and runbooks stay outside the graph.   |
| No community or cluster detection             | Hotspots, god nodes, and architectural seams are not surfaced.|
| No single canonical store                      | There is no PostgreSQL-backed persistence yet; the JSON graph  |
|                                               | snapshot is a notion in the design, not a realized artifact.  |
| No cross-source federation                    | Multi-repo and multi-language projects stay siloed.           |
| No product-grade query model                  | "Why" and "what connects" require manual tool chaining.       |
| Weak query language                            | MoldQL is narrow; ExplorerQL is the planned primary surface   |
|                                               | and has not landed yet. See `query-language-decision.md`.     |
| No contextual help or glossary                | New users see a powerful tool with no on-ramp.                |
| MCP is a toolbox, not a graph OS              | Agentic users have to know every tool name up front.         |
| Exploration paths are not shareable           | Teams cannot point each other at a view.                      |
| No suggested questions                        | Users do not know what to ask next.                           |
| No rationale graph (ADRs, PRs, issues)        | The "why" of the code is not in the graph.                    |
| No corroboration signal                       | Edges backed by multiple sources are not visibly stronger.   |

## What Is Solid and Should Not Be Redone

This audit is honest about gaps, but it would be a mistake to throw away
working pieces. The following are load-bearing and should be extended, not
replaced:

- The symbol, file, and scope node model and its derived metrics.
- The `SymbolRepository` trait as the seam between core and the rest.
- The Mermaid export path; the new view layer should generate Mermaid in
  addition to richer renderings.
- The existing Object Inspector, lens registry, and evidence block UI.
- The exploration path history mechanism; it is the seed of named views.
- The existing MCP tool surface; the new graph-OS layer should wrap it, not
  delete it.
