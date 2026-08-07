# Delta for CI PostgreSQL Pipeline

## MODIFIED Requirements

### Requirement: Test gating respects `TEST_DATABASE_URL`

PostgreSQL-dependent tests MUST distinguish an absent database configuration
from a configured database failure. If `TEST_DATABASE_URL` is genuinely absent,
tests MUST emit a skip notice and the overall run SHALL complete successfully.
If it is present, tests MUST attempt database setup and migrations; any
connection, setup, or migration error MUST fail the test run with a non-zero
exit and MUST NOT be reported as a skip.

(Previously: PG tests skipped when the variable was absent but did not explicitly fail closed on migration errors.)

#### Scenario: PG test skips when env var absent

- GIVEN no environment entry named `TEST_DATABASE_URL` exists
- WHEN `cargo test --workspace` runs
- THEN PG-dependent tests print a skip notice and the overall test run exits zero

#### Scenario: PG test runs when env var present

- GIVEN `TEST_DATABASE_URL` references a reachable compatible PostgreSQL service
- WHEN `cargo test --workspace` runs and database setup succeeds
- THEN PG-dependent tests execute their assertions

#### Scenario: Present but invalid URL is not absence

- GIVEN `TEST_DATABASE_URL` exists but is empty, malformed, or unreachable
- WHEN `cargo test --workspace` runs
- THEN the affected PG-dependent test fails and the overall run exits non-zero
- AND the failure is not reported as a skip

#### Scenario: Migration failure aborts loudly

- GIVEN `TEST_DATABASE_URL` reaches PostgreSQL but a required migration fails
- WHEN `cargo test --workspace` runs
- THEN the affected PG-dependent test and overall run exit non-zero
- AND output reports the migration failure rather than a skip notice
