# API Operability Specification

## Purpose
Define bounded, drainable, reconnecting, and measurable API behavior under production load.

## ADDED Requirements

### Requirement: Request and concurrency limits
The API MUST enforce configured request-body size, request-timeout, and maximum-concurrency limits, returning a deterministic client error when a limit is exceeded.

#### Scenario: Oversized or over-time request
- GIVEN a request exceeds the configured body-size limit or timeout
- WHEN the API processes it
- THEN it rejects or terminates the request with the documented limit error
- AND it does not start unbounded work

#### Scenario: Concurrency cap
- GIVEN the maximum concurrent request or ingest-job limit is reached
- WHEN another request arrives
- THEN the request receives the documented capacity response
- AND existing work continues within the cap

### Requirement: Graceful shutdown and drain
The API MUST stop accepting new work during shutdown and MUST drain accepted in-flight requests and jobs within the configured drain deadline.

#### Scenario: Graceful drain
- GIVEN shutdown begins with active requests
- WHEN the drain period runs
- THEN new work is rejected and accepted work is allowed to finish
- AND shutdown reports success only after completion or deadline handling

#### Scenario: Drain deadline
- GIVEN an accepted operation cannot finish before the drain deadline
- WHEN the deadline expires
- THEN it is reported as interrupted according to the API contract
- AND no new operation is admitted

### Requirement: Pool and LISTEN resilience
The service MUST apply the configured PostgreSQL pool policy and MUST reconnect LISTEN consumers after broker disconnection with backoff, restoring notifications within five seconds when PostgreSQL is available.

#### Scenario: Broker reconnect
- GIVEN a LISTEN connection is dropped while PostgreSQL is available
- WHEN the reconnect policy runs
- THEN the consumer reconnects within five seconds and resumes receiving notifications

#### Scenario: Pool exhaustion
- GIVEN all permitted PostgreSQL connections are occupied
- WHEN another database operation is requested
- THEN it waits or fails according to the pool timeout policy and does not create an unbounded connection

### Requirement: Metrics and SLO evidence
The API and ingest service MUST emit metrics for request limits, concurrency, drain outcomes, job lifecycle, pool health, LISTEN reconnects, and scale workloads. Production-proven SLO evaluation MUST be reproducible from retained run evidence.

#### Scenario: SLO report
- GIVEN a completed scale or nightly run
- WHEN its evidence is collected
- THEN the report includes required metrics, SLO verdicts, and the run identifier

#### Scenario: Failure visibility
- GIVEN a request, job, pool, or reconnect SLO is breached
- WHEN metrics are evaluated
- THEN the run is marked not production-proven and identifies the breached SLO
