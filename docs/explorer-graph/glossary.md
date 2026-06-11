# Glossary

This glossary defines the terms used across the CogniCode Explorer graph
documentation set. The same terms are used in code, in the UI, in the MCP
surface, and in user-facing help. Keeping them aligned is part of the
product. When a term is added in a phase, it lands here in the same
change.

The terms are grouped by concern. The groupings are a reading aid, not
a hierarchy. New terms introduced by the stack, query, and
visualization decisions are in the Stack, Query Language, and
Visualization groups at the end.

## Graph Fundamentals

**Scope**
A lexical or module-level container of code. Scopes nest. In Rust a
scope can be a `mod`, a `crate`, a function body, or a block. Scopes
are the way the graph groups symbols without conflating them with
files or components.

**Symbol**
A named code construct that the graph reasons about. Functions,
methods, types, traits, constants, and statics are all symbols. A
symbol has a kind, a signature, a location, and a visibility.

**File**
A single source file on disk. Files are first-class nodes because
many questions are about files (who owns this file, what changed in
this file, what is the test coverage of this file).

**Call graph**
The directed graph whose nodes are symbols and whose edges are call
relationships. The call graph is the spine of the product. Most
analyses are projections of it.

**Hotspot**
A symbol that is reached by many paths or has high fan-in. Hotspots
are where refactors cost the most and where understanding pays off
the most. The term is borrowed from the hotspot-pinning literature
and is used the same way here.

**Fan-in**
The number of distinct symbols that call into a given symbol,
directly or transitively (the precise definition is documented with
the metric; the intuition is "how many things depend on this").

**Fan-out**
The number of distinct symbols that a given symbol calls into,
directly or transitively. The intuition is "how many things does
this depend on".

**Bridge node**
A symbol, file, or component that connects two otherwise weakly
connected regions of the graph. Bridges are a structural signal: a
bridge is often a coupling risk because changes to it ripple across
both regions.

**Community**
A densely connected region of the graph detected by a clustering
algorithm. CogniCode uses community detection (Leiden-class) to
surface architectural seams without requiring the user to declare
the architecture. A community is not a declared `Component`; it is
an observed cluster that may or may not correspond to one.

## Edges and Their Trust

**Provenance**
The origin story of an edge. Every edge has a `provenance` value
that is one of `extracted`, `inferred`, or `ambiguous`. Provenance
is what lets users tell a hard, source-backed link from a soft,
heuristic link.

**Extracted edge**
An edge derived from a deterministic, inspectable source: the AST,
the file path, the lexical scope, an explicit ADR link, an explicit
issue link. Extracted edges start at confidence `1.0` and are the
default of "this is true".

**Inferred edge**
An edge derived from a heuristic or a learned signal: name
similarity, embedding distance, pattern matching. Inferred edges
are reproducible but not guaranteed. They start in
`[0.5, 0.9]`.

**Ambiguous edge**
An edge that could not be resolved with certainty. It exists in the
graph because the user might want to follow it, but the UI must
make the ambiguity visible. Ambiguous edges have confidence
`<= 0.5`.

**Confidence**
A normalized float in `[0.0, 1.0]` attached to every edge. It is the
numerical expression of "how much should I trust this link". The
rules that assign and update confidence live in `cognicode-core` and
are the same for the UI and the MCP surface.

**Corroboration**
The act of backing an edge with multiple independent pieces of
evidence. A corroboration signal is a ranking input: an edge that
is reached by the call graph, cited in an ADR, and discussed in a
PR is a more confident lead than one that is only topologically
central. Corroboration is bounded: the function that raises
confidence on corroboration has a cap, defined in core.

**Evidence**
A node that backs an edge. Evidence is data, not opinion: a test
result, a benchmark, a documentation quote, a commit message that
names a symbol. Edges can be linked to evidence via the
`corroborated_by` edge kind.

## Product Surfaces

