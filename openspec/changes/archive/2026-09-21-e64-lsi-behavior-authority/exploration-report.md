# Exploration Report — cycle e64 — execution identity + behavior authority (M7.2/M7.3, U52)

> Cycle: A-lite | Milestone: M7 | Phase: explore | Date: 2026-09-15

## Trigger

e63 gave the system memory: a causal, append-only log it can replay. The next
question in the umbrella's progression is not "what can react" but "**who** ran,
and what was that execution **allowed** to do":

```text
M6    the system can prove something happened
e63   the system remembers why it happened
e64   the system knows who executed and what it was allowed to do
e65   the system limits how much it may do
e66   the system remembers what it read
```

Building reactivity before answering e64 would produce a runtime that is
governed by convention, which is the one thing M6 spent five cycles refusing to
accept.

## Two preflight corrections to e63 (WU0)

Found while looking for the ground to build on:

1. **An event could claim another workspace's scope.** `NewIntelligenceEvent.scope`
   is a public field and `validate` only checked the kind, so
   `append(ws_a, event { scope: ws_b })` was recorded as A's history while the
   event asserted it belonged to B. The store and the event must agree on whose
   history this is.
2. **A scope pinned to `SnapshotId::NONE` was accepted.** Same root cause: with a
   public field, the constructor's check is not a gate.
3. `IntelligenceEvent::depth()` claimed to be "causal depth" and could only ever
   return 0 or 1. A wrong accessor is worse than no accessor, because a caller
   trusts it. Real depth belongs to `causal_chain`, where the chain is walkable.

## The architecture question

```text
                      ExecutionContext
                   ┌─────────────────────┐
                   │ ExecutionId         │
                   │ AnalysisScope       │
                   │ ActorRef            │
                   │ CorrelationId       │
                   │ trigger_event       │
                   └──────────┬──────────┘
                              │
               ┌──────────────┴──────────────┐
               ▼                             ▼
      DetectorExecutionRef          BehaviorExecutionRef
               │                             │
        DetectorAuthority                BehaviorClass
```

> `ExecutionContext` says **which execution this is and where it sits in the
> causal history**. `DetectorAuthority` / `BehaviorClass` say **what that
> execution was allowed to do**. They are not mixed.

Four things in the vocabulary turned out to be general rather than owned by
their first consumer, and were moved rather than cloned:

| Concept | Introduced by | Why it is general |
|---------|---------------|-------------------|
| `AnalysisScope` | M6 findings | M7's log already imported it from `findings::scope` |
| `ActorRef` | M7 event log | identifies an *execution*, not an event |
| `CorrelationId` | M7 event log | groups an operation; an operation outlives its events |
| `AdmissionSource` | M6 admission | M7.3 needs the same trust ladder |

## What was delivered (WU0–WU7)

| WU | Delivery |
|----|----------|
| WU0 | `ScopeWorkspaceMismatch`, `InvalidScope` on the event's scope, `depth()` removed, comment fixed |
| WU1 | `domain::execution` (`AnalysisScope`, `ActorRef`, `CorrelationId`, `ExecutionContext`), `domain::trust` (`AdmissionSource`), all with re-export shims |
| WU2 | `DetectorExecutionRef.context`; `ExecutionRequest`; `AnalysisInput.scope == context.scope` or `InputScopeMismatch` |
| WU3 | `BehaviorClass`, `BehaviorEffectKind`, `BehaviorAuthorityPolicy` (the 15-cell table, asserted row by row) |
| WU4 | `BehaviorDefinition`, `BehaviorAdmission`, sealed `BehaviorPermit`, the downgrade rule |
| WU5/WU6 | Minimal `BehaviorRuntime`, `PolicyViolation`, `policy.behavior_output_rejected` |
| WU7 | U52 over a real fact store, with a positive control |

## The design decision that mattered most

**The acceptance fact is valid.** U52 does not make an agent write a fact with
LLM provenance — the kernel already refuses that (ADR-040), and proving it again
would say nothing about the behavior class. The agent attempts a fact that a
`PureDerivation` would be entirely within its rights to commit, and the *only*
reason it is refused is the class of the behavior that asked. The control proves
it: the same draft from a curated `PureDerivation` is accepted and actually
commits.

## Deliberately not built

Scheduler, subscription matching, retry, backoff, workers, parallel behaviors,
durable queue, transport, budgets (e65), read sets (e66). None is needed to
answer "is the authority boundary real?", and building them first would mean
building them on an unproven boundary.

## Open risk recorded honestly

`BehaviorRuntime` has exactly one consumer shape (a synchronous, deterministic
`Behavior` returning a `Vec<BehaviorEffect>`). The boundary is proved; the
production runtime that will use it — matching events to behaviors, deciding when
to run — does not exist and is not implied by this cycle.
