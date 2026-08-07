# Delta for postgres-callgraph-persistence

> E29.1 makes graph writes subordinate to the workspace-scoped ingest
> transaction and replaces destructive persistence with temporal versions.

## ADDED Requirements

### Requirement: Append-Only Temporal Writes

A subordinate `save_call_graph` operation MUST insert new node and edge
versions using the `RevisionId` allocated by `IngestCommit`. It MAY update a
prior version only to set `valid_to`, and MUST use the transaction supplied by
`IngestCommit` without committing independently.

#### Scenario: Save produces versioned rows

- GIVEN `ws1` head=5 and a graph with seven symbols and twelve edges
- WHEN `IngestCommit` invokes `save_call_graph` for revision 6
- THEN new versions start at 6 and superseded versions end at 6
- AND the rows remain staged until the owner transaction commits

#### Scenario: Repeated save preserves history

- GIVEN revision 5 already stores a graph
- WHEN revision 6 stores the same graph
- THEN revision-5 history remains available and revision-6 reads are equivalent

### Requirement: Explicit Delete Records

Removed files MUST produce explicit delete records in the owner transaction.
Their node and edge versions MUST end at the allocated revision; physical
removal is reserved for retention garbage collection.

#### Scenario: Removed file emits a delete record

- GIVEN revision 5 includes `src/x.rs` and revision 6 does not
- WHEN `IngestCommit` publishes revision 6
- THEN a revision-6 delete record exists and `src/x.rs` versions end at 6

#### Scenario: Delete records are deterministic

- GIVEN two commits remove six files in total
- WHEN delete records are queried
- THEN six records are returned in `(revision, source_path)` order

### Requirement: `load_call_graph` Honors `valid_to` Expiry

`load_call_graph(workspace, revision)` MUST return rows where
`valid_from <= revision AND (valid_to IS NULL OR revision < valid_to)`.

#### Scenario: Expired node is hidden

- GIVEN `n1` has `valid_from=3, valid_to=7`
- WHEN `load_call_graph(ws1, 7)` runs
- THEN `n1` does not appear

## MODIFIED Requirements

### Requirement: `save_call_graph` inherent write method

`PostgresRepository::save_call_graph` MUST remain asynchronous,
PostgreSQL-gated, workspace-scoped, and revision-aware. Within ingest it MUST
be a subordinate write operation that receives the transaction and
`RevisionId` owned by `IngestCommit`. It MUST insert temporal node and edge
versions, expire superseded versions, and record explicit deletes in that
transaction. It MUST NOT call `pool.begin()`, open or demote a revision, commit,
or roll back. Only `IngestCommit::execute` MAY publish and return the new
`RevisionId` after the owner transaction commits.
(Previously: `save_call_graph` owned a destructive delete-and-replace
transaction and independently opened and committed the revision.)

#### Scenario: Happy path publishes one revision

- GIVEN empty graph rows for `ws1` and a graph with seven symbols and twelve edges
- WHEN `IngestCommit` invokes `save_call_graph` and commits
- THEN it returns one new revision with seven symbols and twelve edges
- AND exactly one transaction and one commit were used

#### Scenario: Workspace scope is preserved

- GIVEN `ws1` has three symbols at revision 4 and `ws2` has five at revision 7
- WHEN `IngestCommit` saves five symbols for `ws1`
- THEN `ws1` advances to revision 5 with five symbols and `ws2` remains unchanged

#### Scenario: Idempotent re-save is semantically equivalent

- GIVEN a graph was committed once
- WHEN the same graph is committed again
- THEN current counts and semantic rows are equivalent
- AND the prior revision remains historically readable

#### Scenario: Subordinate failure rolls back through the owner

- GIVEN `save_call_graph` fails after staging some revision-6 rows
- WHEN it returns the error to `IngestCommit`
- THEN `IngestCommit` rolls back all revision-6 graph, manifest, and pin changes
- AND `save_call_graph` performs no independent rollback or commit