**Lens**
A way of viewing the same node differently. A symbol viewed through
the "hotspot" lens is sorted by fan-in; viewed through the
"refactor candidate" lens, it is sorted by complexity and churn.
The model is the same; the lens is the projection.

**MoldQL**
The seed query language users type into the explorer's query
field. MoldQL is the grammar that ExplorerQL grows from. Existing
queries written in MoldQL continue to work; the grammar expands
into ExplorerQL in step with the curated question set.

**ExplorerQL**
The product's primary query language. ExplorerQL grows from the
MoldQL grammar into a richer, typed expression language. It
compiles to PostgreSQL queries for persistent traversals and to
`petgraph` calls for algorithmic analyses. It is the public
contract for the explorer's query field and the default compile
target for the MCP server. It is not Cypher, and it is not
openCypher.

**Exploration path**
The history of a user's moves through the graph: a stack of views
with their focused nodes, lenses, and depths. The history is
navigable back and forward. In a later phase, paths become
shareable, named views.

**Named view**
A saved, shareable, link-stable projection of the graph. A named
view is what a user pastes into a chat message or a PR description
to point a teammate at a specific lens on a specific region.
**Implementation (v1):** a `(level, lens, focus_node, max_depth)`
four-tuple plus `name`, `description`, `workspace_id`, `owner`,
`created_at`, persisted in the `named_views` PostgreSQL table
behind the `postgres` feature flag. CRUD is exposed through four
MCP tools: `view_save`, `view_load`, `view_list`, `view_delete`.
`view_load` re-invokes the existing `contextual_view` pipeline so
the rebuilt view always reflects the current graph state — never a
stale snapshot. Without the `postgres` feature, every tool returns
the canonical `"named_views_require_postgres_feature"` soft error
(no panic, no sqlx linked).

**Object Inspector**
The detail panel in the explorer that shows everything known about
the currently focused object: definition, callers, callees,
complexity, evidence, related decisions, suggested questions.

**Suggested question**
A prompt the product offers a user based on the focused object
kind. Suggested questions are the user-facing form of the curated
question set in `target-product-model.md`. They are content, not
code, and they live in the docs/help layer.

## Federation

**Source**
A single ingest pipeline. A source can be a local repository
analyzed by `cognicode-core`, a remote repository, a docs site, an
issue tracker, or a decisions log.

**Space**
A named, addressable unit of organization inside a source. For a
code source, the default space is the workspace. For an issue
source, the space is the project. Spaces are how users refer to
"the auth repo" or "the platform ADRs" without ambiguity.

**Brain**
A queryable model that joins one or more spaces. A brain is what a
user opens in the explorer. A brain has its own row of metadata in
the canonical store, its own confidence model, and its own UI. A
brain can be exported to a versioned JSON snapshot for sharing,
diffing, and offline analysis; the canonical state of a brain
lives in PostgreSQL. The term is borrowed from gbrain; the meaning
is the same.

**Graph signal**
A ranking input that mixes structural and semantic evidence:
fan-in, community centrality, corroboration, recent churn, doc
citations. A graph signal is what makes the answer to "what
matters here" non-trivial.

**MCP**
Model Context Protocol. The wire protocol the CogniCode MCP server
speaks. In this documentation set, "MCP" refers both to the
protocol and to the `cognicode-mcp` crate that implements it.

## C4 Vocabulary

**Component**
A grouped unit of related files with one purpose. A component is
declared (it does not emerge from clustering alone) and is the
unit users think about when they say "the auth module" or "the
billing pipeline".

**Container**
A deployable or runnable unit. A binary crate, a service, a
database. Containers are where `Component`s meet runtime.

**System**
The full product or a major subsystem. The system view aggregates
its containers and the components that compose them.

**Context**
The C4 level above the system: external actors and external
systems. Out of scope for v1. Listed here for completeness.

## Cross-Cutting

**Causal chain**
The sequence of core primitives a higher-level verb called in
answer to a question. The causal chain is returned to the caller
so the answer is auditable.

