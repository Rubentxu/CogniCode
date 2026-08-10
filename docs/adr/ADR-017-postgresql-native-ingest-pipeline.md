# ADR-017: PostgreSQL-native ingest pipeline

**Status**: ACCEPTED
**Date**: 2026-06-15
**Deciders**: User, grill-with-docs session, OpenCode orchestrator

## Context

CogniCode needs one ingest pipeline that scans a workspace, extracts structural
facts, and publishes a queryable graph. The original ADR-017 selected
PostgreSQL-native streaming instead of Graphify-style intermediate files. The
record was removed during broad documentation cleanup, while `CONTEXT.md` and
the implementation continued to depend on its decision.

The original decision also prescribed per-file canonical transactions. The
longitudinal readiness assessment showed that this granularity exposes partial
graphs and cannot preserve exact history. ADR-019 now owns publication
atomicity and temporal semantics.

## Decision

PostgreSQL remains the sole persistence layer for ingest state, canonical graph
facts, scan manifests, graph reports, and publication metadata. The pipeline
does not use JSON manifests or intermediate graph files.

### 1. Pipeline stages remain streaming and bounded

```text
Scan -> Extract -> PgUpsert -> Resolve -> Cluster -> Analyze -> Report
     -> Refresh -> Notify
```

CPU-bound extraction and I/O-bound persistence communicate through bounded
channels. Backpressure must not deadlock when the result count exceeds channel
capacity, and intermediate collections must remain resource-governed.

### 2. `scan_manifest` is canonical change-detection state

`scan_manifest` stores workspace, file path, SHA-256 content hash, language, and
scan timestamp. Content hash is the canonical extraction invalidation key.
Filesystem metadata such as modification time may skip unnecessary hashing when
validated safely, but it is only an optimization and cannot replace content
identity.

### 3. PostgreSQL-native writes preserve workspace isolation

All manifest, node, edge, report, and revision operations are scoped by
workspace. Conflict targets and notification triggers must match the actual
workspace-scoped schema. Initial-load optimizations such as PostgreSQL `COPY`
are permitted when they preserve the same contracts.

### 4. Publication atomicity follows ADR-019

The original per-file transaction rule is superseded. Subordinate upsert stages
may batch work for throughput, but one workspace-scoped `IngestCommit` owns the
canonical publication transaction. Failed extraction or persistence must not
erase last-known-good graph state or publish a partial revision.

### 5. Refresh and notification are derived from committed state

Snapshot refresh, idempotent report generation, and graph-update notifications
run from the committed revision. The publication transaction records a durable
outbox intent; post-commit workers consume that intent and never announce or
cache state that remains uncommitted.

## Alternatives considered

### File-based manifests and graph snapshots

Rejected. They duplicate authority and create recovery races with PostgreSQL.

### Epoch-only source cache

Rejected. Epochs do not prove content identity across restarts or environments.

### Global delete and reinsert

Rejected. It destroys unaffected history and amplifies failure impact.

### A second ingest persistence backend

Rejected. It conflicts with the composition-root and PostgreSQL-canonicality
decisions.

## Consequences

### Positive

- Ingest state and graph publication share one transactional system.
- Change detection survives restarts and remains workspace-scoped.
- Operational tooling needs to observe only PostgreSQL and the runtime pipeline.

### Negative

- PostgreSQL is mandatory for full ingest operation and integration testing.
- Schema, conflict targets, triggers, and Rust writers are tightly coordinated.
- Streaming stages require explicit backpressure and cancellation semantics.

### Mitigations

- Run fresh and populated database migration tests in CI.
- Fail PostgreSQL tests loudly on migration errors.
- Apply E29.0 before temporal migration and E29.1 publication changes.

## Amendment history

- **2026-06-15**: Original decision accepted PostgreSQL-native ingest,
  `scan_manifest`, bounded streaming, and per-file transactions.
- **2026-07-29**: Record restored. Content hash clarified as canonical;
  per-file publication atomicity superseded by ADR-019.

## References

- Historical source commit `6886b9f9`
- [E29.0 proposal](../../openspec/changes/e29-0-trustworthy-delivery-baseline/proposal.md)
- [E29.1 proposal](../../openspec/changes/e29-1-temporal-graph-and-atomic-ingest/proposal.md)
- [ADR-019](./ADR-019-temporal-graph-history-and-atomic-ingest.md)
- [`CONTEXT.md` ingest vocabulary](../../CONTEXT.md#ingest-pipeline)
