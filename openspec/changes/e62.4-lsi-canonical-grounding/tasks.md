# Tasks — cycle e62.4 — canonical grounding, coherence verification (U42, part 2b)

> Cycle: A-lite | Milestone: M6 | Phase: tasks | Date: 2026-09-15

Requirement: U42 (umbrella `cognicode-living-software-intelligence`). This
evolution adds no spec delta of its own.

## WU2 — atomic causal evidence + indexed bindings

- [x] `EvidenceBinding { Grounded { id, fact }, Ungrounded { reason } }`,
      `EvidenceBindings` (index-aligned, `grounded_ids`, `ungrounded`),
      `GroundingFailure` (`NoFact`, `MissingEvidence`, `MissingFact`,
      `EntityFactMismatch`, `AmbiguousRelation`).
- [x] `ProducedEvidence.fact` → `grounding: Option<GroundingRef>`.
- [x] `CausalObservation.fact` removed.
- [x] Assembler claims only grounded ids; steps derive their fact from the
      binding.
- [x] AST: one atom per construct. Graph: source node + one atom per relation +
      sink node; ambiguous parallel relations fail closed with a diagnostic.
      Dataflow: one atom per statement.
- [x] Tests: bindings/index preservation, de-duplication, ungrounded reasons,
      graph atomicity, graph ambiguity (both directions), extended E2E
      assertions.

## WU3 — prepare / finalize

- [x] `DetectorExecutor::prepare(permit, input, execution_id) -> PreparedExecution`
      (scope check, IR validation, planner, backend, kind + ceiling contracts).
- [x] `PreparedExecution` with private fields; `finalize(bindings)` assembles.
- [x] `execute()` re-implemented as `prepare` + `persist` + `finalize`.
- [x] `EvidenceSink::persist(&[ProducedEvidence]) -> EvidenceBindings`.
- [x] Tests: `prepare_persists_nothing_and_finalize_assembles`,
      `a_scope_pinned_to_the_none_sentinel_is_refused`.

## WU4 — canonical write bridge

- [x] `CanonicalEvidenceWriter::persist` (application, feature-gated).
- [x] `FactStore::get` first; entity hint checked against the canonical subject.
- [x] One atomic `append_batch`; ids allocated by the store.
- [x] Store errors stay errors.
- [x] Test: `a_store_failure_is_an_error_not_an_ungrounded_item`,
      `an_entity_hint_that_contradicts_the_fact_is_refused`,
      `a_missing_fact_is_reported_ungrounded_and_never_gates`.

## WU5 — read model

- [x] `EvidenceGrade` lifted to ungated `domain::kernel_ids` (shim re-export).
- [x] `FactDescriptor`, `FactSlot`, `EvidenceDescriptor`,
      `EvidenceResolution`; `EvidenceLookup::resolve` replaces bare existence.
- [x] `KernelEvidenceReadModel::load` — the only I/O; missing ids and dangling
      facts are recorded, store failures are `Err`.
- [x] `provenance(id)` on the model; provenance kept out of the domain
      descriptor.
- [x] In-memory findings double implements `resolve` honestly (no invented
      subject).

## WU6 — coherence verifier + acceptance

- [x] Evidence: resolves, `grade != Refutes`, not dangling, fact snapshot ==
      scope snapshot.
- [x] Causal step: grounded (evidence **and** fact), in the finding's evidence
      set, evidence grades exactly the claimed fact, subject agrees when named.
- [x] New refusals: `UngroundedCausalStep`, `RefutingEvidence`, `DanglingFact`,
      `SnapshotMismatch`, `FactMismatch`, `SubjectMismatch`.
- [x] Unit tests for each refusal.
- [x] Acceptance `tests/findings_canonical_grounding_e2e.rs` (feature
      `evidence-kernel`): AST + Graph + Dataflow positives through the real
      kernel; adversarial A (cross-snapshot, identical ids), B (fact mismatch),
      C (refuting evidence).

## Deferred

- [ ] `ExecutionPlan<Vec<Stage>>`; real tree-sitter extractor;
      `LegacyRuleProvenance` into `Finding`.
- [ ] Grounding from the production producers (M5, graph/DAG lift) so real runs
      can gate: today they are ungrounded by construction.
