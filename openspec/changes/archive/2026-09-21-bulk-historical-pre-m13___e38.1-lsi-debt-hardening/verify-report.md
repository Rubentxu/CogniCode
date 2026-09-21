```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:39d041c70a2d5c1480954532a62b1a1fed6c53d492a6acb242167ec9a4d368e4
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 15/15
scenarios: 27/27
test_command: cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel
test_exit_code: 0
test_output_hash: sha256:d0ce2e83f237546f4a41f238f789070ef9e5c63c79036aae49010a22323eed63
build_command: cargo check -p cognicode-core --features evidence-kernel,multimodal
build_exit_code: 0
build_output_hash: sha256:20a2c9b9ea7e80b3ec82e317a742707cab1c79326e0dba1c8b548058a34402b0
```

## Verification Report

**Change**: e38.1-lsi-debt-hardening
**Version**: N/A (pure debt/refactor cycle — no spec deltas; verified against the five promoted main specs in `openspec/specs/`)
**Mode**: Standard verify

**Scenario-total note (authoritative counts)**: the orchestrator status stated 24 scenarios, but the five retrieved main specs contain **27 scenarios** (per-spec counts 6+8+4+6+3; the stated sum omitted identity-benchmark's 3). Per the count-from-retrieved-specs rule, this report and the validator admission use **15 requirements / 27 scenarios**.

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 15 |
| Tasks complete | 15 |
| Tasks incomplete | 0 |

### Build & Tests Execution
**Build**: ✅ Passed — all six `cargo check` variants exit 0
```text
cargo check -p cognicode-core                                                 → exit 0
cargo check -p cognicode-core --features evidence-kernel                      → exit 0
cargo check -p cognicode-core --features evidence-kernel,multimodal           → exit 0
cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel,multimodal → exit 0
cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel            → exit 0 (DEAD-1 regression proof)
cargo check -p cognicode-runtime                                              → exit 0
```

**Tests**: ✅ 220 passed / 0 failed / 0 skipped (204 lib + 16 integration), plus 42/42 fixture goldens
```text
cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel            → exit 0, 91 passed
cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel                → exit 0, 15 passed
cargo test -p cognicode-core --lib continuity --features evidence-kernel                 → exit 0, 36 passed
cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel      → exit 0, 46 passed
cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal → exit 0, 6 passed
cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel            → exit 0, 10 passed
cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel       → exit 0, 7 passed
cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel        → exit 0, 7 passed
cargo test -p cognicode-core --test workspace_isolation --features evidence-kernel       → exit 0, 2 passed
just lsi-fixtures check                                                                  → exit 0, PASS — all 42 goldens byte-identical
cargo fmt -p cognicode-core --check                                                      → exit 0
cargo clippy -p cognicode-core --lib --tests --features evidence-kernel                  → exit 0; ZERO findings on e38.1-touched paths;
                                                                                           only the documented pre-existing red
                                                                                           (cognicode-macros ×3, generic_graph.rs:467/479 ×2)
```

Equivalence harness per-fixture scores (fresh, threshold 0.99, printed with `--nocapture`):
```text
fixture 'python-hello'      [scored]:      node_score=1.0000 edge_score=1.0000 kind_score=1.0000 legacy(n=7, e=3)  fact(n=7,  e=3,  unresolved=0)
fixture 'rust-hello'        [scored]:      node_score=1.0000 edge_score=1.0000 kind_score=1.0000 legacy(n=5, e=0)  fact(n=5,  e=0,  unresolved=3)
fixture 'multi-lang-types'  [QUARANTINED]: node_score=0.5969 edge_score=0.0714 kind_score=0.5259 legacy(n=94, e=7) fact(n=112, e=23, unresolved=40)
```
Identity benchmark gates (fresh): `precision=1.0000 (>= 0.95) recall=1.0000 (>= 0.9) line_shift_retention=1.0000 (== 1) move_retention=1.0000 (>= 0.99)`; matcher convention digest `fnv1a64:e79f623705344f98` (UNCHANGED — separate from the identity digest); colliding-names reported separately as ambiguous (matched=1, ambiguous=1). Workspace isolation: "7 ws-a outcomes, 7 ws-b outcomes, occurrences disjoint, collisions=0".

Digest pins (fresh): `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` (the ONE conscious CP-6 re-pin, v2 text states the 1-based line rule and references `SymbolFqn`; re-pin history comment present); `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` untouched; identity_benchmark mirror `E37_FACT_IDENTITY_DIGEST` updated to the new identity pin. Digest separation holds.

**Coverage**: ➖ Not available (no coverage tooling or threshold configured for this crate)

### Spec Compliance Matrix
| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| lsi-m0-baseline: Golden fixture byte-stability | Regeneration is byte-identical | `capture_lsi_fixtures.py --check` (via `just lsi-fixtures check`) — 42/42 byte-identical, exit 0 | ✅ COMPLIANT |
| lsi-m0-baseline: Golden fixture byte-stability | Kernel addition does not change consumer outputs | same command on tree containing e36–e38.1 kernel code — 42/42 byte-identical | ✅ COMPLIANT |
| lsi-m0-baseline: Golden fixture byte-stability | Intended behavior change requires explicit re-baseline | `just lsi-fixtures check` exit 0 (no diff); fail-on-diff branch enforced (exit 1 contract) and RED/GREEN-covered by the script `--self-test` (e36 evidence reused; script untouched this cycle) | ✅ COMPLIANT |
| lsi-m0-baseline: Baseline coverage of critical surfaces | Inventory surfaces report coverage | `just lsi-fixtures check` prints the per-surface covered/uncovered list with criticality (fresh output) | ✅ COMPLIANT |
| lsi-m0-baseline: Benchmark baseline artifact | Baseline artifact is captured | `sandbox/results/lsi-baseline/baseline.json` — valid JSON (benchmarks/env/captured_at_utc/commit/schema_version), produced by the unchanged e36 baseline harness | ✅ COMPLIANT |
| lsi-m0-baseline: Benchmark baseline artifact | Kernel change is compared to baseline | `lsi_bench_baseline.py compare` per-benchmark delta report — persisted `delta.json` (e37 3-run evidence reused; harness + baseline.json unchanged since) | ✅ COMPLIANT |
| projection-equivalence-harness: Declared equivalence contract | All fixtures meet the threshold | `tests/equivalence_harness.rs > all_fixtures_meet_the_threshold` — scored fixtures 1.0000/1.0000/1.0000 | ✅ COMPLIANT |
| projection-equivalence-harness: Declared equivalence contract | Fixture below threshold fails the run | `tests/equivalence_harness.rs > fixture_below_threshold_fails_the_run` | ✅ COMPLIANT |
| projection-equivalence-harness: Quarantine of known-unstable surfaces | Quarantined divergence is excluded and reported | `tests/equivalence_harness.rs > quarantined_divergence_is_excluded_and_reported` — multi-lang-types reported [QUARANTINED] every run | ✅ COMPLIANT |
| projection-equivalence-harness: Quarantine of known-unstable surfaces | Unquarantined divergence fails | `tests/equivalence_harness.rs > unquarantined_divergence_fails` | ✅ COMPLIANT |
| projection-equivalence-harness: Deterministic fact identity | Repeated extraction is identical | `src/application/fact_bridge/batch_builder.rs > repeated_extraction_is_identical` (dedupe per task 4.2 — harness-level duplicate deleted, unit kept and green) | ✅ COMPLIANT |
| projection-equivalence-harness: Deterministic fact identity | Convention change requires explicit re-baseline | `tests/equivalence_harness.rs > convention_change_requires_explicit_re_baseline` + `> identity_convention_states_the_declared_rules` — re-pin consistent with v2 text (1-based line rule + `SymbolFqn`), self-checks pass, goldens/scores byte-identical | ✅ COMPLIANT |
| projection-equivalence-harness: No silent cutover | Gate off serves the legacy path | `cargo check -p cognicode-core` (default) exit 0 + 42/42 goldens byte-identical; kernel submodules (except `SymbolFqn`) cfg-gated | ✅ COMPLIANT |
| projection-equivalence-harness: No silent cutover | Failing harness keeps legacy default | structural: no cutover/source-selection mechanism exists in M2 (bridge reachable only behind the feature gate) + `cargo check -p cognicode-runtime` exit 0 | ✅ COMPLIANT |
| generic-graph-projection: Fact-derived generic graph emission | Facts map to nodes and edges | `src/infrastructure/graph/generic_graph_projection.rs > facts_map_to_nodes_and_edges` | ✅ COMPLIANT |
| generic-graph-projection: Fact-derived generic graph emission | Empty snapshot yields empty projection | `src/infrastructure/graph/generic_graph_projection.rs > empty_snapshot_yields_empty_projection` | ✅ COMPLIANT |
| generic-graph-projection: Fact-derived generic graph emission | Clear and rebuild is equivalent | `src/infrastructure/graph/generic_graph_projection.rs > clear_and_rebuild_is_equivalent` + `tests/equivalence_harness.rs > rebuild_from_reread_facts_equals_prior_projection` | ✅ COMPLIANT |
| generic-graph-projection: Consumers remain FactStore-ignorant | Consumer needs only emitted types | `src/infrastructure/graph/generic_graph_projection.rs > consumer_needs_only_emitted_types` | ✅ COMPLIANT |
| entity-continuity: Tiered deterministic continuity matching | Line shift keeps stable identity | `continuity/matcher.rs > t1_line_shift_keeps_stable_identity` + identity_benchmark case 'line-shift' PASS (retention 1.0000) | ✅ COMPLIANT |
| entity-continuity: Tiered deterministic continuity matching | File move keeps stable identity | `continuity/matcher.rs > t2_file_move_keeps_stable_identity` + cases 'move'/'move-edit' PASS (retention 1.0000) | ✅ COMPLIANT |
| entity-continuity: Tiered deterministic continuity matching | Fingerprint is fact-deterministic | `continuity/fingerprint.rs > fingerprint_is_fact_deterministic` + `matcher.rs > result_is_identical_under_shuffled_fact_and_rename_input` | ✅ COMPLIANT |
| entity-continuity: Ambiguous continuity fails closed | Colliding names fail closed | `matcher.rs > t2_candidates_within_epsilon_fail_closed` + `> ambiguous_is_terminal_and_never_rescued_by_a_lower_tier` + colliding-names case ambiguous=1 reported separately | ✅ COMPLIANT |
| entity-continuity: Fail-closed rename evidence port | Version control unavailable degrades safely | `--lib rename_evidence` suite 10/0 (git-absent/non-repo/unborn-rev/malformed fail-closed cases) | ✅ COMPLIANT |
| entity-continuity: Workspace isolation of continuity | Identical workspaces do not collide | `tests/workspace_isolation.rs > identical_workspaces_do_not_collide` — collisions=0, occurrences disjoint | ✅ COMPLIANT |
| identity-benchmark: Pinned scoring gates and fixture coverage | Missed gate fails the run | `tests/identity_benchmark.rs > missed_gate_fails_the_run` + `> precision_gate_failure_names_gate_and_value` | ✅ COMPLIANT |
| identity-benchmark: Pinned scoring gates and fixture coverage | Ambiguous outcome reported separately | `tests/identity_benchmark.rs > ambiguous_outcome_reported_separately` + `> expected_ambiguous_returning_matched_fails_the_case` | ✅ COMPLIANT |
| identity-benchmark: Separate pinned convention digest | Convention change requires explicit re-pin | `tests/identity_benchmark.rs > convention_change_requires_explicit_re_pin` + `> matcher_convention_states_the_declared_rules` — matcher digest `fnv1a64:e79f623705344f98` unchanged while the identity digest was re-pinned (separation holds) | ✅ COMPLIANT |

**Compliance summary**: 27/27 scenarios compliant

### Debt Resolution (this cycle IS debt work)
| Debt item | Status | Fresh evidence |
|-----------|--------|----------------|
| DEAD-1 bench feature gating | ✅ Resolved | `cargo check --bench fact_bridge_benchmarks` exit 0 with BOTH features AND with `evidence-kernel` alone; dual-gated imports/bench now `#[cfg(feature = "multimodal")]` with split `criterion_group!` arms |
| CP-1 typed `SymbolFqn` | ✅ Resolved | `src/domain/evidence_kernel/symbol_fqn.rs` (160 lines): explicit `from_fact_side`/`from_legacy_side`, right-anchored `parse`/`assemble`; 4 round-trip/base tests green inside the 91-test kernel suite; all FQN sites migrated (parser doc, `symbol.rs`, `extractor.rs`, `parse_fqn` delegation, `view.rs`, harness `normalize_legacy_fqn` re-based) |
| CP-2/OE-9 single kind codec + loud fallback | ✅ Resolved | `symbol_kind_detail.rs`: exhaustive encode (no wildcard arm) + `Option`-returning decode + round-trip test green; silent `=> Unknown` replaced by loud panics at `call_graph_projection.rs:677` and `generic_graph_projection.rs:124`; harness kind-multiset assertions live (`kind_score` folded into min score) |
| CP-3 canonical subject grammar + join test | ✅ Resolved | `lsp_facts.rs` `SubjectIndex` normalization (1-based FQN via context, file-path fallback, lexicographic tie-break); contract grammar documented in `fact_bridge/mod.rs`; `batch_builder.rs > lsp_reference_facts_join_tree_sitter_defines_entities` green (RED→GREEN per apply record) |
| CP-6 single digest re-pin | ✅ Resolved | `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` with re-pin history comment; v2 text states 1-based line rule + references `SymbolFqn`; self-check `identity_convention_states_the_declared_rules` green; goldens 42/42 and scores byte-identical |
| DUP-7 shared callee resolver | ✅ Resolved | `pub(crate) resolve_callee_identity` in `call_graph_projection.rs:817` consumed by `generic_graph_projection.rs:208` (import at :53); tie-break unit tests green (3 resolver tests) |
| Trims (SnapshotId/EntityIdTable/RelationKind::name/import-keeper/dup test/OccurrenceId/facade) | ✅ Resolved | `FromStr`/`ParseSnapshotIdError`/`to_revision`/`is_valid` gone; entity_table reduced to `build`+`get`; `RelationKind::name()` deleted (`ns()` kept); import-keeper test deleted (deletion note at `call_graph_projection.rs:2136`); harness duplicate `repeated_extraction` test deleted; `OccurrenceId::new`/`to_entity` deleted (`from_entity` kept); facade re-exports minimized to `SymbolFqn` + `SymbolKindDetail` |
| Reserved docs | ✅ Resolved | `FactStore::facts_of` and `KernelError::Store` doc comments carry "RESERVED (e36 D4/D6 surface, first consumer pending)" verbatim |
| New debt introduced by the refactor | ✅ None found | Diff grep: zero new `#[allow]`, `todo!`, `unimplemented!`, `FIXME/TODO/XXX` on changed crate paths; no new duplication (dedupe direction only: `fact_projection` wrapper deleted, kind_score folded into min_score); clippy delta on e38.1 paths zero |

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| lsi-m0-baseline (3 reqs) | ✅ Implemented | capture script `--check` (42/42 byte-identical, per-surface coverage list, non-zero exit on diff) + `baseline.json`/`delta.json` artifacts |
| projection-equivalence-harness (4 reqs) | ✅ Implemented | threshold 0.99 declared; quarantine list; FNV-1a-pinned convention v2; feature-gated bridge with no cutover mechanism |
| generic-graph-projection (2 reqs) | ✅ Implemented | `FactGenericGraphProjection` emits `GraphNode`/`GraphEdge` only; rebuild equivalence proven in port tests + harness |
| entity-continuity (4 reqs) | ✅ Implemented | T0–T3 tiers, fail-closed ambiguity, fail-closed rename port, workspace isolation |
| identity-benchmark (2 reqs) | ✅ Implemented | pinned gates (0.95/0.90/1.00/0.99) + separate matcher digest pin, self-checked |

### Coherence (Design)
No `design.md` exists for this change (pure debt/refactor cycle; the proposal carries the approach) — the formal design-coherence dimension is **skipped for absence of the artifact**, and the five apply-phase batch deviations were adjudicated against the proposal's approach instead:

| Decision / deviation | Followed? | Notes |
|----------|-----------|-------|
| Un-gated `evidence_kernel` module decl (SymbolFqn-only default surface) | ✅ Yes | Justified in-module: the identity grammar is shared by legacy+fact paths; gating would force duplication. Only `SymbolFqn`/`SymbolKindDetail` appear in the facade; default check exit 0 proves the surface |
| Typed re-base of `normalize_legacy_fqn` instead of deletion (task 2.1 letter) | ✅ Yes (documented deviation) | Achieves CP-1 intent: typed, shape-gated, declared re-base replaces the blind string bump; scores byte-identical; recorded in TESTING-STATE batch 1 |
| Loud decode as panic-at-call-site | ✅ Yes | `decode` returns `Option` (no silent fallback in the codec); consumers panic with context — "loud" per proposal; behavior delta harness-proven harmless (identity gates 1.0000) |
| Re-pin landed in `equivalence_harness/harness.rs` with `identity_benchmark` mirror updated | ✅ Yes | One conscious re-pin; mirror `E37_FACT_IDENTITY_DIGEST` keeps the digest separation auditable; matcher pin untouched |
| `fact_projection` helper removal + kind-multiset folding | ✅ Yes | Dead wrapper removed; `kind_score` folded into `min_score`/`describe` keeps the declared threshold semantics |

### Issues Found
**CRITICAL**: None.

**WARNING**:
1. Scenario-total discrepancy with the orchestrator status: stated 24 scenarios, actual retrieved specs contain 27 (per-spec 6+8+4+6+3; identity-benchmark's 3 omitted from the stated sum). Envelope uses the actual counts (15/27); future phases (archive/envelope consumers) should use 27.
2. Pre-existing clippy red persists (cognicode-macros ×3; `generic_graph.rs:467/479` ×2) — documented before this change, out of scope; `just lint` remains red.
3. e37 10% perf-gate re-adjudication on a quiet machine remains open (sub-µs benchmark noise; carried WARNING) — not re-run here: this cycle changed no perf-relevant code (byte-identical grammar, dead-code trims), and the compare mechanism evidence is reused.
4. Formal UAT runs (U01–U06/U10/U11/U20–U22) remain deferred (A5) — carried gap, honestly recorded in the UAT catalogue.

**SUGGESTION**:
1. S2 (generic-projection equivalence vs legacy projection) still pending — e39 ledger item; until then keep `multi-lang-types` quarantined (0.5969/0.0714/0.5259, reported every run).
2. CP-4 (id-space guard), CP-5 (tie-break test), DUP-2/3/6 (test-support extraction) deferred to e39 per the RETIREMENT-LEDGER — correctly out of scope here.
3. `lsi-m0-baseline` negative-branch (fail-on-diff) evidence was reused from e36 (script unchanged); consider a fresh `--self-test` run when binaries are next built.

### Verdict
PASS WITH WARNINGS
All 15 requirements / 27 scenarios compliant on fresh runtime evidence (220 tests + 42/42 goldens green, exit 0 everywhere, digest separation holds, every targeted debt item resolved in code, zero new lint findings on changed paths); remaining warnings are carried, pre-existing, or explicitly deferred items — none caused by this change.
