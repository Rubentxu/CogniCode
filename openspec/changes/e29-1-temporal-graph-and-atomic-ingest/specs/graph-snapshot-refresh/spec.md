# Delta for graph-snapshot-refresh

> E29.1 makes historical revision snapshots and structural diff consumers use
> the process-wide provider already defined by this capability.

## ADDED Requirements

### Requirement: Cross-Revision Historical Snapshots

`SnapshotProvider::snapshot(workspace, revision)` MUST return rows valid at the
pinned revision whether or not it is the current head.

#### Scenario: Old revision excludes newer rows

- GIVEN `ws1` head=7, with five nodes at revision 5 and eight at revision 7
- WHEN `snapshot(ws1, 5)` runs
- THEN it returns the five-node state from revision 5

#### Scenario: Pinned read survives concurrent commits

- GIVEN a reader pinned to revision 5
- WHEN a concurrent commit publishes revision 6
- THEN another revision-5 snapshot contains no revision-6 rows

### Requirement: Temporal-History Read Path

Historical reads MUST use revision-valid temporal ranges and MUST reject
`RevisionId::NONE` rather than falling back to the head.

#### Scenario: NONE pin is rejected

- GIVEN any workspace
- WHEN `snapshot(ws, RevisionId::NONE)` runs
- THEN it returns `Err(InvalidPin)` with no head fallback

## MODIFIED Requirements

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

`SnapshotError::UnknownRevision { workspace, revision }` MUST be returned when
the pair does not exist. `SnapshotError::InvalidPin` MUST be returned for
`RevisionId::NONE`. The provider MUST serve current and historical temporal
ranges without panic or fallback.
(Previously: the port served pinned snapshots but did not define historical
range semantics or reject `RevisionId::NONE`.)

#### Scenario: Snapshot for pinned revision

- GIVEN `ws1` head=`revision_id = 5`
- WHEN `snapshot(ws1, 5)` is called
- THEN it returns an `Arc<CallGraph>` reflecting revision 5

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=5
- WHEN `snapshot(ws1, 99)` is called
- THEN it returns `Err(UnknownRevision { workspace: "ws1", revision: 99 })`

#### Scenario: current_head returns the live head

- GIVEN two commits advance head from 5 to 7
- WHEN `current_head("ws1")` is called
- THEN it returns `RevisionId(7)`

### Requirement: Provider is constructed once and shared

The composition root MUST construct a single `SnapshotProvider` per process.
Every service that previously held a fixed `Arc<CallGraph>` MUST instead hold
`Arc<dyn SnapshotProvider>`. The same provider MUST serve live snapshots,
historical pins, and structural-diff consumers; the runtime MUST NOT construct
a second temporal or diff provider.
(Previously: one provider was shared for snapshot access; now that same instance
also serves temporal history and structural diff.)

#### Scenario: One provider instance per process

- GIVEN two services needing a snapshot handle
- WHEN the composition root wires them
- THEN both receive clones of the same provider
- AND `Arc::ptr_eq` between them is `true`

#### Scenario: Historical and diff consumers share the provider

- GIVEN a live query, a historical query, and a structural diff
- WHEN the composition root wires their snapshot dependencies
- THEN all three receive clones of the same provider instance
