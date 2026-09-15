# Design — cycle e63 — Intelligence Event Log foundation (M7.1, U50)

> Cycle: A-lite | Milestone: M7 | Phase: design | Date: 2026-09-15

## D1 — The log is a history, not a bus and not a database

```text
Evidence Kernel  = truth      (what is)
Intelligence Log = history    (what happened)
Event Bus        = delivery   (how it reaches someone)
Reactive Runtime = consumer   (what acts)
```

Consequences, all of them visible in the port's *absence* of methods:

- no `publish`, no `subscribe`: delivery is layered on top of commit, never
  fused with it. If they are fused, a crash between the two loses the event
  entirely, and the "history" becomes a function of who was listening;
- no `update`, no `delete`: an entry that can change is not history;
- not a fact store: the log carries `FactBatchCommitted { count, digest }`, not
  the facts. Canonical truth lives in one place.

## D2 — `EventKind` is data, not a variant

`enum EventKind` would be reopened by every milestone, would need a migration
when a variant is renamed, and would make an open vocabulary impossible. A kind
is a validated `namespace.name`; e63 publishes the kinds it emits
(`kernel.source_delta`, `kernel.fact_batch_committed`, `analysis.completed`,
`finding.produced`) plus the reserved behavior/policy kinds, as constructors.
Adding a kind is adding a string. An unknown namespace is *allowed*: a plugin
may publish `company.audit_recorded` without touching a core type.

## D3 — Payload: bounded inline, or digest reference

```text
EventPayloadRef::Inline(BoundedEventPayload)   // ≤ 4096 bytes, enforced on mutation
EventPayloadRef::Artifact { digest, media_type, byte_len }
```

The byte budget is checked on **every** mutation, and the fields are a
`BTreeMap` so insertion order cannot change the rendering — two equal payloads
serialize identically, which is what makes replay comparisons meaningful rather
than accidental.

This is the mechanism behind "a million facts do not produce a million events".
Making it an enum means the compiler and the tests hold the rule; a convention
would erode the first time someone is in a hurry.

## D4 — Ids: one global sequence

```text
per-(ws, snap)  — the evidence kernel's choice: keys the canonical truth space
one global      — the log's choice: makes "this cause is another tenant's event"
                  detectable
```

With per-workspace numbering the same `EventId` exists in every workspace, so a
cause pointing at another tenant's event is indistinguishable from a local one
and `causal_chain` would happily walk across a tenant boundary. One sequence
costs nothing (the log is a single ordered history anyway) and buys a real
isolation check.

## D5 — Append is all-or-nothing, and a cause must exist

A causal log whose edges can dangle is worse than no log: `causal_chain` would
silently truncate and a "why did this happen?" answer would be wrong without
saying so. `append` therefore validates every event — including its cause —
before writing anything, and `causal_chain` fails loud on a cycle rather than
looping.

A cause may be earlier **in the same batch**: a related group of events is
naturally caused intra-batch, and forcing one append per event would make atomic
recording impossible.

## D6 — `causal_chain` returns root-first

`[SourceDelta, FactBatchCommitted, AnalysisCompleted, FindingProduced]` is the
order in which things happened; that is the order a human reads and the order a
replay compares. The implementation walks tip-to-root (predecessors are the
cheap direction) and reverses once, which is also why the test asserts both the
order *and* each `caused_by` edge — the order alone could be produced by
reversing a coincidence.

## D7 — The recorder never guesses a cause

`record_root` and `record_next` are separate calls, and `record_next` fails with
`NoPreviousEvent` when there is nothing to be caused by. A single `record()`
that inferred "caused by the last thing" would produce a *plausible* history
that is not true, and false causal edges are exactly what makes a log useless
for the read-set and replay work that follows.

## D8 — `occurred_at` is supplied by the caller

The domain must not call a clock: a log that cannot be reproduced cannot be
replayed, and replay is the feature. The instant travels with the event like
every other input.

## D9 — `ActorRef` is `(kind, id)`, not a string

M7.3's authority table (PureDerivation / ReactiveAnalysis / AgentBehavior) keys
on the *kind* of actor, and ADR-044 will refuse an agent's extracted fact. A bare
string would throw away the distinction that is about to matter, so the type
carries it from the first event even though nothing enforces it yet.

## D10 — Semantics fixed in the in-memory oracle

Same role the in-memory evidence store plays for the kernel: a small,
obviously-correct implementation that *defines* the port's meaning, so a durable
adapter later has something to be diffed against. It is deliberately not a
sophisticated store, and its tests are written as the specification (ids, atomic
append, cause existence, isolation, immutability, replay determinism).

## D11 — What e63 deliberately does not build

Behaviors, budgets, read sets, scheduling, transport. Each has a home in a later
cut. The risk of building them now is not that they would be wrong; it is that
they would be built on a log whose semantics had not been pinned by tests yet.
