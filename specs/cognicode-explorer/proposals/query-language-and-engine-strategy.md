# Query Language and Engine Strategy

## Goal

Design the query layer for a developer-facing moldable exploration application. The query layer must let humans and agents navigate code scopes, architectural concepts, SOLID lenses, connascence hotspots, diagram outputs, reports, and derived knowledge with minimal effort and high performance.

## Core Finding

Do not create a general graph query language from scratch. Existing graph/query languages already cover that layer: Cypher, GQL, Gremlin, SPARQL, SQL/PGQ, SurrealQL-style graph traversal, and Datalog.

Create a product DSL above them: a small, developer-oriented exploration language that compiles to graph queries, rule evaluations, view generation, and agent prompts.

Working name:

```text
MoldQL
```

MoldQL should not compete with Cypher. It should express the user's intent in the product language:

```text
inspect module "billing" with connascence
show hotspots where strength >= position and locality > module
render as mermaid
explain with evidence
record decision "Introduce value object for payment status"
```

## Why a Product DSL Is Needed

Cypher/GQL are powerful but too low-level for the main UX. Developers should not need to know the graph schema to ask useful questions.

The user thinks in scopes and lenses:

- Scope: repository, module, file, class, function, route, flow, PR.
- Lens: C4, SOLID, connascence, runtime, tests, domain vocabulary, ownership, churn.
- View: graph, table, heatmap, C4, Mermaid, PlantUML, report, image, audio.
- Decision: refactor, test, document, investigate, accept risk, create ADR.

The query language should preserve that mental model.

## Existing Language Evaluation

### Cypher

Best fit for property graph traversal and pattern matching.

Pros:

- Familiar to graph database users.
- Natural for nodes, edges, labels, paths.
- Supported by Kuzu and many graph systems.
- Good target language for generated queries.

Cons:

- Too schema-aware for casual developer exploration.
- Not ideal for domain-specific commands like "inspect with connascence".

Use as: backend/power-user query target.

### GQL

ISO standard graph query language.

Pros:

- Standards direction for property graph querying.
- Good long-term compatibility story.

Cons:

- Ecosystem still newer than Cypher.
- Less familiar today for most developers.

Use as: future-compatible backend target if engine support matures.

### Gremlin

Traversal-oriented graph language.

Pros:

- Powerful for imperative path traversal.

Cons:

- Verbose and less friendly for report/view generation.
- Less aligned with declarative product UX.

Use as: not recommended for primary UX.

### SPARQL/RDF

Best for semantic web and ontology-style data.

Pros:

- Strong semantics and standards.
- Good if the product evolves into linked enterprise knowledge.

Cons:

- RDF triples are less natural for code structure than labeled property graphs.

Use as: optional semantic/ontology export later.

### SQL/PGQ

Property graph queries integrated into SQL.

Pros:

- Useful when mixing graph and tabular analytics.

Cons:

- Rust ecosystem support is still developing.
- More awkward for interactive graph navigation.

Use as: future bridge for analytics, not MVP.

### Datalog

Excellent for recursive rules and static-analysis facts.

Pros:

- Natural for derived facts: cycles, reachability, coupling rules, architecture constraints.
- Strong fit for program analysis.

Cons:

- Not friendly as primary user-facing exploration syntax.
- Runtime integration varies; Souffle is fast but external, DDlog/differential approaches are powerful but complex.

Use as: rule/lens engine for derived facts, not main UX.

### SurrealQL Arrow Traversal

Readable graph traversal syntax using arrows.

Pros:

- Very approachable for path navigation.
- Good inspiration for human-readable scope traversal.

Cons:

- Tied to SurrealDB's data model.
- Not a code-analysis standard.

Use as: syntax inspiration for MoldQL paths.

## Engine Evaluation

### Kuzu

Strong candidate for graph backend.

Pros:

- Embedded property graph database.
- Cypher support.
- Columnar storage, CSR adjacency, vectorized/factorized execution.
- Full-text and vector search.
- Rust bindings.
- Good fit for local-first developer tooling.

Cons:

- Core is C++, not pure Rust.
- Rust integration depends on bindings.

Recommendation: best pragmatic backend candidate for MVP if C++ core is acceptable, but it must sit behind a strict backend abstraction. The product must not expose Kuzu-specific concepts above the storage adapter boundary.

### Pure Rust Graph Engines

Candidates seen in research include Grafeo, OCG, SparrowDB, Ladybug/lbug, BikoDB, and similar projects.

Pros:

- Rust-native integration.
- Potentially easier embedding, WASM, and single-binary story.

Cons:

