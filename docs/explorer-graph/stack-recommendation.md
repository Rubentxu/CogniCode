# Stack Recommendation: Storage, Engine, and Migration Direction

## Bottom Line Up Front

The recommended target stack for the CogniCode Explorer Graph
product is unambiguous: a single canonical database
(PostgreSQL), a single in-memory graph algorithmic layer in
Rust (`petgraph`), and a product-owned query language on top.
SQLite is not the target. JSON graph snapshots are useful as
exports, artifacts, and debugging aids, not as a primary
store. Cypher is interesting but it is not the user-facing
query language. Apache AGE is worth future experimentation
but it is not the foundational dependency for v1 or v2. This
file explains why, what was compared, and how the migration is
staged.

The decisions in this file are coupled with the decisions in
`query-language-decision.md` (the user-facing query surface)
and `visualization-stack.md` (the frontend graph engine).
Read the three together.

## Current Stack Today

The audit in `current-state-audit.md` is the ground truth. The
facts that matter for this decision are:

- The graph lives in memory inside `cognicode-core`. Persistence
  is partial: there is caching, but no first-class, versioned,
  canonical store yet.
- SQLite appeared in earlier design discussions as a possible
  primary store. The product is explicitly moving away from
  that. SQLite is acceptable only as a transitional
  compatibility layer or a local-only utility.
- The JSON graph snapshot appears in the design as a notion
  for export, sharing, and debugging. That role is preserved
  and clarified; the JSON form is not the source of truth.
- The product's query surface is MoldQL, evolved into a thin
  product-owned DSL that compiles to chains of core
  primitives. The exact evolution and its scope is described
  in `query-language-decision.md`.
- The MCP server is the agent-facing surface. It stays
  unchanged in role; only its backing implementation moves.

## What This File Decides

The four coupled decisions:

1. Where the graph lives persistently. **PostgreSQL.**
2. How the graph is shaped. **Typed tables with `ltree`,
   `JSONB`, and `pgvector` for the parts that need them.**
3. What runs the algorithmic analyses. **`petgraph` in Rust
   against in-memory projections of the persistent state.**
4. What the role of the JSON snapshot is. **An export and
   debugging artifact, not a primary store.**

The query language and the visual stack are decided in
`query-language-decision.md` and `visualization-stack.md`.

## Options Considered

### Option A: SQLite plus application graph plus JSON snapshots

A small, embedded SQLite database holds a normalized projection
of the graph. The application owns the graph shape; the
database is a denormalized cache. JSON snapshots are produced
for export, sharing, and diffing.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | Lowest. SQLite is embedded; no separate process.        |
| Single-node scale     | Strong for tens of thousands of nodes.                  |
| Multi-node scale      | Weak. No native client-server mode for concurrent users.|
| Concurrency           | Single-writer. Fine for one user, painful for many.     |
| Vector search         | No native vector type. Embedding search needs a shim.   |
| Hierarchical data     | Recursive CTEs are possible but heavy.                  |
| Schema evolution      | Easy locally. Migration stories diverge from prod.      |
| Graph algorithms      | Application must implement them.                        |
| Tooling maturity      | Mature, ubiquitous.                                     |
| Team familiarity      | High.                                                   |
| Cross-source federation | Possible only at the application layer.               |

**Verdict.** Acceptable for a single-user desktop tool. Not a
fit for a multi-user, federated, agent-facing product with
vector and hierarchical needs. The single-writer model is a
hard ceiling the product outgrows quickly.

### Option B: PostgreSQL relational and semantic graph model

A single PostgreSQL instance holds the canonical graph. Edges
and nodes are typed tables. Recursive CTEs handle traversals
up to a small depth. `ltree` carries the scope hierarchy;
`JSONB` carries flexible payload; `pgvector` carries
embeddings; `sqlx` is the async DB layer in the Rust backend.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | Low. Single open-source server, well understood.        |
| Single-node scale     | Strong. PostgreSQL handles millions of rows with care.  |
| Multi-node scale      | Strong with read replicas and logical replication.      |
| Concurrency           | Mature MVCC; real concurrent writers and readers.       |
| Vector search         | First-class via `pgvector`.                             |
| Hierarchical data     | First-class via `ltree`.                                |
| Flexible payload      | First-class via `JSONB`.                                |
| Schema evolution      | Migrations are a solved problem.                        |
| Graph algorithms      | Persistent queries via SQL. Heavy analytics live in     |
|                       | Rust via `petgraph`.                                    |
| Tooling maturity      | Very mature.                                            |
| Team familiarity      | Growing but strong in the Rust ecosystem.               |
| Cross-source federation | First-class via schemas, schemas per source, and     |
|                       | a `brain` join layer in the application.               |

