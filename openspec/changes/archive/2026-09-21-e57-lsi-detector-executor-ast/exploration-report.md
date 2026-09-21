# Exploration Report — cycle e57 — Detector Executor + AST backend

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

The M6 model (IR, findings, kernel-grounded evidence, causal lineage) is in
place after e52-e56. The pre-condition the reviewer set before building
backends was "a single coherent chain". e57 delivers the first **executable,
safe** traversal of that chain and establishes the mandatory execution seam
for every later backend.

## Purpose (definition of done for this cycle)

> Demonstrate the first executable and safe Detector IR traversal and
> establish the mandatory execution seam for all later backends:
> `Admission → Executor → EvidenceStore → FindingAssembler → Finding → Gate`,
> with an identical `Candidate` detector unable to block.

## Three contract gaps closed by this cycle (from the review)

1. **Authority could be forged at the input.** `DetectorIr.authority` is
   public and deserializable; `DetectorExecutionRef::from_definition` trusted
   it. e57 adds a **DetectorAdmission** boundary: raw definitions are always
   normalised to `Candidate`, and only an explicit `promote(approval)`
   transition yields `Gated`. The executor accepts only an `AdmittedDetector`.
2. **Explanation grounding was shape-only.** `Finding::validate` cannot check
   referential truth. e57 adds `FindingVerifier::verify_for_gate` (evidence
   ids must resolve; causal evidence must belong to the finding). Shape vs
   truth are now distinct (`validate` vs `verify_for_gate`).
3. **Weak digest.** FNV-1a 64 is inadequate for audit/replay identity. e57
   moves to **SHA-256** and **splits** the digest: `semantic_digest`
   `(id, requires, steps)` is stable across authority/name; `instance_digest`
   covers the whole definition.

## Architecture delivered

```
Raw DetectorIr ─► DetectorAdmission ─► AdmittedDetector
                                          │
                        DetectorExecutor ─┴─► BackendRegistry::plan
                                │                    │
                                │                    ▼
                                │              DetectorBackend (AstBackend)
                                │                    │
                                │                    ▼
                                │              DetectorOutcome  (raw facts)
                                ▼
                        EvidenceSink (assigns EvidenceId)
                                │
                                ▼
                    FindingAssembler ──► Finding
                                │
                                ▼
                    FindingVerifier ──► Gate
```

Backends never build `Finding`s: the assembler alone assigns evidence class /
execution ref / evidence ids / origin / causal chain / finding id, so no
backend can diverge or inflate its class.

## Deliberate boundary (honest scope)

The AST backend consumes an abstract `AstInput` (units + constructs). Wiring
a real tree-sitter extractor that produces those constructs is a follow-up;
the backend contract does not change when it lands. The goal of e57 is the
architecture and the safety invariants, not detection coverage — exactly as
agreed ("deliberately modest").

## Files

- `domain/findings/digest.rs` (sha256 + semantic/instance digests)
- `domain/findings/admission.rs` (DetectorAdmission, AdmittedDetector, PromotionApproval)
- `domain/findings/outcome.rs` (EvidenceKind, ProducedEvidence, DetectorMatch, DetectorOutcome)
- `domain/findings/ports.rs` (EvidenceSink, EvidenceLookup)
- `domain/findings/assembler.rs` (FindingAssembler)
- `domain/findings/verifier.rs` (FindingVerifier — U42)
- `domain/findings/execution.rs` (DetectorBackend, BackendRegistry, DetectorExecutor)
- `domain/findings/ast_backend.rs` (AstBackend)
- `infrastructure/findings/in_memory_evidence.rs` (InMemoryEvidenceStore)
- `tests/findings_ast_e2e.rs` (vertical slice)
