# Tasks: E38.1 LSI Debt Hardening

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 600–900 (additions+deletions) |
| Suggested split | PR1 modules → PR2 sites+bench → PR3 subjects → PR4 harness/re-pin → PR5 trims |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Work Units

- U1 identity modules — test: `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`; harness: N/A (domain-only); rollback: revert U1.
- U2 site migration + bench gate — test: `cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel,multimodal`; harness: `just lsi-fixtures check` (42/42); rollback: revert U2.
- U3 subject alignment — test: `cargo test -p cognicode-core --lib fact_bridge`; harness: dual-producer join test; rollback: revert U3.
- U4 harness + re-pin — test: `cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel`; harness: `just lsi-equivalence` + `just lsi-identity`; rollback: revert U4.
- U5 trims — test: `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`; harness: N/A (unit-level proof); rollback: revert U5.

## Phase 1: Identity Modules (U1)

- [x] 1.1 Create `src/domain/evidence_kernel/symbol_fqn.rs`: `SymbolFqn {file, name, line}`; constructors `from_fact_side` (1-based) / `from_legacy_side` (0-based); `assemble`/`parse`; tests: bases + round-trip.
- [x] 1.2 Create `SymbolKindDetail` codec (encode/decode over `SymbolKind`) + exhaustive round-trip test.
- [x] 1.3 Extract `pub(crate)` callee resolver (exact match → lowercase + lexicographic-smallest-FQN tie-break) in `src/infrastructure/graph/`; unit test tie-break.

## Phase 2: Site Migration (U2)

- [x] 2.1 Migrate FQN sites to `SymbolFqn` (byte-identical): `infrastructure/parser/tree_sitter_parser.rs:637`→`domain/aggregates/symbol.rs:24` (0-based); `application/ingest/extractor.rs:92,346` (1-based); `infrastructure/graph/call_graph_projection.rs` `parse_fqn`; `domain/evidence_kernel/continuity/view.rs` `symbol_name_from_fqn`; `infrastructure/graph/generic_graph_projection.rs:53`; drop harness `normalize_legacy_fqn` blind `+1`.
- [x] 2.2 Codec via `SymbolKindDetail`: `application/fact_bridge/tree_sitter_facts.rs:80-105` (forward); `infrastructure/graph/call_graph_projection.rs:794-836` (inverse — silent `_ => Unknown` at :836 → loud fallback, fix comment); `continuity/view.rs:108-113`; two test tables.
- [x] 2.3 Swap `generic_graph_projection.rs:182-192` resolver to shared one (rule = `call_graph_projection.rs:715-724`).
- [x] 2.4 DEAD-1: in `benches/fact_bridge_benchmarks.rs` gate dual-gated imports + generic bench under `multimodal`; criterion groups need both features. Verify: exit 0 both-features; `evidence-kernel` alone → no dual-gated refs.

## Phase 3: Subjects + Harness + Re-pin (U3–U4)

- [x] 3.1 RED: dual-producer join test — `lsp_facts` + tree-sitter, same file; assert entity joins in snapshot view (fails first).
- [x] 3.2 GREEN: document canonical subject grammar in fact-bridge contract docs; normalize subjects in `application/fact_bridge/lsp_facts.rs:75-111` (container → 1-based FQN via context, else fallback); test passes.
- [x] 3.3 Add kind-multiset assertions to equivalence harness scored fixtures.
- [x] 3.4 CP-6: single `PINNED_IDENTITY_DIGEST` re-pin in `tests/identity_benchmark/harness.rs` — text states 1-based line rule + references `SymbolFqn`; self-check assertions; `PINNED_MATCHER_DIGEST` untouched.

## Phase 4: Trims + Scoped Verification (U5)

- [x] 4.1 Delete SnapshotId `FromStr`/`ParseSnapshotIdError`/`to_revision`/`is_valid` (`domain/evidence_kernel/ids.rs:84-146`); `EntityIdTable` accessors (`application/fact_bridge/entity_table.rs:33-56`); `RelationKind::name()` (`domain/evidence_kernel/relation.rs:41-48`); import-keeper test (`call_graph_projection.rs:2136`).
- [x] 4.2 Dedupe `repeated_extraction_is_identical` (keep `batch_builder.rs` unit; drop `tests/equivalence_harness.rs:209-223`); trim `OccurrenceId::new`/`to_entity`; minimize facade re-exports (`domain/evidence_kernel/mod.rs:42-66`).
- [x] 4.3 Mark `FactStore::facts_of` + `KernelError::Store` docs "reserved (e36 D4/D6 surface, first consumer pending)".
- [x] 4.4 Verify scoped: `--lib evidence_kernel --features evidence-kernel`, `--lib fact_bridge`, `--lib continuity`, `--lib call_graph_projection`; equivalence scores 1.0/1.0; `--test identity_benchmark` + `--test workspace_isolation` 1.0; `cargo check -p cognicode-core` ± features; `just lsi-fixtures check` 42/42; `just lsi-equivalence`; `just lsi-identity`; clippy delta zero; fmt. Never full suite.