**Verdict.** This is the recommended path. It satisfies every
shape of question the curated question set needs without
introducing a second system to operate. The single-database
discipline is the operational win.

### Option C: PostgreSQL plus Apache AGE

Apache AGE is a graph extension for PostgreSQL. It exposes
openCypher as a query language and stores the graph as labeled
edges and vertices inside the database.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | Same as PostgreSQL. The extension is the cost.         |
| Single-node scale     | Comparable to PostgreSQL.                               |
| Multi-node scale      | Comparable to PostgreSQL.                               |
| Concurrency           | Comparable to PostgreSQL.                               |
| Vector search         | Still via `pgvector`; AGE is orthogonal.                |
| Hierarchical data     | Still via `ltree`.                                      |
| Query language        | openCypher. Familiar to graph users.                    |
| Maturity              | Young extension. Breaking changes between versions.     |
| Coupling              | High. The product's primary query surface becomes the   |
|                       | extension's grammar.                                    |
| Vendor neutrality     | Reduced. Opting into an extension with its own roadmap. |
| Engine ownership       | Reduced. The graph engine lives in the database.        |

**Verdict.** Interesting for future experimentation, in
particular for power users who specifically want a
Cypher-shaped surface. Not the foundation for v1 or v2. The
product's primary query surface must not be coupled to a
young extension's grammar. AGE may be revisited later as a
secondary, opt-in backend; it is not the v1 store and not the
v1 query language.

### Option D: Neo4j

A dedicated graph database with Cypher as the primary query
language, stored procedures for analytics, and a strong
visualization story.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | High. Another system to license, operate, and back up.  |
| Single-node scale     | Strong. Purpose-built for graph workloads.              |
| Multi-node scale      | Strong. Causal clustering and read replicas.            |
| Concurrency           | Strong.                                                 |
| Vector search         | Added later, not first-class.                           |
| Hierarchical data     | Possible, not idiomatic.                                |
| Schema evolution      | Possible, with its own migration story.                 |
| Graph algorithms      | Library of production-grade algorithms.                 |
| Tooling maturity      | Mature.                                                 |
| Team familiarity      | Mixed. Smaller Rust-plus-graph overlap.                 |
| Cost                  | License cost for enterprise features.                   |
| Coupling              | High. Primary surface is Cypher; engine is proprietary. |

**Verdict.** A second database is operationally expensive and
unnecessary. PostgreSQL plus `petgraph` covers the algorithmic
needs. Adopting Neo4j also re-opens the Cypher-as-primary
question, which is rejected for the reasons in
`query-language-decision.md`. Neo4j is not adopted.

### Option E: SurrealDB

A multi-model database with a graph layer, a document layer,
and embedded scripting.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | Low. Single binary.                                     |
| Single-node scale     | Strong.                                                 |
| Multi-node scale      | Improving. Distributed mode is young.                   |
| Concurrency           | Strong.                                                 |
| Vector search         | Native.                                                 |
| Hierarchical data     | Possible, not idiomatic.                                |
| Schema evolution      | Flexible.                                               |
| Graph algorithms      | Built-in, smaller library than Neo4j.                   |
| Tooling maturity      | Young.                                                  |
| Team familiarity      | Low.                                                    |
| Ecosystem             | Small relative to PostgreSQL.                           |
| Rust client maturity  | Improving; not at the level of `sqlx` for PostgreSQL.   |

**Verdict.** A fascinating single-binary option, but the
ecosystem, maturity, and team familiarity are not where the
product needs them. PostgreSQL has the stronger foundation
and a deeper Rust ecosystem.

### Option F: In-memory graph plus JSON snapshots only

The graph lives in process memory. JSON snapshots are the only
persistent form. There is no relational store.

