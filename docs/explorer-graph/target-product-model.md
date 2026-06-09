# Target Product Model

This file defines the product model CogniCode Explorer is becoming. It is
deliberately written as a design target, not as a description of today's
codebase. The audit in `current-state-audit.md` documents the gap between
this target and the present state.

The model has six parts: node kinds, edge kinds, provenance and confidence,
source/space/brain, abstraction levels, and the core user questions the
product exists to answer. A C4-style navigation section maps the levels to
familiar architectural thinking.

The model is realized on the stack documented in
`stack-recommendation.md` (PostgreSQL plus `petgraph` plus `sqlx`), the
query surface documented in `query-language-decision.md` (ExplorerQL
evolved from MoldQL, with Cypher explicitly not the primary language), and
the visualization stack documented in `visualization-stack.md`
(Cytoscape.js plus `elkjs` plus D3.js). Read this file as the model; read
the three decision files as the commitments that make the model
buildable.

## Node Kinds

Nodes are the entities the graph reasons about. The set is intentionally
small and orthogonal.

| Node kind       | What it represents                                   | Examples                                   |
| --------------- | ---------------------------------------------------- | ------------------------------------------ |
| `File`          | A single source file on disk.                        | `src/auth/session.rs`                      |
| `Symbol`        | A named code construct. Functions, types, traits.    | `fn login`, `struct User`                  |
| `Scope`         | A lexical or module-level container.                 | `crate::auth::session`, `mod user`         |
| `Component`     | A grouped unit of related files with one purpose.    | "authentication", "billing pipeline"       |
| `Container`     | A deployable or runnable unit.                       | A binary crate, a service, a database.     |
| `System`        | The full product or a major subsystem.               | "CogniCode core", "billing backend"        |
| `Decision`      | An architectural decision or rationale record.       | An ADR, a PR description, a design note.   |
| `Doc`           | A documentation artifact outside the code.           | A runbook, a README, a tutorial.           |
| `Issue`         | A tracked concern from an external system.           | A GitHub issue, a Jira ticket.             |
| `Evidence`      | A piece of supporting data backing an edge or claim. | A test result, a benchmark, a quote.       |

A node is not duplicated across kinds. A symbol is not also a file. The
edges between kinds carry the relationships (a symbol "lives in" a file; a
file "belongs to" a component).

## Edge Kinds

Edges are typed, directional, and carry provenance. The set below is the
minimum needed to express the core user questions in the next section.

| Edge kind           | From           | To             | Meaning                                          |
| ------------------- | -------------- | -------------- | ------------------------------------------------ |
| `calls`             | `Symbol`       | `Symbol`       | Direct call relationship.                        |
| `called_by`         | `Symbol`       | `Symbol`       | Reverse of `calls`. Same edge, opposite direction.|
| `lives_in`          | `Symbol`       | `File`         | The symbol's definition file.                    |
| `belongs_to`        | `File`         | `Scope`        | The lexical container of the file.               |
| `part_of`           | `Scope`        | `Component`    | The component the scope belongs to.              |
| `deployed_as`       | `Component`    | `Container`    | The container the component is part of.          |
| `in_system`         | `Container`    | `System`       | The system the container is part of.             |
| `references`        | Any code node  | `Doc`, `Issue` | Loose textual reference, source-extracted.       |
| `cites`             | `Doc`, `Issue` | Any code node  | A documentation or issue node that discusses the |
|                     |                |                | code node.                                       |
| `justifies`         | `Decision`     | Any code node  | The decision is the rationale for the node.      |
| `justified_by`      | Any code node  | `Decision`     | Reverse of `justifies`.                          |
| `resolves`          | `Issue`        | Any node       | The issue is resolved by the node.               |
| `corroborated_by`   | Any edge       | `Evidence`     | The edge is backed by a piece of evidence.       |

The set is open. New edge kinds are added when a new class of question
appears that none of the existing edges can answer.

## Provenance and Confidence Model

Every edge in the graph has a `provenance` field and a `confidence` field.
These are the difference between a graph users trust and a graph they
second-guess.

### Provenance

`provenance` is an enum with three values:

- `extracted` - the edge was derived from a deterministic, inspectable
  source: the AST, the file path, the lexical scope, an explicit ADR link,
  an explicit issue link in a commit message.
- `inferred` - the edge was derived from a heuristic, an embedding
  similarity, a name match, or a learned signal. It is reproducible but
  not guaranteed.
- `ambiguous` - the edge could not be resolved with certainty. It exists
  in the graph because the user might want to follow it, but the UI must
  make the ambiguity visible.

The `provenance` field also carries a `source_ref` pointing at the
specific artifact that produced the edge (a span, a line, a commit, a
file path). The `current-state-audit.md` notes that today this is partial;
making it complete is a Phase 1 task.

### Confidence

`confidence` is a normalized float in `[0.0, 1.0]`. The values are not
arbitrary. They follow rules:

- `extracted` edges start at `1.0` and never drop below `0.9` unless
  later evidence contradicts them.