- Many are new or less proven.
- Maturity, query completeness, and long-term maintenance need careful verification.

Recommendation: evaluate with prototypes, but do not bet the MVP on an immature engine without benchmarks.

### SurrealDB

Pros:

- Rust-based, multi-model, approachable graph traversal syntax.
- Good for app data plus graph-like relationships.

Cons:

- Not specialized for code graph analytics.
- Query semantics are product/database-specific.

Recommendation: useful inspiration or app metadata store, but not first choice for code graph core.

### DataFusion

Pros:

- Rust-native, very extensible, high-performance analytic query engine.
- Excellent for tables, Arrow, aggregations, reports.
- Can support custom logical/physical operators.

Cons:

- Graph path querying is not its native strength.
- SQL/PGQ support appears exploratory rather than ready.

Recommendation: use for analytic/reporting plane, not as the primary graph traversal engine in MVP.

### Differential Dataflow / Datalog

Pros:

- Strong for incremental derived facts over changing graph data.
- Excellent fit for continuously updated lenses such as reachability, cycles, architectural rule violations, and connascence-derived facts.

Cons:

- Higher implementation complexity.
- Not a friendly direct product query interface.

Recommendation: future advanced lens engine after MVP.

## Recommended Architecture

Use a layered query architecture:

```text
Human / Agent Intent
    -> MoldQL
    -> Query Planner
    -> Graph Backend Queries + Lens Rules + View Pipeline
    -> Evidence Pack + Visualization + Explanation
```

### Layer 1: Storage Model

Use a labeled property graph as the primary model.

Core nodes:

- Repository
- Module
- File
- Class
- Function
- Method
- Interface
- Route
- Test
- ADR
- RuntimeTrace
- View
- Decision

Core edges:

- CONTAINS
- DEFINES
- CALLS
- IMPORTS
- IMPLEMENTS
- EXTENDS
- USES
- TESTS
- CHANGED_WITH
- DEPENDS_ON
- VIOLATES
- HAS_CONNASCENCE
- HAS_VIEW
- SUPPORTS_DECISION

### Layer 2: Backend Query Language

Use Cypher first, because it is pragmatic and widely understood.

Example backend query:

```cypher
MATCH (m:Module {name: $module})-[:CONTAINS*]->(f:Function)
MATCH (f)-[c:HAS_CONNASCENCE]->(other)
WHERE c.strength >= $min_strength
RETURN f, c, other
ORDER BY c.degree DESC
LIMIT 50
```

### Layer 3: MoldQL Product DSL

MoldQL should be small, composable, and safe.

Example:

```text
inspect module "billing"
with connascence strength >= position
where locality > module
show hotspots
render mermaid
explain evidence
```

More examples:

```text
map repo with c4 containers render plantuml
```

```text
inspect function "PaymentService.charge"
trace callers depth 3
show impact
```

```text
review pr current
with impact, tests, solid
render html
```

```text
find modules where cycles > 0
show dependency graph
explain risks
```

### Layer 4: Lens Engine

Lenses transform evidence into judgments or derived facts.

Examples:

- Connascence lens: strength, degree, locality.
- SOLID lens: SRP/OCP/LSP/ISP/DIP heuristics.
- C4 lens: system/container/component/code zoom.
- Runtime lens: order, timing, retries, errors.
- Domain lens: vocabulary, duplicated concepts, leaked infrastructure terms.
- Testing lens: coverage links, missing tests, blast radius.

### Layer 5: View Pipeline

Views consume query/lens results and produce artifacts.

Outputs:

- Interactive graph JSON.
- Mermaid.
- PlantUML.
- C4.
- HTML report.
- SVG/PNG.
- Audio summary.
- Agent evidence pack.

## MCP Integration

Expose both low-level and high-level tools.

### Low-Level MCP Tools

- `query_graph(cypher)`
- `get_node(id)`
- `get_neighbors(id, direction, depth)`
- `search_symbols(query)`
- `get_evidence(ids)`

### MoldQL MCP Tools

- `run_moldql(query)`
- `explain_moldql(query)`
- `suggest_queries(scope)`
- `inspect_scope(scope_id, lens)`
- `generate_view(scope_id, lens, format)`

### Agent-Facing Prompts

- `understand_scope`
- `review_change`
- `find_design_risks`
- `generate_architecture_report`
- `explain_visualization`

## How Moldable Development Influences the Language

Glamorous Toolkit's most relevant idea is not its syntax. The relevant idea is contextuality.

MoldQL should be context-aware:

```text
from current module show callers
from selected class show reasons-to-change
from current PR show blast-radius
from this hotspot suggest next views
```

