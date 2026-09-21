# Archive Manifest — cp0-backstage-fit-spike

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/cp0-backstage-fit-spike` |
| Path | a-min (spike + verdict) |
| Date | 2026-09-21 |
| Actor | jcode-orchestrator |
| Status | **SUPERSEDED — `archive-with-fit-decision`** |
| Superseded at | 2026-09-21T06:45Z |

## Outcome

`FIT_WITH_CONSTRAINTS` (per `fit-decision.md`). Backstage is an
adequate optional host shell for a future CogniCode Control Plane,
provided the integration stays in the service-backed shape
demonstrated in this cycle: host renders and navigates, CogniCode
Rust remains the single source of truth, no authority crosses the
boundary.

The four critical criteria all passed with real tests:

| Criterion | Verdict |
|---|---|
| No domain logic in Backstage backend plugin (stateless HTTP proxy; 4 tests) | PASS |
| No canonical state in host (restart/replica safe) | PASS |
| Hand-off to Explorer via ContextCapsule v1 + link (no iframe, bounded/versioned/refs-only; 7 tests) | PASS |
| Identity propagated as context header without authority crossing (probe reports `authority: none`) | PASS |

The one criterion that **did not pass** was frontend-build-and-run-on-Linux
(no browser UAT in this environment; ESM/React peer resolution friction).
That is a COVERAGE gap, not a SHAPE gap; the shape held.

The fit-decision explicitly notes: **Backstage is fit as a shell, and
only for as long as the Control Plane needs multi-plugin host capabilities
the Explorer does not provide**. The Control Plane re-evaluation
checkpoint (per the e78 consumer-count rule) is preserved.

## Commits produced

| SHA | Subject |
|-----|---------|
| `10fa1dcb` | feat(cp0): Backstage fit spike, outcome FIT_WITH_CONSTRAINTS |

## Cross-references

- Cycle artifacts: `characterization.md`, `design.md`, `fit-decision.md`,
  `proposal.md`, `state.yaml`, `tasks.md`, `verification-report.md`
- Sequencer (next): CP1.0 implementation cycle (already archived at
  `openspec/changes/archive/2026-09-21-cp1-control-plane-first-cycle/`)
- Forward condition (from `fit-decision.md`): when Control Plane exposes
  real e77 architecture through a headless read model, re-run the e78
  consumer-count checkpoint at that moment. CP0 does not resolve e78.

## Closure semantics

```text
implementation      CLOSED (real, on main; verdict FIT_WITH_CONSTRAINTS)
verification       PASS-with-coverage-gap (frontend-build-and-run untested)
archive closure    DONE   (this manifest)
forward condition  PRESERVED — e78 consumer-count re-evaluation required
                       when Control Plane exposes real architecture
```
