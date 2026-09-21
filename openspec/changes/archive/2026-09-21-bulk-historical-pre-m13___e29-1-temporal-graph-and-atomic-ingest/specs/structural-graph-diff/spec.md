# structural-graph-diff Specification

## Purpose

A structural diff compares two pinned revisions of one workspace and returns
deterministic added, removed, and changed node and edge sets.

## ADDED Requirements

### Requirement: Diff Input Pin

`diff(workspace, from, to)` MUST pin two valid revisions from the same
workspace. Unknown, reclaimed, or cross-workspace pins MUST fail closed.

#### Scenario: Two valid revisions are compared

- GIVEN `ws1` has revisions 5 and 7
- WHEN `diff(ws1, 5, 7)` runs
- THEN it returns a `GraphDiff` pinned to revisions 5 and 7

#### Scenario: Unknown revision fails closed

- GIVEN `ws1` head=7
- WHEN `diff(ws1, 5, 99)` runs
- THEN it returns `DiffError::UnknownRevision` for revision 99

### Requirement: Set Difference on Nodes

Node logical identity MUST reuse the canonical persisted `NodeId` tuple
`(workspace_id, id, kind)` from `generic-graph-model`. Temporal `valid_from`
distinguishes versions but is not part of logical diff identity. The diff MUST
partition nodes into added, removed, and changed sets; a changed node keeps its
logical identity but has different properties.

#### Scenario: Added and removed nodes

- GIVEN revision 5 has `{a, b}` and revision 7 has `{b, c}`
- WHEN the revisions are diffed
- THEN `added_nodes=[c]`, `removed_nodes=[a]`, and `changed_nodes=[]`

#### Scenario: Same identity with changed properties

- GIVEN `n1` has `fan_out=2` at revision 5 and `fan_out=5` at revision 7
- WHEN the revisions are diffed
- THEN `n1` appears only in `changed_nodes` with old and new values

### Requirement: Set Difference on Edges

Edge logical identity MUST reuse the canonical persisted tuple
`(workspace_id, source_id, source_kind, target_id, target_kind, kind)` from
`generic-graph-model`. Temporal `valid_from` distinguishes versions but is not
part of logical diff identity. The diff MUST partition edges into added,
removed, and changed sets; changed edges MUST expose old and new provenance and
confidence.

#### Scenario: Edge addition and removal

- GIVEN revision 5 has `a->b` and revision 7 has `a->c`
- WHEN the revisions are diffed
- THEN `added_edges=[a->c]` and `removed_edges=[a->b]`

#### Scenario: Edge metadata changes

- GIVEN `a->b` changes from `(Extracted, 1.0)` to `(Inferred, 0.7)`
- WHEN the revisions are diffed
- THEN `changed_edges` contains both metadata versions for `a->b`

### Requirement: Deterministic Typed Output

`GraphDiff` MUST be serializable and byte-stable for identical inputs. Every
set MUST be sorted by identity, and hashes MUST be backend-neutral. `DiffError`
MUST distinguish unknown revisions, cross-workspace pins, and reclaimed data.

#### Scenario: Same inputs repeat byte-for-byte

- GIVEN `(ws1, 5, 7)` produces `D1`
- WHEN the same diff runs again
- THEN `D2 == D1` byte-for-byte and their hashes match

#### Scenario: Diff against self is empty

- GIVEN `ws1` revision 5
- WHEN `diff(ws1, 5, 5)` runs
- THEN every added, removed, and changed set is empty

#### Scenario: Reclaimed revision is rejected

- GIVEN revision 3 has been reclaimed
- WHEN `diff(ws1, 3, 7)` runs
- THEN it returns `DiffError::EmptyRevision`

## Dependencies

`temporal-graph-history` is the sole graph-state foundation for this
capability. `structural-graph-diff` MUST consume committed temporal revisions
and MUST NOT be a dependency of `atomic-ingest-commit`.
