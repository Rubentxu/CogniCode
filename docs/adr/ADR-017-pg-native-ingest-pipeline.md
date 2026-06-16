# ADR-017: PostgreSQL-Native Ingest Pipeline

**Status:** Accepted (revised)  
**Date:** 2026-06-15  
**Source:** grill-with-docs session — Graphify alignment + critical review

## Context

CogniCode needs an ingest pipeline that scans source files, extracts structural
information, and persists it as a queryable graph. Graphify uses file-based
persistence (`graph.json`, `manifest.json`, cache dir). CogniCode's stack is
Rust + PostgreSQL + rayon + ArcSwap.

The existing `save_call_graph()` method (`postgres_repository.rs:214`) does a
full `DELETE FROM call_edges; DELETE FROM symbols;` then re-inserts everything.
This global delete-and-replace is incompatible with incremental updates.

## Decision

PostgreSQL is the **single persistence layer**. No intermediate files. The
pipeline operates **per-file transactionally**, not globally.

### Pipeline: 9 stages

```
Scan → Extract → PgUpsert → Resolve → Cluster → Analyze → Report → Refresh → Notify
```

### Scan: mtime-first, hash-second

Two-phase change detection to avoid hashing every file on every scan:

1. `stat()` each file (rayon parallel) — near-free
2. `SELECT file_path, mtime, content_hash FROM scan_manifest WHERE ...`
3. If `mtime` unchanged → **Unchanged** (skip hash + skip extract)
4. If `mtime` changed → SHA256 hash, compare with stored hash
5. If hash unchanged → **Unchanged** (content identical, mtime was touched)
6. If hash changed → **Changed**

This avoids hashing 90%+ of files on incremental scans.

### Extract: rayon parallel, streaming output

`rayon::par_iter` over Changed|New files. Each file parsed via tree-sitter using
`LanguageConfig`. Results stream through a **bounded** `tokio::sync::mpsc`
channel (capacity 100) to the ingester — CPU-bound parsing and I/O-bound DB
writes overlap. Backpressure prevents OOM on large projects.

### PgUpsert: per-file transactional

Replaces the global `DELETE ALL + re-INSERT` with **per-file transactions**:

```sql
BEGIN;
-- Delete only nodes/edges from THIS file
DELETE FROM graph_edges WHERE source_id IN
    (SELECT id FROM graph_nodes WHERE source_path = $1);
DELETE FROM graph_nodes WHERE source_path = $1;
-- Insert new
INSERT INTO graph_nodes ... ON CONFLICT (id) DO UPDATE SET ...;
INSERT INTO graph_edges ... ON CONFLICT (source_id, target_id, kind) DO UPDATE SET ...;
-- Update manifest
INSERT INTO scan_manifest ... ON CONFLICT (file_path) DO UPDATE SET ...;
COMMIT;
```

Batched 10 files per transaction to reduce COMMIT (fsync) overhead.

### Bulk initial load: COPY

First scan (1000+ files) uses `sqlx COPY FROM STDIN BINARY` — 10-100x faster
than individual INSERTs. Incremental scans (1-10 files) use transactional
DELETE+INSERT (fast enough for small batches).

### Refresh: incremental via GraphDiffCalculator

Instead of reloading the ENTIRE `CallGraph` from PG, use the existing
`GraphDiffCalculator::calculate_diff()` to emit `GraphEvent`s for just the
changed files, then `GraphCache::apply_events()`. This turns a O(N) reload
into O(Δ) update. For first scan (everything new), equivalent to full load.

### Notify: PG trigger

`CREATE TRIGGER` on `graph_nodes` fires `NOTIFY graph_updated` automatically.
The Explorer's SSE listener picks it up. No explicit NOTIFY in Rust code.

## Rationale

- **Per-file upsert** fixes the global delete-and-replace problem. Only changed
  files touch the DB.
- **mtime-first** avoids the SHA256 cost for unchanged files.
- **Bounded mpsc** prevents memory blowup on large repos. Natural backpressure.
- **COPY** makes first scan of large projects viable (<15s for 1000 files).
- **GraphDiffCalculator** makes incremental Refresh O(Δ) instead of O(N).
- **PG trigger** decouples notification from Rust code.

## Consequences

- Requires running PostgreSQL (already the case for Explorer).
- Per-file transactions need `workspace_id` filtering (ADR-020).
- COPY binary protocol needs careful type encoding (sqlx supports it).
- GraphDiffCalculator only handles symbol-level diffs; edge diffs are future work.

## Alternatives Considered

- **Global save_call_graph (current):** rejected — destroys entire graph on
  every scan. Incompatible with incremental.
- **File-based manifest (Graphify style):** rejected — duplicates state between
  files and PG. PG is the sole persistence backend (CONTEXT.md Composition Root).
- **Materialized views for Refresh:** rejected — `REFRESH MATERIALIZED VIEW`
  locks tables during refresh, blocking Explorer reads.