**Bridge node**
Defined in the Graph Fundamentals section above. Listed again here
because in the C4 view, a bridge is often the seam between two
components, and the term means the same thing at every level.

**Curated question set**
The list of questions the product commits to answering, in
`target-product-model.md`. The set is closed. New questions
motivate new edge kinds or new views, not new ad hoc tools.

## Stack

**Canonical store**
The single, authoritative database that holds the graph state.
The canonical store is PostgreSQL. Everything else is either a
cache, an export, or a debugging aid. Two truths drift; one truth
stays coherent.

**PostgreSQL**
The open-source relational database that is the canonical store
of the CogniCode Explorer graph. The product relies on three of
its extension-grade features: `ltree` for the scope hierarchy,
`JSONB` for flexible payload, and `pgvector` for embeddings.

**SQLite**
A small, embedded relational database. SQLite is acceptable in
the CogniCode Explorer product only as a transitional
compatibility layer or a local-only utility, behind a feature
flag. It is not the target primary store. It is removed from
the default development and CI configuration in Phase 3 of the
roadmap.

**`sqlx`**
The async, compile-time-checked SQL toolkit for Rust that the
backend uses against PostgreSQL. `sqlx` is the only async DB
adapter the explorer and the MCP server should depend on for
the canonical store.

**`petgraph`**
The Rust graph library that implements the in-memory
algorithmic layer. `petgraph` carries path finding, centrality,
community detection, and the impact blast radius against
in-memory projections of the persistent graph. It is not the
storage; it is the analytics.

**`ltree`**
A PostgreSQL extension that represents labels stored in a
tree-like hierarchy. `ltree` is how CogniCode Explorer stores
the scope hierarchy in PostgreSQL and how it answers "is X
under Y?" without recursive CTEs.

**`JSONB`**
The PostgreSQL binary JSON type. `JSONB` carries the flexible
payload on nodes and edges (provenance details, lens
parameters, raw source references) where a strict schema would
be the wrong tool.

**`pgvector`**
The PostgreSQL extension for vector similarity search.
`pgvector` is how the product stores and searches embeddings
(when the product grows to use them) inside the canonical
store, without a second system.

**Export artifact**
A non-canonical, versioned form of the graph produced from the
canonical store for sharing, diffing, or offline analysis. The
JSON graph snapshot is the canonical example. An export
artifact is never the source of truth.

## Query Language

**Cypher**
A graph query language originally from Neo4j. Cypher is
attractive for its pattern-matching idiom; the CogniCode
Explorer product explicitly does not adopt it as the primary
user-facing query language. The reasoning is in
`query-language-decision.md`.

**Apache AGE**
A PostgreSQL extension that exposes openCypher. AGE is
interesting for future power-user experimentation but is not
the foundation of the v1 or v2 stack.

**openCypher**
An open-source variant of Cypher. openCypher inherits the
tradeoffs of Cypher; the product's reasoning against Cypher
applies to openCypher as well.

## Visualization

**Cytoscape.js**
The graph theory library and renderer that the CogniCode
Explorer uses for the main interactive graph. Cytoscape.js
owns the graph data model, the interaction model, the
selection model, and the rendering. The team owns the views,
the lenses, and the integration.

**`elkjs`**
The JavaScript build of the Eclipse Layout Kernel. `elkjs`
produces high-quality hierarchical, layered, and orthogonal
layouts. It is a layout engine only; it does not render or
handle interaction. The product pairs `elkjs` with
Cytoscape.js: `elkjs` produces the layout, Cytoscape.js
renders it.

**D3.js**
A low-level visualization toolkit. D3.js is the right tool
for the specialized analytic views the product needs
(heatmaps, time series, chord diagrams, small multiples) and
is the wrong tool for the main interactive graph. D3.js is a
supporting library, not a graph engine.

**React Flow**
A node-based UI library for building editors and diagrams.
React Flow is the right tool for editor-shaped surfaces
inside the product and the wrong tool for the main
interactive graph. It is not the main renderer.
