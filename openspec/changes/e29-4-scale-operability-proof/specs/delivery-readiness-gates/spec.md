# Delta for delivery-readiness-gates

> **Validation precondition:** E29.0 MUST be implemented and archived first so
> `delivery-readiness-gates` exists as a canonical capability. This cross-change
> delta is intentionally not valid as a standalone change.

## ADDED Requirements

### Requirement: Production-proven maturity gate
The delivery process MUST withhold the `production-proven` designation until scale budgets, lifecycle guarantees, API operability limits, pool policy, LISTEN recovery, metrics, and SLOs are verified. The gate MUST distinguish implementation-complete (the code, configurations, and infrastructure needed to run scale and operability workloads are in place) from maturity-evidence (retained scheduled-run evidence accumulated over time). Implementation-complete checks MAY pass before any maturity evidence exists; evidence accumulates separately and is reported as it grows.

#### Scenario: Implementation-complete without evidence
- GIVEN all required implementation-complete checks pass (code, config, infrastructure, fixtures, and per-run measurement capture are in place) and no qualifying nightly-run evidence has been retained yet
- WHEN delivery readiness is evaluated
- THEN the implementation-complete portion passes
- AND `production-proven` is not yet permitted because maturity-evidence is still empty

#### Scenario: Gate passes
- GIVEN all required scale, lifecycle, operability, and SLO checks have passed AND retained maturity-evidence satisfies the evidence rule
- WHEN delivery readiness is evaluated
- THEN the production-proven gate passes
- AND the release may be designated `production-proven`

#### Scenario: Gate blocks on one failure
- GIVEN any required budget or SLO check has failed or lacks evidence
- WHEN delivery readiness is evaluated
- THEN the gate fails
- AND `production-proven` is not permitted

### Requirement: Executable maturity-evidence rule

Maturity evidence SHALL accumulate as qualifying scheduled runs complete on
`main`. `production-proven` MUST require at least seven consecutive qualifying
runs spanning at least seven calendar days, with every mandatory 10 MB/100 MB,
incremental, query/render, lifecycle, API, pool, LISTEN, metrics, and SLO check
passing in each run. A failed, incomplete, or unretained mandatory run resets
the consecutive qualification count; prior records remain retained for audit.
The gate implementation MAY ship while evidence accumulates. The 1 GB fixture
lane is optional extended evidence and MUST NOT participate in the mandatory
qualification rule.

#### Scenario: Partial evidence is retained but does not qualify
- GIVEN fewer than seven qualifying nightly runs have completed on `main`
- WHEN maturity-evidence is evaluated
- THEN the partial evidence is reported and retained
- AND implementation-complete status MAY ship
- AND `production-proven` remains withheld

#### Scenario: 1 GB lane is optional evidence
- GIVEN the optional 1 GB fixture lane is disabled or unavailable
- WHEN maturity-evidence is evaluated
- THEN the maturity-evidence rule does not require it
- AND the absence of 1 GB evidence does not block `production-proven`

#### Scenario: Streak interruption
- GIVEN a nightly run fails, is incomplete, or lacks retained evidence
- WHEN the streak is evaluated
- THEN the consecutive-run count for the maturity-evidence streak resets
- AND any already-counted qualifying runs before the interruption remain in the retained evidence set

## MODIFIED Requirements

### Requirement: Delivery readiness is conjunctive
A delivery candidate MUST be eligible to merge only when its migration, PostgreSQL verification, Explorer build, and critical smoke gates all pass. If any required gate fails, the readiness verdict MUST be non-zero. The separate `production-proven` designation MUST additionally require retained scale, lifecycle, operability, pool, LISTEN, metrics, and SLO evidence evaluated through the maturity gate.
(Previously: A delivery candidate was eligible to merge only when its migration, PostgreSQL verification, Explorer build, and critical smoke gates all pass; production-proven scale and operability evidence was not part of the conjunctive readiness contract.)

#### Scenario: Every gate passes
- GIVEN all required readiness gates report success for one candidate
- WHEN the aggregate readiness verdict is evaluated
- THEN the candidate is eligible to merge and the verdict exits zero

#### Scenario: One gate fails
- GIVEN at least one required readiness gate reports failure
- WHEN the aggregate readiness verdict is evaluated
- THEN the verdict exits non-zero and the candidate is not eligible to merge

#### Scenario: Baseline and production evidence pass
- GIVEN fresh-DB, ingest, compile, smoke, scale, lifecycle, API, and SLO checks pass AND retained maturity-evidence satisfies the evidence rule
- WHEN readiness is evaluated
- THEN the delivery gate passes
- AND the release may be designated `production-proven`

#### Scenario: Missing production evidence
- GIVEN baseline checks pass but retained maturity-evidence is insufficient for the `production-proven` designation
- WHEN readiness is evaluated
- THEN merge eligibility MAY still be granted
- AND `production-proven` is not permitted until the maturity-evidence rule is satisfied
