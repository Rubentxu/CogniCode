# e82.1 — Behavior Event Boundary (P1.4 closure)

> Cycle: e82.1-lsi-behavior-event-boundary | Phase: characterize + apply | Date: 2026-09-17
> Closure pattern: administrative archive (git-level).

## WU0 — exact existing event semantics (the equivalence oracle)

Before the change, `behavior.budget_exhausted` was built by
`application::intelligence_log::CausalRecorder::record_behavior_budget_exhausted`
(`recorder.rs:122`). Captured contract:

```text
kind          EventKinds::behavior_budget_exhausted()
caused_by     Some(started_event)          (NOT the rejection event)
scope         context.scope.clone()
workspace     context.scope.workspace
actor         the runtime's actor (ActorRef::kernel() in the E2E fixture)
correlation   context.correlation
occurred_at   runtime's EventTime (self.now)
payload name  "budget exhausted"
payload fields (exact order)
              behavior_id, effective_class, kind, remaining, attempted, execution_id
```

Expected causal edge: `behavior.started → behavior.budget_exhausted`.

## WU1 — the change

Removed `use crate::application::intelligence_log::CausalRecorder;` from
`domain/behaviors/runtime.rs` and emitted the event through the **same** domain
port path already used for `behavior.started`, `policy.behavior_output_rejected`
and `behavior.completed`:

```rust
self.append(
    context,
    &actor,
    EventKinds::behavior_budget_exhausted(),
    budget_payload(&[
        ("behavior_id",         admitted.id().as_str().to_string()),
        ("effective_class",     class.name().to_string()),
        ("kind",                exhaustion.kind.name().to_string()),
        ("remaining",           exhaustion.remaining.to_string()),
        ("attempted",           exhaustion.attempted.to_string()),
        ("execution_id",        context.execution_id.to_string()),
    ])?,
    Some(started_event),
)
```

`budget_payload(...)` was already present in `runtime.rs` and previously unused;
it is now the payload builder. Field order matches the recorder exactly.

`CausalRecorder` itself is **untouched**: it stays a legitimate application-layer
helper for complex causal chains, and other consumers keep using it. No new port
was introduced; the domain already owned the right abstraction.

## WU2 — semantic equivalence

`behavior_budget_e2e::e82_1_budget_exhausted_event_shape_is_unchanged` pins the
complete shape: kind, causal edge (and that the cause is the `behavior.started`
event, not the rejection event), scope, payload summary (`"budget exhausted"`),
and all six payload fields with their values.

Existing coverage verified the causal chain and the event kind but **not** the
payload, which is why this test was added rather than assumed.

## WU3 — architecture self-host closure

```text
before:  exactly 1 drift   (domain::behaviors::runtime -> application::intelligence_log::CausalRecorder)
after:   exactly 0 drifts
```

No other drift appeared. The `#[ignore]` on
`architecture_self_host_e2e::self_host_evaluator_finds_zero_drift_on_clean_source`
was removed: it existed solely to protect this known drift, and the test now
passes as part of normal verification.

## WU4 — debt closure

`DEBT-SDDK-004` is RESOLVED: all four drifts repaired, architecture drifts 4 → 0.

## Observation: one unreproduced baseline-checker drift

During the e82.1 verification gate, one `scripts/check_known_failures.py` run
reported a DRIFT verdict. It could not be reproduced:

```text
run 1 (during the first post-edit rebuild)   DRIFT reported
run 2, 3, 4, 5                               OK: observed failure set matches baseline (41 entries)
default lib suite x3                         identical FAILED sets (md5-identical)
rustc-suite under 64-core CPU load           all pass
```

What is established:

* The baseline file `scripts/known_failures.yaml` was **not modified**.
* The default lib failure set is deterministic and matches the baseline exactly
  across three consecutive full runs.
* The crate contains lib tests that spawn `rustc`
  (`infrastructure::verification::rust_verifier`), one of which is already
  `#[ignore]`d with the reason "passes individually, fails in parallel suite due
  to temp dir + rustc process contention". A load-sensitive test in the default
  suite is the most plausible source of a one-off spurious verdict.
* The e82.1 change touches event emission in `domain/behaviors/runtime.rs`,
  which is unrelated to that surface.

Not reproduced means not explained. It is recorded here rather than absorbed, and
the baseline was deliberately **not** regenerated (the directive forbids
absorbing a new failure into the baseline).
