# ADR-019: Legacy Tables as SQL VIEWs (Replaces Dual-Write)

**Status:** Accepted (supersedes original dual-write decision)  
**Date:** 2026-06-15  
**Source:** Critical review — simplification of ADR-019 dual-write

## Context

ADR-019 originally proposed dual-writing to both `graph_nodes`/`graph_edges`
(canonical) and `symbols`/`call_edges` (legacy) in the same transaction. This
doubles write volume and creates a synchronization burden.

The critical review identified that `symbols` and `call_edges` can be **SQL
VIEWs** projected from `graph_nodes`/`graph_edges` — eliminating dual-write
entirely. The Explorer's `load_call_graph()` already reads via `SELECT ... FROM
symbols/call_edges`, so swapping tables for views requires **zero code changes**
in the Explorer.

## Decision

Replace the `symbols` and `call_edges` **tables** with **read-only VIEWs**
projected from `graph_nodes` and `graph_edges`.

```sql
DROP TABLE IF EXISTS symbols;
DROP TABLE IF EXISTS call_edges;

CREATE VIEW symbols AS
SELECT
    gn.id,
    gn.source_path AS file_path,
    gn.label AS name,
    SPLIT_PART(REPLACE(gn.kind, 'symbol.', ''), '.', 1) AS kind,
    (gn.properties->>'line')::INTEGER AS line,
    (gn.properties->>'column')::INTEGER AS "column",
    (gn.properties->>'complexity')::INTEGER AS complexity
FROM graph_nodes gn
WHERE gn.kind LIKE 'symbol.%';

CREATE VIEW call_edges AS
SELECT
    ge.source_id AS caller_id,
    src.label AS caller_name,
    ge.target_id AS callee_id,
    tgt.label AS callee_name,
    REPLACE(ge.kind, 'dependency.', '') AS dependency_type,
    ge.provenance,
    ge.confidence
FROM graph_edges ge
JOIN graph_nodes src ON src.id = ge.source_id
JOIN graph_nodes tgt ON tgt.id = ge.target_id
WHERE ge.kind LIKE 'dependency.%';
```

The pipeline writes to **only two tables**: `graph_nodes` + `graph_edges`.

## Rationale

- **Single write path.** The pipeline writes to `graph_nodes`/`graph_edges`
  only. No projection logic, no dual-write transaction overhead.
- **Zero Explorer changes.** `load_call_graph()` does `SELECT ... FROM symbols`
  — the VIEW is transparent. Same columns, same row shape.
- **Always consistent.** A VIEW is computed at query time — there is no
  eventual consistency window between canonical and projected tables.
- **Storage savings.** No duplicate data. For a 10k-symbol project, saves ~2MB.
- **Simpler pipeline.** The PgUpsert stage writes to 2 tables, not 4.

## Consequences

- `symbols` and `call_edges` are now **read-only**. Any code that INSERTs into
  them must be migrated to write to `graph_nodes`/`graph_edges` instead.
  The existing `save_call_graph()` method is the primary affected caller — it
  is replaced by the pipeline's PgUpsert stage.
- VIEW query performance depends on PG optimizer. The JOINs in `call_edges`
  may need materialized indexes. If performance degrades, a materialized view
  refreshed on scan-complete is the fallback.
- The `id SERIAL PRIMARY KEY` on `symbols` becomes `id TEXT` (from
  `graph_nodes.id`). Any code relying on integer IDs must adapt.

## Alternatives Considered

- **Dual-write (original ADR-019):** rejected — doubles writes, creates sync
  burden. The VIEW approach achieves the same Explorer compatibility with
  half the writes and no projection code.
- **Materialized views:** `REFRESH MATERIALIZED VIEW` locks during refresh.
  Regular views compute at query time with no locking.
- **Generic-only (drop legacy tables):** rejected — requires rewriting
  `load_call_graph()`, `SymbolRow` mapping, and all Explorer consumers.
  VIEWs provide the same compatibility transparently.
