# Archive Manifest — e82.1 Behavior Event Boundary

> Cycle: e82.1-lsi-behavior-event-boundary | Phase: archive | Date: 2026-09-17
> Closure pattern: administrative archive (git-level).

## Result

```text
P1.4                         RESOLVED
DEBT-SDDK-004                RESOLVED
domain -> application drift  ZERO
event semantics              UNCHANGED
new port introduced          NO
```

## What changed

One production behaviour, minimally: `behavior.budget_exhausted` is now emitted
through the **domain** `&dyn IntelligenceEventStore` path already used by every
other `BehaviorRuntime` event, instead of the application-layer `CausalRecorder`
helper. `budget_payload(...)` (previously unused) builds the payload;
`caused_by = Some(started_event)` is preserved.

`CausalRecorder` is untouched and remains a legitimate application helper.
No `BehaviorEventRecorder` port was created: the domain already owned the right
abstraction, and a second indirection for a single call would have been
accidental abstraction.

## Verification

| Check | Result |
|-------|--------|
| behavior runtime lib tests | 17 passed / 0 failed |
| `behavior_budget_e2e` (incl. new shape test) | 7 passed / 0 failed |
| `behavior_authority_e2e` | 6 passed / 0 failed |
| causal-chain regression | covered by existing E2E assertions |
| event payload equivalence | new `e82_1_budget_exhausted_event_shape_is_unchanged` |
| e77 architecture self-host | 0 drift (was 1), no longer `#[ignore]`d |
| `intelligence_event_log_e2e` | 4 passed / 0 failed |
| e64/e65 behavior authority + budget | GREEN |
| `cargo test -p cognicode-core --lib` | failure set == `scripts/known_failures.yaml` exactly (41 entries) |
| same, `--features evidence-kernel` | 2571 passed / 2 failed (the classified DEBT-SDDK-006 entries) |
| `cargo build --workspace` | GREEN |
| clippy on the touched surface | no new warnings (the one remaining `runtime.rs` hit is pre-existing and merely shifted; the previously-warned unused `budget_payload` is now used) |

## Recorded, not absorbed: one unreproduced baseline-checker drift

One `scripts/check_known_failures.py` run reported DRIFT during the gate. It did
not reproduce:

```text
run 1 (first post-edit rebuild)   DRIFT reported
runs 2-6                          OK, baseline exact (41 entries)
default lib suite x3              md5-identical FAILED sets
rustc verifier suite under load   all pass
```

`scripts/known_failures.yaml` was **not modified**. The most plausible mechanism
is a load-sensitive test in the default lib suite (the crate spawns `rustc` in
lib tests and already `#[ignore]`s one for exactly that reason), but it is
unconfirmed; it is recorded here rather than hidden. See
`characterization.md` for the full observation.

## Commits

* **Implementation:** `2bfc1383` — `fix(e82.1): emit behavior.budget_exhausted
  through the domain port`.
* **Archive:** this commit — this manifest + `state.yaml` update.

## State transition

```text
e82.1 CLOSED
P1.4 RESOLVED; DEBT-SDDK-004 RESOLVED; architecture drift 0

NEXT: e82.2 (historical score correctness), then STOP before e83.
```
