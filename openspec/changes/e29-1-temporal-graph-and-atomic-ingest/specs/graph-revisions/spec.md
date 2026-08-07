# Delta for graph-revisions

> E29.1 extends revision lifecycle with one transaction owner, retention,
> garbage collection, session pins, and post-commit report outbox linkage.

## ADDED Requirements

### Requirement: Atomic Commit Lifecycle

A workspace-scoped `IngestCommit` MUST own the transaction that allocates a
revision, demotes the prior head, coordinates subordinate graph persistence,
and writes report-outbox intents or session pins. Revision persistence MUST use
that owner transaction and MUST NOT commit independently. No new revision is
visible before the owner commits.

#### Scenario: Commit opens and demotes atomically

- GIVEN `ws1` head=5
- WHEN `IngestCommit` commits revision 6
- THEN revision 6 is head and revision 5 is no longer head
- AND both changes become visible in the same commit

#### Scenario: Failed commit leaves no half-open row

- GIVEN `ws1` head=5
- WHEN `IngestCommit` fails before its owner transaction commits
- THEN revision 6 is absent and revision 5 remains head

### Requirement: Retention Window Cooperation

Revisions older than `current_head - R` MUST be eligible for garbage collection
only when no report, session, or protected marker pins them.

#### Scenario: Retention window is honored

- GIVEN `ws1` head=10, `R=3`, and no pins on revisions 1 through 6
- WHEN `gc_candidates(ws1)` runs
- THEN revisions 1 through 6 are listed and revisions 7 through 10 are not

### Requirement: Report outbox and session pin linkage

Sessions MUST persist the revision they use. `IngestCommit` MUST write a
report-outbox intent carrying the published revision in its owner transaction;
an idempotent worker MUST persist the completed report and its revision pin only
after commit. Completed reports and session pins protect referenced revisions
from garbage collection.

#### Scenario: New report is generated after commit

- GIVEN `IngestCommit` publishes revision 6 and its report-outbox intent
- WHEN the post-commit report worker succeeds
- THEN the completed report carries `commit_revision_id = 6`
- AND report failure cannot roll back revision 6

#### Scenario: Session pin survives later commits

- GIVEN a session pins revision 5
- WHEN a later commit publishes revision 6
- THEN the session still pins revision 5

### Requirement: Protected Pins Block GC

`gc_revisions` MUST reject the entire request when any selected revision is
still pinned.

#### Scenario: Protected revision aborts GC

- GIVEN revision 4 is selected and a session pins revision 4
- WHEN `gc_revisions(ws1, [1, 2, 3, 4])` runs
- THEN it returns `Err(ProtectedPin)` and removes no revisions

## MODIFIED Requirements

### Requirement: Open revision on ingest commit

The ingest pipeline MUST open exactly one new revision per successful commit:
allocate the next monotonic `revision_id` for that workspace, insert a
`graph_revisions` row with `head_of = true`, and atomically demote the previous
head. The workspace-scoped `IngestCommit` MUST own the transaction and MUST
commit the revision together with subordinate graph writes, session pins, and
report-outbox intents. Completed reports MUST be generated after commit. A
failed commit MUST NOT leave an open revision.
(Previously: the revision opened atomically, but the contract did not make
`IngestCommit` the sole transaction owner or include session pins and report
outbox intents.)

#### Scenario: First and subsequent commits are monotonic

- GIVEN an empty workspace `ws1`
- WHEN two sequential commits complete
- THEN heads are `(1, true)` then `(2, true)`
- AND no two rows share `(workspace_id, revision_id)`

#### Scenario: Workspaces do not share counters

- GIVEN `ws1` head=3 and `ws2` head=5
- WHEN `ws1` ingests again
- THEN `ws1` head becomes 4, not 6

#### Scenario: Failed commit leaves no half-open revision

- GIVEN `ws1` head=4
- WHEN a commit fails mid-transaction
- THEN `ws1` still has head=4 and no new revision row

#### Scenario: Revision and report intent share the owner transaction

- GIVEN `ws1` head=5 and a pending report request
- WHEN `IngestCommit` publishes revision 6
- THEN revision 6 and a report-outbox intent pinned to 6 become visible together
- AND no completed report is visible until the post-commit worker succeeds
