# Design — cycle e62.2 — analysis scope pinning

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Scope as execution identity

```
AnalysisInput { scope: Some(AnalysisScope), ast, graph, dataflow }
        │
        ▼  DetectorExecutor::execute   (MissingScope otherwise)
DetectorExecutionRef { detector, version, authority, digests, execution_id, scope }
        │
        ▼
Finding.detector.scope
        │
        ▼  FindingVerifier::verify_for_gate
   finding.scope == lookup.scope   else   ScopeMismatch   (checked FIRST)
```

`FactId` and `EvidenceId` are canonical per snapshot, so `(workspace, snapshot,
id)` — not the id alone — identifies a fact. Keeping the scope on the execution
(rather than passing it freely to the verifier) means the finding *carries* the
facts of its provenance, and verification cannot silently use another snapshot.

## Why the check must precede id resolution

If ids were resolved first, a lookup hydrated from the wrong snapshot would
still answer "all ids exist". The failure has to happen before any lookup, so
that a wrong read model can never produce a false "verified".

## `None` scopes

`DetectorExecutionRef.scope` is `Option<AnalysisScope>`:

- production runs always set it (the executor refuses to run without one);
- the legacy `QualityIssue` projection has none, because a legacy row was never
  produced by a scoped analysis run.

Verification compares the two options directly, so an unscoped finding can only
be verified by an unscoped lookup — it cannot borrow a snapshot's authority.

## Next (e62.3)

Grounding (`GroundingRef` on constructs/nodes/edges/statements), atomic causal
evidence, the canonical write bridge (`ProducedEvidence → kernel Evidence`,
async, outside the domain) and the `EvidenceDescriptor` read model with the
full verifier.
