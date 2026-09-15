# Exploration Report — cycle e63 — Intelligence Event Log foundation (M7.1, U50)

> Cycle: A-lite | Milestone: M7 | Phase: explore | Date: 2026-09-15

## Trigger

M6 just finished giving CogniCode canonical truth: facts, evidence, findings,
and a verifier that refuses to gate on anything incoherent. M7's umbrella
contracts ask for reactivity on top of that — and reactivity built directly on
truth is reactivity built on nothing remembered. The first cut therefore gives
the system **memory before it gives it reflexes**:

```text
M6      Facts → Evidence → Finding → Verify
M7.1    Facts → Evidence → Finding
                  └────────────► Event History
M7.2+   Event History → Reactive Runtime → bounded governed behaviors
```

## The boundary that decides the design

Three contracts in the umbrella were enough to fix the shape:

| Contract | Consequence |
|----------|-------------|
| Intelligence Event Log (U50) | append-only, causal, content-addressed payloads |
| Behavior Authority (U52) | actors must be typed now, even before behaviors exist |
| Execution Read Sets (U61) | executions must be nameable and correlated |

and one boundary that had to be written down before any code:

```text
Event Log   = durable causal history   ← this cycle
Event Bus   = delivery mechanism       ← later, and never fused with commit
Reactive Runtime = consumer/executor   ← later
```

The log must survive the process dying between "commit" and "notify", so
`publish(event) → handler` can never be the source of truth. The port therefore
has no `publish` and no `subscribe` at all: it has `append`.

## What had to be decided before writing code

### An enum for `EventKind` would have been wrong

`enum EventKind { FactBatchCommitted, AnalysisStarted, … }` reopens on every
milestone, needs a migration whenever a variant is renamed, and makes an open
vocabulary (`plugin.*`, `company.*`) impossible. A kind is a validated
namespaced name; the kinds e63 emits are published as constructors, so adding a
kind is adding a string.

### A million facts must not produce a million events

`FactBatchCommitted { snapshot, count, digest }` — not `FactCreated × 1_000_000`.
The umbrella already required content-addressed payloads; making the payload an
*enum* of `Inline(bounded)` | `Artifact(digest)` is what turns that requirement
into something the compiler and the tests can hold, instead of a convention.

### Ids are the store's job

The exact lesson from `EvidenceId`: a producer that numbers its own records
collides with the next producer. The log's ids come from the store, and the
choice of **one global sequence** rather than per-workspace (as the evidence
kernel does) is what makes "this cause belongs to another workspace" detectable
at all — with per-workspace numbering the same id exists everywhere.

### `occurred_at` is supplied, not read

A domain that calls a clock cannot be replayed, and a log that cannot be
replayed is not a history. The caller passes the instant.

## What was delivered (WU0–WU8)

- `domain::naming` — the shared `namespace.name` grammar, lifted out of the M6
  findings module (the e56 precedent) so one validator serves both surfaces.
- `EventId` (ungated kernel ids), `CorrelationId`, `EventKind`, `ActorRef`.
- `IntelligenceEvent` (immutable) / `NewIntelligenceEvent`.
- `EventPayloadRef` with the inline budget and the digest reference.
- `IntelligenceEventStore` + `InMemoryEventLog` (the reference oracle).
- `CausalRecorder` (application) used at real boundaries.
- The U50 acceptance test over the real slice.

## Deliberately not built

Behaviors, `BehaviorClass`, budgets, read sets, scheduler, transport,
CloudEvents, an event bus, a consumer registry, projections. Each has a named
home in a later cut (e64 behaviors/U52, e65 budgets, e66 read sets/U61).

## Open risk recorded honestly

The log currently has one producer boundary (the U50 slice) and no durable
adapter. Until a second consumer exists, "replay" is proved only in memory. The
oracle was written to be the thing a durable adapter is diffed against, so the
next cut can add persistence without changing the domain.
