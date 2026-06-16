# ADR-023: Bulk Load Strategy + Advisory Locks + Error Isolation

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** Critical review — performance + robustness

## Context

Three operational concerns that affect the pipeline's production readiness:

1. **First scan** of a large project (1000+ files) needs maximum throughput.
   Individual INSERTs in a transaction are too slow.
2. **Concurrent scans** of the same workspace must be prevented — they would
   corrupt the graph with interleaved DELETE+INSERT.
3. **Per-file extraction failures** (syntax errors, encoding issues, unsupported
   grammar) must not abort the entire scan.

## Decision

### 1. COPY for bulk initial load

For first scan (or `--force` rebuild), use `sqlx` binary COPY protocol:

```rust
// Phase 1: Extract all files in parallel (rayon)
let all_results: Vec<ExtractionResult> = changed_files.par_iter()
    .map(|f| extract_safe(&config, f))
    .collect();

// Phase 2: COPY into PG
let mut tx = pool.begin().await?;
// COPY graph_nodes FROM STDIN BINARY
let mut copy = tx.copy_in(
    "COPY graph_nodes (id, kind, label, source_path, properties, workspace_id) \
     FROM STDIN BINARY"
).await?;
for node in all_nodes {
    copy.send(&binary_encode_node(&node)?).await?;
}
copy.finish().await?;
// Same for graph_edges
tx.commit().await?;
```

For incremental scans (1-10 changed files), use per-file transactional
DELETE+INSERT (ADR-017). Individual INSERTs are fast enough for small batches.

**Decision rule:**
- `changed_files.len() > 50` → COPY path
- `changed_files.len() <= 50` → transactional per-file path

### 2. Advisory lock for exclusive scan

```rust
// At scan job start:
sqlx::query("SELECT pg_advisory_lock(hashtext($1))")
    .bind(&workspace_id)
    .execute(&pool).await?;

// ... pipeline runs ...

// At scan job end (or on drop):
sqlx::query("SELECT pg_advisory_unlock(hashtext($1))")
    .bind(&workspace_id)
    .execute(&pool).await?;
```

If another scan for the same workspace is requested while the lock is held,
the API returns `409 Conflict { "error": "scan_already_running", "job_id": "..." }`.

The lock is session-scoped — if the process crashes, the lock is released
when the PG connection drops. No stale locks.

### 3. Error isolation per-file

```rust
fn extract_safe(config: &LanguageConfig, path: &Path) -> ExtractionResult {
    match extract(config, path) {
        Ok(result) => result,
        Err(e) => {
            tracing::warn!(
                file = %path.display(),
                error = %e,
                "extraction failed — skipping"
            );
            ExtractionResult::failed(path, e.to_string())
        }
    }
}
```

Failed files are recorded in `scan_manifest` with `status = 'error'` and
`error_msg = e.to_string()`. The scan continues. The job result includes
a `failed_files` summary:

```json
{
    "status": "completed",
    "symbols": 3421,
    "edges": 8723,
    "duration_ms": 12453,
    "failed_files": [
        { "path": "src/broken.rs", "error": "SyntaxError at line 42" }
    ]
}
```

## Rationale

- **COPY** is PostgreSQL's fastest ingest path — 10-100x faster than INSERTs
  for large batches. `sqlx::CopyIn` supports binary protocol.
- **Advisory locks** are PG-native, session-scoped, and crash-safe. No
  application-level lock manager needed.
- **Error isolation** follows Graphify's `_safe_extract()` pattern. A single
  broken file never blocks the scan.

## Consequences

- COPY binary protocol requires manual type encoding (TEXT, JSONB, TIMESTAMPTZ).
  `sqlx` provides `encode::Encode` for each type — the binary format is
  straightforward but must be tested.
- The 50-file threshold is heuristic. It can be tuned via config.
- Advisory locks consume one PG connection for the lock holder. The pool
  must account for this.
- Error isolation means the graph may be incomplete (some files skipped).
  The `failed_files` summary makes this visible to the user.

## Alternatives Considered

- **Always COPY:** rejected — COPY has setup overhead. For 1-10 files,
  transactional INSERT is faster.
- **Application-level mutex:** rejected — doesn't survive process crashes.
  Advisory locks are PG-native and crash-safe.
- **Fail-fast on extraction errors:** rejected — one broken file would block
  the entire scan. Graphify's per-file error handling is the proven pattern.
