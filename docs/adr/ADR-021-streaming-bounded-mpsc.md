# ADR-021: Streaming Architecture — Bounded mpsc + Rayon Bridge

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** Critical review — memory pressure on large projects

## Context

The Extract stage uses `rayon::par_iter` for parallel tree-sitter parsing
(CPU-bound). The PgUpsert stage uses `sqlx` for transactional DB writes
(I/O-bound). These have different execution models:

- `rayon` runs on a dedicated thread pool (CPU cores).
- `sqlx` runs on the tokio async runtime.
- Collecting all extraction results in memory before writing risks OOM on
  large projects (10k+ files = potentially GBs of node/edge data).

## Decision

Bridge rayon and tokio with a **bounded `tokio::sync::mpsc` channel** and
**stream results** from extractor to ingester.

```rust
// Bounded channel: max 100 files in flight
let (tx, mut rx) = tokio::sync::mpsc::channel::<ExtractionResult>(100);

// rayon side: parallel extraction sends through channel
let extraction_handle = tokio::task::spawn_blocking(move || {
    changed_files.par_iter().for_each(|file| {
        let result = extract_safe(&config, file);
        // tx.blocking_send() blocks the rayon worker if the channel
        // is full → natural backpressure
        tx.blocking_send(result).ok();
    });
});

// tokio side: ingester consumes from channel
let ingest_handle = tokio::spawn(async move {
    let batch = Vec::with_capacity(10);
    while let Some(result) = rx.recv().await {
        batch.push(result);
        if batch.len() >= 10 {
            pg_upsert_batch(&pool, &batch).await?;
            batch.clear();
        }
    }
    if !batch.is_empty() {
        pg_upsert_batch(&pool, &batch).await?;
    }
});
```

### Design rules

1. **Channel capacity: 100.** Enough to keep the ingester busy without letting
   the extractor run away with memory. Each `ExtractionResult` is ~10-100KB
   (one file's nodes + edges), so 100 in flight = max ~10MB — bounded.
2. **`blocking_send` on rayon side.** Rayon workers are NOT tokio tasks. Use
   `tx.blocking_send()` which is safe to call from a synchronous context.
3. **Batched ingestion: 10 files per transaction.** Accumulate 10 results
   before committing. Reduces COMMIT (fsync) overhead by 10x.
4. **Error isolation.** `extract_safe()` catches per-file errors and sends an
   `ExtractionResult::Failed` through the channel. The ingester records the
   failure in `scan_manifest.status` and continues.

## Rationale

- **Backpressure.** A bounded channel naturally throttles extraction to match
  ingestion speed. If the DB is slow, rayon workers block on send — no OOM.
- **Overlap.** The ingester starts writing while extraction is still running.
  CPU (parsing) and I/O (DB writes) proceed in parallel.
- **Simplicity.** One channel, two tasks. No external queue, no actor framework.

## Consequences

- Total scan time = max(extract_time, ingest_time), not extract_time +
  ingest_time. For a CPU-bound project, extraction dominates. For a
  network-latency-bound DB, ingestion dominates.
- Channel capacity is a tuning parameter. 100 is conservative; can be adjusted
  via config. Too low = underutilized DB. Too high = memory pressure.
- The `spawn_blocking` + `par_iter` pattern means rayon's thread pool size
  caps extraction parallelism. Default is `num_cpus`.

## Alternatives Considered

- **Collect-then-write (unbounded):** simpler but risks OOM. For a 10k-file
  project with avg 50KB per extraction result, that's 500MB in memory.
- **Unbounded channel:** no backpressure. Same OOM risk as collect-then-write
  but spread over time.
- **External queue (Redis, RabbitMQ):** overkill. The workload is single-node,
  in-process. A tokio channel is sufficient.
