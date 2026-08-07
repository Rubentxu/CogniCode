# graph-revisions Specification (NEW)

## Purpose

Immutable canonical graph revisions in PostgreSQL. A revision is opened
on ingest commit; every read pins one workspace and one revision and
fails closed when the pair is unknown.

## Requirements

### Requirement: RevisionId value object

`RevisionId(u64)` in
`crates/cognicode-core/src/domain/value_objects/revision_id.rs` MUST
derive `Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
Serialize, Deserialize`. `RevisionId(0)` is `RevisionId::NONE`; a revision
is valid iff `> 0`. `Display` produces `"rev:{n}"`; `from_str` is the
inverse.

#### Scenario: Round-trip and reserved sentinel

- GIVEN `RevisionId(7)` and `RevisionId::NONE`
- WHEN formatted then parsed
- THEN `RevisionId(7)` round-trips
- AND `RevisionId::NONE.is_valid()` is `false`

### Requirement: graph_revisions table

Add `graph_revisions(workspace_id TEXT, revision_id BIGINT, created_at
TIMESTAMPTZ DEFAULT now(), head_of BOOLEAN DEFAULT false, PRIMARY KEY
(workspace_id, revision_id))` with partial unique index
`idx_graph_revisions_head ON graph_revisions(workspace_id) WHERE head_of`.
At any time exactly one row per workspace has `head_of = true`.
Migration is additive.

#### Scenario: New table exists with head uniqueness

- GIVEN an empty database
- WHEN migration `m0017_graph_revisions` runs
- THEN `graph_revisions` exists with the columns above
- AND inserting a second `head_of = true` row for the same workspace is rejected

### Requirement: Open revision on ingest commit

The ingest pipeline MUST open exactly one new revision per successful
commit: allocate the next monotonic `revision_id` for that workspace,
insert a `graph_revisions` row with `head_of = true`, atomically demote
the previous head, all in one transaction. A failed commit MUST NOT
leave an open revision.

#### Scenario: First and subsequent commits are monotonic

- GIVEN an empty workspace `ws1`
- WHEN two sequential commits complete
- THEN heads are `(1, true)` then `(2, true)`
- AND no two rows share `(workspace_id, revision_id)`

#### Scenario: Workspaces do not share counters

- GIVEN `ws1` head=3 AND `ws2` head=5
- WHEN `ws1` ingests again
- THEN `ws1` head becomes `4` (NOT `6`)

#### Scenario: Failed commit leaves no half-open revision

- GIVEN `ws1` head=4
- WHEN a commit fails mid-transaction
- THEN `graph_revisions` for `ws1` still has head=4 AND no new row

### Requirement: Read pins one workspace + revision

Reads MUST accept `(WorkspaceId, RevisionId)` and reject unknown pairs
with typed `RevisionError::UnknownRevision { workspace, revision }` —
never `None`, never panic, never silent fall-back to head.

#### Scenario: Pinned read succeeds

- GIVEN `ws1` head=`revision_id = 3`
- WHEN a read pins `(ws1, 3)`
- THEN the result is `Ok(snapshot)` with rows for `ws1`

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=3
- WHEN a read pins `(ws1, 99)`
- THEN the result is `Err(UnknownRevision { workspace: "ws1",
  revision: 99 })`

#### Scenario: Cross-workspace pin is rejected

- GIVEN `ws1` revision `3` exists AND `ws2` does not
- WHEN a read pins `(ws2, 3)`
- THEN the result is `Err(UnknownRevision { workspace: "ws2", revision: 3 })`

## Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Process restart after commit | Read on `(ws, head)` is `Ok` |
| Concurrent ingests on same workspace | Exactly one opens; the other waits or fails closed |
| `workspace_id` already exists on `graph_nodes` (m0010) | Reused; no column add |

## Out of Scope

Graph analytics (E28.2+); MoldQL Pattern Profile (E28.1); retention
beyond `head_of` partial index; cross-workspace federation.

## Dependencies

Existing `workspace_id` column (m0010); `scan_manifest` table (m0010);
ADR-014 §1.