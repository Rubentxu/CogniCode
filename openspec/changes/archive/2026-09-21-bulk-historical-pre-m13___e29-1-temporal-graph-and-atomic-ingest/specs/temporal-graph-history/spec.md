# temporal-graph-history Specification

## Purpose

Append-only node and edge versions preserve exact graph history. Revision pins
select valid ranges, while retention and garbage collection reclaim only
unprotected history.

## ADDED Requirements

### Requirement: Version-table identity and valid ranges

Temporal history MUST use additive `graph_node_versions` and
`graph_edge_versions` tables (or an equivalent representation that preserves
the same keys). Node-version identity MUST be
`(workspace_id, id, kind, valid_from)`. Edge-version identity MUST be
`(workspace_id, source_id, source_kind, target_id, target_kind, kind,
valid_from)`. Every version MUST carry `valid_from` and nullable `valid_to`
revision bounds. The migration MUST backfill existing current rows with a valid
starting revision. New versions MUST set `valid_from = new_revision` and
`valid_to = NULL`. The existing `graph_nodes` and `graph_edges` identities
remain the current-head projection and MUST NOT be overloaded to store multiple
versions under their non-temporal primary keys.

#### Scenario: Temporal columns are available

- GIVEN a populated database
- WHEN the temporal migration completes
- THEN node and edge version rows carry valid revision ranges and temporal keys
- AND existing rows remain readable at the current head

### Requirement: Pinned-Range Historical Reads

A read pinned to `(workspace, revision)` MUST return rows where
`valid_from <= revision AND (valid_to IS NULL OR revision < valid_to)`. It MUST
NOT depend on the current head or silently fall back.

#### Scenario: Old revision excludes newer rows

- GIVEN revision 3 has five nodes and revision 4 adds three nodes
- WHEN `read_at(ws1, 3)` runs
- THEN exactly the five revision-3 nodes are returned

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=5
- WHEN `read_at(ws1, 99)` runs
- THEN it returns `Err(UnknownRevision)` with no head fallback

### Requirement: Append-Only Mutation

A changed node or edge MUST create a new version. The prior version MAY be
updated only to set `valid_to = new_revision`, and both changes MUST use the
workspace-scoped transaction owned by `IngestCommit`.

#### Scenario: Update versions rather than overwrites

- GIVEN node `n1` has `valid_from=3, valid_to=NULL`
- WHEN revision 5 changes `n1`
- THEN a new version starts at 5 and the prior version ends at 5
- AND reads at revisions 3 and 5 return their respective properties

### Requirement: Retention Window

A configurable retention window `R` MUST mark revisions older than
`current_head - R` as reclaimable only when no report, session, or protected pin
references them.

#### Scenario: Old unpinned revision is reclaimable

- GIVEN `ws1` head=10, `R=3`, and no pins on revisions 1 through 6
- WHEN `gc_candidates(ws1)` runs
- THEN revisions 1 through 6 are listed and revisions 7 through 10 are not

#### Scenario: Pinned revision is protected

- GIVEN the same history and a session pin on revision 4
- WHEN `gc_candidates(ws1)` runs
- THEN revision 4 is not listed

### Requirement: GC Atomically Retires Superseded Revisions

`gc_revisions` MUST remove the selected reclaimable versions and revision rows
in one workspace-scoped transaction. It MUST fail closed without removing
anything if any selected revision is pinned.

#### Scenario: GC removes only unprotected history

- GIVEN revisions 1 through 3 are reclaimable
- WHEN `gc_revisions(ws1, [1, 2, 3])` succeeds
- THEN their temporal rows and revision records are removed
- AND protected and current revisions remain intact

#### Scenario: Stray pin aborts GC

- GIVEN revision 4 is selected but a session still pins it
- WHEN `gc_revisions(ws1, [1, 2, 3, 4])` runs
- THEN it returns `Err(ProtectedPin)` and removes no rows

## Dependencies

This temporal storage capability depends on `graph-revisions` and database
persistence primitives only. It MUST NOT depend on `atomic-ingest-commit` or
`structural-graph-diff`. The directed dependency graph is:

- `temporal-graph-history` -> `atomic-ingest-commit` (storage feeds commit)
- `temporal-graph-history` -> `structural-graph-diff` (storage feeds diff)
