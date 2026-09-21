# Archive Manifest — e62.2 — analysis scope pinning (U42, part 1)

> Cycle: A-lite | Milestone: M6 | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e62.2-lsi-analysis-scope` |
| Milestone | **M6** — U42 (part 1 of 2) |
| Final status | **ARCHIVED** |
| Base SHA | `ce2196f4` (post-e62.1) |
| Verify verdict | **PASS** |

## What was delivered

| WU | Fix |
|----|-----|
| WU1 | `AnalysisScope { workspace, snapshot }`; `AnalysisInput.scope`; the executor **requires** it (`MissingScope`); `DetectorExecutionRef.scope` captures it; `execution_ref(execution_id, scope)` |
| WU4a | `EvidenceLookup::scope()`; `FindingVerifier` checks scope **before resolving any id**; `VerificationError::ScopeMismatch` |

## Evidence

- `domain::findings` 101; `application::findings` 21; AST E2E 6; Graph E2E 4;
  Dataflow E2E 7; axiom-import E2E 3.
- Adversarial test with identical ids in two snapshots rejects the wrong-scope
  lookup and cannot block.
- `cargo check --workspace --all-targets` 0 errors; fmt clean; baseline
  unchanged.

## Remaining for U42 (e62.3)

Grounding metadata (`GroundingRef` on AST construct, Graph node **and edge**,
Dataflow statement), atomic causal evidence (one atom per supporting fact, so
`CausalStep.fact == Evidence.fact`), the canonical write bridge
(`ProducedEvidence → kernel Evidence`, async, outside the domain — avoiding a
false architectural positive), the `EvidenceDescriptor` read model +
`KernelEvidenceReadModel::load`, the full verifier (fact → snapshot,
`grade != Refutes`, causal membership) and the AST/Graph/Dataflow conformance
UAT.

## Why no tag

Roadmap work in progress; v1.0.0 tag cuts frozen by the user.
