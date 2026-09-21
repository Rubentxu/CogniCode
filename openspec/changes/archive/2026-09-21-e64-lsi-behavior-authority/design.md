# Design — cycle e64 — execution identity + behavior authority (M7.2/M7.3, U52)

> Cycle: A-lite | Milestone: M7 | Phase: design | Date: 2026-09-15

## D1 — Identity and authority are separate types

```text
ExecutionContext    which execution this is, and where in history
DetectorAuthority   what a detector execution was allowed to do
BehaviorClass       what a behavior execution was allowed to do
```

Authority is never derived from the context, and never from the actor's kind.
It comes from admission, travels in a permit, and is carried *beside* the
context. That separation is what stops "who ran it" from silently becoming "what
it may do".

## D2 — `trigger_event`, not `caused_by`

```text
SourceDelta event
      │ trigger_event
      ▼
ExecutionContext ──► behavior.started ──caused_by──► policy.behavior_output_rejected
```

`caused_by` is the edge **between two log records**. `trigger_event` says which
event originated an **execution**. They are different relations, and using one
word for both is how a causal graph acquires edges that were never true. The
runtime uses `trigger_event` for `behavior.started` and `caused_by` for
everything after it, so the chain reads in the order things happened.

## D3 — Moving vocabulary beats cloning it

Each of `AnalysisScope`, `ActorRef`, `CorrelationId` and `AdmissionSource` was
introduced by one milestone and needed by the next. Cloning would have produced
two definitions that eventually disagree — the same failure mode that made
`EvidenceGrade` (e62.4) and the namespaced-name grammar (e63) get lifted. All four
moved to `domain::execution` / `domain::trust` with re-export shims, so every old
path still resolves and there is exactly one definition.

## D4 — Two scope questions, one answer required

```text
AnalysisInput.scope        what snapshot were these views projected from
ExecutionRequest.scope     what snapshot does this run claim to execute against
```

Different questions, and a run against A fed with views from B would produce
findings whose ids resolve in the wrong snapshot — the failure mode U42 closed,
one layer up. `ExecutionError::InputScopeMismatch` fails loud before planning and
before any backend runs.

## D5 — The class is not the actor

```text
ActorKind::Agent    ⇒ AgentBehavior        ← rejected
ActorKind::Behavior ⇒ ReactiveAnalysis     ← rejected
```

A human may trigger an `AgentBehavior`; an agent may request a trusted
`ReactiveAnalysis`. Neither transforms the other, so the class belongs to the
*definition* and is fixed by admission. The U52 fixture uses a **human** actor for
the agent-classed behavior precisely to make that independence load-bearing
rather than decorative.

## D6 — Effects, not deliverables

```text
effect                      PureDerivation  ReactiveAnalysis  AgentBehavior
CommitCanonicalFact              yes              no              no
RecordEvidence                   yes             yes             yes
RecordHypothesis                  no             yes             yes
ProposeChange                     no              no             yes
ExecuteAdmittedAnalysis           no             yes             yes
```

Modelling "may fabricate a finding" would open a second path into findings and
undo M6's single seam. A behavior may only ask to *run an admitted analysis*, and
the `ExecutionPermit` that would let it run cannot be minted by a behavior. The
table is asserted row by row (15 cells) so a change to it is a visible decision.

## D7 — Admission, not declaration

```text
BehaviorDefinition ──► BehaviorAdmission ──► BehaviorPermit (sealed)
```

The permit has private fields, a private seal, no public constructor and no
`Serialize` — the `ExecutionPermit` pattern, for the same reason: authority is
minted, never restored from storage by accident.

```text
Builtin | HumanCurated  →  keep the declared class
AiGenerated | Imported  →  AgentBehavior
```

An AI-proposed `PureDerivation` does not error; it runs as an agent and therefore
cannot commit facts. **A declaration never escalates** — the M6 rule "an AI
detector is a `Candidate`", one level up. A definition whose declared class
forbids its own declared effects *is* refused, because admission would downgrade
it anyway and silently surprising its author is worse.

## D8 — Refusal before the adapter, not after

```text
Runtime: for each effect { if !policy.allows(effective_class, kind) { record; continue } sink.apply(effect) }
```

There is no branch in which an unauthorized effect reaches an adapter and is
undone afterwards, so an adapter never has to defend against one. That is what
makes the guarantee structural rather than a promise.

## D9 — A fact *draft*, not a kernel fact

`FactDraft { subject, predicate, object, snapshot }` is deliberately not a kernel
`Fact`: a behavior does not own canonical truth, it asks for it. Keeping the draft
in the behavior domain avoids dragging the gated kernel into it, and the point of
the boundary is that the refusal happens *before* anything could become a fact.

## D10 — What the runtime is not

No subscription matching, scheduler, retry, backoff, workers, parallel behaviors,
durable queue or transport. Each is a later cut, and each would otherwise be built
on a boundary that had not been tested yet.
