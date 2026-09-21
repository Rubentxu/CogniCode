# Exploration Report — cycle e62.2 — analysis scope pinning (U42, part 1)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e62.1 pinned the kernel's evidence store to the snapshot. The same review point
applies one level up: **the snapshot must be part of a detector execution's
identity**, otherwise:

```text
workspace W
  snapshot A: FactId(1), EvidenceId(1)
  snapshot B: FactId(1), EvidenceId(1)

finding produced in A, read model hydrated from B
   → every id resolves → a false "verified"
```

## What was delivered (part 1 of the user's e62.2 list)

### WU1 — scope pinning

- `AnalysisScope { workspace: WorkspaceId, snapshot: SnapshotId }` in
  `domain::findings`.
- `AnalysisInput.scope` — the views are projected *from* a scope.
- `DetectorExecutor::execute` **requires** it
  (`ExecutionError::MissingScope`): a run that is not pinned could never be
  verified.
- `DetectorExecutionRef.scope` captures it, so the finding carries the scope it
  was produced in (`None` only for the legacy QualityIssue projection).
- `ExecutionPermit::execution_ref(execution_id, scope)`.

### WU4a — scope-first verification (the decisive property)

`EvidenceLookup` now declares the scope it was hydrated for
(`fn scope(&self) -> Option<&AnalysisScope>`), and `FindingVerifier` checks
**scope before resolving any id**:

```text
finding.scope == lookup.scope   else   VerificationError::ScopeMismatch
```

The adversarial test uses the **same** numeric ids in both snapshots:

```text
A: EvidenceId(1) ; B: EvidenceId(1)
finding(scope A) + lookup(scope B)  →  REJECT
finding(scope A) + lookup(scope A)  →  verifies
finding(scope A) + lookup(scope B)  →  can_block == false
```

So the system does not depend on the numbers happening to differ.

## Deliberately remaining for e62.3

- **Grounding metadata** (`GroundingRef` on AST construct, Graph node **and
  edge**, Dataflow statement) so analysis DTOs carry the canonical
  `(EntityId, FactId)` they were projected from; never fabricate a fact.
- **Atomic causal evidence**: one evidence atom per supporting fact, each with
  its own `FactId`, so `CausalStep.fact == resolved Evidence.fact` is literally
  true (replaces "one EvidenceId shared by a whole path" — the exception U42
  authorises).
- **Canonical write bridge**: `ProducedEvidence → kernel Evidence`, persisted
  async outside the domain (no `block_on` in the domain); avoids the "false
  architectural positive" of preloading equivalent evidence by hand.
- **`EvidenceDescriptor` read model** + `KernelEvidenceReadModel::load(...)`,
  keeping `FindingVerifier` sync.
- **Full verifier**: evidence → fact → snapshot + `grade != Refutes` +
  causal membership; `Refutes` must never enable the gate.
- **AST/Graph/Dataflow conformance** and the write-side UAT.
