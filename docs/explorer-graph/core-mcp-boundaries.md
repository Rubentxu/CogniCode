# Core, MCP, Explorer: Crate Boundaries

This file fixes what belongs in each of the three product crates, what
belongs in a future docs/help/glossary layer, and the anti-patterns the
team should refuse. The boundaries are about concerns, not lines of code.
Specific modules and traits will change; the responsibilities do not.

The persistence and engine decisions behind these boundaries are in
`stack-recommendation.md`. The query language decisions are in
`query-language-decision.md`. The frontend library decisions are in
`visualization-stack.md`. This file states which crate owns each of
those concerns; the decision files state why.

## What Belongs in `cognicode-core`

`cognicode-core` is the analytical engine. It is the only crate that may
hold an authoritative view of the graph data model. Everything else
talks to it through traits.

| Concern                                              | In core? | Notes                                  |
| ---------------------------------------------------- | -------- | -------------------------------------- |
| The graph data model (nodes, edges, kinds)           | Yes      | Authoritative.                         |
| The `Repository` trait and its implementations       | Yes      | The seam that the rest of the system   |
|                                                      |          | depends on. Implementations are        |
|                                                      |          | `InMemory`, `Sqlite` (compatibility,   |
|                                                      |          | behind a feature flag), and `Postgres` |
|                                                      |          | (target).                              |
| The `sqlx` async DB adapter for PostgreSQL           | Yes      | The async layer against the canonical  |
|                                                      |          | store.                                 |
| The `petgraph` in-memory algorithmic layer           | Yes      | Path, centrality, community detection, |
|                                                      |          | impact analysis, against projections   |
|                                                      |          | of the persistent state.               |
| Source ingestion: lexing, parsing, AST extraction    | Yes      | Determinism is a core promise.         |
| Edge extraction (calls, lives_in, belongs_to)        | Yes      | Source-of-truth edges.                 |
| Edge inference (semantic, heuristic)                 | Yes      | With explicit `provenance = inferred`. |
| Provenance and confidence fields                      | Yes      | Defined and computed here.             |
| Cycle, hotspot, dead-code, impact, path analysis     | Yes      | Read-side, derived from the graph.     |
| Versioned JSON graph snapshot schema (export only)   | Yes      | The artifact used for export, diff,    |
|                                                      |          | debugging, and offline analysis. Not   |
|                                                      |          | the canonical store.                   |
| Multi-source federation                              | Partial  | Core holds the model. A brain layer    |
|                                                      |          | (in core or in a thin crate) holds the |
|                                                      |          | runtime.                               |
| UI, lenses, ExplorerQL grammar                       | No       | Belongs in the explorer.               |
| MCP tool definitions                                 | No       | Belongs in the MCP server.             |
| Help content, glossary, tutorials                    | No       | Belongs in the docs/help layer.        |

The discipline of `cognicode-core` is: it has no opinions about how its
output is rendered or how it is queried. It answers calls; it does not
shape views.

## What Belongs in `cognicode-explorer`

`cognicode-explorer` is the interactive front end. It is the only place
where views, lenses, and contextual help are defined.

| Concern                                              | In explorer? | Notes                                |
| ---------------------------------------------------- | ------------ | ------------------------------------ |
| Object Inspector, lenses, evidence blocks            | Yes          | Already there; extend, do not redo.  |
| ExplorerQL grammar (evolved from MoldQL) and         | Yes          | The grammar, the autocomplete, the   |
| its parser                                           |              | safety net. Compiles to core          |
|                                                      |              | primitives.                           |
| ExplorerQL execution engine                           | Partial      | Lives in core; the explorer compiles |
|                                                      |              | and dispatches.                       |
| Main interactive graph rendering                     | Yes          | Cytoscape.js.                         |
| Hierarchical and C4 layout engine                    | Yes          | `elkjs`, consumed by Cytoscape.js.    |
| Specialized analytic views                           | Yes          | D3.js for heatmaps, time series,      |
|                                                      |              | small multiples, chord diagrams.      |
| Named views and exploration path persistence         | Yes          | User-facing artifact.                |
| Suggested questions per object kind                  | Yes          | Drives "what can I do here?".        |
| Contextual help surfaces                             | Yes          | Bound to focused object and view.    |
| Onboarding and progressive disclosure                | Yes          | Product-grade UX.                    |
| Authoritative graph data model                       | No           | Delegated to core.                   |
| MCP tool wiring                                      | No           | Delegated to the MCP server.         |
| The persistence decision                             | No           | Delegated to core.                   |

The discipline of `cognicode-explorer` is: it consumes the core model
through a stable interface. It does not reach into the core crate to
read or mutate graph state directly. It does not make persistence
decisions; the canonical store is PostgreSQL, full stop. It does not
adopt Cypher or openCypher as a primary surface.

## What Belongs in `cognicode-mcp`

`cognicode-mcp` is the agent-facing surface. It exposes a small, sharp
set of tools and a graph-aware session.

