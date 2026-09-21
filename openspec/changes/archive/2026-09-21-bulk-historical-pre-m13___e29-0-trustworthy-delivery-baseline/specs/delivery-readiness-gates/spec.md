# Delivery Readiness Gates Specification

## Purpose

Define the observable gates that prevent delivery until the critical PostgreSQL,
Explorer, and end-to-end paths are verifiably ready.

## ADDED Requirements

### Requirement: Delivery readiness is conjunctive

A delivery candidate MUST be eligible to merge only when its migration,
PostgreSQL verification, Explorer build, and critical smoke gates all pass. If
any required gate fails, the readiness verdict MUST be non-zero.

#### Scenario: Every gate passes

- GIVEN all required readiness gates report success for one candidate
- WHEN the aggregate readiness verdict is evaluated
- THEN the candidate is eligible to merge and the verdict exits zero

#### Scenario: One gate fails

- GIVEN at least one required readiness gate reports failure
- WHEN the aggregate readiness verdict is evaluated
- THEN the verdict exits non-zero and the candidate is not eligible to merge

### Requirement: Fresh database migration readiness

A delivery candidate MUST apply the complete migration chain to an empty,
supported PostgreSQL database without manual intervention. Applying remaining
migrations to a supported populated database MUST preserve its existing rows.
Any migration error MUST fail the gate and identify the failing migration.

#### Scenario: Fresh database migration succeeds

- GIVEN an empty PostgreSQL 16 database and valid credentials
- WHEN the readiness gate applies the complete migration chain
- THEN every migration completes and the resulting schema accepts graph data
- AND the migration gate reports success without manual correction

#### Scenario: Populated database migration preserves data

- GIVEN a supported database with prior migrations applied and persisted rows
- WHEN the readiness gate applies the remaining migration chain
- THEN migration completes without deleting or changing those rows
- AND the resulting schema is ready for graph operations

#### Scenario: Fresh database migration fails

- GIVEN an empty PostgreSQL database where one migration cannot complete
- WHEN the readiness gate applies the migration chain
- THEN the gate exits non-zero and identifies the failing migration
- AND the delivery candidate is not marked ready

### Requirement: PostgreSQL verification fails closed

When `TEST_DATABASE_URL` is present, PostgreSQL-dependent verification MUST run
and MUST treat connection, setup, or migration errors as failures. It MUST NOT
classify such errors as skips.

#### Scenario: PostgreSQL verification succeeds

- GIVEN `TEST_DATABASE_URL` references a reachable compatible database
- WHEN PostgreSQL-dependent verification runs and migrations succeed
- THEN its assertions execute and the gate reports success

#### Scenario: PostgreSQL migration error is loud

- GIVEN `TEST_DATABASE_URL` references a database whose migration fails
- WHEN PostgreSQL-dependent verification runs
- THEN the gate exits non-zero with the migration failure
- AND no affected verification is reported as skipped

### Requirement: Explorer build readiness

The Explorer production build invoked by `npm run build` MUST complete
successfully before delivery. Any compile or type-check error MUST fail the
readiness gate.

#### Scenario: Explorer builds successfully

- GIVEN the Explorer dependencies and supported toolchain are available
- WHEN `npm run build` runs in the Explorer workspace
- THEN it exits zero and produces its build output

#### Scenario: Explorer contains a type error

- GIVEN the Explorer source contains a compile-time type error
- WHEN `npm run build` runs in the Explorer workspace
- THEN it exits non-zero and the delivery candidate is not marked ready

### Requirement: Critical smoke flow

The readiness gate MUST verify the ordered critical flow: open a workspace,
start a scan, observe the Job succeed, observe a graph node count greater than
zero, and render the landing view. Failure or timeout at any stage MUST block
delivery.

#### Scenario: Critical flow is healthy

- GIVEN a reachable PostgreSQL service and an extractable workspace fixture
- WHEN the smoke gate opens the workspace, starts a scan, polls the Job to success, reads graph statistics, and opens the landing view
- THEN the reported graph node count is greater than zero
- AND the landing view renders and the smoke gate passes

#### Scenario: Critical flow is incomplete

- GIVEN any critical stage fails, times out, or reports a zero graph node count
- WHEN the smoke gate evaluates the flow
- THEN it exits non-zero and identifies the failed stage
- AND the delivery candidate is not marked ready
