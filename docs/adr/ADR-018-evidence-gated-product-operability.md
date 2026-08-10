# ADR-018: Evidence-gated product operability and durable jobs

**Status**: ACCEPTED (promoted 2026-08-10)
**Date**: 2026-07-29
**Deciders**: User, OpenCode orchestrator

## Context

CogniCode has repeatedly reported backend foundations as complete while the
critical user path remained unexecutable. Current ingest jobs are process-local,
can report completion after failure, cannot be cancelled reliably, and disappear
on restart. Existing performance budgets cover micro-operations rather than
complete ingest, query, and render workflows.

ADR-012 defines user-visible completion. It does not define integration maturity,
durable asynchronous work, operational limits, or the evidence required before
calling a capability production-proven.

## Decision

CogniCode will report maturity explicitly and will require retained evidence for
production claims. Asynchronous ingest jobs and critical API operations will be
durable, bounded, observable, and cancellable.

### 1. Maturity has four explicit states

| State | Meaning |
|---|---|
| `foundation shipped` | Internal contracts exist and focused tests pass. |
| `integrated` | The production composition path executes end to end. |
| `user-visible` | ADR-012 discoverability, inspection, use, and interaction validation pass. |
| `production-proven` | Repeated retained scale and operability evidence satisfies declared budgets. |

Lower states must not be described as higher states. A change may merge when its
implementation-complete gates pass, while production evidence continues to
accumulate on `main`.

### 2. Readiness is conjunctive

The delivery gate covers fresh PostgreSQL migration, ingest integrity, Explorer
compilation, the open-to-landing smoke path, loud PostgreSQL test failure, and
all capability-specific checks. One failed mandatory gate prevents a readiness
claim.

### 3. Jobs are durable state machines

Ingest jobs use a PostgreSQL-backed `JobStore` and explicit `pending`, `running`,
`completed`, `failed`, and `cancelled` states. Progress, error details, revision
linkage, cancellation, restart recovery, and concurrency control are part of the
contract. A failed scan cannot be recorded as completed.

### 4. API operability is bounded and observable

Critical APIs enforce request-size, timeout, and concurrency limits; drain
in-flight work during graceful shutdown; reconnect PostgreSQL listeners with
bounded backoff; and expose metrics sufficient to evaluate service-level
objectives.

### 5. Production evidence is deterministic and retained

Scale lanes use seeded, generated fixtures and versioned budgets. Required CI
evidence covers 10 MB and 100 MB ingest, incremental change ratios, and bounded
query/render graphs. A 1 GB lane is optional extended evidence and cannot be an
unconditional merge or `production-proven` criterion.

Exact budgets, fixture sizes, and evidence windows belong in executable
specifications and configuration, not in this ADR.

## Alternatives considered

### Binary `DONE` status

Rejected. It conflates internal code completion with product and production
readiness.

### Process-local job state

Rejected. It loses status on restart and cannot support reliable recovery.

### Manual load testing

Rejected. Unretained, non-deterministic evidence cannot justify roadmap claims.

### Horizontal clustering before single-node proof

Rejected. Distribution would multiply failure modes before basic operability is
measured.

## Consequences

### Positive

- Roadmap and release claims become auditable.
- Job failures, cancellation, and recovery survive process boundaries.
- Performance and operability regressions become visible before promotion.

### Negative

- Adds PostgreSQL writes, CI cost, and lifecycle complexity.
- Production-proven status may lag implementation completion.
- Teams must maintain fixtures, budgets, and retained evidence.

### Mitigations

- Keep ordinary merge gates small and deterministic.
- Run heavier evidence lanes on `main` or scheduled workflows.
- Separate implementation-complete from maturity-evidence status.

## References

- [E29.0 proposal](../../openspec/changes/e29-0-trustworthy-delivery-baseline/proposal.md)
- [E29.4 proposal](../../openspec/changes/e29-4-scale-operability-proof/proposal.md)
- [ADR-012](./ADR-012-ui-visible-capability-contract.md)
- [ADR-014](./ADR-014-moldql-pattern-graph-analytics-platform.md)
- Local SDD evidence: `plans/cognicode-capability-readiness-assessment-2026-07-29.md` (not version-controlled by policy)

## Implementation Log

- **2026-08-10 (E31-C)**: Evidence-gated product operability codified by the release-readiness-gate spec. The 12-gate scorecard operationalizes evidence via the openspec_conformance.py harness.
