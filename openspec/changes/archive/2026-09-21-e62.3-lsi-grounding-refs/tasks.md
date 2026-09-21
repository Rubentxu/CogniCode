# Tasks — cycle e62.3 — grounding refs, scope sentinel, evidence allocator (U42, part 2a)

> Cycle: A-lite | Milestone: M6 | Phase: tasks | Date: 2026-09-15

Requirement: U42 (umbrella change `cognicode-living-software-intelligence`).
This evolution adds no spec delta of its own.

## WU0a — reject the invalid snapshot sentinel

- [x] `AnalysisScopeError::InvalidSnapshot`.
- [x] `AnalysisScope::try_new` + `AnalysisScope::is_valid`.
- [x] `ExecutionError::InvalidScope`, raised in `DetectorExecutor::execute`
      before planning.
- [x] Tests: `none_snapshot_is_rejected` (unit),
      `a_scope_pinned_to_the_none_sentinel_is_refused` (executor; also asserts
      no evidence was recorded).

## WU0b — the store allocates evidence ids, atomically

- [x] `NewEvidence { fact, grade, provenance }`.
- [x] `EvidenceStore::append_batch(ws, snap, Vec<NewEvidence>) -> Vec<EvidenceId>`.
- [x] `InMemoryEvidenceStore`: per-`(ws, snap)` id sequence, allocate-then-commit,
      `add` advances the allocator.
- [x] Tests: `append_batch_allocates_unique_ids_per_snapshot`,
      `explicit_add_advances_the_allocator`.

## WU1 — `GroundingRef`, additive

- [x] `domain::findings::grounding::GroundingRef` (Copy, serde,
      `entity_agrees_with`).
- [x] `Option<GroundingRef>` on `AstConstruct`, `GraphNode`, `GraphEdge`,
      `DataflowStatement` (all `#[serde(default, skip_serializing_if)]`).
- [x] All existing literals updated to `grounding: None`; no backend reads it
      yet.
- [x] Tests: `fact_only_grounding_has_no_entity_hint`,
      `entity_hint_is_checked_against_the_canonical_subject`,
      `grounding_round_trips`.

## Deferred (next slice — U42 part 2b)

- [ ] WU2 atomic causal evidence + `EvidenceBindings`.
- [ ] WU3 `prepare()` / `PreparedExecution::finalize()`.
- [ ] WU4 async canonical write bridge.
- [ ] WU5 `KernelEvidenceReadModel::load()` + `EvidenceLookup::resolve()`.
- [ ] WU6 full coherence verifier + AST/Graph/Dataflow conformance + the three
      adversarial UATs (A: scope, B: fact mismatch, C: refuting evidence).
