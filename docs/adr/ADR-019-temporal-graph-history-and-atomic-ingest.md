# ADR-019: Temporal graph history and atomic ingest commits

**Status**: SUPERSEDED (promoted 2026-08-10)
**Date**: 2026-07-29
**Deciders**: User, OpenCode orchestrator

## Context

CogniCode assigns graph revisions, but the current ingest write path still
destructively replaces rows in per-file or per-batch transactions. Revision
metadata therefore does not prove that an exact historical graph remains
queryable. A crash can expose a partially updated workspace, and failed
extraction can remove the last-known-good representation of a file.

Longitudinal code intelligence requires stronger semantics. Saved exploration,
reports, structural diffs, MoldQL runs, and analytics must refer to an immutable
workspace revision whose node and edge state can be reproduced later.

## Decision

PostgreSQL will store append-only temporal node and edge versions. One
workspace-scoped `IngestCommit` will own the transaction that publishes a graph
revision.

### 1. Historical state is exact

Every canonical node and edge version must declare the revision interval in
which it is valid. The physical representation may use validity ranges or
equivalent version tables, but a pinned read must reconstruct the exact graph at
that revision without consulting mutable process state.

### 2. One transaction publishes one revision

`IngestCommit` is the sole transaction owner. It applies new and changed facts,
explicit file deletions, manifest changes, a durable report-outbox intent, and
head publication atomically. Subordinate persistence operations use the owner
transaction and must not commit independently.

A revision becomes visible only after the transaction commits. A failed scan or
extraction leaves the workspace head and last-known-good graph unchanged.

### 3. Derived artifacts remain revision-pinned

Graph reports, exploration sessions, structural diffs, MoldQL executions, and
analytics runs record the `RevisionId` from which they were computed. Reports
are generated idempotently after commit from the published pin; report failure
updates the outbox status but does not roll back or invalidate the graph
revision. A structural diff compares two immutable pins and returns
deterministic added, removed, and changed node and edge sets.

### 4. Retention is pin-aware

Retention may retire superseded revisions after a configured window, but it
must preserve revisions pinned by retained reports, sessions, evidence, or
other durable artifacts. Garbage collection must never alter the current head.

## Alternatives considered

### Mutable head tables plus revision metadata

Rejected. Metadata alone cannot reproduce historical graph state.

### Per-file or per-batch canonical commits

Rejected. They expose mixed revisions and cannot provide all-or-nothing ingest.

### Process-local snapshots as historical truth

Rejected. Snapshots are derived execution projections and do not survive all
process, deployment, or retention boundaries.

### Full serialized graph copies as the only history model

Rejected. They simplify reads but impose excessive storage and diff costs.
Checkpoint snapshots may complement, but not replace, temporal canonical rows.

## Consequences

### Positive

- Enables truthful time travel and deterministic structural diffs.
- Prevents partial graph publication and last-known-good data loss.
- Gives reports, sessions, queries, and analytics reproducible provenance.

### Negative

- Adds migration risk, write amplification, and retention complexity.
- Requires temporal uniqueness and referential-integrity rules.
- Makes ingest orchestration responsible for one larger transaction boundary.

### Mitigations

- Use additive migration and backfill before switching reads.
- Gate publication with crash, rollback, and populated-database tests.
- Benchmark write amplification and apply pin-aware retention.

## References

- [E29.1 proposal](../../openspec/changes/e29-1-temporal-graph-and-atomic-ingest/proposal.md)
- [E29.0 proposal](../../openspec/changes/e29-0-trustworthy-delivery-baseline/proposal.md)
- [ADR-014](./ADR-014-moldql-pattern-graph-analytics-platform.md)
- [ADR-017](./ADR-017-postgresql-native-ingest-pipeline.md)
- Local SDD evidence: `plans/cognicode-capability-readiness-assessment-2026-07-29.md` (not version-controlled by policy)

## Implementation Log

- **2026-08-10 (E31-C)**: Temporal append-only model preserved by ADR-026 (LadybugDB revision pinning). Atomic ingest commits implemented as IngestCommitPort per ADR-028. Renumbered to ADR-019 to resolve numbering conflict with ADR-015-e28-6-admission-decisions.
