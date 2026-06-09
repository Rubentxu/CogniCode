# Query and Navigation Model

This file proposes the query and navigation model CogniCode Explorer is
moving toward. It is written as product design on top of the model
described in `target-product-model.md`, and as API design for the
surfaces in `core-mcp-boundaries.md`. The model has two layers: a
small set of low-level graph primitives, and a higher-level set of
user-facing verbs that compile to chains of those primitives.

The intent is that the user never needs to know which layer they are
on. A new user asks "what connects X and Y"; a power user types
`explorql: neighbors(X) -[cites]-> *`; an agent calls a single MCP
tool. All three are answered by the same engine.

The language decisions behind this surface are documented in
`query-language-decision.md`. In short: the primary query language is
a product-owned DSL (ExplorerQL, evolved from MoldQL) that compiles
to PostgreSQL queries for persistent traversals and to `petgraph`
calls for algorithmic analyses. Cypher is interesting but is not the
primary surface; openCypher via Apache AGE is not the foundation.

## Low-Level Graph Primitives

The primitives are the verbs the rest of the system is built on.
Each one is a pure function from the graph to a result, with
provenance and confidence attached. They are exposed both to the
explorer (as building blocks for views) and to the MCP server (as
tools).

| Primitive      | Signature (shape)                                          | Returns                                |
| -------------- | ---------------------------------------------------------- | -------------------------------------- |
| `path`         | `(from, to, max_depth, level_filter) -> paths[]`            | Shortest paths with their edges.       |
| `neighbors`    | `(node, direction, edge_kinds, depth, level_filter) -> n[]` | Neighbors with provenance and confidence.|
| `subgraph`     | `(root, radius, level_filter) -> subgraph`                 | A bounded subgraph ready to render.    |
| `cluster`      | `(level, algorithm, params) -> communities[]`              | Detected communities with members.     |
| `explain`      | `(edge_or_node) -> evidence_chain`                         | The chain of evidence backing a claim. |

These are the only graph-aware operations the rest of the system
needs. Anything more complex is a composition of these five. The
discipline is to add a new primitive only when a new class of
question cannot be expressed with the existing five; otherwise,
the question is answered by composing them.

### Result envelope

Every primitive returns the same envelope. The envelope is a
contract, not a convenience; it is what makes the system auditable.

- `payload` - the data the caller asked for.
- `provenance` - per-edge provenance (`extracted`, `inferred`,
  `ambiguous`).
- `confidence` - per-edge confidence, in `[0.0, 1.0]`.
- `causal_chain` - the chain of primitive calls that produced the
  result. This is what makes the answer auditable.
- `suggested_followups` - a small list of higher-level verbs that
  are likely useful next.

The envelope is what the MCP server returns. It is what the explorer
renders. It is what tests assert against.

## Higher-Level UX Verbs

The higher-level verbs are the curated question set, expressed in a
shape a UI can put behind a button and an agent can call by name.
The set is closed; new questions motivate new primitives, not new
ad hoc tools.

| Verb                | Question it answers                              | Implementation (rough)                              |
| ------------------- | ------------------------------------------------ | --------------------------------------------------- |
| `why`               | Why is this node the way it is?                  | `explain(node)` + `justified_by` traversal.          |
| `what_connects`     | What connects X and Y?                           | `path(X, Y)` + common ancestor + shared component.  |
| `what_changed`      | What changed recently around this node?          | Diff two snapshots; return added / removed edges.   |
| `what_is_risky`     | What is risky to change here?                    | Combine fan-in, complexity, churn, test coverage.   |
| `where_does_it_belong` | Where does this fit in the architecture?      | `part_of` / `in_system` climb-up.                   |
| `what_justifies`    | What decision or rationale backs this design?    | `justified_by` traversal + `cites` corroboration.   |
| `what_is_the_shape` | What is the shape of this codebase?              | `cluster(level=code)` + summary of god nodes and    |
|                     |                                                  | bridges.                                            |
| `where_to_start`    | What should I read first in this repo?           | A graph-generated trail: top communities, then      |
|                     |                                                  | their top hotspots, then their tests.               |

The verbs are not strings in code; they are the names of MCP tools.
The MCP server is the canonical home of the verb definitions; the
explorer's buttons and the suggested-questions UI are views on top
of them.

### Composition rules

The verbs are allowed to compose. `what_changed(node)` can be
combined with `where_does_it_belong(node)` to answer "what changed
in the system this node is part of?". The composition is explicit:
the higher-level verb lists the lower-level verbs it intends to
call, and the envelope includes the union of their causal chains.

The composition is also bounded. A composition that calls more
than a small, fixed number of primitives is rejected. The product
is a navigation tool, not a query optimizer.

## MCP Tool Ideas

