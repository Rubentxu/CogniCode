# atomic-ingest-commit Specification

## Purpose

A workspace-scoped `IngestCommit` owns one transaction that publishes temporal
graph state, manifest changes, revision pins, and a report-outbox intent.
Idempotent report generation runs after commit from that published revision.
Failed commits preserve the last-known-good revision.

## ADDED Requirements

### Requirement: Single-Transaction Commit Boundary

`IngestCommit::execute` MUST be the sole owner of exactly one transaction for
one `WorkspaceId`. It MUST allocate the revision, invoke subordinate graph
writes with that transaction and revision, apply explicit deletes, update the
manifest and revision pins, and commit once. Subordinate `save_call_graph`
operations MUST use the owner transaction and MUST NOT begin, commit, or roll
back independently.

#### Scenario: Full commit succeeds atomically

- GIVEN `ws1` head=5, graph additions, and three deletes
- WHEN `IngestCommit::execute` completes successfully
- THEN it returns `RevisionId(6)` and publishes the complete revision-6 state
- AND exactly one workspace-scoped transaction commits

#### Scenario: Subordinate graph write does not commit

- GIVEN an open `IngestCommit` transaction for `ws1` revision 6
- WHEN subordinate `save_call_graph` finishes successfully
- THEN its writes remain uncommitted until `IngestCommit` commits
- AND it does not open or close another transaction

#### Scenario: Failure rolls back every staged write

- GIVEN a graph write fails after the revision row is staged
- WHEN `IngestCommit` handles the failure
- THEN it returns a typed error, rolls back the owner transaction, and leaves head=5
- AND revision 6, graph changes, manifest changes, report intents, and pins are absent

### Requirement: Last-Known-Good Preservation

A failed or interrupted `IngestCommit` MUST NOT erase, expire, or expose changes
to any prior revision. Readers MUST continue to resolve the prior head.

#### Scenario: Crash preserves last-known-good

- GIVEN `ws1` head=5 with 20 nodes
- WHEN the commit process stops before the owner transaction commits
- THEN `load_call_graph(ws1, 5)` returns the unchanged 20-node graph
- AND `load_call_graph(ws1, 6)` returns `Err(UnknownRevision)`

### Requirement: Post-commit reports and durable revision pins

`IngestCommit` MUST persist session pins and a report-outbox intent carrying the
new revision. An idempotent post-commit worker MUST generate the report by
reading that committed revision and MUST store the same `RevisionId`. Report
failure MUST retain a failed/pending outbox state and MUST NOT roll back or
invalidate the published graph revision.

#### Scenario: Report uses committed revision

- GIVEN an `IngestCommit` publishes revision 6 and its report-outbox intent
- WHEN the post-commit report worker consumes the intent
- THEN it reads revision 6 and the report carries `commit_revision_id = 6`

#### Scenario: Report failure does not corrupt graph publication

- GIVEN revision 6 committed and its report worker fails
- WHEN report status is inspected
- THEN revision 6 remains the workspace head and is fully readable
- AND the outbox records the failure for retry without publishing a partial report

#### Scenario: Existing session pin survives

- GIVEN a session pins revision 5
- WHEN a later commit publishes revision 6
- THEN the session still pins revision 5 and revision-5 reads succeed

### Requirement: Concurrent Commits Serialize per Workspace

Concurrent commits for the same workspace MUST serialize so only one can
publish the next revision. Commits for different workspaces MAY proceed
independently.

#### Scenario: Same-workspace race

- GIVEN two concurrent commits for `ws1` at head=5
- WHEN both attempt to publish revision 6
- THEN exactly one succeeds and the other returns `ConcurrentCommit`
- AND `ws1` head is 6, not 7

## Dependencies

`temporal-graph-history` is the storage foundation for this capability.
`atomic-ingest-commit` MUST NOT depend on `structural-graph-diff`; diff consumes
committed temporal revisions independently.
