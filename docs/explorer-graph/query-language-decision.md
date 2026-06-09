# Query Language Decision

## Bottom Line Up Front

Cypher is a good language. It is not the user-facing query
language for CogniCode Explorer. The product's primary query
surface is a product-owned DSL that grows from the existing
MoldQL grammar into a richer expression we call ExplorerQL.
The DSL compiles to PostgreSQL queries for the persistent
graph and to `petgraph` calls for the in-memory analytics.
This file explains the reasoning and shows the recommended
query forms.

## Why Cypher Is Attractive

Cypher is the most widely used graph query language. Its
attractiveness is real, and the product team recognizes it.

- It is a pattern-matching language. The unit of expression
  is a shape: a node, an edge, a property. This matches how
  developers think about a graph.
- It is short. The common idioms (`(n)-[r]->(m)`) read like
  the diagrams they describe.
- It has a strong mental model. The graph is the data; the
  query is a walk on it.
- The ecosystem knows it. Anyone who has used Neo4j or
  Apache AGE already reads Cypher.
- It is well documented. The reference materials are deep.

For a power user who wants to write a complex query, Cypher
is a reasonable surface. The product team explicitly
acknowledges this.

## Why We Are Not Choosing Cypher as the Primary Explorer Language

Five reasons, in order of weight.

1. **Coupling.** Cypher is owned by Neo4j. Its openCypher
   variant is a partial mirror with a different trajectory.
   Binding the Explorer's primary surface to either locks
   the product to a vendor's roadmap and grammar choices.
   The product commits to a long-lived graph model with
   provenance, confidence, and named views. A third-party
   grammar is the wrong foundation.
2. **Onboarding.** The average user of the Explorer is not
   a graph user. They are a developer who wants to answer a
   question about their code. Asking them to learn a query
   language before they can ask "what is risky to change
   here?" is a barrier the product cannot afford. The
   on-ramp is the curated question set, not a language.
3. **Curated question set.** The product commits to a closed
   set of questions in `target-product-model.md`. The set is
   large enough to be useful and small enough to be
   learnable. Cypher is general; the product is not. A
   general grammar invites questions the product cannot
   answer well, and that mismatch erodes trust.
4. **Error model.** Cypher errors are technical. The
   Explorer's error model is pedagogical: a bad query
   produces a message that names the bad part, suggests
   the closest valid form, and links to the glossary. A
   product-owned DSL is the only way to keep that promise
   across every surface (the explorer, the MCP server, the
   CLI).
5. **Autocomplete.** The Explorer's autocomplete is bound
   to the focused object kind. Cypher's autocomplete would
   be a generic language server; the product's autocomplete
   is a teaching tool. The teaching tool wins.

## Why Apache AGE Is Not the Foundation for v1 or v2

Apache AGE is a graph extension for PostgreSQL that exposes
openCypher. The reasons it is not the foundation are:

- **Maturity.** The extension is young. Its breaking
  changes are real and recent. A foundational dependency
  has to be boring; AGE is not boring yet.
- **Grammar coupling.** The reasons for not adopting Cypher
  apply to openCypher as well: the grammar is a third-party
  contract the product does not control.
- **Diminishing return.** PostgreSQL plus recursive CTEs
  plus `ltree` plus `petgraph` covers every question in the
  curated set. AGE is a solution in search of a problem the
  product has.
- **Migration cost.** Adopting AGE means moving from a thin
  DSL that compiles to SQL and Rust to a DSL that compiles
  to AGE-flavored openCypher. The first is small; the second
  is a new backend the product would then have to maintain.
- **Engine ownership.** Moving the graph engine into the
  database reduces the role of the Rust analytical layer
  and of `petgraph`. The product's differentiator is the
  combination of a typed graph model and Rust analytics;
  AGE pushes the analytics into the database and weakens
  that combination.

AGE may be worth future experimentation, particularly for
power users who specifically want graph-style queries. That
work belongs in a separate, opt-in surface, not in the core
MCP or explorer path. The opt-in surface must not couple the
product to AGE's grammar.

## The Recommended Query Model

The model has three layers. The user picks the layer they
want; the engine does the rest. The three layers cooperate;
they do not compete.

### Layer 1: User-facing product verbs

The default surface. Buttons, suggested questions, and the
"what can I do here?" panel. The user names a verb, names
an object, and the engine returns a result with provenance
and confidence.

These verbs are the curated question set documented in
`query-and-navigation.md` (`why`, `what_connects`,
`what_changed`, `what_is_risky`, `where_does_it_belong`,
`what_justifies`, `what_is_the_shape`, `where_to_start`).
They are the public contract. The MCP server exposes them
as named tools. The explorer exposes them as buttons and
as suggested questions.

### Layer 2: ExplorerQL (evolved MoldQL)

A small, typed expression language typed into the
explorer's query field. Its job is to express the curated
question set in a way a power user can write, save, and
share. Its grammar grows in step with the verbs above.

The shape of the language:

- It is expression-shaped, not statement-shaped. A query
  returns a value.
- It is typed. Nodes, edges, levels, and provenance are
  nominal types.
