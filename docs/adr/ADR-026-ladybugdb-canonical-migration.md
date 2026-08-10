# ADR-026: LadybugDB as sole canonical graph store

**Status**: EXECUTED (full PostgreSQL removal landed in e29-7-remove-postgres-repository, 2026-08-03)
**Date**: 2026-07-30
**Deciders**: User, OpenCode orchestrator

## Context

LadybugDB is a fork of Kùzu (MIT licensed, v0.19.0 on crates.io as `lbug`). It is an embedded graph database that:
- Is schema-full (NOT schema-less), each node table requires pre-defined typed columns with a mandatory primary key
- Supports `MAP<STRING,STRING>` for flexible properties (emergent/unknown schema fields)
- Supports multi-label nodes (v0.18+)
- Uses a single `READ_WRITE` Database object per `.lbdb` file; multiple concurrent `Connection` objects within the same process; multi-process requires all readers to be `READ_ONLY`
- Uses WAL + checkpoint for serializable ACID transactions, one writer per Database
- Has SERIAL primary keys, `COPY FROM Parquet/CSV`, `MERGE` with `ON MATCH SET`/`SKIP_DUPLICATE_PK`
- Ships Cypher-compatible query language, vector HNSW extension, FTS BM25, PageRank, Louvain
- Builds C++ from source via the `lbug` crate (expect 5–15 min build time)

The existing CogniCode architecture is hexagonal with ports in `domain/`:
- `Repository` (symbol read), `GraphRepository` (generic graph read + FTS), `GraphQueryPort` (call graph navigation), `GraphExecutor` (GraphPlan execution — backend-neutral), `IacRepository`, `SearchProvider`, `SourceExtractor`, `InvestigationStore`, `RunLineageStore` — all in domain/
- `GraphStore` (sync blob persistence), `Repository` (async query) — in domain/traits/
- `ViewSpecRepository` — wrongly located in `interface/mcp/handlers/mod.rs:338`, needs to move to `domain/ports/`

PostgreSQL currently owns: `graph_nodes`, `graph_edges`, `symbols`, `call_edges`, `scan_manifest`, `graph_reports`, `graph_revisions`, `spaces`, `issues`, `baselines`, `rules`, `api_routes`, `api_route_edges`, `investigations`, `investigation_evidence`, `investigation_artifacts`, `exploration_sessions`, `named_views`, `view_specs`, `analytics_run_lineage`, `descriptor_limits` — 24 tables total.

The existing ADR-014 chose PostgreSQL as canonical. ADR-019 chose temporal append-only node/edge versions. ADR-017 chose PostgreSQL-native streaming ingest. All three are superseded by this ADR.

## Decision

LadybugDB becomes the sole canonical graph store. PostgreSQL is removed as a production dependency.

### 1. All state in LadybugDB

Every table that was in PostgreSQL becomes a node table or relationship table in LadybugDB. This includes: `graph_nodes`, `graph_edges`, `symbols`, `call_edges`, `scan_manifest`, `graph_reports`, `graph_revisions`, `spaces`, `issues`, `baselines`, `rules`, `api_routes`, `api_route_edges`, `investigations`, `investigation_evidence`, `investigation_artifacts`, `exploration_sessions`, `named_views`, `view_specs`, `analytics_run_lineage`, `descriptor_limits`.

All-as-nodes: `FileRecord`, `Revision`, `Issue`, `Investigation`, `ViewSpec`, `ExplorationSession`, `AnalyticsRun`, etc. all become nodes. See ADR-027 for the full hybrid schema strategy.

### 2. Hybrid schema strategy (typed columns + MAP<STRING,STRING>)

Stable fields with known types use typed columns. Emergent or unknown properties use `MAP<STRING,STRING>`. This is NOT schema-less — every node table requires pre-defined schema. Multi-label nodes supported (LadybugDB v0.18+). See ADR-027 for the full strategy.

### 3. Temporal model preserved (valid_from / valid_to via revision pinning)

The ADR-019 temporal append-only model is preserved. LadybugDB's revision pinning (`workspace_id` + `revision_id` on every node/edge) achieves the same immutability semantics. The `IngestCommit` transaction atomically publishes: graph delta + manifest delta + revision publication + report outbox + lineage record.

### 4. GraphExecutor seam maintained

The existing `GraphExecutor` trait (backend-neutral, object-safe, `Send+Sync+'static`) remains the seam. `LadybugGraphExecutor` implements it the same way `PgGraphExecutor` and `SnapshotGraphExecutor` do. The conformance harness from E28.2 validates equivalence.

