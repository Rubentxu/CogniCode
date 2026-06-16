# ADR-022: PG Trigger NOTIFY + Incremental Refresh via GraphDiffCalculator

**Status:** Accepted  
**Date:** 2026-06-15  
**Source:** Critical review — notification + refresh optimization

## Context

After the PgUpsert stage writes changes to `graph_nodes`/`graph_edges`, two
things must happen:

1. **The in-memory `GraphCache` (ArcSwap) must be updated** so the Explorer
   serves fresh data. The current approach would reload the ENTIRE CallGraph
   from PG — O(N) for every scan.

2. **The Explorer frontend must be notified** that the graph changed. The
   current `GraphCache` has a `broadcast::Sender<GraphEvent>` but it only fires
   from Rust code, not from external DB changes.

## Decision

### Part 1: Incremental Refresh via GraphDiffCalculator

Use the existing `GraphDiffCalculator::calculate_diff()` + `GraphCache::
apply_events()` for incremental updates instead of full reload.

```rust
// Refresh stage:
// 1. Load only changed-file symbols from PG
let new_symbols = load_symbols_for_files(&pool, &changed_files).await?;

// 2. Load old symbols for those files from the in-memory graph
let old_symbols = graph_cache.get().symbols_for_files(&changed_files);

// 3. Calculate diff
let events = GraphDiffCalculator::calculate_diff(&old_symbols, &new_symbols);

// 4. Apply incrementally (ArcSwap swap under the hood)
graph_cache.apply_events(&events)?;
// → broadcast::send(GraphEvent::GraphModified)
```

For first scan (all files are New), this produces a full load via events —
equivalent to `GraphCache::set()` but through the event system.

For incremental scan (1-10 files changed), this is O(Δ) — only changed
symbols are cloned, diffed, and applied.

### Part 2: PostgreSQL Trigger for automatic NOTIFY

```sql
CREATE OR REPLACE FUNCTION notify_graph_change() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('graph_updated', json_build_object(
        'workspace_id', NEW.workspace_id,
        'source_path', NEW.source_path,
        'action', TG_OP
    )::text);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER graph_nodes_notify
    AFTER INSERT OR UPDATE OR DELETE ON graph_nodes
    FOR EACH ROW EXECUTE FUNCTION notify_graph_change();
```

The Explorer backend runs a `LISTEN graph_updated` listener. On notification,
it invalidates its SWR cache and pushes an SSE event to the frontend.

```rust
// Explorer backend startup
let mut listener = pool.acquire().await?;
listener.execute("LISTEN graph_updated").await?;
tokio::spawn(async move {
    loop {
        let notification = listener.recv().await?;
        // Push to SSE clients
        sse_broadcaster.send(GraphUpdateNotification {
            workspace_id: parse_workspace(&notification),
            ..
        });
    }
});
```

## Rationale

- **O(Δ) refresh.** Incremental scans update only changed symbols in the
  ArcSwap cache. No full reload from PG.
- **`GraphDiffCalculator` already exists** with full test coverage
  (`graph_event.rs:104`). Reuse, don't rebuild.
- **PG trigger decouples notification.** Any DB change — from the pipeline,
  from a manual SQL UPDATE, from a future file watcher — triggers the
  notification. No need to remember to call NOTIFY in every Rust code path.
- **Explorer SSE** is already wired for `GraphEvent` via `broadcast::Sender`.
  The LISTEN → SSE bridge is a thin adapter.

## Consequences

- `GraphDiffCalculator` currently handles symbol-level diffs only (not edge
  diffs). The `DependencyAdded`/`DependencyRemoved` events exist but are
  marked "for future use" in the code. **Edge diffing must be implemented**
  for the full incremental refresh to work.
- The PG trigger fires per-row. A batch upsert of 1000 nodes fires 1000
  NOTIFY events. Mitigation: debounce in the Explorer listener (ignore
  duplicates within 500ms).
- The `LISTEN` connection occupies one pool slot permanently. The pool must
  be sized accordingly (already max 8, sufficient).

## Alternatives Considered

- **Full reload after every scan:** O(N) even for 1-file change. Unacceptable
  for iterative development. 1-2s latency on a 10k-symbol graph.
- **Explicit NOTIFY in Rust code:** fragile — any code path that modifies
  `graph_nodes` must remember to NOTIFY. A trigger is fail-safe.
- **Polling:** Explorer polls `GET /api/workspaces/:id/status` every N seconds.
  Rejected — latency and wasted requests.