- It is bounded. New surface area is added only when a
  curated question needs it.
- It is composable. A query can be named, saved, and
  referenced from another query.
- It is auditable. Every compiled query carries the chain
  of lower-level primitives it expanded to, so the result
  is explainable end to end.

The compilation has two targets:

- **Persistent traversals** compile to parameterized SQL
  against PostgreSQL. Recursive CTEs, `ltree` ancestors, and
  `JSONB` predicates carry the work.
- **Algorithmic analyses** compile to `petgraph` calls. Path
  finding, betweenness, community detection, and the
  impact blast radius run in Rust against in-memory
  projections of the persistent graph.

The split is explicit. Anything the database can do well,
the database does. Anything the database cannot do well,
Rust does. The split is the boundary between
`cognicode-core`'s SQL adapter and its `petgraph` adapter.

### Layer 3: Optional advanced surface (future)

A future, opt-in surface may expose a Cypher-inspired
syntax for power users. It would not be the primary
surface. It would be a sandbox, behind a feature flag,
with its own documentation, its own error model, and its
own deprecation policy.

The advanced surface must not couple the product to
Cypher. The surface borrows Cypher's idioms where they
help; it rejects them where they conflict with the
Explorer's model (provenance, confidence, level filters,
named views). The surface may compile to the same SQL and
`petgraph` targets as Layer 2; it does not compile to AGE
or to a Cypher engine.

## Examples of Recommended Query Forms

The examples below use the current MoldQL grammar and the
Phase 2 and Phase 3 expansions documented in
`query-and-navigation.md`. They are illustrative; the final
grammar is decided in step with the verb set.

### Verb-style

The user picks a verb. The engine returns the standard
envelope.

```text
why(symbol = auth::login)
what_connects(auth::login, billing::charge)
what_changed(component = auth, since = 7d)
what_is_risky(symbol = auth::login)
where_does_it_belong(symbol = auth::login)
what_justifies(symbol = auth::login)
what_is_the_shape(level = component)
where_to_start(brain = cognicode-core)
```

### ExplorerQL-style

The user writes a typed expression. The engine compiles it
and returns the standard envelope.

```text
// direct neighbors of a symbol, restricted by level and edge kind
neighbors(auth::login, direction = out, edge_kinds = [calls],
         depth = 1, level = code)

// paths between two symbols, capped by depth and level
path(auth::login, billing::charge, max_depth = 8, level = code)

// a subgraph around a symbol, with provenance and confidence filters
subgraph(auth::login, radius = 2, level = code)
  where provenance in {extracted, inferred}
    and confidence >= 0.7

// communities at the code level, with a parameter
cluster(level = code, algorithm = leiden, resolution = 1.0)
```

### Compositions

The verb and the expression styles compose.

```text
// "what changed in the system this node belongs to?"
what_changed(node = where_does_it_belong(symbol = auth::login).system,
             since = 30d)

// "show me the risky parts of the communities in this container"
subgraph(container = billing, level = component)
  | what_is_risky(level = component)
```

### Named queries

A query can be named, saved, and referenced from another
query. Named queries are content; they live in the
docs/help layer with the same lifecycle as glossary
entries.

```text
// saved as "auth-callers-with-evidence"
neighbors(auth::login, direction = in, depth = 2)
  where confidence >= 0.8
```

A second query can call it:

```text
what_is_risky(subgraph = @auth-callers-with-evidence)
```

## Error Model

ExplorerQL errors are content, not exceptions. The error
message names the bad part, suggests the closest valid
form, and links to the glossary entry for the term. The
error model is the same as the one in
`query-and-navigation.md` for MoldQL. The advanced surface
must adopt the same error model; otherwise, it is not part
of the product.

The error model is one of the surfaces new users see. It
is part of the on-ramp, not a developer-experience
afterthought.

## Autocomplete and Discovery

Autocomplete suggests:

- The verbs in the curated question set, in priority order
  for the focused object kind.
- The node kinds, edge kinds, and levels the focused object
  can see.
- The names of saved queries the user can call.
- The closest valid form when the current input is not
  valid.

The suggestions are the on-ramp. A user who never types
ExplorerQL can still answer every question the product
commits to; a user who does type it gets a faster path.

## What This File Decides

- The product's primary query surface is the product-owned
  ExplorerQL, with the user-facing verbs as the default
  expression of it.
- Cypher is not the primary surface.
- openCypher via Apache AGE is not the foundation.
- A future, opt-in Cypher-inspired surface is allowed but
  must not couple the product to Cypher.
- The compile targets are PostgreSQL (persistent
  traversals) and `petgraph` (algorithmic analyses).
- The query language and the storage layer are coupled by
  the trait in `core-mcp-boundaries.md`, not by a shared
  grammar.

## Related Documents

- `stack-recommendation.md` - the storage and engine
  choice that the queries compile against.
- `query-and-navigation.md` - the verbs, primitives, and
  envelope.
- `target-product-model.md` - the model the queries act
  on.
- `core-mcp-boundaries.md` - where the language and its
  compiler live.
- `glossary.md` - the terms the error model links to.
- `roadmap.md` - the staged growth of the language.
