# ADR-019: Dual-Write Projection (GenericGraph → CallGraph)

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** grill-with-docs session — Graphify alignment

## Context

CogniCode has two parallel graph models:

- **CallGraph** — code-only (`Symbol` + edges with `DependencyType`). Stored
  in `symbols` + `call_edges` tables. Consumed by the Explorer, MCP tools, and
  CLI today.
- **GenericGraph** — multimodal (`GraphNode` with `NodeKind` + `GraphEdge` with
  `EdgeKind`). Stored in `graph_nodes` + `graph_edges` tables. Defined but
  not yet populated by any extractor.

The ingest pipeline (ADR-017) produces `GraphNode` + `GraphEdge` as its
canonical output. The Explorer and MCP consumers expect `CallGraph`. The
question is: which table set does the pipeline write to?

## Decision

The pipeline writes to **both table sets in a single transactional pass**
(dual-write projection).

1. **Canonical write:** `graph_nodes` + `graph_edges` (the generic tables).
2. **Projection write:** `symbols` + `call_edges` (the legacy tables),
   projected from the generic nodes/edges where `NodeKind = Symbol(...)`.

Both writes happen in the same per-file transaction. The projection is a
straightforward mapping:
- `GraphNode { kind: Symbol(SymbolKind::Function), ... }` → `symbols` row
- `GraphEdge { kind: Dependency(Calls), ... }` → `call_edges` row

## Rationale

- **No Explorer rewrite.** The Explorer loads from `symbols` + `call_edges`
  today. Dual-write means it continues working without any code changes.
- **Multimodal-ready.** The generic tables are populated from day one. When
  doc/ADR extractors arrive (Phase 2), they write to `graph_nodes/edges`
  and the generic tables are already warm.
- **Transactional consistency.** Both table sets are always in sync because
  they're written in the same transaction. No eventual consistency window.
- **Simple projection.** The mapping from `GraphNode(Symbol(_))` to `Symbol`
  row is a trivial field copy — no complex transformation logic.

## Consequences

- Storage overhead: each code symbol/edge is stored twice (generic + legacy).
  For a 10k-symbol project, this is ~2MB extra — negligible.
- The projection logic must stay in sync with any schema changes to either
  table set. This is a maintenance cost, but contained to the `PgUpsert`
  stage.
- Future deprecation: when the Explorer migrates to read from `graph_nodes`/
  `graph_edges` directly, the dual-write can be dropped and the legacy tables
  archived. This is a non-breaking future change.

## Alternatives Considered

- **Generic-only:** write only to `graph_nodes`/`graph_edges`. Explorer
  migrates to read from generic tables. Rejected — requires rewriting the
  Explorer's `postgres_bridge`, all `CallGraphRepository` consumers, and the
  `SymbolRepository` port. Too much scope for v1.
- **Legacy-only:** write only to `symbols`/`call_edges`. Generic tables stay
  empty. Rejected — loses the multimodal-ready property and requires a
  migration later when doc extractors arrive.
- **Materialized view:** `CREATE MATERIALIZED VIEW symbols AS SELECT ... FROM
  graph_nodes WHERE kind LIKE 'symbol.%'`. Rejected — `REFRESH MATERIALIZED
  VIEW` locks the table during refresh, blocking Explorer reads during scan.