| Aspect                | Assessment                                              |
| --------------------- | ------------------------------------------------------- |
| Operational cost      | Lowest.                                                 |
| Single-node scale     | Bounded by process memory.                              |
| Multi-node scale      | Not possible without a shared store.                    |
| Concurrency           | Bounded by process concurrency model.                   |
| Vector search         | Application code.                                       |
| Hierarchical data     | Application code.                                       |
| Schema evolution      | Format versioning.                                      |
| Graph algorithms      | First-class in `petgraph`.                              |
| Cross-source federation | Not possible at the data layer.                       |
| Agent-facing API      | Awkward; each agent must rebuild the graph.             |

**Verdict.** Good for a research prototype, not for the
product. Multi-user, federated, agent-facing work needs a
shared, durable store. The graph state has to outlive the
process.

## Comparison Summary

| Option                     | Fit for the product    | Primary concern                    |
| -------------------------- | ---------------------- | ---------------------------------- |
| A. SQLite plus app plus JSON | Marginal             | Single-writer; no shared state     |
| B. PostgreSQL relational   | Strong                 | None that is blocking              |
| C. PostgreSQL plus AGE     | Possible, deferred     | Couples product to a young grammar |
| D. Neo4j                   | Rejected               | Second database; Cypher coupling   |
| E. SurrealDB               | Rejected               | Ecosystem and maturity             |
| F. In-memory plus JSON     | Rejected               | No shared, durable state           |

## Recommended Target Stack

| Layer                  | Choice                                                     |
| ---------------------- | ---------------------------------------------------------- |
| Canonical store        | PostgreSQL                                                 |
| Async DB layer         | `sqlx`                                                     |
| Hierarchical data      | `ltree`                                                    |
| Flexible payload       | `JSONB`                                                    |
| Embeddings             | `pgvector`                                                 |
| Graph algorithm core   | `petgraph` (in-memory, Rust)                                |
| Query language         | Product-owned DSL: evolved MoldQL, called ExplorerQL       |
| External API surface   | MCP (unchanged)                                            |
| Export and artifact    | JSON graph snapshots (for export, diff, debugging)         |
| Transitional compat    | SQLite, only during migration, behind a feature flag        |

The export and artifact role of JSON is preserved. Users can
export a named view to JSON for sharing, diffing, and offline
analysis. The engine reads the JSON only as a debugging aid
and as an import path. The canonical state of truth is in
PostgreSQL.

## Migration Direction

The migration is staged and reversible at every step.

1. **Phase 1.** Introduce a `Repository` trait in
   `cognicode-core` that does not commit to a backing store.
   Ship three implementations: `InMemory` (default in tests),
   `Sqlite` (compatibility), and `Postgres` (target).
2. **Phase 2.** Land the `Postgres` implementation behind the
   same trait. The explorer and the MCP server continue to
   work against the in-memory and SQLite implementations. CI
   runs all three.
3. **Phase 3.** Migrate the default development and CI
   configuration to PostgreSQL. The SQLite path remains
   available behind a feature flag for local-only use and for
   tools that genuinely need an embedded store.
4. **Phase 4.** Retire SQLite as a default. The `Repository`
   trait stays; the default does not. JSON snapshots become a
   pure export and import format with no path back into the
   canonical state.

The key invariant is that the trait surface is stable. New
backing stores plug in; nothing else has to change. The
`cognicode-explorer` and `cognicode-mcp` crates never reach
into the persistence layer directly; they consume the trait.

## What This File Explicitly Rejects

- Two databases in production. PostgreSQL is the only one.
- JSON as the source of truth. JSON is export, import, and
  debugging only.
- Cypher, openCypher, or any third-party graph grammar as
  the primary user-facing query language.
- Apache AGE as a foundational dependency.
- Neo4j or SurrealDB as a second system.
- An embedded-only persistence story for the product. The
  graph state must outlive the process.
- Letting the explorer hold the persistent graph. The
  persistence decision belongs to the backend.

## Related Documents

- `query-language-decision.md` - the user-facing query model.
- `visualization-stack.md` - the frontend stack.
- `target-product-model.md` - the shape of the graph.
- `core-mcp-boundaries.md` - where persistence lives.
- `roadmap.md` - the migration phases.
- `current-state-audit.md` - the present-state gap.
