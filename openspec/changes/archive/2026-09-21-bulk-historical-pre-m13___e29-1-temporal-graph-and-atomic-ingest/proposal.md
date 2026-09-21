# Proposal: Temporal Graph History & Atomic Ingest Commit (E29.1)

## Intent
Ingest mutates the canonical graph destructively: `pg_upsert_streaming` DELETE-then-INSERTs per-file batches, so history is lost each cycle, a crash leaves a half-updated graph, and a failed extraction already erased the prior rows. E29.1 makes it atomic and time-travelable.

## Scope

### In Scope
- Append-only temporal node/edge versions (exact history), revision-pinned
- Single atomic `IngestCommit` (persist → delete → manifest → head + report outbox), no per-batch partial commits
- Explicit `delete` change records — removed files are first-class, not silent loss
- Last-known-good: a failed extraction MUST NOT erase prior rows
- Sessions and report-outbox intents link to `RevisionId`; reports compute idempotently after commit from that pin
- Retention/GC for superseded revisions (bounded history)
- Structural diff between two pinned revisions
- ONE shared `SnapshotProvider` in the runtime composition root

### Out of Scope
- MoldQL temporal syntax (E28.3+); analytics; frontend; replication

## Capabilities
> Researched `openspec/specs/`.

### New Capabilities
- `atomic-ingest-commit`: one transactional commit opens the revision, applies all adds/deletes, all-or-nothing; last-known-good survives failure.
- `temporal-graph-history`: append-only node/edge versioning with revision-pinned exact historical reads + retention/GC.
- `structural-graph-diff`: deterministic added/removed/changed node & edge diff between two pinned revisions.

### Modified Capabilities
- `graph-revisions`: lifecycle extended to retention/GC, session pins, and report-outbox linkage; graph publication is atomic.
- `graph-snapshot-refresh`: the ONE shared `SnapshotProvider` serves temporal history; cross-revision diff is now supported.
- `postgres-callgraph-persistence`: write path becomes append-only temporal; explicit deletes recorded.

## Approach
Store temporal versions in additive version tables keyed by canonical identity plus `valid_from`. `IngestCommit` opens the revision, inserts versions, expires prior versions, records explicit deletes and a report-outbox intent, then publishes the head in one transaction. An idempotent worker computes reports after commit from the pinned revision. Diff = set-difference of revision pins; GC retires unpinned revisions past the window.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `application/ingest/pg_upsert_stage.rs` | Modified | Non-destructive append; atomic commit; explicit deletes |
| `application/ingest/{controller,service}.rs` | Modified | `IngestCommit`; report/session `RevisionId` linkage |
| `infrastructure/persistence/postgres_repository.rs` | Modified | Temporal save/load; delete records; diff |
| `infrastructure/persistence/m00XX_temporal.sql` | New | `valid_from`/`valid_to`, retention, GC migration |
| `crates/cognicode-runtime` (composition root) | Modified | Single shared `SnapshotProvider` |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Schema migration on populated DB | High | Additive nullable + backfill + companion down-migration; compatibility-tested revert |
| Append-only write amplification | Med | Retention + GC; benchmark throughput Δ ≤ 5% (entropy budget) |
| Partial commit under crash | Med | Single-TX; no visible revision until COMMIT; fail-closed reads |
| Breaking destructive upsert callers | Med | Adapter; contract tests pin behavior before change |

## Rollback Plan
Single `git revert`. Temporal schema is additive (nullable columns / side-table); the companion down-migration drops them and restores destructive upsert. A reverted build re-running migrations MUST NOT alter existing rows.

## Dependencies
- E29.0 (trustworthy delivery baseline)
- E28.0 (`graph-revisions`, `SnapshotProvider`)
- ADR-014 §1 (canonical state — one workspace + revision)

## Success Criteria
- [ ] Crash mid-ingest leaves graph at last-good revision; no partial rows visible
- [ ] Two commits yield a non-empty diff; deleted files are explicit deletes
- [ ] Pinned read at revision N byte-identical regardless of later commits
- [ ] One `SnapshotProvider` shared process-wide (`Arc::ptr_eq == true`)
- [ ] `graph_reports` / sessions carry their computed `RevisionId`
- [ ] GC retires unpinned revisions past the window; throughput regresses ≤ 5%