| Concern                                              | In MCP?     | Notes                                |
| ---------------------------------------------------- | ----------- | ------------------------------------ |
| Tool definitions                                     | Yes         | The MCP contract.                    |
| Session lifecycle (open brain, attach, detach)       | Yes         | Long-lived graph sessions for agents.|
| Question routing: a single "ask" entry point that    | Yes         | The graph-OS idea; not a list of     |
| dispatches to specialized tools                      |             | one-shot tools.                      |
| `path`, `neighbors`, `subgraph`, `cluster`, `explain` | Yes         | The lower-level graph primitives.    |
| `why`, `what connects`, `what changed`,              | Yes         | The higher-level UX verbs, exposed   |
| `what is risky`, `where does this belong`,           |             | as MCP tools so agents can call them |
| `what justifies this`                                |             | directly.                            |
| View rendering, lenses, ExplorerQL editor           | No          | Belongs in the explorer.             |
| Authoritative graph data model                       | No          | Belongs in core.                     |

The discipline of `cognicode-mcp` is: it is the place where the product's
verbs are exposed in a form agents can call. The explorer may use the
same verbs internally, but the MCP surface is the contract.

## What Belongs in the Docs, Help, and Glossary Layer

This layer does not exist as a crate yet. The roadmap adds it. Its
concerns are: the glossary, the contextual help content, the
tutorials, the suggested questions, and the example brain that ships
with the product.

| Concern                                              | In docs/help? | Notes                               |
| ---------------------------------------------------- | ------------- | ----------------------------------- |
| Glossary content (terms and definitions)             | Yes           | One source of truth, versioned.     |
| Contextual help text per object kind and view         | Yes           | Tied to the explorer's view IDs.    |
| Suggested questions per object kind                  | Yes           | The list behind "what can I do?".   |
| Onboarding flows, sample brains, tutorials           | Yes           | Ship with the product.              |
| UI strings and labels                                 | No            | Live with the explorer for now.     |
|                                                    |               | Promote to this layer if they        |
|                                                    |               | become content the user reads.      |

The discipline of this layer is: content is data, not code. Help text,
glossary entries, and suggested questions are stored in structured
files that the explorer and the MCP server can load and version. This
is what makes them translatable, testable, and editable by people who
are not engineers.

## Anti-Patterns

These are the patterns the team should refuse, with reasons.

| Anti-pattern                                              | Why it is wrong                                       |
| --------------------------------------------------------- | ----------------------------------------------------- |
| Putting UI logic in `cognicode-core`.                     | Couples the engine to a front end and blocks reuse    |
|                                                           | from the MCP server.                                  |
| Putting graph mutation in `cognicode-explorer`.           | Two sources of truth; the explorer starts to drift    |
|                                                           | from the model.                                       |
| Putting the glossary in code as constants.                | Translators and writers cannot touch it; it rots.     |
| Exposing every core function as an MCP tool.              | The MCP surface becomes a junk drawer. Keep it        |
|                                                           | question-shaped.                                      |
| Hiding provenance behind a single "trust me" boolean.     | Loses the difference between extracted, inferred, and  |
|                                                           | ambiguous. The product's credibility depends on that. |
| Treating "contextual view" as a hardcoded panel.          | Locks the UX to today's notions of a view. Define     |
|                                                           | views declaratively so they can evolve.               |
| Implementing federation by string-joining graphs.         | The brain model is the contract. Federation is its    |
|                                                           | job. Do not bolt it on.                               |
| Letting the explorer hold the persistent graph.           | The graph belongs in core. The explorer is a viewer.  |
| Mixing "help" text and "UI" labels in the same files.     | Translators, accessibility reviewers, and writers     |
|                                                           | cannot work efficiently. Keep them separate.          |
| Storing confidence as a free-form float without rules.    | Edges start to disagree across the system. Define     |
|                                                           | the rules in one place (core) and enforce them.       |
| Building a "context" C4 level before the lower levels     | The climb-up only works if the levels below are       |
| are stable.                                               | real. Build code, component, container, system in     |
|                                                           | that order.                                           |
| Treating SQLite as the target primary store.              | The product is multi-user and agent-facing. SQLite    |
|                                                           | is acceptable only as a transitional compatibility    |
|                                                           | layer behind a feature flag.                          |
| Making the JSON graph snapshot the source of truth.       | JSON is for export, import, and debugging. The       |
|                                                           | canonical state lives in PostgreSQL. Two truths       |
|                                                           | drift; one truth stays coherent.                      |
| Adopting Cypher, openCypher, or AGE as the primary        | Couples the product's primary surface to a            |
| user-facing query surface.                                | third-party grammar and roadmap. ExplorerQL is the    |
|                                                           | primary surface.                                      |
| Letting the frontend hold the persistence decision.       | The frontend is a viewer. The persistence decision    |
|                                                           | belongs in the backend. The frontend consumes the     |
|                                                           | core trait.                                           |
| Letting the explorer reach into the DB adapter directly. | The `Repository` trait is the seam. The explorer      |
|                                                           | and the MCP server both consume it; neither reaches   |
|                                                           | around it.                                            |
| Letting a single feature flag quietly re-enable a         | The roadmap's discipline is that the v1 and v2        |
| rejected direction (SQLite-as-target, Cypher-as-         | target stack is fixed. Feature flags are migration    |
| primary, AGE-as-foundation).                              | tools, not escape hatches.                            |
