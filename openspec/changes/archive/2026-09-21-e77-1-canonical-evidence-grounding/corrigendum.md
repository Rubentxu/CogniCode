# Corrigendum — e77 canonical evidence grounding

> Cycle: corrective | Target: e77-lsi-executable-architecture | Phase: corrigendum | Date: 2026-09-17

## Purpose

This document is the **corrigendum** of the e77 first slice
(commits `20764bf5` + `fe8804b1`). It records the trust-boundary
gap that was discovered during e77 closure and the corrective
work performed under the **e77.1** cycle. The original e77
archive-manifest and the e77 commits are **not rewritten**; this
corrigendum is the authoritative reference for the closed
state of M10's first slice.

## What e77 first slice shipped (commit `fe8804b1`)

The first slice of e77 added:

* Typed constraint vocabulary (Layer / Forbidden / Namespace).
* Admission flow with promoted-admitter gate and idempotency.
* An evaluator that emitted **`Finding`** instances with:
  - A minted `DetectorAuthority::Gated` field.
  - A synthetic `EvidenceId` minted via FNV-1a fingerprint of
    `(constraint_id, file_path, line, dependency_path)`.
  - A placeholder `DetectorDigests::of_for_kind` factory.
* A self-host test demonstrating that the evaluator is
  load-bearing.
* A 10-test adversarial E2E matrix.

## The gap (discovered during closure review)

During closure review, three trust-boundary issues were
identified:

1. **Synthetic gate authority**. The evaluator minted
   `DetectorAuthority::Gated` for each emitted finding. Because the
   evaluator was the only authority source, *admission* of a
   constraint was, in effect, equivalent to *gating* on it. This
   collapses the e77 admission gate into a binary switch with no
   intermediate "admitted but ungrounded" state.

2. **Synthetic evidence ids**. The evaluator minted FNV-1a
   fingerprints and used them as `EvidenceId`s. FNV-1a is a
   **deduplication key**, not a canonical anchor. Using it as an
   `EvidenceId` bypasses the canonical EvidenceLookup (e67) and
   lets the architecture surface gate findings without ever
   consulting the EvidenceStore / FactStore.

3. **Placeholder detector digests**. The `DetectorDigests::of_for_kind`
   factory was a placeholder for "what the detector actually saw".
   Any future detector could supply the same value and gate
   findings without canonical backing.

The gap was load-bearing because **e79 (AI Foundation)** is the
next cycle after e77, and e79 introduces LLM-generated
hypotheses that flow through the same admission surface. Without
this corrigendum, e79 would have inherited the synthetic minting
path and could have gated `DetectorAuthority::Gated` on a
hypothesis without canonical evidence.

## The corrective cycle (e77.1)

e77.1 performs the minimum corrective work needed to restore the
trust boundary **without** redesigning the rule model, parser,
or admission flow.

### What e77.1 changes

* **New domain DTO**: `domain::architecture::ArchitectureViolation`.
  It carries the violation identity (a `ViolationId` computed as
  FNV-1a of `(constraint_id, file_path, line, dependency_path)`),
  the constraint it came from, the file/module/line/dependency,
  the from-layer, an optional `GroundingRef`, and a rationale.
  It carries **no** `DetectorAuthority` field, **no** `EvidenceId`
  field, and **no** `DetectorDigests` field.

* **Evaluator return type**: the evaluator's primary output is
  `EvaluationReport { violations, statements_examined, matches }`
  instead of `Vec<Finding>`. The type system now makes it
  impossible for `ArchitectureEvaluator::evaluate` to produce a
  gateable `Finding` by itself — the surface carries no findings
  field.

* **Removed placeholder factory**: `DetectorDigests::of_for_kind`
  is no longer used by the architecture path. The placeholder
  remains in the wider codebase for other detectors (e.g.
  runtime traces) but is not consumed by the architecture module.

* **Canonical grounding bridge**: a new
  `application::architecture::ArchitectureGroundingBridge` converts
  `Vec<ArchitectureViolation>` into `Vec<ProducedEvidence>`. The
  bridge:
  - Maps each violation to a `ProducedEvidence` with kind
    `EvidenceKind::ArchitectureSource` (a new variant added to the
    existing enum).
  - Propagates the violation's optional `GroundingRef` to the
    `ProducedEvidence`'s `grounding` field.
  - Lets the canonical `CanonicalEvidenceWriter` (e67) validate
    the fact, persist a real `EvidenceId`, and route through the
    `FindingVerifier` (e67 / e69).

* **New evidence kind**: `EvidenceKind::ArchitectureSource`. Its
  `EvidenceClass` is `C` (static architectural observation with no
  runtime confirmation). The verifier already treats `C` as
  *partial evidence* — it can gate, but only after canonical
  grounding.

