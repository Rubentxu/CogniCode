# Design — cycle e59 — graph detector backend

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## WU-0 promotion target

```
ExecutionPermit
      │ promotion_target()
      ▼
PromotionTarget { detector_id, version, semantic_digest, source }
      │ PromotionRequest::for_permit(permit, approver)
      ▼
 ApprovalVerifier  ──►  VerifiedPromotion (sealed)
      │
      ▼  promote(permit, verified)  ⇒ verified.target == permit.promotion_target()
 ExecutionPermit (Gated)
```

| Change | valid approval reuse? |
|--------|-----------------------|
| display-name rename | ✅ (semantic digest unchanged) |
| Candidate → Gated | ✅ (same target) |
| version bump | ❌ new approval |
| logic change | ❌ new approval |
| policy change | ❌ new approval (semantic digest) |
| admission source change | ❌ new approval |

## Graph semantics

```
MATCH  subject            declare observed subjects (informational here)
FLOW   source ->* sink    bounded BFS from source-subject nodes to sink nodes
EXCLUDE path_contains X   drop paths whose nodes include subject X
PRODUCE kind
```

Determinism: source nodes visited in id order, neighbours expanded in id
order, sinks reported in id order.

Evidence: one `GraphPath` (class B) per accepted path; causal chain
`Source → Flow → Sink`, each step carrying the path evidence index.

## Seam unchanged

```
admission → permit → plan(GraphQuery) → GraphBackend
   → DetectorOutcome(GraphPath evidence, DetectorMatch{kind from PRODUCE})
   → EvidenceSink → FindingAssembler → Finding(B)
   → FindingVerifier → FindingGate
```

No branch was added to the executor, assembler or verifier for graph.
`GraphBackend` advertises **exactly** `{GraphQuery}` and ceiling `B`.

## Deferred

- Multi-stage plans (`ExecutionPlan<Vec<Stage>>`) for detectors needing
  capabilities no single backend provides.
- Dropping `DetectorMatch.kind` in favour of the `PRODUCE` kind.
