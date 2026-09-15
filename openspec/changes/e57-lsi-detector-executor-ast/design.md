# Design — cycle e57 — Detector Executor + AST backend

> Cycle: A-lite | Milestone: M6 | Date: 2026-09-15

## Trust boundary

```
Raw DetectorIr (authority field is untrusted)
        │
        ▼  DetectorAdmission::admit  → always Candidate
   AdmittedDetector { definition(normalised), version, authority, admission }
        │
        ▼  DetectorAdmission::promote(approval)  → Gated   (only path)
   AdmittedDetector (Gated)
        │
        ▼  DetectorExecutor::execute(&AdmittedDetector, …)
```

`admit` overwrites `definition.authority` with `Candidate`, so a definition
submitted as `{ generated_by_llm, authority: Gated }` cannot carry gate
authority into execution. `AdmittedDetector::execution_ref` sources the
authority from the admission record, never from the caller's definition.

## Digest split

| Digest | Inputs | Stable across | Use |
|--------|--------|---------------|-----|
| `semantic_digest` | `(id, requires, steps)` | rename, promote | replay / comparison |
| `instance_digest` | whole definition | — | audit of the instance |

Both SHA-256 (`sha256:<64hex>`). FNV-1a removed.

## Execution seam

```
DetectorExecutor::execute
  1. re-validate admitted.definition            (never trust unvalidated input)
  2. registry.plan(requires)                    → DetectorBackend or PlanError
  3. backend.run(admitted, input)               → DetectorOutcome
  4. sink.record(produced_evidence…)            → Vec<EvidenceId>  (index-aligned)
  5. admitted.execution_ref(execution_id)       → DetectorExecutionRef
  6. FindingAssembler::assemble(…)              → Vec<Finding>
```

Backends return raw facts only. The assembler is the single `Finding` factory,
so AST/graph/dataflow backends cannot diverge on `evidence_class`,
`DetectorExecutionRef`, `EvidenceId`, `origin`, causal chain or finding id.

## Verification split (U42)

```
Finding::validate()                 shape     (no store)
FindingVerifier::verify_for_gate()  truth     (evidence resolves; causal grounding)
FindingVerifier::can_block()        truth ∧ gate ∧ authority
```

## AST backend scope

Modest by design: `MATCH <subject>` over an abstract `AstInput`
(units → constructs). One evidence item + one match per hit. Real detector
metadata (severity/risk) is not in the IR yet, so the backend proposes a
documented default (Warning/Medium); the assembler still owns evidence class.

## Known limitations / follow-ups

- Real tree-sitter extractor producing `AstUnit`/`AstConstruct` (adapter).
- Severity/risk metadata in the IR (`PRODUCE … severity=…`).
- Graph (e58) and dataflow (e59) backends reuse this seam unchanged.
- `EvidenceSink` binds to the kernel `EvidenceStore` in the wiring cycle
  (RETIREMENT-LEDGER C1 renegotiation respected: we did not bake the kernel
  port's caller-assigned-id shape).
