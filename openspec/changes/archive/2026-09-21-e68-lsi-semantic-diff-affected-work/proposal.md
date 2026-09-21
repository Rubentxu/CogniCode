# e68 — Semantic Diff + Affected Work

## Status

```text
e67 implementation   CLOSED
e67 verification     PASS
e68 implementation   CLOSED
e68 verification     PASS
e67 SDDK lifecycle   not formally instantiated
e68 SDDK lifecycle   not formally instantiated
```

## Goal

Map a **semantic change in the world** (`Snapshot N → Snapshot N+1`) onto
**which logical work must re-execute**, using the existing e66 execution
read-sets, and emit a structured `SchedulingReason` for every decision.

## Non-goals (deferred to e69+)

- No job execution.
- No CI orchestration (GitHub Actions, Jenkins, etc.).
- No `EvidenceBundle` producer.
- No `PolicyGate`.
- No `why_scheduled` CLI.
- No query-dependency generalization (e.g. `Query(QueryDependency)`,
  `Predicate(RelationKind)`, `EntitySet(...)`).
- No persisted `WorkPlan` — `FactDelta` and `AffectedWorkPlan` are **derived**
  operational values, not canonical Facts. The kernel holds truth about the
  software; the planner derives decisions.
- No event bus, no packs.

## Architectural decisions

1. **Diff granularity is `Fact` semantics**, not `GroundingRef` and not
   `EvidenceDescriptor`. `GroundingRef` only links a projection to a Fact;
   `EvidenceDescriptor` belongs to verification. Identity of the diff is
   `(subject, predicate, object)` only.

2. **`FactId` is snapshot-scoped** and MUST NOT participate in semantic
   equality. Renumbering facts across snapshots MUST produce an empty delta
   when the underlying `(subject, predicate, object)` triples are unchanged.

3. **Modify = `removed + added`**:
   `A --calls--> B` becoming `A --calls--> C` decomposes into
   `removed(A,calls,B) + added(A,calls,C)`. No `Changed` variant needed.

4. **Additions and truncated/incomplete dependency information** MUST produce
   `Unknown` / conservative fallback, NEVER `Unaffected`. The previous
   read-set cannot have read a fact that did not yet exist; missing
   information collapses to conservative fallback.

5. **Logical work vs execution instance** is explicit:
   ```text
   WorkId           = opaque logical identity (do NOT yet build a job taxonomy)
   ExecutionDependency { work: WorkId, execution: ExecutionId, read_set: ReadSet }
   ```
   We never return `ExecutionId(734) must rerun` because the rerun will have
   a different `ExecutionId`. We return `WorkId("e2e-suite:auth") must rerun`.

6. **`SchedulingReason` originates in the planner**, not reconstructed later.
   Reconstructing it downstream is brittle and ungrounded.

7. **`Unknown ≠ Unaffected`**. These are different dispositions. The CLI
   `why_scheduled` (e70) will rely on this distinction.

## Decomposition (three WUs)

### WU1 — Semantic Fact Delta

Pure, deterministic, no I/O. Inputs: snapshot N facts, snapshot N+1 facts.
Output: `FactDelta`.

UAT:

```text
same semantics + different FactIds     → empty delta
one relation added                     → exactly one Added
one relation removed                   → exactly one Removed
object changed                         → one Removed + one Added
input ordering changes                 → identical delta
cross-workspace / invalid snapshot pair → reject
```

### WU2 — Affected Work Planner

Composes `FactDelta` + `ExecutionDependency` list. Uses e66 `ReadSet`
introspection (`contains`, `is_truncated`).

Rules:

```text
removed fact ∈ readset                       → Affected
changed semantic fact previously read        → Affected
unrelated existing fact changed              → Unaffected
added fact                                   → Unknown
truncated readset                            → Unknown
```

Every `Affected`/`Unknown` carries a structured `SchedulingReason`.

### WU3 — Adversarial invariants

Same matrix as the user's spec, including the decisive case:

```text
planner must never turn uncertainty into Unaffected
```

Also:

- cross-workspace or invalid snapshot pair → reject
- unordered input → deterministic identical plan
- same semantic world but renumbered FactIds → EMPTY delta

## Architecture invariants preserved

- `SemanticFactDelta` and `AffectedWorkPlan` are derived values, not canonical
  Facts. Nothing is written to the Evidence Kernel.
- Domain layer unchanged in e68.
- No `Handle::current().block_on` bridge.
- No new authority path.