- `inferred` edges start at the score produced by the heuristic, in
  `[0.5, 0.9]`.
- `ambiguous` edges start at `<= 0.5` and never exceed `0.5`.

Edges can be corroborated. When multiple `Evidence` nodes back the same
edge, the confidence is raised by a small, bounded function of the number
and quality of corroborating sources. The function is defined in the
implementation; the contract here is that corroboration is visible and
bounded, not unbounded self-promotion.

The UI uses confidence in three ways: it sorts results by it, it lets
the user filter by it, and it styles edges so strong edges look different
from weak ones.

## Source, Space, and Brain Model

The model is layered. From bottom to top:

- A `source` is a single ingest pipeline. A source can be a local
  repository analyzed by `cognicode-core`, a remote repository, a docs
  site, an issue tracker, or a decisions log.
- A `space` is a named, addressable unit of organization inside a source.
  For a code source, the default space is the workspace. For an issue
  source, the space is the project. Spaces are how users refer to
  "the auth repo" or "the platform ADRs" without ambiguity.
- A `brain` is a queryable model that joins one or more spaces. A brain
  is what a user opens in the explorer. A brain has its own confidence
  model, its own UI, and its own row of metadata in the canonical
  store.

The canonical state of a brain lives in PostgreSQL. The brain's data
is stored as typed tables keyed by `brain_id`, joined across spaces by
the brain's federation rules. A brain can be exported to a versioned
JSON snapshot for sharing, diffing, and offline analysis; the JSON
form is an export artifact, not the source of truth. The rationale and
the rejected alternatives are in `stack-recommendation.md`.

This layering is what makes federation tractable. A user opens a "core
brain" that joins the cognicode-core space, the cognicode-explorer
space, and the cognicode-mcp space. The brain can be saved, named, and
reopened. Adding a new repo is a space, not a rebuild.

## Abstraction Levels

The graph has four named levels. They are C4-style. The product's
narrative arc is "from code to system" and back.

| Level       | Node kinds in focus       | Question the level answers                    |
| ----------- | ------------------------- | --------------------------------------------- |
| Code        | `Symbol`, `File`, `Scope` | What does this function do? Who calls it?     |
| Component   | `Component`               | What is this part of the system for?          |
| Container   | `Container`               | What runs together? Where is it deployed?     |
| System      | `System`                  | What are the moving parts of the product?     |

The levels are projections, not separate graphs. A single persistent
graph is projected into each level by selecting the relevant node kinds
and the edges that connect them at that level. This is the key
implementation idea: one graph, many views.

The `part_of`, `deployed_as`, and `in_system` edges are how a user
climbs levels. The `lives_in` and `belongs_to` edges are how a user
drills down. Climbing and drilling preserve context: the user always
knows where they came from and can return.

## Core User Questions

The product is justified by the questions it answers. The list below is
the minimum viable set for v1 of the graph model. New questions
motivate new edge kinds or new views, not new products.

| Question                                     | What the answer needs to include                      |
| -------------------------------------------- | ----------------------------------------------------- |
| What does this symbol do?                    | Definition, callers, callees, complexity, evidence.   |
| Who calls this?                              | Direct callers, transitive callers, hotspots.         |
| What does this call?                         | Direct callees, transitive blast radius.              |
| What connects X and Y?                       | Paths, common ancestors, shared components.           |
| What changed recently around this node?      | Time-windowed diffs, new edges, dropped edges.        |
| What is risky to change here?                | Fan-in, complexity, test coverage, recent churn.      |
| Where does this belong in the architecture?  | The climb-up path: file, scope, component, container. |
| What justifies this design?                  | The decision graph: ADRs, PRs, issues, evidence.      |
| What is the shape of this codebase?          | Communities, god nodes, bridges, dead regions.        |
| What should I read first in this repo?       | A starter trail generated from the graph.             |

Each of these maps to one or more of the verbs in
`query-and-navigation.md`. They are not free-form natural language
questions; they are a curated set whose answers are computable from the
graph today.

## C4-Style Navigation, Mapped

C4 is a useful organizing frame because most engineering teams already
speak it. The mapping is explicit.

| C4 level   | CogniCode node kinds                  | Default edges                       |
| ---------- | ------------------------------------- | ----------------------------------- |
| Code       | `Symbol`, `File`, `Scope`             | `calls`, `called_by`, `lives_in`,   |
|            |                                       | `belongs_to`                        |
| Component  | `Component`                           | `part_of` to `Container`            |
| Container  | `Container`                           | `in_system` to `System`             |
| System     | `System`                              | The system view aggregates the rest |
| Context    | (out of scope for v1)                 | External actors, not modeled yet    |

The Context level is explicitly out of scope for the first iteration.
Adding it later means introducing an `Actor` or `ExternalSystem` node
kind, which is a clean extension because the edge model already
generalizes.

The "contextual" view of any node is the projection that includes that
node, its neighbors at the same level, and its parents and children at
adjacent levels. This is the GT-inspired view: a node with the
abstractions above and below it, presented in one panel.