That means the query runner receives a context object:

```json
{
  "current_scope": "module:billing",
  "selected_nodes": ["function:PaymentService.charge"],
  "active_lens": "connascence",
  "workspace_root": "/repo",
  "user_goal": "understand risk before refactor"
}
```

This context lets the same query mean the right thing inside the UI, IDE, report, or agent session.

## Product UX Principles

- Default to suggested queries, not a blank query editor.
- Every scope page should show contextual search, contextual views, and contextual actions.
- Keep raw Cypher available but behind an advanced mode.
- Make queries explainable: show the generated backend query and evidence sources.
- Let agents propose queries, but require evidence-backed outputs.
- Let users save useful queries as reusable views.
- Treat reports as snapshots of a query plus evidence, not manually written documents.

## Rust Implementation Strategy

### MVP

- Rust service with MCP server.
- Kuzu backend through Rust bindings, or a small graph abstraction trait if backend uncertainty remains.
- MoldQL parser using `chumsky`, `pest`, or `nom`.
- Query planner that compiles MoldQL into Cypher plus lens execution steps.
- View generators for Mermaid and HTML first.

### Core Rust Traits

```rust
trait GraphStore {
    fn query(&self, query: GraphQuery) -> Result<QueryResult, GraphError>;
    fn get_node(&self, id: NodeId) -> Result<Node, GraphError>;
    fn neighbors(&self, id: NodeId, spec: TraversalSpec) -> Result<Subgraph, GraphError>;
}

trait Lens {
    fn id(&self) -> LensId;
    fn apply(&self, input: EvidenceSet, context: QueryContext) -> Result<LensResult, LensError>;
}

trait ViewRenderer {
    fn format(&self) -> ViewFormat;
    fn render(&self, result: LensResult, context: QueryContext) -> Result<Artifact, RenderError>;
}
```

Use enums for closed MVP concepts, traits for plugin extension points.

## Recommendation

Build MoldQL, but only as a product DSL over existing engines.

Do not build a general graph database or general graph query language first.

Start with:

```text
Kuzu/Cypher backend + Rust MCP server + MoldQL compiler + lens/view plugins
```

Kuzu/Cypher is an implementation choice, not a product dependency. The stable product contract is `GraphStore` plus a small internal graph query IR. If a Rust-native graph stack becomes mature enough later, the migration should replace only the backend adapter and possibly the IR-to-backend compiler, not MoldQL, lenses, views, MCP tools, saved reports, or UI workflows.

## Backend Portability Requirement

The architecture must support a future pivot from Kuzu/Cypher to a full Rust-native stack.

Non-negotiable boundaries:

- MoldQL compiles to an internal query IR, not directly to Cypher.
- Kuzu-specific Cypher lives only in the Kuzu adapter.
- Nodes, edges, paths, subgraphs, query results, and graph schema are represented with product-owned Rust types.
- Lenses consume `EvidenceSet` and `Subgraph`, not database-specific rows.
- View renderers consume `LensResult`, not Cypher records.
- MCP tools expose product concepts: scopes, lenses, evidence, views, decisions.
- Raw Cypher is allowed only in advanced/debug tooling.

Suggested adapter shape:

```rust
trait GraphStore {
    fn capabilities(&self) -> GraphCapabilities;
    fn schema(&self) -> Result<GraphSchema, GraphError>;
    fn execute(&self, query: GraphQueryIr) -> Result<QueryResult, GraphError>;
    fn get_node(&self, id: NodeId) -> Result<Node, GraphError>;
    fn get_subgraph(&self, seed: NodeId, spec: TraversalSpec) -> Result<Subgraph, GraphError>;
}

trait GraphCompiler {
    fn compile(&self, query: GraphQueryIr) -> Result<BackendQuery, CompileError>;
}
```

Backend adapters:

- `KuzuGraphStore`: MVP adapter compiling IR to Cypher.
- `RustNativeGraphStore`: future adapter over a Rust graph engine.
- `InMemoryGraphStore`: tests, demos, and deterministic examples.
- `GraphifyJsonStore`: optional adapter for static `graphify-out/graph.json` exploration.

Migration test:

```text
The same MoldQL query must return equivalent EvidenceSet output on KuzuGraphStore and InMemoryGraphStore.
```

This prevents backend leakage early.

## First Vertical Slice

```text
Query:
inspect module "billing" with connascence show hotspots render html

System:
MoldQL -> Cypher -> connascence lens -> HTML/Mermaid view -> agent explanation -> saved report
```

This proves the complete product value: fast exploration, graph-backed evidence, contextual visualization, AI enrichment, and reusable output.
