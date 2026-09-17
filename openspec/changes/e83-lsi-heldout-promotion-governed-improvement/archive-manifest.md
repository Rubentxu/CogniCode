# Archive Manifest — e83 Held-out Promotion + Governed Improvement

> Cycle: e83-lsi-heldout-promotion-governed-improvement | Phase: archive | Date: 2026-09-17
> Closure pattern: administrative archive (git-level). M13 closure cycle.

## Required exit report

```text
dataset split                      CLOSED / inherited e81
shadow evaluation                  CLOSED / inherited e82
scoring correctness                CLOSED / inherited e82.2
architecture drift                 ZERO / inherited e82.1

promotion lineage                  HARDENED
candidate freeze                   PROVEN
CONFIRM candidate identity         PROVEN
held-out policy                    CLOSED
OPTIMIZE cannot mask CONFIRM       PROVEN

Trial candidate identity           PROVEN
Trial PolicyGate                   REQUIRED
external human approval            REQUIRED for automated author
GovernedImprovementPermit          SEALED
world-drift fail-closed            PRESERVED

governed improvement E2E           PROVEN
physical source mutation           NOT PERFORMED (unchanged since e73; Applied is the authority/apply marker)
audit receipt                      PROVEN
event-log chain                    NOT WIRED (typed outcomes + receipt are the audit artifacts; adapter seam documented, no event bus added)

DEBT-SDDK-004                      RESOLVED
DEBT-SDDK-006                      STILL EXPLICIT

baseline transient                 NOT REPRODUCED
baseline stability closure gate    GREEN
```

## The invariant, demonstrated

```text
OPTIMIZE improvement + CONFIRM unacceptable regression = NO governed promotion
```

`wu12_optimize_win_confirm_loss_blocks_the_governed_path` builds exactly the
adversarial case: OPTIMIZE gains a TP and loses an FP, CONFIRM gains an FN.
Trial PASS, `CleanPromotionReady` and a verified external human approval are all
obtained independently, and a real `PromotionPermit` is issued. The held-out gate
still returns `PolicyOutcome::Block`, so no sealed `HeldOutGatePass` exists, and
`GovernedImprovementPermit` (whose only constructor requires one) cannot be built.
OPTIMIZE never reaches the decision at all.

## M13 closure

```text
13.1 Historical replay dataset        SATISFIED (e81)
13.2 OPTIMIZE/CONFIRM separation       SATISFIED (e81/e82)
13.3 FailureRegime taxonomy            SATISFIED (e82)
13.4 Shadow comparison                 SATISFIED (e82)
13.5 Held-out promotion gate           SATISFIED (e83)
13.6 Governed improvement E2E          SATISFIED (e83)
```

`tasks.md` checkboxes were NOT rewritten (historical record).

## ADR-051

Moved PROPOSED → **ACCEPTED** (local, `docs/adr/` is gitignored) after its own
validation clause was actually satisfied and documented: the spike/UAT passes,
and affected consumers plus the rollback path are written into the ADR. The status
was not changed merely because e83 exists.

## Verification

| Check | Result |
|-------|--------|
| e83 `governed_improvement` (WU12–WU22 all green) | 23 passed / 0 failed |
| e82 `shadow_evaluation` | 20 passed / 0 failed |
| e81 `historical_replay` | 23 passed / 0 failed |
| e76 `self_hosting` | 71 passed / 0 failed |
| e80a `promotion_authority` | 42 passed / 0 failed |
| e80b `application::ai` | 48 passed / 0 failed |
| e82.1 architecture self-host | 0 drift |
| `cargo test --lib --features evidence-kernel` | 2599 passed / 2 failed (the classified DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings |
| **WU23** 5 consecutive checker runs | exact (41 entries) |
| **WU23** 3 full lib failure-set captures | md5-identical `f27744948a9c475a5d0513968a5cda1c` |

No new failure occurred; nothing needed re-characterization.

## WU21 authority matrix — status

```text
A. HeldOut PASS + Trial PASS + CleanPromotionReady + LlmAgent + no approval => no permit   PROVEN
B. HeldOut BLOCK + Trial PASS + approval => no GovernedImprovementPermit                  PROVEN (structural: no pass exists)
C. HeldOut Insufficient + approval => no governed permit                                  PROVEN
F. fake PromotionAuthorization => e80a protections unchanged                              inherited e80a
G. permit for a different candidate world => governed wrapper rejects / cannot be minted   PROVEN (WU14-D/E)
H. world drift after permit => governed apply rejects via e73                             PROVEN (WU21-H)
```

## Explicit non-goals (honored)

No Backstage / Control Plane, no e78 Packs, no DEBT-SDDK-006 repair, no automatic
candidate search/ranking/weighted score, no synthetic canonical Evidence, no
binary attestation, no real source mutation, no live LLM, no git push, no event
bus. `known_failures.yaml` untouched.

## Commits

* **Implementation:** `feat(e83): held-out promotion gate and governed improvement`.
* **Archive:** this commit — this manifest + `state.yaml` update.

## State transition

```text
e83 CLOSED
M13 13.1-13.6 SATISFIED
ADR-051 ACCEPTED

STOP. Next: final umbrella/roadmap reconciliation
(M11/M13 status, ADR status, DEBT-SDDK-006, e78 consumer-count checkpoint)
before any entry into the post-roadmap Control Plane package.
```
