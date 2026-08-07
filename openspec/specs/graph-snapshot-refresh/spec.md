# graph-snapshot-refresh Specification (NEW)

## Purpose

Tie snapshot lifecycle to a graph revision. Repositories consume a
`SnapshotProvider` instead of a fixed `Arc<CallGraph>`. Changes to
`graph_nodes` OR `graph_edges` trigger a fresh snapshot; a subsequent
in-process query observes the new snapshot without a process restart.

## Requirements

### Requirement: SnapshotProvider port

The system MUST expose a `SnapshotProvider` port:

```text
trait SnapshotProvider: Send + Sync {
    fn snapshot(&self, ws: &WorkspaceId, rev: RevisionId)
        -> Result<Arc<CallGraph>, SnapshotError>;
    fn current_head(&self, ws: &WorkspaceId)
        -> Result<RevisionId, SnapshotError>;
    fn subscribe(&self) -> broadcast::Receiver<SnapshotEvent>;
}
```

`SnapshotError::UnknownRevision { workspace, revision }` is returned
when the pair does not exist. The provider MUST NOT panic or fall back.

#### Scenario: Snapshot for pinned revision

- GIVEN `ws1` head=`revision_id = 5`
- WHEN `snapshot(ws1, 5)` is called
- THEN the result is `Ok(Arc<CallGraph>)` reflecting rows at that revision

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=5
- WHEN `snapshot(ws1, 99)` is called
- THEN the result is `Err(UnknownRevision { workspace: "ws1", revision: 99 })`

#### Scenario: current_head returns the live head

- GIVEN two commits advancing head from 5 to 7
- WHEN `current_head("ws1")` is called
- THEN the result is `Ok(RevisionId(7))`

### Requirement: Provider is constructed once and shared

The composition root MUST construct a single `SnapshotProvider` per
process. Every service that today holds a fixed `Arc<CallGraph>` MUST
instead hold `Arc<dyn SnapshotProvider>`.

#### Scenario: One provider instance per process

- GIVEN two services needing a snapshot handle
- WHEN the composition root wires them
- THEN both receive clones of the SAME provider
- AND `Arc::ptr_eq` between them is `true`

### Requirement: Edge changes trigger snapshot refresh

`notify_graph_change` MUST fire on `INSERT/UPDATE/DELETE` against
`graph_edges` AS WELL AS `graph_nodes`. Payload MUST include
`{ workspace_id, table: 'nodes'|'edges', action, revision_id }`. The
listener MUST debounce/batch per workspace.

#### Scenario: Edge insert fires notification

- GIVEN an open listener
- WHEN `INSERT INTO graph_edges …` runs in `ws1`
- THEN a `SnapshotEvent { workspace: "ws1", table: "edges", revision_id: <new> }`
  is delivered within the batch window

#### Scenario: Notification payload is batched

- GIVEN 50 sequential edge inserts in the same workspace within 100 ms
- WHEN the listener drains
- THEN at least one notification carrying the final revision id is seen
- AND NOT 50 individual events

### Requirement: Post-ingest queries observe the new snapshot

A read issued AFTER an ingest commit completes MUST observe the new
revision-pinned snapshot, without process restart, and MUST NOT observe
a partial/intermediate state.

#### Scenario: Sequential commit and read

- GIVEN `ws1` head=5
- WHEN `ingest_commit()` returns OK and then
  `snapshot(ws1, current_head("ws1"))` runs
- THEN the result is `Ok` with rows reflecting the just-committed state

#### Scenario: Pinned read survives concurrent ingest

- GIVEN a reader pinned to `(ws1, 5)`
- WHEN a concurrent ingest advances head to 6
- THEN the pinned reader's `snapshot(ws1, 5)` continues to return the
  revision-5 snapshot until it releases the pin

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Listener task dies after notification | New subscriber gets next event; no replay |
| Provider dropped while read in flight | Read returns `SnapshotError::Unavailable`; no UB |
| Multiple workspaces in same DB | Independent heads; listener demuxes by `workspace_id` |
| `graph_reports` writes (m0010) | MUST NOT trigger `SnapshotEvent` |

## Out of Scope

Cross-revision diff / time-travel; persistent event log; replication.

## Dependencies

`graph-revisions` (same change); ADR-022; ADR-035.