### What e77.1 does **not** change

* The constraint rule model (`LayerDependencyRule`,
  `ForbiddenDependencyRule`, `NamespaceBoundaryRule`) is
  unchanged.
* The `use` parser is unchanged.
* The promoted-admitter admission gate is unchanged.
* The architecture registry is unchanged.
* The M9 promotion authority is unchanged.
* The PolicyGate is unchanged.
* The FindingVerifier is unchanged.

The minimum-scope discipline is enforced by the test surface:
the new `EvaluationReport` type and the bridge together guarantee
that **no path through the architecture module can produce a
gateable `Finding` without going through canonical evidence**.

## Verification

WU3 adds 10 adversarial E2E tests
(`crates/cognicode-core/tests/architecture_e77_1_wu3_canonical_grounding_e2e.rs`)
that encode the load-bearing properties of this corrigendum as
runtime checks:

| Property | Guarantee |
|----------|-----------|
| 1 | An admitted constraint + violation + no grounding cannot gate (the canonical EvidenceLookup rejects ungrounded items). |
| 2 | The evaluator's primary output does **not** mention `EvidenceId` — synthetic ids are no longer minted by the architecture surface. |
| 3 | The evaluator's primary output does **not** mention `DetectorAuthority` — synthetic `Gated` minting is removed. |
| 4 | The evaluator's primary output does **not** mention `DetectorDigest` — the placeholder factory is not used. |
| 5 | A violation that **does** carry a `GroundingRef` produces a `ProducedEvidence` with the same grounding, and a violation without grounding produces ungrounded evidence. |
| 6 | ADR text alone still yields zero violations. |
| 7 | A non-admitted constraint still yields zero violations. |
| 8 | **`ArchitectureEvaluator::evaluate` cannot, by itself, produce a gateable `Finding`** — the type `EvaluationReport` has no `findings` field. |
| 9 | Violation identity is deterministic across runs. |
| 10 | `ViolationId` and `EvidenceId` are distinct types (the runtime check documents that the wrap is structurally possible but semantically wrong; correctness comes from the verifier rejecting ungrounded evidence). |

The 4-test WU0 characterization
(`architecture_e77_1_wu0_gap_characterization_e2e.rs`)
demonstrates the gap and its closure: 3 of the 4 tests failed
before WU1 and pass after WU1.

The regression sweep covers:

* `domain::architecture::` (7 tests).
* `application::architecture::` (16 tests).
* `domain::findings::` (112 tests).
* `findings` (broader, 136 tests).
* `canonical` (6 tests).
* `grounding` (6 tests).
* `promotion` (4 tests).
* `policy` (6 tests).
* `verifier` (16 tests + 1 ignored).
* `evidence_bundle` (e69, 14 tests).
* `policy_gate` (e69, 14 tests).
* `promote` (M9, 6 tests).
* `self_hosting` (e76, 66 tests).
* `architecture_drift_e2e` (10 tests).
* `architecture_self_host_e2e` (2 pass + 1 ignored documentando
  DEBT-SDDK-004).
* `findings_canonical_grounding_e2e` (e67, 10 tests with
  `evidence-kernel` feature).

**Total: 451 tests pass, 0 fail, 2 ignored (both documented as
debts).**

## State of M10 (Executable Architecture) after e77.1

M10's first slice is **closed with corrigendum**. The
capability surface is:

* Typed constraint vocabulary (Layer / Forbidden / Namespace).
* Admission flow with promoted-admitter gate and idempotency.
* An evaluator that produces `ArchitectureViolation` (not
  `Finding`).
* A canonical grounding bridge to `ProducedEvidence`.
* A self-host test proving the evaluator is load-bearing.
* A 10+10 adversarial E2E matrix proving the load-bearing
  properties ("ADR text alone → ZERO violations", "admission
  alone does NOT gate", "no synthetic evidence id", "no
  synthetic gate authority", "no placeholder detector digests",
  "evaluate cannot produce gateable finding by itself").

What is still out of scope (unchanged from e77):

* Persistence adapter.
* Cross-crate checking.
* Pack format / capability advertisement (e78).
* Wire-up to the EvidenceBundle / PolicyGate pipeline (the
  shape is ready; the wire is a follow-on).
* Remediation of the 4 real drifts (DEBT-SDDK-004).

## Forward pointer

The next cycle is **e79 (AI Foundation)**. With the e77.1
corrigendum in place, e79 can introduce LLM-generated
hypotheses that flow through the canonical evidence pipeline
without inheriting the synthetic minting path. The
`EvidenceKind::Hypothesis` variant already exists and maps to
`EvidenceClass::D` (lowest). The canonical writer will route
hypotheses through the same `EvidenceLookup` as architecture
sources, requiring a real fact anchor before the verifier will
accept them.
