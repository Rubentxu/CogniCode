# Exploration: ExplorerQL Grammar

## Current State

The codebase already has **MoldQL**, a hand-written recursive-descent parser in
`crates/cognicode-explorer/src/moldql/` (858 lines in `parser.rs`, thoroughly tested).
MoldQL supports two operations: `FIND` (filtering with `WHERE`, scoping with `IN SCOPE`,
lens application with `APPLY`) and `EXPLORE` (BFS graph traversal with `THROUGH
direction DEPTH n`).

The parser is **hand-written with zero dependencies** — the codebase explicitly
rejects parser generators for small grammars (comment in `parser.rs` line 5-6:
"Zero new dependencies. The grammar is small (5 clauses, 1 precedence level) so
a parser-combinator crate would add more weight than value.").

MoldQL is exposed as the MCP tool `explorer_query_moldql` and compiles queries
into calls against the `SymbolRepository` trait and `QualityRepository`.

The **ask-router** (`crates/cognicode-explorer/src/ask/`) provides NL-to-primitive
routing with 8 priority-ordered patterns: PathBetween, ForwardReach, BackwardReach,
CodeQuality, Architecture, WorkspaceOverview, ComponentCluster, GenericDescription.
These 8 patterns are the seed for ExplorerQL queries.

## Affected Areas

- `crates/cognicode-explorer/src/moldql/parser.rs` — The existing recursive-descent
  parser that ExplorerQL must evolve from. The Cursor abstraction and error model
  (line/column diagnostics) are reusable.
- `crates/cognicode-explorer/src/moldql/ast.rs` — Current AST is `MoldQLQuery`
  enum with `Find` and `Explore` variants. Must expand to `path`, `neighbors`,
  `subgraph`, `cluster`, and `explain` expression variants plus filters.
- `crates/cognicode-explorer/src/moldql/executor.rs` — Executes MoldQL queries
  against `SymbolRepository`. ExplorerQL needs a richer executor that compiles
  to both SQL (PostgreSQL via `sqlx`) and `petgraph` algorithmic calls.
- `crates/cognicode-explorer/src/mcp.rs` — The `explorer_query_moldql` tool
  should evolve to also accept ExplorerQL syntax. A new tool name like
  `explorer_query` or evolving the existing tool is a naming decision.
- `crates/cognicode-core/src/domain/` — The graph data model needs provenance
  and confidence fields on edges (Phase 1). ExplorerQL filters reference these.
- `crates/cognicode-core/src/application/services/impact_analysis.rs` —
  The `ImpactAnalysisService` provides subgraph/cluster/explain operations that
  ExplorerQL primitives compile to.
- `docs/explorer-graph/query-and-navigation.md` — The 5 primitives spec
  (path, neighbors, subgraph, cluster, explain) that define ExplorerQL's scope.
- `docs/explorer-graph/target-product-model.md` — The provenance enum
  (`extracted`, `inferred`, `ambiguous`) and confidence rules that ExplorerQL
  must express.

## Approaches

### 1. Continue hand-written recursive descent (RECOMMENDED)

Extend the existing `Cursor`-based hand-written parser. Move the parser from
`moldql/` to a new `explorerql/` module while keeping the MoldQL subset working.

- **Pros**:
  - Zero new dependencies — consistent with codebase philosophy
  - Existing `Cursor`, `ParseError`, and test infrastructure are battle-tested
  - Full control over error model (pedagogical messages, glossary links)
  - Grammar stays small (expression-shaped, not statement-shaped per the spec)
  - Easy to evolve incrementally in step with the curated question set
- **Cons**:
  - Recursive-descent can become unwieldy if the grammar grows unexpectedly
  - No formal grammar specification (mitigated by document-first approach)
  - Precedence handling must be done manually (manageable for expression grammar)
- **Effort**: Medium

### 2. Introduce `winnow` parser combinator

Add `winnow` (modern successor to `nom` with streaming support) for pattern-based
parsing.

- **Pros**:
  - Declarative grammar definition, easier to read and maintain
  - Built-in error recovery
  - Good for the opt-in advanced Cypher-inspired surface (Phase 4)
- **Cons**:
  - New dependency (conflicts with codebase's zero-dependency philosophy)
  - Learning curve for team
  - The existing hand-written parser works well for the current grammar size
  - Error model customization is possible but requires more work than hand-written
- **Effort**: Medium (adds dependency, but may reduce parser line count)

### 3. Introduce `pest` PEG parser

Use pest's `.pest` grammar file to define ExplorerQL syntax declaratively.

- **Pros**:
  - Formal grammar as a separate file — self-documenting
  - Good error messages out of the box
  - Widely used in Rust ecosystem
- **Cons**:
  - New dependency + build-time code generation
  - Less control over error model than hand-written
  - Overkill for the v1 grammar (expression-shaped, 5-8 clause types)
  - The existing pattern is hand-written; pest would be a style mismatch
- **Effort**: Low/Medium (pest is well-documented, but integration is non-trivial)

### 4. Introduce `lalrpop` LR parser generator

Generate a parser from a BNF-like grammar specification.

- **Pros**:
  - Formal, deterministic parsing
  - Good for statement-shaped languages
- **Cons**:
  - ExplorerQL is expression-shaped, not statement-shaped — LR is the wrong tool
  - Requires ANTLR-style grammar file, build script, generated code
  - Heavyweight for the grammar scope
  - Build-time dependency
- **Effort**: High — not recommended

## Grammar Scope for v1

Based on the roadmap (Phase 3) and `query-and-navigation.md`:

### Must-have (Phase 2 expansion):
- `path(from, to, max_depth, level_filter)` — shortest paths
- `neighbors(node, direction, edge_kinds, depth, level_filter)` — graph neighbors
- `subgraph(root, radius, level_filter)` — bounded subgraph
- `cluster(level, algorithm, params)` — community detection
- `explain(edge_or_node)` — evidence chain

### Must-have (Phase 3 expansion):
- Provenance filter: `where provenance in {extracted, inferred}`
- Confidence filter: `where confidence >= 0.7`
- Boolean composition: `and`, `or`, `not`
- Joins across levels: `subgraph(component) > symbols`
- Named queries: `@query-name`

### Deferred (Phase 4):
- Time-windowed queries: `subgraph(X, depth, since=t)`
- Source filters: `cites(X) where source in spaces(a, b)`
- Cypher-inspired advanced surface (opt-in, separate grammar)

## Compilation Targets

ExplorerQL compiles to **two targets**, per `query-language-decision.md`:

1. **PostgreSQL queries** (persistent traversals): Recursive CTEs, `ltree`
   ancestors, `JSONB` predicates. These go through `cognicode-core`'s
   `SymbolRepository` trait and the `sqlx` adapter. Used for: `neighbors`,
   `subgraph` at large scale, `path` with many nodes.

2. **`petgraph` calls** (algorithmic analyses): Path finding (BFS/Dijkstra),
   centrality (betweenness), community detection (Leiden), impact blast radius.
   Used through `ImpactAnalysisService`. Used for: `cluster`, `path` (small
   scale), SCC detection.

The boundary is explicit in the design: anything the database can do well,
the database does. Anything the database cannot do well (graph algorithms),
`petgraph` does in Rust. The split is the boundary between `cognicode-core`'s
SQL adapter and its `petgraph` adapter.

**The executor should NOT directly call MCP tools** — it compiles to service
calls and core primitives, which are what the MCP tools also consume.

## AST Shape

The ExplorerQL AST extends the existing `MoldQLQuery` enum:

```rust
pub enum ExplorerQLQuery {
    // Existing (backward-compatible)
    Find(FindQuery),
    Explore(ExploreQuery),

    // New primitives
    Path(PathQuery),
    Neighbors(NeighborsQuery),
    Subgraph(SubgraphQuery),
    Cluster(ClusterQuery),
    Explain(ExplainQuery),

    // Composition
    Pipe { left: Box<ExplorerQLQuery>, right: Box<ExplorerQLQuery> },
    NamedRef { name: String },
}

pub struct PathQuery {
    pub from: ObjectRef,
    pub to: ObjectRef,
    pub max_depth: u32,
    pub level_filter: Option<Level>,
    pub provenance_filter: Option<ProvenanceFilter>,
    pub confidence_filter: Option<ConfidenceFilter>,
}
// ... similar for NeighborsQuery, SubgraphQuery, ClusterQuery, ExplainQuery
```

Key AST design decisions:
- Expression-shaped, not statement-shaped — every query returns a value
- Typed: nodes, edges, levels, provenance are nominal types
- Composable: queries can be piped (`|`) and referenced by name (`@name`)
- The AST carries enough info to compile to either SQL or petgraph

## MCP Exposition

ExplorerQL can be exposed as:

1. **Evolve `explorer_query_moldql`** — the existing tool accepts both MoldQL
   and ExplorerQL syntax. Backward-compatible with existing callers.

2. **New `explorer_query` tool** — a dedicated ExplorerQL tool. Clean separation
   but breaks the existing tool's promise.

3. **Both** — `explorer_query_moldql` delegates to the ExplorerQL parser when
   it detects the richer syntax. Zero breakage, maximum reuse.

**Recommendation: Option 3.** The existing `explorer_query_moldql` tool should
accept ExplorerQL syntax transparently. MoldQL is a proper subset; the parser
can detect which grammar is being used from the leading keyword.

ExplorerQL can also be exposed as:
- An HTTP endpoint in `crates/cognicode-explorer/src/bin/api.rs`
- A query field in the Cytoscape.js explorer frontend (Phase 3)
- A utility the ask-router can call when NL classification confidence is low

## Relationship with Ask-Router

The ask-router (`cognicode_ask`) and ExplorerQL are **complementary**, not competing:

| Aspect | Ask-Router | ExplorerQL |
|--------|-----------|------------|
| Input | Natural language | Typed expression |
| User | New users, agents | Power users, saved queries |
| Confidence | 0.5–1.0 (regex match score) | 1.0 (deterministic parse) |
| Granularity | Coarse (8 patterns) | Fine (full primitives + filters) |
| Error model | Fallback + alternatives | Pedagogical (names bad part + suggestion) |

The relationship: the ask-router classifies NL → dispatches primitives.
ExplorerQL is the direct form of those primitives. A user can start with
NL questions and graduate to ExplorerQL. The ask-router may eventually
compile NL questions into ExplorerQL expressions (Phase 4+).

## Provenance/Confidence Filtering

Based on `target-product-model.md`, ExplorerQL v1 grammar must support:

```text
subgraph(auth::login, radius = 2, level = code)
  where provenance in {extracted, inferred}
    and confidence >= 0.7
```

The filter grammar:

```
provenance_filter ::= "where provenance in {" provenance_val ("," provenance_val)* "}"
                    | "where provenance = " provenance_val
confidence_filter ::= "where confidence" op number
boolean_comb ::= "and" | "or" | "not"
```

The AST nodes for provenance and confidence already exist in the target model
(`provenance` enum, `confidence` f64). ExplorerQL adds the parse-time surface
and the executor applies them as post-filters after the primitive returns.

## Recommendation

**Continue with hand-written recursive descent**, extending the existing
`moldql/parser.rs` into a new `explorerql/` module. The grammar is small
(expression-shaped, 5 clause types), the existing `Cursor` and error model
are battle-tested, and the codebase has an explicit "zero new dependencies"
philosophy.

**Grammar scope for v1**: The 5 primitives (path, neighbors, subgraph, cluster,
explain) plus provenance/confidence filters, boolean composition, and named
queries. Defer joins-across-levels and time-windowed queries to Phase 4.

**Compilation**: ExplorerQL AST compiles to PostgreSQL SQL for persistent
traversals and `petgraph` calls for algorithmic analyses. The executor selects
the target based on the primitive (subgraph → SQL, cluster → petgraph, etc.)
and the data size.

**AST**: Extend `MoldQLQuery` with new variants for each primitive plus
`Pipe` and `NamedRef` for composition. Keep the `Cursor` and `ParseError`
abstractions from the existing parser.

**MCP**: `explorer_query_moldql` evolves to accept ExplorerQL syntax
transparently. MoldQL is a proper subset.

## Risks

- **Grammar scope creep**: The curated question set is closed — new grammar
  surface appears only when a new question needs it. Mitigation: lock the
  grammar to the 5 primitives for v1; reject expansion requests unless they
  map to a new curated question.
- **Parser complexity**: Hand-written recursive descent can become unreadable
  if the grammar grows. Mitigation: keep the parser in one module, test each
  clause independently, and refactor to `winnow` only if the grammar exceeds
  ~1500 lines of parser code.
- **Provenance/confidence fields availability**: ExplorerQL filters on provenance
  and confidence, but these fields are currently `Partial` or `Missing` in
  the codebase (per `current-state-audit.md`). Mitigation: the grammar can
  accept the filters syntactically, but the executor degrades gracefully
  (runtime check, warn if fields are not populated).
- **Two compilation targets**: The SQL/petgraph split could produce inconsistent
  results if the same query compiles differently. Mitigation: the executor
  decides the target per-primitive, not per-query; a query always uses the
  same target for a given primitive.

## Ready for Proposal

Yes — the exploration has clear findings. The orchestrator should tell the user:
- Parser: hand-written, extend existing MoldQL parser
- Grammar v1 scope: 5 primitives + provenance/confidence filters + boolean + named queries
- Compilation: PostgreSQL SQL and petgraph
- AST: Extend MoldQLQuery with new variant types
- MCP exposure: Evolve explorer_query_moldql to accept ExplorerQL syntax
- Relationship: ExplorerQL is the typed expression form of the ask-router's 8 NL patterns
