# Delta for plan-limits

## ADDED Requirements

### Requirement: Persistent descriptor limit policy

Every admitted analytics descriptor MUST declare its complexity class,
applicable limit dimensions, defaults, hard maxima, and whether each breach is
hard failure or soft truncation. The policy MUST be durably retrievable and
immutable for that descriptor version. A policy change MUST require a new
version and MUST change the normalized plan hash.

#### Scenario: Complete policy survives restart

- GIVEN an admitted descriptor with a complete limit policy
- WHEN the registry restarts and the descriptor version is loaded
- THEN the same defaults, maxima, and breach behaviors are available

#### Scenario: Incomplete policy blocks admission

- GIVEN a descriptor omitting its complexity class or truncation behavior
- WHEN admission is attempted
- THEN `DescriptorIncomplete` lists the omissions and the algorithm is not admitted

#### Scenario: Policy change requires version change

- GIVEN an admitted descriptor version whose hard node maximum changes
- WHEN admission reuses the same descriptor version
- THEN admission is rejected as `DescriptorVersionConflict`

### Requirement: Effective analytics limits

Every analytics run MUST resolve effective limits before execution. A caller MAY
tighten descriptor defaults but MUST NOT exceed hard maxima, remove required
limits, or request an unbounded traversal. Bounded shortest paths MUST have an
effective hop bound.

#### Scenario: Caller tightens a default

- GIVEN a descriptor default of 1,000 result rows and a caller limit of 100
- WHEN limits are resolved
- THEN the effective result-row limit is 100

#### Scenario: Caller attempts to widen a hard maximum

- GIVEN a descriptor hard maximum of 10,000 visited nodes
- WHEN a caller requests 20,000
- THEN validation returns `LimitPolicyViolation` before execution

#### Scenario: Shortest path remains bounded

- GIVEN a bounded-shortest-path request with no caller or descriptor hop bound
- WHEN limits are resolved
- THEN validation returns `MissingLimit(MaxHops)` and no run begins

### Requirement: Analytics limit outcome

Analytics runs MUST reuse the E28.2 typed hard-error and soft-truncation
semantics. Elapsed time, memory, cancellation, visited nodes, and visited edges
are hard boundaries and MUST fail without partial success. Result rows, path
count, and response bytes are soft boundaries and MUST return a deterministic
prefix with the exact truncation dimension. A descriptor MAY make a soft
dimension stricter, but MUST NOT soften a hard E28.2 dimension. Run lineage and
every product surface MUST expose the outcome.

#### Scenario: Soft limit truncates visibly

- GIVEN a run with `max_result_rows=10` that finds 30 rows
- WHEN the soft boundary fires
- THEN 10 rows and `ResultRowsLimit` are returned and recorded in lineage

#### Scenario: Hard limit returns typed error

- GIVEN a run that exceeds its memory maximum
- WHEN the hard boundary fires
- THEN it returns `LimitExceeded(Memory)` with no partial success

#### Scenario: Zero path count is empty

- GIVEN a run with `max_path_count=0`
- WHEN execution begins
- THEN it returns an empty, non-error result rather than an implicit unbounded run
