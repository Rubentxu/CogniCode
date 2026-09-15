# Exploration Report — cycle e62.4 — canonical grounding, coherence verification (U42, part 2b)

> Cycle: A-lite | Milestone: M6 | Phase: explore | Date: 2026-09-15

## Trigger

e62.3 put `GroundingRef` on the analysis DTOs and made the kernel allocate
evidence ids. What remained was the part that makes grounding mean something:
the bridge that turns a projection into kernel truth, the read model that
carries that truth back, and a verifier that checks **coherence** instead of
mere reference existence.

Four problems had to be solved together, and each one shapes the others.

### 1. Evidence had to become atomic

The invariant "one evidence id per path" cannot be grounded: a graph path's
truth lives in *relations*, and a dataflow path's in *statements*. A single blob
can only be attributed to one fact, so the causal chain could never be checked.
Each element of a witness now produces its own atom, and a causal step cites the
atom that supports it.

### 2. Partial grounding had to be representable

Some atoms ground, some do not (a synthesised dataflow statement has no single
canonical fact). Compacting the ungrounded ones away would renumber the very
indices the backend already referenced, so `EvidenceBindings` is index-aligned
with `produced_evidence` and reports `Grounded`/`Ungrounded` per index.

### 3. Evidence must be persisted outside the domain

The kernel store is async; the domain forbids `block_on`. `DetectorExecutor`
therefore splits into `prepare` (admission, planner, backend, contracts) and
`finalize` (assemble), with `PreparedExecution` in between holding private
fields so the checks cannot be skipped.

### 4. Persistence had to be atomic and id-free

`append_batch` (e62.3) already gives both: the store allocates ids and commits
the batch or nothing. The bridge never numbers evidence itself.

## What was delivered

| WU | Delivery |
|----|----------|
| WU2 | `EvidenceBindings`; `ProducedEvidence.grounding`; `CausalObservation.fact` removed; per-atom evidence in AST/Graph/Dataflow; graph ambiguity fails closed |
| WU3 | `DetectorExecutor::prepare` / `PreparedExecution::finalize`; `EvidenceSink::persist(&[ProducedEvidence])` |
| WU4 | `CanonicalEvidenceWriter` (fact-authority check, atomic batch, no fabrication) |
| WU5 | `KernelEvidenceReadModel::load` (only I/O) + `EvidenceLookup::resolve`; `EvidenceGrade` lifted ungated |
| WU6 | Coherence verifier + the three adversarial acceptance cases |

## Deviations from the review, and why

- **`CausalObservation.fact` was removed entirely** rather than left in place.
  Keeping it while the assembler overrides it from the binding would leave a
  second, unverified source of truth for the same claim. The fact of a step is
  now *derived* from its evidence; it cannot be stated.
- **`provenance` is not on the domain `EvidenceDescriptor`.** The descriptor is
  the payload of an ungated domain port, and `ProvenanceRecord` lives behind the
  kernel feature. The kernel read model retains provenance and exposes
  `provenance(id)`; verification does not reason about it.
- **`FactDescriptor.subject` is `Option<EntityId>`.** The kernel always attests
  a subject; a pure existence store cannot. The rule is fail-closed: a step that
  names a subject requires the fact to agree.
- **Ambiguous parallel relations leave the hop ungrounded** rather than dropping
  the match: the reachability result is real and worth reporting, it just cannot
  be attributed to an arbitrary fact.

## Deferred (not started)

- `ExecutionPlan<Vec<Stage>>`, real tree-sitter extractor, `LegacyRuleProvenance`
  into `Finding` (needs an execution-context carrier; putting it in the IR would
  corrupt the semantic digest).
- Grounding in the *production* pipeline: M5 and the graph/DAG producers do not
  emit canonical facts yet, so real runs are ungrounded by construction and
  cannot gate. That is the honest state of the world, not a defect of this
  cycle: the fixture fixtures in the acceptance test are what a grounded
  producer will look like.