The MCP surface in Phase 2 is built from the primitives and the
verbs above. The list below is the v1 contract; it is small on
purpose.

### Lower-level tools

- `cognicode_path` - the `path` primitive.
- `cognicode_neighbors` - the `neighbors` primitive.
- `cognicode_subgraph` - the `subgraph` primitive.
- `cognicode_cluster` - the `cluster` primitive.
- `cognicode_explain` - the `explain` primitive.

### Higher-level tools

- `cognicode_why` - the `why` verb.
- `cognicode_what_connects` - the `what_connects` verb.
- `cognicode_what_changed` - the `what_changed` verb.
- `cognicode_what_is_risky` - the `what_is_risky` verb.
- `cognicode_where_does_it_belong` - the `where_does_it_belong`
  verb.
- `cognicode_what_justifies` - the `what_justifies` verb.
- `cognicode_what_is_the_shape` - the `what_is_the_shape` verb.
- `cognicode_where_to_start` - the `where_to_start` verb.

### Session tool

- `cognicode_brain_session` - open, attach, ask, close. A long-lived
  graph session. Required for stateful verbs and for any verb that
  relies on the brain layer (Phase 4).

### Single "ask" entry point

- `cognicode_ask` - takes a natural-language question, routes it
  to the right verb, returns the standard envelope. The routing
  is the public surface; the chain of verbs it used is in
  `causal_chain` so the caller can audit it.

The discipline is: every tool returns the same envelope. Tools do
not invent their own shapes. A caller can treat any tool as a
function from a request to an envelope.

## MoldQL Expansion and the ExplorerQL Direction

MoldQL is the seed of the query language users type in the
explorer's query field. The product's primary query language is
ExplorerQL, a richer expression that grows from the MoldQL grammar
in step with the curated question set. The full reasoning is in
`query-language-decision.md`; the summary is that the language is
product-owned, compiles to PostgreSQL and `petgraph`, and is not
coupled to Cypher or to Apache AGE.

The grammar below is the v1 expansion. It is staged and tied to
the verbs above, not to a free-form grammar. New surface area
appears only when a curated question needs it.

### v1 grammar (already partly there)

- `node_id` - resolve a node by id.
- `kind` - filter by node kind.
- `level` - filter by abstraction level.

### Phase 2 expansion

- `neighbors(X)` - call the `neighbors` primitive.
- `path(X, Y)` - call the `path` primitive.
- `subgraph(X, depth)` - call the `subgraph` primitive.
- `cluster(level)` - call the `cluster` primitive.

### Phase 3 expansion

- Filters on `provenance` and `confidence`.
- Boolean composition: `and`, `or`, `not`.
- Joins across levels: `subgraph(component) > symbols`.
- Named queries: save a query by name and call it from another
  query.

### Phase 4 expansion

- Time-windowed queries: `subgraph(X, depth, since=t)`.
- Source filters: `cites(X) where source in spaces(a, b)`.
- An opt-in, Cypher-inspired advanced surface for power users,
  with its own documentation, error model, and deprecation
  policy. The surface borrows Cypher's idioms where they help;
  it does not couple the product to Cypher and does not use
  Apache AGE as a backend.

### Error model

ExplorerQL errors are content, not exceptions. The error message
names the bad part, the closest valid form, and a link to the
glossary entry for the term. The query field is one of the
surfaces new users see; the error model is part of the on-ramp.

### Autocomplete

Autocomplete suggests verbs, node kinds, edge kinds, and
projection levels. The suggestions are tied to the focused
object's kind. A symbol-focused query field suggests
`neighbors`, `path`, `where_does_it_belong` first; a component-
focused field suggests `cluster`, `subgraph`, `what_changed`
first. The suggestions are the on-ramp into the curated
question set; the user does not have to know the language to
use the product.

## Connection to the C4 Levels

The primitives and the verbs are level-aware. Every call accepts
a `level_filter` and every call's result carries the level of
each node it returns. The level is the user's anchor when they
ask "is this the right abstraction?".

- `where_does_it_belong(symbol)` climbs the levels. Its result
  is a path from `Symbol` to `Scope` to `Component` to
  `Container` to `System`.
- `what_connects(symbol, symbol)` answers in the level of the
  caller by default, but can be re-asked at any level. Two
  symbols are usually connected at the code level; two
  components are usually connected at the component level.
- `subgraph` is the primitive that most explicitly takes a
  level. A code-level `subgraph` is a call subgraph; a
  component-level `subgraph` is a component dependency
  subgraph; a system-level `subgraph` is a deployable view.

The levels are projections of one graph, not separate graphs. The
discipline is: every primitive accepts a level, and every result
carries the level of its nodes. The UI then renders the right
view for the level the caller is on.
