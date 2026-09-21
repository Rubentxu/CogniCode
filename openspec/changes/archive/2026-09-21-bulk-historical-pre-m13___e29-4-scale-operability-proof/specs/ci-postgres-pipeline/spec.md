# Delta for ci-postgres-pipeline

## ADDED Requirements

### Requirement: Nightly scale/load lane
The CI pipeline MUST provide a scheduled nightly lane that runs the deterministic ingest scale and API operability workloads against PostgreSQL 16. The lane MUST publish pass/fail evidence and MUST NOT run on every pull request unless explicitly requested.

#### Scenario: Nightly execution
- GIVEN the scheduled nightly trigger occurs
- WHEN the pipeline runs
- THEN it starts PostgreSQL 16 and executes the 10 MB and 100 MB scale workloads
- AND it stores workload measurements and SLO verdicts

#### Scenario: Optional 1 GB execution
- GIVEN the optional large-fixture lane is enabled
- WHEN the nightly pipeline selects it
- THEN it generates the 1 GB fixture on demand and reports its result separately

## MODIFIED Requirements

### Requirement: `.github/workflows/ci.yml` runs the workspace test matrix
The CI workflow MUST: (a) trigger on `push` and `pull_request` to `main`; (b) run on `ubuntu-latest`; (c) start a `postgres:16` service with the same env vars as docker-compose; (d) cache `~/.cargo` and the workspace `target/`; (e) run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`. The workflow MUST export `TEST_DATABASE_URL` pointing at the service. Scale/load workloads MUST be isolated to the scheduled nightly lane rather than the pull-request matrix.
(Previously: The workflow ran only the workspace test matrix and had no scale/load lane.)

#### Scenario: Push to main triggers the workflow
- GIVEN a commit on `main`
- WHEN GitHub Actions runs
- THEN the workflow executes all four jobs (fmt, clippy, test, build) and passes
- AND it does not require the nightly scale lane

#### Scenario: PG service is reachable from the runner
- GIVEN the workflow's `services.postgres` block
- WHEN any `cargo test` step runs
- THEN `TEST_DATABASE_URL=postgres://cognicode:cognicode@localhost:5432/cognicode` connects to the live service (healthcheck passes before the test step)

#### Scenario: Cached build shortens re-runs
- GIVEN a previous successful run cached `target/`
- WHEN a new commit triggers the workflow
- THEN the second run completes in <60% of the cold-run wall time

#### Scenario: Nightly lane uses PostgreSQL
- GIVEN the scheduled nightly trigger
- WHEN the scale lane starts
- THEN its PostgreSQL service is healthy before workloads begin
- AND `TEST_DATABASE_URL` connects to that service
