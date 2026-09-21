# Archive Manifest — cycle e62.3 — grounding refs, scope sentinel, evidence allocator (U42, part 2a)

> Cycle: A-lite | Milestone: M6 | Phase: archive | Date: 2026-09-15

## Cycle identity

| Item | Value |
|------|-------|
| Cycle | e62.3 |
| Milestone | M6 — Findings & Detector IR |
| Requirement | U42 (umbrella: `cognicode-living-software-intelligence`) |
| Path | A-lite |
| Base HEAD | `005bb878` (e62.2 impl) |
| Spec delta | none (evolution carries no duplicate spec delta; requirement ids stay in the umbrella) |

## Commits

| Commit | Kind | Summary |
|--------|------|---------|
| (impl) | `feat(cognicode-core)` | scope sentinel refusal + store-allocated atomic evidence batches + additive `GroundingRef` |
| (archive) | `docs(openspec)` | e62.3 cycle artifacts + state |

## Delivered

### WU0a — invalid snapshot sentinel refused

- `AnalysisScopeError::InvalidSnapshot`, `AnalysisScope::try_new`,
  `AnalysisScope::is_valid`.
- `ExecutionError::InvalidScope` from `DetectorExecutor::execute`, before
  planning and before any backend runs.
- Tests: `none_snapshot_is_rejected`,
  `a_scope_pinned_to_the_none_sentinel_is_refused`.

### WU0b — evidence ids allocated by the store, atomically

- `NewEvidence { fact, grade, provenance }`.
- `EvidenceStore::append_batch(ws, snap, batch) -> Vec<EvidenceId>`:
  per-`(workspace, snapshot)` sequence, allocate-all-then-commit-all.
- `InMemoryEvidenceStore::add` advances the allocator past an explicit id.
- Tests: `append_batch_allocates_unique_ids_per_snapshot`,
  `explicit_add_advances_the_allocator`.

### WU1 — `GroundingRef`, additive

- `domain::findings::grounding::GroundingRef { entity: Option<EntityId>, fact: FactId }`
  with the authority rule `fact = authority / entity = hint` expressed by
  `entity_agrees_with`.
- `Option<GroundingRef>` on `AstConstruct`, `GraphNode`, `GraphEdge`,
  `DataflowStatement`.
- Tests: `fact_only_grounding_has_no_entity_hint`,
  `entity_hint_is_checked_against_the_canonical_subject`,
  `grounding_round_trips`.

## Evidence

| Check | Result |
|-------|--------|
| `cargo test -p cognicode-core --lib domain::findings` | 106 passed |
| `cargo test -p cognicode-core --lib application::findings` | 21 passed |
| `cargo test -p cognicode-core --features evidence-kernel --lib evidence_kernel` | 88 passed |
| `findings_ast_e2e` / `findings_graph_e2e` / `findings_dataflow_e2e` / `findings_axiom_import_e2e` | 6 / 4 / 7 / 3 passed |
| `cargo check --workspace --all-targets` | 0 errors |
| `cargo fmt --all --check` | clean |
| `scripts/check_known_failures.py` | exit 0 — 41 entries, unchanged |

## Durable knowledge learned

- Evidence ids must be allocated by the component that owns snapshot-wide
  uniqueness (the store), not by the producer; per-execution counters are
  structurally wrong because executions repeat within a snapshot.
- Evidence persistence should be atomic per execution, so a mid-batch failure
  cannot leave orphaned evidence that no finding cites.
- `SnapshotId::NONE` is a sentinel that must be refused at the trust boundary,
  not only at the constructor: fixtures legitimately build scopes with the
  infallible constructor.
- Grounding on an analysis projection must distinguish authority (`fact`) from
  navigation hint (`entity`), and must be optional so "ungrounded" is
  representable rather than fabricated.

## Deferred (open work, recorded)

- U42 part 2b: WU2 atomic causal evidence + `EvidenceBindings`; WU3
  `prepare()` / `PreparedExecution::finalize()`; WU4 async canonical write
  bridge; WU5 `KernelEvidenceReadModel::load()`; WU6 full coherence verifier and
  the three adversarial UATs.
- From e62.2: `ExecutionPlan<Vec<Stage>>`, real tree-sitter extractor,
  `LegacyRuleProvenance` into `Finding`.