### 5. Composition root selects adapter

The composition root in `cognicode-runtime` selects between adapters via feature flag. During the migration transition both `ladybug` and `postgres` adapters exist. After migration, `postgres` feature flag and adapter are removed.

## Schema design

### Node tables (~22)

Full list: `Workspace`, `Space`, `Revision`, `FileRecord`, `SourceFile`, `Symbol`, `Decision`, `Doc`, `Evidence`, `Issue`, `Component`, `Container`, `System`, `Route`, `Rule`, `Baseline`, `Investigation`, `EvidenceItem`, `Artifact`, `ExplorationSession`, `NamedView`, `ViewSpec`, `GraphReport`, `AnalyticsRun`, `DescriptorLimits`

### Relationship tables (~20)

Full list: `Calls`, `Imports`, `Defines`, `Cites`, `Justifies`, `Resolves`, `PartOf`, `HttpCalls`, `DefinedIn`, `ScannedIn`, `HasIssue`, `PinnedIn`, `SavedAs`, `RunsOn`, `TrackedBy`, `Generates`, `BelongsTo`, `Contains`, `References`, `Annotates`

### Temporal columns

Every node and relationship table carries: `valid_from INT64` (revision_id), `valid_to INT64` (revision_id or -1 for current). This achieves ADR-019 immutability semantics.

## Alternatives considered

### Keep PostgreSQL as canonical

Rejected — relational model is not optimal for graph workloads; ORMs add impedance mismatch; LadybugDB's embedded nature eliminates network round-trips for local analysis.

### Use Neo4j as canonical

Rejected — requires separate process, network topology, licensing. LadybugDB is embedded, MIT, zero-ops for local use.

### Keep both PostgreSQL and LadybugDB

Rejected — synchronization cost, two sources of truth, doubles the verification burden. Single canonical store.

### Use SQLite with graph extension

Rejected — no mature graph extension with Cypher support matches LadybugDB's feature set (HNSW, FTS, PageRank).

## Consequences

### Positive

- Embedded, zero-ops deployment (single `.lbdb` file)
- Graph-native storage (adjacency lists, not relational joins)
- Cypher query language for graph operations
- Vector HNSW, FTS BM25, PageRank, Louvain built-in
- MIT license, no vendor lock-in
- Single writer per DB (no distributed consensus needed)
- All-as-nodes matches CogniCode's node-centric domain model

### Negative

- Migration of 24 PostgreSQL tables to LadybugDB schema
- All existing ports that currently map to PostgreSQL must map to LadybugDB Cypher
- Data migration from PostgreSQL to LadybugDB required
- `COPY FROM Parquet/CSV` is the fastest migration path
- Multi-process access requires READ_ONLY connections for all but one process
- No `LISTEN/NOTIFY` equivalent — snapshot refresh must poll or use file-system watching

### Mitigations

- Spike validation before committing to migration (S1-S6 in `ladybug-spike-validation/spec.md`)
- Port abstraction layer so migration is isolated to adapters
- Both adapters exist during transition period
- Conformance harness from E28.2 validates LadybugGraphExecutor parity

## Out of scope

- Distributed LadybugDB cluster (single-process embedded only)
- Graph mutation from MoldQL (v1 remains read-only)
- Full Cypher compatibility claim
- WASM browser deployment of LadybugDB
- Multi-writer concurrency (single writer is sufficient for single-workspace analysis)

## References

- [ADR-014](./ADR-014-moldql-pattern-graph-analytics-platform.md) (SUPERSEDED)
- [ADR-019](./ADR-019-temporal-graph-history-and-atomic-ingest.md) (SUPERSEDED)
- [ADR-017](./ADR-017-postgresql-native-ingest-pipeline.md) (SUPERSEDED)
- [ADR-027](./ADR-027-ladybugdb-hybrid-schema-strategy.md)
- [ADR-028](./ADR-028-ladybugdb-port-abstraction-architecture.md)
- [LadybugDB documentation](https://docs.ladybugdb.com)
- [lbug crate](https://crates.io/crates/lbug)
- [Graph executor port spec](../specs/graph-executor-port/spec.md)
- [Executor equivalence conformance spec](../specs/executor-equivalence-conformance/spec.md)
- [LadybugDB spike validation spec](../specs/ladybug-spike-validation/spec.md)
- [LadybugDB graph schema spec](../specs/ladybug-graph-schema/spec.md)
- [E28 roadmap](../ROADMAP.md#graph-query--analytics-platform-e28)
