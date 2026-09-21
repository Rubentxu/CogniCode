# Design — cycle e58.1 — trust/policy hardening

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Authority: unforgeable by construction

```
                 (private seal)
DetectorAdmission ──► ExecutionPermit ──► DetectorExecutor::execute(&permit, …)
      ▲                    ▲
      │ admit/promote      │ restore (re-validate + policy)
Raw DetectorIr      AdmittedDetectorRecord (serde)
```

- `ExecutionPermit { admitted: AdmittedDetector, _seal: AdmissionSeal }` —
  private fields, `AdmissionSeal` has no public constructor, no
  `Serialize`/`Deserialize`. External code cannot build one; JSON cannot
  mint one.
- `AdmittedDetector` keeps its fields public (inspection) but is **not**
  serializable and **not** accepted by the executor.
- `restore` is the only deserialization path into a permit, and it applies
  policy: a `Gated` record with no recorded approval becomes `Candidate`.
  (Approval *authenticity* is a governance concern for M9/M13; this is not
  cryptographic.)

## Epistemic authority: backend cannot inflate

```
DetectorBackend::evidence_ceiling()  AST=C  Graph=B  Dataflow=B  Runtime=A  LLM=D
DetectorBackend::run()               → DetectorOutcome (no severity/risk)
        │
        ▼  executor check: kind.class() must NOT be stronger than ceiling
   BackendContractViolation::EvidenceCeilingExceeded
        │
        ▼
DetectorIr::policy { default_severity, default_risk }  → Finding.severity/risk
```

`DetectorMatch` carries neither severity nor risk: the backend observes, the
detector's policy decides. Evidence class is derived from the evidence kind
and capped by the backend ceiling.

## IR rule V9

A detector with any executable step (MATCH/FLOW/EXCLUDE/VERIFY) must declare
at least one capability. An empty `requires` is legal only for a PRODUCE-only
aggregator, so the planner can never hand an unconstrained detector to an
arbitrary backend.

## Digests

`semantic_digest = sha256(id, requires, steps)` (unchanged; authority, name
and policy excluded). `instance_digest = sha256(whole definition)` now
covers `policy`, so a policy change is visible to audit but does not alter
the semantic identity.

## Deliberately open

U42 full causal grounding requires the kernel evidence adapter
(`CausalStep → EvidenceId → Evidence → FactId → Fact → Entity/Snapshot`).
`EvidenceLookup` is the seam; `state.yaml` keeps U42 incomplete.
