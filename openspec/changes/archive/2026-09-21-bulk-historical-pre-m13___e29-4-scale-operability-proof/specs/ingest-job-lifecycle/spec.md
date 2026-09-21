# Ingest Job Lifecycle Specification

## Purpose
Define durable, observable, and recoverable lifecycle behavior for ingest jobs.

## ADDED Requirements

### Requirement: Durable job state and progress
Each ingest job MUST expose durable state, progress, timestamps, and failure context. State MUST be recoverable after process restart.

#### Scenario: Progress is observable
- GIVEN an ingest job is running
- WHEN work advances through measurable stages
- THEN persisted status reports the current stage and progress
- AND a client can observe monotonic progress until completion or terminal failure

#### Scenario: Restart recovery
- GIVEN a job state was persisted before the service stopped
- WHEN the service restarts
- THEN the job is returned with its last durable state and progress
- AND an interrupted non-terminal job is explicitly marked recoverable or failed

### Requirement: Correct terminal outcomes
A job MUST enter `Completed` only when ingest succeeds, MUST enter `Failed` with an error when ingest fails, and MUST enter `Cancelled` when cancellation is accepted.

#### Scenario: Ingest failure
- GIVEN an ingest job encounters an ingest error
- WHEN the job terminates
- THEN its durable state is `Failed`, not `Completed`
- AND the failure context is available to status consumers

#### Scenario: Cancellation
- GIVEN a running job accepts a cancellation request
- WHEN cancellation takes effect
- THEN the job stops further ingest work and reaches `Cancelled`
- AND subsequent cancellation requests do not restart it

### Requirement: Restart-safe status access
Job status queries MUST remain available for completed, failed, cancelled, and recovered jobs without requiring the originating process instance.

#### Scenario: Terminal status after restart
- GIVEN a job reached a terminal state and the process restarted
- WHEN a client requests the job status
- THEN the same terminal state and final progress are returned
