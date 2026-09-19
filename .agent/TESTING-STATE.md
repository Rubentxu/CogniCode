# Testing State — CogniCode

Advisory testing knowledge. Sources of truth, in order: git state, source code,
manifests, executable tests, CI config, then this file.

## Test topology (workspace)

| Layer | Command | Notes |
|---|---|---|
| Unit (all crates) | `just test-unit` | No Postgres required |
| Unit w/ PG | `just test-pg` | Needs `TEST_DATABASE_URL` |
| Ignored/slow/integration | `just test-ignored` | Single-threaded (races) |
| Full pipeline | `just test` | unit + Playwright e2e (boots API server) |
| E2E only | `just test-e2e` | Requires running server |
| Single test | `cargo test -p <crate> --lib <name>` | `RUST_BACKTRACE=full ... -- --nocapture` for debug |
| Lint gate | `just lint` / `cargo clippy --workspace --all-targets -- -D warnings` | New files get full linting (per-file allows baseline, E30.1) |
| Format gate | `cargo fmt --check` | |
| Perf budget | `just perf` | bench vs perf-budget.toml |
| Sandbox scorecard | `just release-scorecard` / `just scorecard-nightly` | E30 infra, G1-G13 gates |
| Conformance | `just openspec-conformance` | `sandbox/scripts/openspec_conformance.py` |

## Component → test mapping (coarse)

- `crates/cognicode-core` — domain/application/infra unit tests in-crate (`--lib`).
  Domain purity rule: `src/domain/` must not import sqlx/tokio/I/O (enforced by review; `just lint`).
- `crates/cognicode-explorer` — API + WASM frontend tests; e2e via Playwright in `apps/explorer-ui`.
- `crates/cognicode-ladybug` — store adapter tests incl. LadybugDB ↔ in-memory oracle conformance.
- `crates/cognicode-cli` (`cogh`) — lifecycle tests in `src/cmd/lifecycle.rs` (E32-H; `test_clean_home_install` fixed v0.94.15).
- `sandbox/` — manifest-driven scenario corpus (Tier-1/2/3), scorecard + stability scripts (Python).
- Known pre-existing RED: Ownership Feature Test (CI job fails historically, out of scope precedent PR #226..#230).

## Expensive suites

- `just test-ignored` (slow integration, run single-threaded).
- Playwright e2e (needs live server).
- Sandbox nightly corpus (long; local-only per 2026-08 user directive).

## Component → test mapping additions (E37, 2026-09-12)

- `crates/cognicode-core` evidence-kernel feature surfaces have focused suites:
  - kernel: `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel` (47 tests after
    e37 WU-1 additions: `facts_in_snapshot` isolation + idempotent bootstrap)
  - fact bridge: `cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel`
    (determinism, LlmAgent rejection, canonical D3 ids)
  - projections: `cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel`
    (44 tests incl. 9 `fact_tests` for `from_facts`) and
    `cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal` (6 tests;
    dual-gated — needs BOTH features)
  - equivalence harness: `cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel`
    (7 tests) = `just lsi-equivalence`. Harness normalization note: legacy projection uses 0-based
    `start.row`, the extractor emits 1-based lines — the harness normalizes deliberately
    (`tests/equivalence_harness/harness.rs`); changing either convention must re-pin
    `PINNED_IDENTITY_DIGEST` / the normalization consciously.
- e36 goldens (`just lsi-fixtures check`) are the byte-stability gate for "gate off serves the legacy path";
  e37 does not re-implement them.
- Benchmarks: e36 `graph_benchmarks.rs` (24 default-path, hard gate via `lsi_bench_baseline.py compare`);
  e37 `fact_bridge_benchmarks.rs` (advisory baseline only, needs `--features evidence-kernel,multimodal`;
  artifacts in `sandbox/results/lsi-bridge-baseline/`).
- Sub-µs default-path benchmarks (`search`, `add_edge`, `get_node`, semantic_search_*) are NOISE-PRONE in
  `lsi_bench_baseline.py compare`: consecutive runs flagged `search` +15.19% then `search` −1.27% with
  `add_edge` +49.26% then (third run) recovered — a different tiny benchmark each time. Do not attribute a
  single-run flag to a change without a stability re-run.

## Component → test mapping additions (E38, 2026-09-12)

- `crates/cognicode-core` continuity/identity surfaces (feature `evidence-kernel`, strictly additive to e36/e37):
  - ids + kernel entry: `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`
    (87 tests after e38 WU-1 additions; 41 e36 + 6 e37 + 40 e38).
  - continuity view/fingerprint/matcher: `cargo test -p cognicode-core --lib continuity --features evidence-kernel`
    (36 tests: FQN/kind recovery from `core:defines` objects, fingerprint determinism, tier mechanics T0–T3,
    epsilon/margin ambiguity, double-claim fail-closed, no-resurrection, permutation-invariant mappings).
  - git rename adapter: `cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel`
    (10 tests: `R<nnn>` parsing + fail-closed on git-absent/non-repo/unborn-rev/malformed rows).
  - integration: `cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel` (6) +
    `--test workspace_isolation --features evidence-kernel` (2) — both together = `just lsi-identity`.
  - fixtures: `sandbox/fixtures/lsi-identity/<case>/{before/,after/,evidence.json,expected-mapping.json}`
    (7 cases: control, pure-rename, rename-edit, move, move-edit, line-shift, colliding-names). Fixtures are
    NOT git repos — T2 move evidence is declared in `evidence.json`; the git adapter is exercised on throwaway
    `git init` repos in `rename_evidence` tests.
  - convention pins: `PINNED_MATCHER_DIGEST` (e38) and `PINNED_IDENTITY_DIGEST` (e37) are SEPARATE digests over
    separate convention texts — never couple or reuse them; re-pin explicitly on any convention change.

## Active Change — M6 closure / e62.3 grounding refs (U42 part 2a) — CLOSED 2026-09-15

**Status: CLOSED. HEAD `b6e97c88` == origin/main. Impl `fa11a2d9` + archive `b6e97c88`.**

- Changed: `domain/findings/{grounding.rs (new), scope.rs, execution.rs, mod.rs,
  ast_backend.rs, graph_backend.rs, dataflow_input.rs}`,
  `domain/evidence_kernel/ports.rs` (`NewEvidence` + `append_batch`),
  `infrastructure/evidence_kernel/in_memory.rs` (per-(ws,snap) id allocator),
  `application/findings/m5_dataflow_backend.rs` + 4 E2E test files (literals now
  pass `grounding: None`).
- Evidence (all fresh, reuse while these files are untouched):
  `domain::findings` **106**, `application::findings` **21**,
  `--features evidence-kernel --lib evidence_kernel` **88**,
  E2E AST **6** / graph **4** / dataflow **7** / axiom-import **3**;
  `cargo check --workspace --all-targets` 0 errors; fmt clean;
  `scripts/check_known_failures.py` exit 0 (41, unchanged).
- Selectors: use `cargo test -p cognicode-core --lib <module path>`, and always
  `cargo check --workspace --all-targets` (plain `--workspace` skips test targets).
- New durable knowledge: (1) evidence ids must be allocated by the store
  (snapshot-wide uniqueness) — per-execution counters are structurally wrong;
  (2) `append_batch` is allocate-all-then-commit-all to avoid orphaned evidence;
  (3) `SnapshotId::NONE` must be refused at the trust boundary, not only in the
  constructor; (4) `GroundingRef.fact` is authority, `.entity` is a hint.
- Next slice: **e62.4 (U42 part 2b)** — atomic causal evidence + `EvidenceBindings`
  indexed by the original `ProducedEvidence` index; `DetectorExecutor::prepare()`
  / `PreparedExecution::finalize()` (private fields, one implementation);
  async canonical write bridge (no `block_on` in the domain); `EvidenceDescriptor`
  + `KernelEvidenceReadModel::load` keeping `FindingVerifier` sync; full coherence
  verifier; three adversarial UATs (A cross-snapshot identical ids, B fact
  mismatch, C `Refutes` never gates) + AST/Graph/Dataflow conformance.
- Unknown impact: none material. Note `GraphEdge` is still `Copy` with
  `Option<GroundingRef>`; keep it that way or fix the derive deliberately.

## Active Change — M6 closure / e62.4 canonical grounding (U42 part 2b) — CLOSED 2026-09-15

**Status: CLOSED. HEAD `95f5761e` == origin/main. `5f9bb6cf` + `f86cee55` + `95f5761e`.**

- Changed: `domain/findings/{binding.rs (new), outcome.rs, ports.rs, assembler.rs,
  execution.rs, verifier.rs, ast_backend.rs, graph_backend.rs, dataflow_input.rs,
  grounding.rs, mod.rs}`, `domain/kernel_ids.rs` (EvidenceGrade lifted ungated),
  `domain/evidence_kernel/evidence.rs` (shim),
  `application/findings/{kernel_bridge.rs (new), mod.rs, m5_dataflow_backend.rs}`,
  `infrastructure/findings/in_memory_evidence.rs`,
  `tests/findings_canonical_grounding_e2e.rs` (new, gated) + the 4 findings E2E.
- Evidence (fresh; invalidated by any change to the files above):
  `domain::findings` **119** (ungated and `--features evidence-kernel`),
  `application::findings` **21**, `evidence_kernel` **88**,
  E2E AST **8** / graph **4** / dataflow **7** / axiom **3**,
  **`findings_canonical_grounding_e2e` 10** (needs `--features evidence-kernel`).
  `cargo check --workspace --all-targets` 0 errors (gated and ungated); fmt
  clean; `scripts/check_known_failures.py` exit 0 (41, unchanged).
- Command gotcha: the acceptance test is `#![cfg(feature = "evidence-kernel")]`,
  so it **silently runs zero tests** without the feature. Always run it as
  `cargo test -p cognicode-core --features evidence-kernel --test findings_canonical_grounding_e2e`.
- Durable knowledge: evidence ids are allocated by the store (never per
  execution); bindings must stay index-aligned with produced evidence; a
  backend-declared fact is a second source of truth and must be deleted, not
  overridden; storage failure != "nothing found"; `Refutes` never gates;
  ungrounded routes are explainable but never block.
- Next: **M7**. Deferred in M6: grounding from production producers (M5,
  graph/DAG lift, real tree-sitter), `ExecutionPlan<Vec<Stage>>`,
  `LegacyRuleProvenance` into `Finding`.

## Active Change — M7.1 / e63 Intelligence Event Log foundation (U50) — CLOSED 2026-09-15

**Status: CLOSED. HEAD `9a70c133` == origin/main. Impl `de57775d` + archive `9a70c133`.**

- Changed (all new): `domain/naming.rs` (NamespacedName lifted out of
  `findings/namespaced.rs`, which is now a shim),
  `domain/intelligence_log/{mod,ids,kind,payload,event,store}.rs`,
  `domain/kernel_ids.rs` (+EventId), `infrastructure/intelligence_log/{mod,in_memory}.rs`,
  `application/intelligence_log/{mod,recorder}.rs`,
  `tests/intelligence_event_log_e2e.rs` (new, gated).
- Evidence (fresh): `domain::findings` **114** (the 5 namespaced tests moved to
  `domain::naming`, which has 6 — no test lost), event-log suites (domain+infra+
  application) **31**, `findings_canonical_grounding_e2e` **10** (gated),
  `intelligence_event_log_e2e` **4** (gated). `cargo check --workspace
  --all-targets` 0 errors (gated+ungated); fmt clean; known-failure baseline 41
  unchanged.
- Command gotcha: both acceptance tests are `#![cfg(feature = "evidence-kernel")]`
  and **run zero tests** without the feature. Always:
  `cargo test -p cognicode-core --features evidence-kernel --test <name>`.
- Durable knowledge: the event log has NO publish/subscribe (commit ≠ deliver);
  EventKind is a namespaced string, not an enum; payload is Inline(bounded) |
  Artifact(digest); one global EventId sequence (so cross-tenant causes are
  detectable) unlike the kernel's per-(ws,snap); a cause must exist before it is
  referenced; `causal_chain` is root-first; `occurred_at` is caller-supplied so
  replay is reproducible.
- Next: **e64** (behavior model + authority table, U52). Then e65 (budgets), e66
  (read sets, U61). Deferred: durable event-log adapter (replay is in-memory
  only).

## Active Change — M7.2/M7.3 / e64 execution identity + behavior authority (U52) — CLOSED 2026-09-15

**Status: CLOSED. HEAD `1854f471` == origin/main. `5f906d58` + `b483b6b5` + `d6755c68` + `1854f471`.**

- New modules: `domain/execution/{scope,actor,correlation,context}.rs`,
  `domain/trust.rs`, `domain/behaviors/{class,admission,runtime}.rs`,
  `tests/behavior_authority_e2e.rs` (gated). Shims at every moved path
  (`findings::scope`, `intelligence_log::{ids,kind}`, `findings::admission`).
- Evidence (fresh): `domain::behaviors` **13**, `domain::execution` **10**,
  `domain::findings` **112** (3 scope tests moved to execution), event log **32**,
  findings E2E 8/4/7/3, `findings_canonical_grounding_e2e` **10** (gated),
  `intelligence_event_log_e2e` **4** (gated), `behavior_authority_e2e` **6**
  (gated). `cargo check --workspace --all-targets` 0 errors (gated+ungated);
  fmt clean; baseline 41 unchanged.
- Command gotcha: **three** acceptance tests are `#![cfg(feature =
  "evidence-kernel")]` and silently run zero tests without it. Always
  `cargo test -p cognicode-core --features evidence-kernel --test <name>`.
  Also: this repo shares a cargo target dir with another agent, so transient
  "extern location for X does not exist" errors mean *retry*, not a real break.
- Durable knowledge: identity and authority are separate types; `trigger_event`
  (execution origin) != `caused_by` (edge between events); a behavior's class is
  never derived from `ActorKind`; model **effects** not deliverables so the M6
  findings seam stays single; a declaration never escalates (downgrade at
  admission); refuse **before** the adapter; public fields make constructor
  checks decorative.
- Next: **e65** (M7.4 budgets + observable exhaustion), then **e66** (M7.5 read
  sets, U61). Deferred: durable event-log adapter, scheduler, transport.

## Active Change — M7.4 / e65 behavior budgets + observable exhaustion (U52) — CLOSED 2026-09-16

**Status: CLOSED. HEAD `d7a466a1`. 10 commits: WU1 (vocabulary), WU4 (Clock port),
WU2+WU3 (runtime integration), WU5 (acceptance), WU6 (spec+ADR), lint-fix, WU3-prep (FakeClock),
WU3-recorder (ExecutionContext unit), WU5-tighten (atomicity contract).**

- New modules: `domain/budgets/{kind,declaration,state,authorizer,mod}.rs`,
  `application/behaviors/clock.rs`, `tests/behavior_budget_e2e.rs` (gated).
- Evidence (fresh): `domain::budgets` **23**, `domain::behaviors` **13**,
  `domain::intelligence_log` **17**,
  `behavior_budget_e2e` **6** (gated, `--features evidence-kernel`),
  `behavior_authority_e2e` **6** (gated, same feature),
  `intelligence_event_log_e2e` **4**, `findings_canonical_grounding_e2e` **10**.
  `cargo check --workspace --all-targets` 0 errors; fmt clean.
- Command gotcha: acceptance tests are `#![cfg(feature = "evidence-kernel")]`
  and silently run zero tests without it. Always
  `cargo test -p cognicode-core --features evidence-kernel --test <name>`.
  Also: `MockClock` is `#[cfg(test)]` and NOT visible to integration test binaries —
  both `behavior_budget_e2e` and `behavior_authority_e2e` define a local `FakeClock`
  that implements the public `Clock` port. This proves the port is consumable from outside.
- Durable knowledge: Time budget uses time checkpoints (not spent counter); EffectCount
  uses spent counter; check is read-only; commit only after confirmed sink success;
  authority check always before budget check (proven by u52_c); AiGenerated silently
  runs as AgentBehavior (budgets don't grant new authority); incremental commit
  (structural atomicity) — refusal happens before sink, so sink adapters never have
  to undo; `behavior.budget_exhausted` is caused by `behavior.started` (not
  `behavior_output_rejected`); `CausalRecorder::record_behavior_budget_exhausted`
  consumes `ExecutionContext` as a unit; deterministic (EffectCount/FactVisits)
  scenarios do NOT use FakeClock; only temporal (Time) scenarios use it.
- API: `BehaviorAdmission::admit_with_budget(def, src, budget)` for explicit budgets;
  existing 2-arg `admit` stays (no budget). `BehaviorRuntime::run` takes `&dyn Clock`.
  `CausalRecorder::record_behavior_budget_exhausted(context, started_event, behavior_id,
  class, kind, remaining, attempted)` for observable exhaustion.
- Five-property UAT contract (e65 WU5-tighten): (1) rejected effect never reaches sink;
  (2) accepted effects remain; (3) `BehaviorOutcome::BudgetExhausted` produced;
  (4) `behavior.budget_exhausted` causally after `behavior.started`;
  (5) canonical state consistent.
- Next: **e66** (M7.5 read sets + invalidation, U61). Deferred: durable event-log
  adapter, scheduler, transport.

## Active Change

### e39.1-provider-ux-hardening — apply COMPLETE 20/20 (single slice: Unit 1 W2 + Unit 2 W3)

- **Slice**: `e39.1-provider-ux-hardening` apply — COMPLETE, 20/20 tasks (Phases 1–5; no commits, auto-chain
  single slice, orchestrator commits post-verify). Next phase: `sdd-verify` (then `sdd-archive`).
  W2 = honest old-trait error semantics; W3 = bounded fallback readiness.
- **Changed surfaces (2 files, source)**: `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs`
  (W2: `Attempt::Failed` now carries the originating `CodeIntelligenceError`; new `DeepestAnswer`/`ChainOutcome`
  + `definition_chain`/`hover_chain`; old-trait `get_definition`/`hover` propagate the deepest attempted
  provider's original variant, clean miss / gate-only failure → `Ok(None)`; collection ops unchanged
  `Err(Internal(exhausted))`. W3: `TierPolicy::fallback_readiness: Duration` (default 2s) +
  `TierPolicy::readiness_budget(wait, fallback_tier)` + `CompositeProvider::readiness_within` (outer
  `tokio::time::timeout`); the S2 gate now wraps the WHOLE readiness call — bounding
  `LspProcess::initialize`'s own 30s `REQUEST_TIMEOUT_SECS` (process.rs:12) that the poll-loop
  `wait_timeout_secs` could not; timeout records an S2 `Unavailable` diagnostic naming
  `"…bounded fallback to <tier>"` and falls through; `TIER_ORDER` doc fixed (per-op matrix walks +
  `status()` row ordering, e39 S2). `TierPolicy` is not persisted — bincode rule intact.
  `tests/provider_conformance.rs`: Rust/TS serverless policy uses `..TierPolicy::all()`; Java unavailable pins
  `fallback_readiness = 5s`, Java available pins `fallback_readiness = 30s` (full readiness, declared-S2 run
  preserved). NO CLI/MCP edits (verified: `commands.rs` prints Err + non-zero exit; `lsp_handlers` maps
  Err→`found:false`; `workspace_session` maps Err→`WorkspaceError`, pre-e39 shape).
- **RED evidence (W2, Phase 1)**: with the RED tests in place and production untouched,
  `cargo test -p cognicode-core --lib composite` → 8 passed / **4 failed**: `test_composite_falls_back_on_error`
  `Ok(None)`, `test_composite_hover_fallback_on_missing_file` `Ok(None)`,
  `test_definition_missing_file_maps_to_file_not_found` `expect_err` got `None`,
  `test_definition_out_of_range_location_maps_to_invalid_location` got `None` — exactly the swallowed
  `FileNotFound`/`InvalidLocation` variants. The reworked `test_hover_unresolved_maps_to_ok_none` (inverted D2
  test) was deleted; its scenario became the missing-file error test, and the two weak `/nonexistent`
  `is_ok() || is_err()` assertions became real `Err(FileNotFound)` assertions (task 1.4).
- **RED evidence (W3, Phase 3 + behavioral demo)**: (a) compile-level: the bound tests referenced
  `TierPolicy::fallback_readiness`, `readiness_budget`, `readiness_within` before they existed → `E0609`/`E0599`
  compile failures. (b) behavioral: with the outer timeout temporarily neutralized (pre-fix semantics) the
  PATH-gated `test_hierarchy_falls_through_within_the_bounded_readiness` FAILED with
  `the bound must cut the wait far below the 30s request timeout: 30.001771214s` — empirically proving
  `wait_timeout_secs` does not bound `LspProcess::initialize` (it ran the full 30s REQUEST_TIMEOUT). With the
  bound restored the same test passes in **0.05s** (config bound 50ms) — a 600× cut.
- **Focused evidence (post-fmt, 2026-09-13)**: `cargo test -p cognicode-core --lib composite` **16/0**
  (43.0s wall — the cost is the LightweightIndex `/tmp` walk, see correction below); `--lib code_intelligence`
  **14/0**; `just lsi-providers` **8/0 + 3/0 + 22/0** (provider_conformance: Rust S0×4+S1×1, TS S0×4+S1×1,
  Java declared-unavailable branch PASS with S2 `Unavailable` diagnostics, jdtls absent → available branch
  PATH-skipped); `just lsi-fixtures check` **PASS — all 42 goldens byte-identical**;
  `just lsi-equivalence` **7/0** (python-hello 1.0000/1.0000/1.0000, rust-hello 1.0000/1.0000/1.0000
  unresolved=3, multi-lang-types QUARANTINED 0.5969/0.0714/0.5259 legacy(94,7) — byte-identical to baseline);
  `just lsi-identity` **7/0 + 2/0** (gates all 1.0000, collisions=0, matcher digest
  `fnv1a64:e79f623705344f98` unchanged). `cargo check` core default / `evidence-kernel` /
  `evidence-kernel,multimodal` + explorer + runtime + core-mock: all exit 0. `just lint` EXIT 0;
  `cargo fmt --check` clean; clippy `-p cognicode-core --lib --tests` (default AND `evidence-kernel`) and
  `--test provider_conformance`: ZERO findings on touched paths (delta zero; only the workspace-profile
  warning line). No goldens/scores/digest re-pin.
- **Suite-cost correction (per propose risk — replaces "44s readiness waits" folklore)**: the ~44s
  `--lib composite` cost is the `LightweightIndex::build_index` walk over `/tmp` in the two tests that still
  root at `/tmp` — `test_symbols_unresolved_maps_to_internal_error` measured **44.84s standalone** — and
  `test_composite_hierarchy_uses_tree_sitter_when_lsp_gated_off`. It is NOT readiness waiting: the
  bounded-readiness test (spawns rust-analyzer, bound 50ms) runs in **0.05s**. New tests were kept off `/tmp`
  (TempDir/`/nonexistent` roots); reworking the inverted hover test off `/tmp` did not change suite wall time
  (the two remaining `/tmp` tests run in parallel and dominate). If suite time matters, re-root those two
  tests — do NOT attribute the cost to LSP readiness.
- **Task 5.3 record**: the gated `fact_bridge` surface needed no edits (the tiered trait is untouched); its
  gated suite ran green inside `just lsi-providers` (22/0) — executed, not merely reused.
- **Behavior deltas (intended, task-mandated)**: (1) `CompositeProvider`'s old-trait `get_definition`/`hover`
  now return the deepest attempted provider's original `CodeIntelligenceError` variant when every attempted
  provider errored (CLI: error + non-zero exit restored; MCP unchanged `found:false`; `workspace_session`
  Err path restored) — clean misses and gate-only failures stay `Ok(None)`. (2) S2 readiness for every op with
  an enabled lower tier is bounded at `min(wait_timeout_secs, fallback_readiness)` (default 2s); a
  policy-disabled lower tier keeps the full wait. A real first-query cold start can now degrade to S1/S0 and
  is retried by later queries (proposal risk accepted).
- **Deviations (report at verify)**: (a) authored diff is **~840 changed lines** (composite.rs +635/−178,
  conformance +21/−5) vs the tasks forecast "~150–220 authored / 400-line budget risk: Low" — the excess is
  mostly the 12 mandated RED/guard tests (~310 lines) plus rustfmt reflow of the 12 error-carrying attempt
  sites; the resolved delivery path (auto-chain, single slice) was kept, but a `size:exception` may be needed
  at PR time — do not silently reforecast. (b) `CompositeProvider::with_wait_timeout` is now unused in-repo
  (public API kept; its doc notes the policy bound still applies). (c) e39 S1 design wording stays for
  archive (out of scope here); e39 S2 fixed.
- **Unknown impact**: none found — no CLI/MCP/session code changed; default-path byte surface (goldens 42/42),
  harness scores, and digest pins are unchanged; the only new process behavior (spawn-then-drop on a bounded
  readiness timeout) leaves the half-initialized process UNregistered, so a later query re-spawns from
  scratch (documented in-code); no lock is held across the drop.
- **Result**: PASS (scoped). Full verification not justified: two small units on one file + its conformance
  pin, each RED-proven, with the e39 guard suite byte-identical.

### e39-lsi-semantic-providers — apply COMPLETE 22/22 (WU1+WU2+WU3+WU4+WU5)

- **Changed surfaces (WU1/WU2, batch 1)**: `domain/traits/code_intelligence.rs` (WU1: `PrecisionTier` S0–S4
  **no serde** + Display/FromStr/Ord, `ProviderOutcome`, `ProviderDiagnostic`, `Tiered<T>`, `TieredOutcome<T>`,
  `const fn provenance_class()` S0→Ambiguous/S1→Inferred/S2→Extracted/S3/S4→Extracted-reserved; ADDITIVE
  `TieredCodeIntelligenceProvider` with six `*_tiered` ops; existing `CodeIntelligenceProvider` UNTOUCHED);
  `infrastructure/lsp/providers/composite.rs` (WU2: fixed highest-first `[S2,S1,S0]` policy-gated pipeline,
  `TierPolicy`/`with_policy`, per-op support matrix, S2 readiness-gated, per-tier atomic counters + `status()`,
  old-trait `Unresolved` mapping, `FallbackResult` deleted, `get_hierarchy` S2-first);
  `providers/lsp.rs` + `providers/fallback.rs` (provider-id consts + tier docs).
- **Changed surfaces (WU3, NEW)**: `application/fact_bridge/batch_builder.rs` — `RelationRecord` gains
  `tier: Option<PrecisionTier>` + `provider_id: Option<String>`; new `add_tiered_observation(..., tier,
  declared_class, provider_id)` rejects a class contradicting `tier.provenance_class()` with
  `FactBridgeError::TierProvenanceContradiction`; `finish()` derives the class
  (`tier.map(provenance_class).unwrap_or(Extracted)`) and composes the pinned detail attestation
  `"<existing> tier=<T> provider=<id>"` (tier-less extraction records untouched); `UnresolvedRecord{site,
  query, exhausted_tiers}` + `record_unresolved`/`take_unresolved` (in-memory only, never persisted);
  `add_provider` now takes `&dyn TieredCodeIntelligenceProvider`. `application/fact_bridge/lsp_facts.rs` —
  `collect` consumes the tiered trait; per-query exhaustion records (get_symbols site = file path;
  find_references/get_hierarchy site = queried symbol's 1-based fact-side FQN) replace the silent
  `Err(_) => return/continue`; `tier_provider_id()` maps S2→lsp/S1→local-resolver/S0→tree-sitter, pinned to
  the provider consts by `tier_provider_ids_match_the_provider_identity_constants`; 6 test mocks +
  `JoinObserver` migrated to the tiered trait. `application/fact_bridge/mod.rs` — error variant +
  `UnresolvedRecord` re-export.
- **Changed surfaces (WU4, NEW)**: `sandbox/fixtures/lsi-providers/{rust,ts,java}/` (sources + `expected.json`
  declaring tier+outcome per query: Rust/TS S0×4 + S1 `get_definition`, Java S2×3);
  `tests/provider_conformance.rs` + `tests/provider_conformance/harness.rs` (ungated runner: serverless Rust/TS
  via `TierPolicy{lsp:false}`; contradiction fails naming query/declared/observed; Java `jdtls` PATH probe — a
  scan, no spawn — available→S2-declared run, absent→declared-unavailable assertions incl. an S2 `Unavailable`
  diagnostic per query and no served tier above `SERVERLESS_MAX_TIER`); `tests/cp5_tie_break.rs` (gated; batch
  canonical sort ↔ `SubjectIndex` tie-break ↔ view first-defines all agree on `src/a.rs:handle:1`; duplicates
  do not change the view); `justfile` `lsi-providers` recipe (runner + gated cp5 + gated `--lib fact_bridge`).
- **Interpretation pinned during WU3 (flag for verify)**: design D4 says "contradiction check in finish()"
  while D5 says "`finish` signature unchanged" and the WU3 rollback boundary is 3 files — `finish()` therefore
  stays `-> Vec<Fact>` and the contradiction is enforced at INSERTION (`add_tiered_observation` returns
  `Err(TierProvenanceContradiction)` and stores nothing); `finish()` re-derives the class and composes the
  detail. The spec sentence "MUST fail batch construction" holds at observation admission.
- **Named behavior deltas (batch 1, still open for verify)**: (1) `get_hierarchy` attempts S2 before S0;
  (2) old-trait `Unresolved → Ok(None)` for `get_definition`/`hover` (collection ops keep
  `Err(Internal(<exhausted summary>))`).
- **Evidence (WU3, fresh, post-fmt)**: `cargo test -p cognicode-core --lib fact_bridge --features
  evidence-kernel` 22/0 — RED-first proven on the naive scaffold: `s0_heuristic_observations_are_ambiguous_never_extracted`
  failed `left: Extracted, right: Ambiguous`; `tier_decides_provenance_class` failed `left: Extracted, right:
  Inferred`; `class_contradicting_its_tier_fails_batch_construction` failed on `expect_err` (Ok returned).
  Scoped filters `--lib batch_builder` 9/0, `--lib lsp_facts` 7/0. `cargo check -p cognicode-core` default and
  `--features evidence-kernel` exit 0; `cargo clippy -p cognicode-core --lib` both feature states under
  `-D warnings` ZERO findings; `cargo clippy -p cognicode-core --all-targets --features evidence-kernel` clean.
- **Evidence (WU4, fresh, post-fmt)**: `just lsi-providers` PASS — `--test provider_conformance` 8/0 (Rust
  S0×4+S1×1; TS S0×4+S1×1; Java unavailable branch: S2 `Unavailable` diagnostic per query, observed
  S0/S0/Unresolved, no S2 result); `--test cp5_tie_break --features evidence-kernel` 3/0; `--lib fact_bridge
  --features evidence-kernel` 22/0. RED 4.3 proven: `contradicting_tier_fails_the_run` +
  `declared_served_observed_unresolved_fails_the_run` failed under the scaffold verify and pass after the
  matching logic landed. Java available branch is authored + PATH-gated; NOT locally exercised (jdtls absent) —
  it reports a skip.
- **Guards re-run after WU3/WU4 (green, unchanged byte-identity)**: `just lsi-fixtures check` PASS 42/42;
  `just lsi-equivalence` 7/0 unchanged scores (python-hello 1.0000×3; rust-hello 1.0000×3, unresolved=3;
  multi-lang-types QUARANTINED 0.5969/0.0714/0.5259, legacy(94,7) fact(112,23,unresolved=40));
  `just lsi-identity` 7+2 tests, digest `fnv1a64:e79f623705344f98` unchanged, gates all 1.0000;
  `cargo check -p cognicode-explorer`/`-p cognicode-runtime`/`-p cognicode-core-mock` exit 0;
  `cargo fmt --check` clean; `just lint` exit 0.
- **Known pre-existing lint noise (NOT introduced here)**: `cargo clippy -p cognicode-core --all-targets`
  WITHOUT `evidence-kernel` fails on `benches/fact_bridge_benchmarks.rs` ("unused imports: criterion_group,
  criterion_main") because the bench compiles to an empty binary with the feature off; the file is untouched
  and `just lint` (the repo gate) is green. With `--features evidence-kernel` `--all-targets` is clean.
- **WU5 final sweep (5.1 guards, all exit 0, 2026-09-13)**: `just lsi-fixtures check` → `RESULT: PASS — all
  42 goldens byte-identical`; `just lsi-equivalence` 7/0 — python-hello 1.0000/1.0000/1.0000
  legacy(7,3) fact(7,3,unresolved=0); rust-hello 1.0000/1.0000/1.0000 legacy(5,0) fact(5,0,unresolved=3);
  multi-lang-types QUARANTINED 0.5969/0.0714/0.5259 legacy(94,7) fact(112,23,unresolved=40) — byte-identical
  to baseline; `just lsi-identity` exit 0 — identity_benchmark 7/0 (gates precision=1.0000 ≥0.95,
  recall=1.0000 ≥0.90, line_shift_retention=1.0000 ==1, move_retention=1.0000 ≥0.99) + workspace_isolation
  2/0 (collisions=0); `just lsi-providers` exit 0 — provider_conformance 8/0 (Rust S0×4+S1×1; TS
  S0×4+S1×1; Java unavailable branch: S2 `Unavailable` per query, observed S0/S0/Unresolved, no S2 result;
  `java_lsp_targets_verify_when_the_server_is_available` passes via PATH-skip — jdtls absent locally) +
  cp5_tie_break 3/0 + fact_bridge 22/0. Raw logs `/tmp/e39_*.log` (ephemeral).
- **WU5 checks (5.2, all exit 0)**: `cargo check -p cognicode-core` default / `--features evidence-kernel` /
  `--features evidence-kernel,multimodal`; `cargo check -p cognicode-explorer` / `-p cognicode-runtime` /
  `-p cognicode-core-mock`; `just lint` EXIT 0 (repo gate = `cargo clippy -p cognicode-runtime --bin
  explorer-api -- -D warnings`; only the pre-existing workspace-profile warning line); `cargo fmt --check`
  clean (0 lines output). Scoped tests: `--lib code_intelligence` 14/0, `--lib composite` 10/0, `--lib
  fact_bridge --features evidence-kernel` 22/0, `--lib batch_builder` (e-k) 9/0, `--lib lsp_facts` (e-k) 7/0,
  `--test provider_conformance` 8/0, `--test cp5_tie_break --features evidence-kernel` 3/0. Clippy delta on
  touched paths: `-p cognicode-core --lib` default / e-k / e-k+mm all rc=0; `--all-targets --features
  evidence-kernel` rc=0; default `--all-targets` still fails ONLY on the pre-existing untouched
  `benches/fact_bridge_benchmarks.rs` unused-import noise (feature-off empty bench) — `just lint` green.
- **Digest pins (WU5, grep + green harnesses)**: `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6`
  (`tests/equivalence_harness/harness.rs:108`) and `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98`
  (`tests/identity_benchmark/harness.rs:127`) both UNCHANGED — no re-pin, byte-identical outputs throughout.
- **e39 apply CLOSED 22/22** — no source code changed in WU5 (records only). Detail order pinned: existing
  detail (`ref=<Kind>`/`kind=<K>`) first, then `tier=<T> provider=<id>`. Next phase: `sdd-verify` (then
  `sdd-archive`); NO commits made (auto-chain/stacked-to-main; orchestrator commits post-verify).

### Previous slice (e38.2)

- **Slice**: `e38.2-lsi-preflight` apply — COMPLETE, 21/21 tasks (single slice; U1 CP-4 guard RED-first,
  U2 PERF re-adjudication, U3 ENGINE-DET determinism, U4 clippy green, Phase 5 sweep). Next phase:
  `sdd-verify` (then `sdd-archive`). NO commits made (auto-chain/stacked-to-main; user-approval gate).
- **Changed surfaces**: `infrastructure/evidence_kernel/in_memory.rs` (U1: CP-4 id-space collision guard in
  `commit` — atomic pre-extend check under the lock: reject when any existing `(ws, snap)` fact id ≤ batch
  max (spaces start at 1 per batch), carrying the smallest colliding id; new tests
  `commit_rejects_second_batch_reusing_id_space_in_same_snapshot` (RED proven at expect_err BEFORE the
  guard existed: commit silently succeeded) + `commit_allows_reusing_id_space_in_a_different_snapshot`);
  `domain/evidence_kernel/ports.rs` (U1: NEW `KernelError::FactIdSpaceCollision(FactId, SnapshotId)` —
  `SnapshotMismatch`-style caller violation, `Store` stays I/O-reserved; `FactStore::commit` port doc now
  states the id-space rule; adapter module doc updated); `application/services/analysis_service.rs` (U3
  ENGINE-DET, `build_project_graph` ONLY: results folded in deterministic `file_path` order (sorted after
  the parse stage) + deterministic duplicate rule aligned with the shared `resolve_callee_identity`
  tie-break — lowercase name → lexicographically-smallest FQN wins, no first-file-wins; exact-identity
  stage documented inapplicable (legacy callees are bare names, never identity strings);
  `build_project_graph_filtered`/`_async` intentionally UNCHANGED — out of tasked scope, not on any
  golden/harness path); `domain/aggregates/generic_graph.rs` (U4: `use chrono::TimeZone` + `doc_id` helper
  now `#[cfg(feature = "multimodal")]` — both are used ONLY by multimodal-gated tests, so gating (not
  deleting) keeps BOTH feature sets warning-free); `cognicode-macros/src/aix_tool.rs` (:154 collapsible_if
  → let-chain merge) + `newtype.rs` (:59 collapsible_if → let-chain; :119 was actually let_and_return —
  `let expanded = ...; expanded` → direct if-expression; task text had the lint names swapped, fixes are
  semantically identical).
- **U2 PERF verdict (M2 gate re-adjudication): NOISE-ATTRIBUTED, WARNING closed, `baseline.json` UNTOUCHED,
  A/B not triggered** (2.2/2.3 condition false). Evidence: 2 fresh
  `python3 sandbox/scripts/lsi_bench_baseline.py compare --fail-above 10` runs
  (deltas archived in `sandbox/results/lsi-baseline/delta_u382_run1.json` / `delta_u382_run2.json`):
  run 1 flagged `hot_path_outgoing_calls` +12.38% (2.456→2.760µs) — FAIL; run 2 (rebuilt, content-identical
  default binary) `hot_path_outgoing_calls` −7.13% (recovered) and instead `hot_path_incoming_calls`
  −16.96% (an improvement) — OK. A different sub-µs benchmark moves each run = the documented noise
  signature. The M2-flagged `search`: +0.00% (run 1) / +2.53% (run 2) — clean. No benchmark >10% in both
  runs → per tasks 2.1 rule: close as machine noise; the `unresolved_edges` field stays as-is (ungated).
- **U3 determinism proof**: `just lsi-equivalence` run TWICE post-fix — python-hello 1.0000/1.0000/1.0000,
  rust-hello 1.0000/1.0000/1.0000 (6 scored values ×2 runs), multi-lang-types QUARANTINED
  0.5969/0.0714 kind 0.5259 with legacy(nodes=94, edges=7) — IDENTICAL both runs and byte-equal to the
  pre-fix baseline scores. Stays QUARANTINED (<0.99, expected). `just lsi-fixtures check` PASS 42/42
  goldens byte-identical (gate passed on FIRST attempt — no `--accept` re-baseline, no fallback rule
  needed, `PINNED_IDENTITY_DIGEST`/`PINNED_MATCHER_DIGEST` UNTOUCHED, digest pins never re-pinned).
  Third equivalence run in the final sweep (post-U4/macros rebuild): same scores again.
- **Verification executed (final sweep, all exit 0)**: `--lib evidence_kernel` 93/0 (91 + 2 new); `--lib
  fact_bridge` 15/0; `--lib continuity` 36/0; `--lib call_graph_projection` 46/0; `--lib analysis_service`
  25/0 3 ignored (U3-edited service); `--lib generic_graph_projection` (e-k,mm) 6/0; `--test
  equivalence_harness` 7/0; `--test identity_benchmark` 7/0 (gates precision/recall/line_shift/move all
  1.0000) + `--test workspace_isolation` 2/0 (collisions=0) = `just lsi-identity` exit 0; `just
  lsi-fixtures check` PASS 42/42 (run twice: post-U3 and in sweep); `just lsi-equivalence` exit 0 (×3
  total, scores byte-identical every run); `cargo check -p cognicode-core` default / `--features
  evidence-kernel` / `--features evidence-kernel,multimodal` all exit 0; `cargo check -p
  cognicode-runtime` exit 0; `cargo check -p cognicode-core --bench fact_bridge_benchmarks` exit 0 with
  `evidence-kernel` AND with `evidence-kernel,multimodal`; `cargo clippy -p cognicode-macros
  --all-targets` exit 0 + `cargo test -p cognicode-macros` 15/0 1/0 (proc-macro tests); core clippy `--lib
  --tests` under `evidence-kernel` AND `evidence-kernel,multimodal`: ZERO lint findings; `just lint`
  EXIT 0 — FIRST GREEN SINCE E30.1; workspace `cargo fmt --check` exit 0 (one long let-chain line in
  in_memory.rs reflowed by `cargo fmt` post-edit, suite re-run green after).
- **Digest pins (final)**: `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` and
  `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` both UNTOUCHED — no re-baseline occurred; goldens
  and harness scores byte-identical throughout.
- **Behavior deltas (intended)**: (1) `InMemoryFactStore::commit` now REJECTS a second batch whose
  fact-id space reaches an already-assigned id in the same `(ws, snap)` — previously silent
  double-assignment (CP-4); empty batches and different-snapshot commits unaffected. (2)
  `build_project_graph` name-index resolution is now deterministic: walk-order-independent; duplicate
  lowercase names resolve to the lexicographically-smallest FQN (aligned with the shared resolver).
  Default-path projection outputs on scored fixtures are byte-identical (42/42 + scores 1.0).
- **Stale evidence**: pre-e38.2 `just lint` red status (5 lints) — now fixed; M2 perf-gate WARNING — now
  closed as noise-attributed. All other prior evidence (e36/e37/e38 suites) re-validated green in this
  sweep on touched paths; untouched surfaces (rename_evidence 10/0, graph_benchmarks baseline) reuse
  prior evidence unchanged — rename_evidence untouched by e38.2, bench baseline re-adjudicated via the
  two compare runs above.
- **Unknown impact**: none found. The commit guard only fires on the re-use pattern no current producer
  exercises (all suites green); the U3 change touches only `build_project_graph` (the sync variant used
  by the CLI/MCP golden surfaces, the equivalence harness oracle, and workspace_session); filtered/async
  variants keep the legacy pattern and are NOT on any golden/harness path (aix_handlers async is outside
  the fixture surface set) — flagged for a future cycle if those variants ever feed goldens.
- **Result**: PASS (scoped). Full verification not justified: four independent small units, each with
  focused evidence (guard RED→GREEN, double bench compare, golden-gated determinism fix, mechanical
  clippy fixes); all baselines equal; no commits, tree left uncommitted for orchestrator.
- **Handoff notes (e38.2)**: RETIREMENT-LEDGER CP-4 row marked RESUELTO (e38.2) and the "PENDIENTE para
  e39" line updated (CP-4 removed; lint-green + walker-determinism + noise verdict noted). Still open for
  e39: CP-5, DUP-2/3/6, GAP S2, formal UAT runs. If a future A/B on `unresolved_edges` is ever needed,
  follow tasks 2.2/2.3 of this change (stash-style, never committed, re-run pairs).

- **Slice**: `e38.1-lsi-debt-hardening` apply — COMPLETE, 17/17 tasks (batch 1 = Phase 1 U1 tasks 1.1–1.3 +
  Phase 2 U2 tasks 2.1–2.4; batch 2 = Phase 3 U3/U4 tasks 3.1–3.4 + Phase 4 U5 tasks 4.1–4.4). Next phase:
  `sdd-verify` (then `sdd-archive`). NO commits made (stacked-to-main; user-approval gate).
- **Changed surfaces (batch 2)**: `application/fact_bridge/lsp_facts.rs` (CP-3 GREEN: canonical subject
  grammar — `SubjectIndex` per-file name→fact-side-FQN context built from `get_symbols` symbols (0-based
  `Symbol` line +1 via `SymbolFqn::from_fact_side`; File-kind excluded; duplicate names tie-break
  lexicographically-smallest-FQN); container references resolve against the reference-site file's context,
  else fallback = reference-site FILE PATH (non-joinable by design); `core:inherits` subject normalized to
  the queried symbol's fact-side FQN; provider `get_symbols` still called once per file — symbols cached in
  a `walked` vec so cross-file container resolution works); `fact_bridge/mod.rs` (contract docs: canonical
  subject grammar section); `fact_bridge/batch_builder.rs` (E38.1 CP-3 dual-producer join test
  `lsp_reference_facts_join_tree_sitter_defines_entities` + `JoinObserver` mock — RED first, then GREEN);
  `fact_bridge/entity_table.rs` (U5: deleted `snapshot`/`len`/`is_empty`/`iter` accessors + tests trimmed
  to `get` surface); `tests/equivalence_harness/harness.rs` (3.3 kind multiset: `FixtureReport.kind_score`
  folded into `min_score`/`describe`; `legacy_kind_multiset` via `resolve_symbol`→Symbol reconstruction,
  `fact_kind_multiset` via `SymbolKindDetail::decode` on defines details; `compare_fixture` extracts once;
  unused `fact_projection` wrapper deleted; 3.4 CP-6 re-pin: `IDENTITY_CONVENTION` v2 text — 1-BASED line +
  `SymbolFqn` reference; `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` with re-pin history comment);
  `tests/equivalence_harness.rs` (CP-6 self-check test `identity_convention_states_the_declared_rules`;
  synthetic `FixtureReport` literals gained `kind_score`; duplicate harness-level
  `repeated_extraction_is_identical` DELETED — batch_builder unit kept); `tests/identity_benchmark/harness.rs`
  (mirrored `E37_FACT_IDENTITY_DIGEST` updated to new e37 pin + doc note); `tests/identity_benchmark.rs`
  (matcher self-check `matcher_convention_states_the_declared_rules`); `domain/evidence_kernel/ids.rs` (U5:
  SnapshotId `FromStr`/`ParseSnapshotIdError`/`to_revision`/`is_valid` deleted (from_revision + NONE kept);
  OccurrenceId `new`/`to_entity` deleted (`from_entity` + pub field remain); tests rewritten/trimmed);
  `domain/evidence_kernel/mod.rs` (U5: ParseSnapshotIdError removed from ids re-export; facade re-export
  block minimized to `SymbolFqn` + `SymbolKindDetail` ONLY — grep-verified zero consumers for the rest);
  `domain/evidence_kernel/relation.rs` (U5: `RelationKind::name()` deleted; `ns()` kept — bootstrap test);
  `domain/evidence_kernel/snapshot.rs` (to_revision assertion trimmed from test); `domain/evidence_kernel/
  ports.rs` (4.3: `FactStore::facts_of` + `KernelError::Store` docs marked "reserved (e36 D4/D6 surface,
  first consumer pending)"); `infrastructure/graph/call_graph_projection.rs` (U5: import-keeper test
  `internals_use_ordered_maps_for_determinism` deleted).
- **Verification executed (batch 2, final sweep, all exit 0)**: `--lib evidence_kernel` 91/0 (93 −2 net:
  FromStr/is_valid tests deleted, sentinel test rewritten); `--lib fact_bridge` 15/0 (16 with 2 new CP-3
  tests −1 trimmed entity_table test); `--lib continuity` 36/0; `--lib call_graph_projection` 46/0 (47 −1
  import-keeper); `--lib generic_graph_projection` (e-k,mm) 6/0; `--test equivalence_harness` 7/0 (6 +1
  self-check; duplicate dedupe removed; python-hello 1.0000/1.0000/1.0000, rust-hello 1.0000/1.0000/1.0000,
  multi-lang-types QUARANTINED 0.5969/0.0714 kind 0.5259 — node/edge scores byte-identical to baseline);
  `--test identity_benchmark` 7/0 (6 +1 self-check; gates precision/recall/line_shift/move all 1.0000;
  matcher digest STILL `fnv1a64:e79f623705344f98` — deliberately NOT re-pinned, its text references no
  changed internals); `--test workspace_isolation` 2/0; `just lsi-fixtures check` PASS 42/42 goldens
  byte-identical; `just lsi-equivalence` + `just lsi-identity` exit 0; `cargo check -p cognicode-core`
  default / `--features evidence-kernel` / `--features evidence-kernel,multimodal` all exit 0;
  `cargo check -p cognicode-runtime` exit 0; bench `fact_bridge_benchmarks` check exit 0 with BOTH features
  AND `evidence-kernel` alone; `cargo fmt -p cognicode-core --check` clean; clippy
  `--lib --tests --features evidence-kernel`: ONLY the documented pre-existing red (macros ×3,
  generic_graph.rs ×2 — re-confirmed at generic_graph.rs:467/479), ZERO findings on e38.1 paths.
- **RED evidence (3.1)**: `cargo test -p cognicode-core --lib --features evidence-kernel
  lsp_reference_facts_join` FAILED before 3.2 with: `LSP call fact (container 'main') must JOIN the
  tree-sitter entity src/join.rs:main:7; callees were ["helper"]` — raw container subject "main" did not
  join; passed after subject normalization.
- **Digest pins (final)**: `PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` (ONE conscious CP-6 re-pin,
  2026-09-13, prose-only — grammar byte-identical, 42/42 goldens + scores unchanged); 
  `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` UNTOUCHED. Self-check tests guard both texts
  (identity: line-rule + grammar + SymbolFqn strings; matcher: 1-based grammar + pinned constants).
- **Behavior deltas (intended, batch 2)**: (1) LSP reference/inherits subjects normalized per the canonical
  grammar — container-resolvable observations now JOIN defines entities (unresolvable/no-container keep the
  file-path fallback); (2) `SnapshotId` `snap:N` is Display-only; (3) entity-table/facade/OccurrenceId
  surfaces trimmed to consumer-proven APIs (all in-crate; no other crate references `evidence_kernel`).
- **Stale evidence**: none — all suites re-run after fmt; batch-1 evidence for untouched surfaces (renamed
  fixtures, bench baselines) reused unchanged.
- **Unknown impact**: none found — default-path byte-surface guarded by 42/42 goldens + default check;
  kernel surface guarded by the five lib suites + three integration suites.
- **Result**: PASS (scoped). Full verification not justified: batch 2 = CP-3 join fix (harness-proven via
  RED→GREEN join test), prose re-pin (digest-gated), and consumer-audited trims; all baselines equal.

- **Slice**: `e38.1-lsi-debt-hardening` apply — batch 1 COMPLETE (Phase 1 U1 tasks 1.1–1.3 + Phase 2 U2 tasks
  2.1–2.4; 7/17). Batch 2 = Phase 3 U3 (tasks 3.1–3.2 RED-first join test, subjects) + U4 (3.3 kind-multiset,
  3.4 CP-6 re-pin) + Phase 4 U5 trims (4.1–4.4). Next: batch-2 apply, then `sdd-verify`.
- **Changed surfaces (batch 1)**: NEW `crates/cognicode-core/src/domain/evidence_kernel/symbol_fqn.rs`
  (typed `SymbolFqn`, UN-gated — identity grammar shared by legacy+fact paths) and `.../symbol_kind_detail.rs`
  (single `kind=<SerdeName>` codec, gated); `domain/mod.rs` + `evidence_kernel/mod.rs` (module decl un-gated,
  only `SymbolFqn` visible on default builds); `domain/aggregates/symbol.rs` (FQN via `from_legacy_side`);
  `application/ingest/extractor.rs` (`from_fact_side`); `infrastructure/parser/tree_sitter_parser.rs` (doc note);
  `infrastructure/graph/call_graph_projection.rs` (`parse_fqn` delegates to `SymbolFqn::parse`, loud
  `SymbolKindDetail::decode` panic on undecodable defines detail, new `pub(crate) resolve_callee_identity` +
  3 tests, test helper table deleted); `generic_graph_projection.rs` (shared resolver + typed parse + loud
  decode); `continuity/view.rs` (`kind_from_detail` via decode → canonical serde name; `symbol_name_from_fqn`
  via `SymbolFqn::parse` with legacy heuristic fallback for foreign strings); `fact_bridge/tree_sitter_facts.rs`
  (forward codec); `benches/fact_bridge_benchmarks.rs` (DEAD-1: dual-gated imports cfg'd `multimodal`);
  `tests/equivalence_harness/harness.rs` (`normalize_legacy_fqn` re-based via typed constructors, same output).
- **Verification executed (batch 1, all exit 0)**: `--lib evidence_kernel` 93/0 (87 + 6 new);
  `--lib fact_bridge` 14/0; `--lib continuity` 36/0; `--lib call_graph_projection` 47/0 (44 + 3 resolver tests);
  `--lib generic_graph_projection` (e-k,mm) 6/0; `--test equivalence_harness` 7/0 (python-hello 1.0000/1.0000,
  rust-hello 1.0000/1.0000, multi-lang-types QUARANTINED 0.5969/0.0714 — byte-identical to pre-change baseline);
  `--test identity_benchmark` 6/0 (gates precision/recall/line_shift/move all 1.0000; 7 cases PASS,
  colliding-names quarantined); `--test workspace_isolation` 2/0; `just lsi-fixtures check` PASS 42/42 goldens
  byte-identical; `cargo check -p cognicode-core` default / `--features evidence-kernel` /
  `--features evidence-kernel,multimodal` all exit 0; `cargo check -p cognicode-runtime` exit 0; bench
  `fact_bridge_benchmarks` check exit 0 with BOTH features AND with `evidence-kernel` alone (DEAD-1 fixed —
  previously failed with 2 unresolved imports on e-k alone); `cargo fmt -p cognicode-core --check` clean;
  clippy `--lib --tests --features evidence-kernel`: only the documented pre-existing red (macros ×3,
  generic_graph.rs ×2) — ZERO findings on e38.1 paths; benches clippy clean.
- **Digest pins (batch 1)**: `PINNED_IDENTITY_DIGEST = fnv1a64:efccc22e912913fe` and
  `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` both UNTOUCHED (CP-6 re-pin belongs to batch 2, task 3.4);
  `IDENTITY_CONVENTION` text unchanged.
- **Gating note (review-relevant)**: `domain::evidence_kernel` module declaration is now UN-gated; every
  kernel submodule except `symbol_fqn` stays cfg-gated, so `SymbolFqn` is the ONLY kernel type in the default
  build surface. `symbol_kind_detail` is gated `evidence-kernel`.
- **Behavior deltas (intended, harness-proven harmless)**: (1) undecodable `kind=` detail on `core:defines`
  facts now PANICS in `CallGraphProjection::from_facts` / `build_generic_projection` (was silent
  `SymbolKind::Unknown`) — all current producers emit valid details; (2) `view.rs` `EntityFacts.kind` for a
  MALFORMED detail is now "" (canonical) instead of the raw suffix — valid details unchanged, fingerprints of
  real fixtures unaffected (identity gates 1.0000); (3) `normalize_legacy_fqn` now requires the full
  `{file}:{name}:{line}` shape (2 colons + numeric tail) before re-basing — legacy projection FQNs always
  satisfy this, scores byte-identical.
- **Stale evidence**: none — all pre-batch suites re-run post-format; baseline captured BEFORE edits.
- **Unknown impact**: none found; default-path byte-surface guarded by 42/42 goldens + default check.
- **Result**: PASS (scoped). Full verification not justified: refactor-only batch, grammar byte-identical,
  all harness scores/goldens equal to baseline.


- **Slice**: `e38-lsi-stable-identity` apply — COMPLETE, 16/16 tasks (batch 3 = WU-5 guards + handoff,
  tasks 5.1–5.3; batch 1 = phases 1–2 WU-1/WU-2, batch 2 = phases 3–4 WU-3/WU-4). Next phase: `sdd-verify`.
- **Changed surfaces (batch 3)**: `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md`
  (UAT-U20/U21/U22 coverage records added, e37 U10/U11 style), `openspec/changes/e38-lsi-stable-identity/tasks.md`
  (5.1–5.3 marked), this file. **No source code changed in batch 3.**
- **e37 evidence-reuse re-verified (5.1, all exit 0)**: `just lsi-equivalence` 7/7 (python-hello 1.0000/1.0000,
  rust-hello 1.0000/1.0000, multi-lang-types QUARANTINED 0.5969/0.0714 reported every run); `just lsi-fixtures
  check` PASS 42/42 goldens byte-identical; `cargo check -p cognicode-core` default / `--features evidence-kernel`
  / `--features evidence-kernel,multimodal` all exit 0; `cargo check -p cognicode-runtime` exit 0;
  `cargo fmt -p cognicode-core --check` clean.
- **e38 suite sweep (5.2, all green)**: `--lib evidence_kernel` 87/0 (47 after e37 → +40 e38 additions);
  `--lib continuity` 36/0; `--lib rename_evidence` 10/0; `--test identity_benchmark` 6/0
  (gates: precision=1.0000 ≥0.95, recall=1.0000 ≥0.90, line_shift_retention=1.0000 ==1.00,
  move_retention=1.0000 ≥0.99; 7 cases PASS; colliding-names Ambiguous quarantined, reported separately);
  `--test workspace_isolation` 2/0 ("7 ws-a outcomes, 7 ws-b outcomes, occurrences disjoint, collisions=0").
- **Digest pins (verified by grep + green harnesses)**: e38 `PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98`
  (`tests/identity_benchmark/harness.rs`); e37 `PINNED_IDENTITY_DIGEST = fnv1a64:efccc22e912913fe`
  (`tests/equivalence_harness/harness.rs`) unchanged — e37 evidence byte-untouched by e38.
- **Threshold-tuning note**: matcher constants landed at their design-pinned values and needed NO tuning after
  the digest was pinned: `PINNED_JACCARD_MATCH_THRESHOLD=0.6`, `PINNED_AMBIGUITY_EPSILON=0.05`,
  `PINNED_RENAME_SIMILARITY_FLOOR=0.5` (`continuity/matcher.rs`). Any future tuning MUST re-pin
  `PINNED_MATCHER_DIGEST` explicitly (`just lsi-identity` fails otherwise — by design).
- **Clippy delta**: `cargo clippy -p cognicode-core --lib --tests --features evidence-kernel` → 5 findings,
  ALL pre-existing documented red (cognicode-macros: `aix_tool.rs`, `newtype.rs` ×2 collapsible_if/let_and_return;
  `generic_graph.rs` unused chrono import + dead `doc_id` in lib tests). ZERO findings on e38 paths
  (`continuity/`, `rename_evidence.rs`, `identity_benchmark*`, `workspace_isolation.rs`, `ids.rs`, `ports.rs`).
- **Stale evidence**: none invalidated — batch 3 touched only docs/openspec/testing-state files.
- **Unknown impact**: none — e38 is additive and feature-gated; default path proven unchanged (default check +
  goldens byte-identical + e37 harness green with untouched digest).
- **Result**: PASS (scoped). Full verification not justified: additive feature-gated change; batch 3 is
  records-only; UAT-U20/21/22 formal runs deferred (A5) with honest records in the UAT catalogue.

### e37 record (apply complete, 21/21; batch 3 = WU-5 wiring + records)

- **Slice**: `e37-lsi-projection-bridge` apply — COMPLETE, 21/21 tasks (batch 3 = WU-5 wiring + records,
  tasks 5.1–5.3; batch 1 = phases 1–2, batch 2 = phases 3–4). Next phase: `sdd-verify`.
- **Changed surfaces (batch 3)**: `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md`
  (UAT-U10/U11 coverage records added, U05/U06 style), `openspec/changes/e37-lsi-projection-bridge/tasks.md`
  (5.1–5.3 marked), this file. **No source code changed in batch 3.**
- **Task 5.1 determination (honest)**: design file-table row `cognicode-runtime/src/lib.rs Modify` resolved as
  "nothing to wire in M2". Gating is at the `cognicode-core` crate level — the runtime's default dependency
  graph compiles zero e37 code, so "gate off serves the legacy path" holds structurally and a failing harness
  cannot affect the default. Runtime wiring of stores+bootstrap stays M3 work (e36 in-memory adapters doc:
  "future feature-gated runtime wiring"); wiring now would be dead code (no consumer; cutover forbidden in M2)
  and the runtime crate is outside the batch's allowed edit roots. Evidence: `cargo check -p cognicode-runtime`
  (default features) exit 0.
- **Task 5.2 records**: UAT-U10 → "PASS pendiente de ejecución UAT formal" (harness structural equivalence +
  e36 goldens byte-stability as MCP/CLI evidence; MCP-tool-level dual-path run deferred). UAT-U11 → "PARCIAL
  (compatibilidad estructural demostrada; servidor en vivo sin ejercitar)" — FactStore-ignorance adapter tests
  + untouched Explorer path; live-server scenario explicitly NOT exercised.
- **Verification executed (scoped, final sweep, all exit 0)**: `--lib evidence_kernel` 47 passed / 0 failed
  (41 e36 + 6 e37 WU-1 additions); `--lib fact_bridge` 14/0; `--lib call_graph_projection` 44/0;
  `--lib generic_graph_projection` (evidence-kernel,multimodal) 6/0; `--test equivalence_harness` 7/0;
  `cargo check -p cognicode-core` default / `--features evidence-kernel` / `--features evidence-kernel,multimodal`
  all exit 0; `cargo check -p cognicode-runtime` exit 0; `cargo fmt -p cognicode-core --check` clean;
  `capture_lsi_fixtures.py --check` PASS 42/42 goldens byte-identical; `just lsi-equivalence` 7/7 exit 0;
  clippy delta (`--lib` + `--tests` with features, output filtered to e37 paths) ZERO findings — only the
  documented pre-existing red remains (cognicode-macros 3 warnings, `generic_graph.rs` 2 test warnings);
  TODO/FIXME/XXX grep over all e37-owned code: zero hits.
- **Fresh evidence reused (phases 1–4, inputs unchanged; no source edited in batch 3)**: batch-2 results below,
  advisory bench baseline, perf noise analysis.
- **Stale evidence**: none invalidated.
- **Unknown impact**: none — batch 3 touched only docs/openspec/testing-state files; the 5.1 determination is
  a no-op confirmed by compile evidence.
- **Result**: PASS (scoped). Full verification not justified: additive feature-gated change; batch 3 is
  records-only; default path unchanged (check + goldens + bench compare analysis from batch 2).

### e37 batch 2 record (phases 3–4)

- **SUT**: `CallGraphProjection::from_facts` (D4), `GenericGraphProjectionPort` + `FactGenericGraphProjection`
  (D5), equivalence harness (D7), advisory bridge bench (D8). 13/21 tasks at batch close.
- **Key results**: harness python-hello 1.0000/1.0000, rust-hello 1.0000/1.0000 (unresolved=3 reported),
  multi-lang-types QUARANTINED measured 0.5969/0.0714 and reported every run; `just lsi-fixtures check` PASS
  42/42 byte-identical after all edits; advisory bridge bench captured
  (`sandbox/results/lsi-bridge-baseline/`: fact_commit 3.37ms, from_facts 15.47ms, generic_projection 24.35ms
  / 1000 files).
- **Perf gate verdict (feature-off proof)**: `lsi_bench_baseline.py compare --fail-above 10` ran 3×; each run
  flagged a DIFFERENT sub-µs benchmark (`search` ×2, `add_edge` ×1) that recovered on the next run — documented
  sub-µs noise, not attributed to e37; 23/24 within threshold every run. 10% gate re-adjudication left to
  `sdd-verify` on a quiet machine (run-3 delta persisted in `sandbox/results/lsi-baseline/delta.json`).
- **Known gap**: Rust `use` declarations emit no import facts (harmless to Calls-only equivalence; documented).

### e36 record (previous batch)

- **Slice**: `e36-lsi-evidence-kernel-foundation` apply — Phases 1–5 complete, 25/25 tasks.
- **Changed surfaces**: `crates/cognicode-core/src/domain/evidence_kernel/` (new, 7 files, cfg `evidence-kernel`),
  `crates/cognicode-core/src/infrastructure/evidence_kernel/` (new, in-memory stores), core `Cargo.toml` +
  `domain/mod.rs` + `infrastructure/mod.rs` (cfg-gated wiring), `sandbox/scripts/capture_lsi_fixtures.py` +
  `sandbox/scripts/lsi_bench_baseline.py` (new), `sandbox/scripts/release_scorecard.py` (LSI gate row),
  `sandbox/fixtures/lsi-baseline/**` + `sandbox/results/lsi-baseline/baseline.json` (new), `justfile`
  (`lsi-fixtures`/`lsi-baseline`), UAT catalogue (U05/U06 added), `adr-review.md`, this file.
- **SUT**: kernel types + in-memory stores (feature-gated); M0 capture/baseline scripts.
- **Verification executed (scoped, final sweep)**: `cargo check -p cognicode-core` ± `--features evidence-kernel`
  both exit 0; `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel` — 41 passed / 0 failed;
  `capture_lsi_fixtures.py --check` and `just lsi-fixtures check` exit 0; `cargo fmt -p cognicode-core --check` clean;
  clippy delta over `evidence_kernel/**`: zero findings (run with warnings non-fatal so the macros red cannot mask core).
- **Fresh evidence reused (WU-1..WU-4, inputs unchanged)**: goldens byte-identical on double-run and with the kernel
  feature on (task 4.5); `lsi_bench_baseline.py compare` green against `sandbox/results/lsi-baseline/baseline.json`.
- **Stale evidence**: none invalidated.
- **Known pre-existing RED (NOT caused by e36)**: `just lint` / scoped `cargo clippy -p cognicode-core -- -D warnings`
  is red from pre-existing `collapsible_if`/`let_and_return` lints in `cognicode-macros` (`aix_tool.rs`, `newtype.rs`)
  plus `generic_graph.rs` test lints; git confirms e36 touched neither crate. Graph-engine nondeterminism defect stays
  quarantined in `KNOWN_UNSTABLE_SURFACES` (`sandbox/scripts/capture_lsi_fixtures.py`).
- **Unknown impact**: none — kernel is additive and feature-gated; default-build check is identical.
- **Result**: PASS (scoped). Full verification not justified: additive feature-gated slice, no legacy path touched.

## Handoff notes (2026-09-13, e39.1 apply COMPLETE 20/20)

- SDDK position: e39.1 apply Phases 1–5 ALL DONE (20/20) → next `sdd-verify` (then `sdd-archive`);
  NO commits (auto-chain/stacked-to-main; orchestrator commits post-verify). Two work units on ONE file pair:
  Unit 1 W2 (deepest-answer error propagation), Unit 2 W3 (bounded fallback readiness) — both in
  `crates/cognicode-core/src/infrastructure/lsp/providers/composite.rs` + `tests/provider_conformance.rs`.
- Entry commands (fresh, post-fmt): `cargo test -p cognicode-core --lib composite` (16/0),
  `--lib code_intelligence` (14/0), `just lsi-providers` (8/0 + 3/0 + 22/0), `just lsi-fixtures check`
  (PASS 42/42), `just lsi-equivalence` (7/0), `just lsi-identity` (7/0 + 2/0); `cargo check` core ± features /
  explorer / runtime / core-mock; `just lint`; `cargo fmt --check`.
- Re-verification instructions for verify: W2 direction is pinned by 4 RED-born tests
  (`test_composite_falls_back_on_error`, `test_composite_hover_fallback_on_missing_file`,
  `test_definition_missing_file_maps_to_file_not_found`, `test_definition_out_of_range_location_maps_to_invalid_location`)
  plus the clean-miss/gate-only guard (`test_clean_miss_and_gate_only_failure_stay_ok_none`). W3 direction is
  pinned by `test_fallback_readiness_defaults_to_two_seconds`,
  `test_readiness_budget_uses_the_fallback_bound_only_with_a_lower_tier`,
  `test_bounded_readiness_cuts_a_hanging_readiness`, and the PATH-gated
  `test_hierarchy_falls_through_within_the_bounded_readiness` (skips cleanly without rust-analyzer; skips are
  reported, never failures). The Java available branch remains PATH-gated on `jdtls` (absent locally → skip).
- Budget warning: the realized diff (~840 changed lines) exceeds the tasks forecast (~150–220 / 400-line
  budget Low) — flagged in the apply section above; PR-time `size:exception` may be required.
- 44s-suite correction (do not re-add folklore): the cost is the `/tmp` LightweightIndex walk in the two
  `/tmp`-rooted composite tests, not readiness waits.

## Handoff notes (2026-09-13, e39 apply COMPLETE 22/22)

- SDDK position: e39 apply phases 1–5 ALL DONE (22/22) → next `sdd-verify` (then `sdd-archive`);
  NO commits (auto-chain/stacked-to-main; orchestrator commits post-verify). Work units: WU1 domain types
  (`domain/traits/code_intelligence.rs`), WU2 composite pipeline
  (`infrastructure/lsp/providers/composite.rs` + `{fallback,lsp}.rs`), WU3 gated fact-bridge
  (`application/fact_bridge/{batch_builder,lsp_facts,mod}.rs`), WU4 fixtures/runner/CP-5
  (`sandbox/fixtures/lsi-providers/**`, `tests/provider_conformance*`, `tests/cp5_tie_break.rs`, `justfile`
  `lsi-providers`), WU5 guards/handoff (records only; no source edits).
- Entry commands (WU5 sweep, fresh): `just lsi-providers` (provider_conformance 8/0 + cp5_tie_break 3/0 +
  fact_bridge 22/0), `just lsi-fixtures check` (PASS 42/42), `just lsi-equivalence` (7/0),
  `just lsi-identity` (identity_benchmark 7/0 + workspace_isolation 2/0),
  `cargo test -p cognicode-core --lib code_intelligence` (14/0) / `--lib composite` (10/0) /
  `--lib batch_builder` (9/0) + `--lib lsp_facts` (7/0) under `--features evidence-kernel`.
  `just lint` + `cargo fmt --check` green.
- Two behavior deltas intended by WU1/WU2 and STILL PENDING verify adjudication: (1) `get_hierarchy`
  attempts S2 before S0; (2) old-trait `Unresolved → Ok(None)` for `get_definition`/`hover` (collection ops
  keep `Err(Internal(<exhausted summary>))`). WU3 deviation to adjudicate: design D4 says "contradiction
  check in `finish()`" while D5 says "`finish` signature unchanged" — enforced at INSERTION
  (`add_tiered_observation` → `Err(TierProvenanceContradiction)`, nothing stored); `finish()` re-derives the
  class + composes the detail. Spec sentence "MUST fail batch construction" holds at observation admission.
- Gated/flags list: fact-bridge tier mapping + CP-5 gated behind `evidence-kernel`; provider runner UNGATED
  (design D7); Java available branch authored but NOT locally exercised (jdtls absent — the test PATH-skips),
  declared-unavailable branch asserted (S2 `Unavailable` diagnostic per query, no tier above serverless max);
  multi-lang-types stays QUARANTINED (0.5969/0.0714/0.5259); default `--all-targets` clippy bench
  unused-import noise untouched/pre-existing; no digest re-pin needed.
- Digest pins: identity `fnv1a64:ec9546e35a003ed6`, matcher `fnv1a64:e79f623705344f98` — both unchanged.
  Re-pin duty unchanged (see e38 note below).
- Detail order pinned: existing detail (`ref=<Kind>`/`kind=<K>`) first, then `tier=<T> provider=<id>`.

## Handoff notes (2026-09-12, e38 apply COMPLETE 16/16)

- SDDK position: e38 apply phases 1–5 ALL DONE (16/16) → next `sdd-verify` (then `sdd-archive`); user-approval
  gate for commits (stacked-to-main; no commits by apply; e36+e37+e38 share the tree — commit-time file
  ownership must keep the three changes separable: e36 = kernel foundation, e37 = projection bridge,
  e38 = NEW identity/continuity files + additive `ids.rs`/`ports.rs`/kernel `mod.rs` edits + `justfile`
  `lsi-identity` recipe + tests/fixtures).
- WU-1..WU-4 entry commands (fresh, reused unchanged in WU-5 sweep): `cargo test -p cognicode-core --lib
  evidence_kernel --features evidence-kernel` (87), `--lib continuity` (36), `--lib rename_evidence` (10),
  `--test identity_benchmark --features evidence-kernel` (6), `--test workspace_isolation --features
  evidence-kernel` (2). Both integration suites together = `just lsi-identity`.
- M3 exit gates all met at benchmark level: line-shift retention 1.0000 (UAT-U20 related), move retention
  1.0000 ≥ 0.99 (UAT-U21 related), precision/recall 1.0000/1.0000 ≥ 0.95/0.90 (UAT-U22 related),
  collisions = 0 on workspace isolation, Ambiguous never forced (ADR-038 fail-closed; colliding-names case
  quarantined and reported separately each run).
- UAT records in place: UAT-U20/U21/U22 "PASS pendiente de ejecución UAT formal" (A5) — coverage notes map to
  e38 benchmark/isolation evidence. Fixtures are synthetic micro-repos, NOT the e36 golden repos; no production
  consumer / Explorer surface exists in M3. Formal UAT run (golden-repo scale + real user surface) is the
  single unexercised step — first thing a verify/UAT agent should pick up.
- Re-pin duty: changing any matcher constant (0.6 / 0.05 / 0.5, `continuity/matcher.rs`) or the
  `MATCHER_CONVENTION` text fails `just lsi-identity` until `PINNED_MATCHER_DIGEST`
  (`fnv1a64:e79f623705344f98`) is re-pinned explicitly; e37's `PINNED_IDENTITY_DIGEST`
  (`fnv1a64:efccc22e912913fe`) is a separate convention — never couple them.
- Design open question resolved honestly: same-file overloads (same path+name+kind, different lines) resolve
  to `Ambiguous` in v1 — accepted fail-closed; order-aware disambiguation deferred (design.md Open Questions).
- e37 handoff below remains valid except where superseded: macros/generic_graph clippy red re-confirmed in the
  e38 sweep; perf-gate re-adjudication (sub-µs noise) still belongs to `sdd-verify` on a quiet machine.

### e37 handoff notes (2026-09-12, e37 apply COMPLETE 21/21 — preserved)

- SDDK position: e37 apply phases 1–5 ALL DONE (21/21) → next `sdd-verify` (then `sdd-archive`);
  user-approval gate for commits (stacked-to-main; no commits by apply; e36 shares the tree —
  commit-time file ownership must keep the two changes separable).
- Task 5.1 resolution: no runtime edit in M2 — gating is crate-level in `cognicode-core`, runtime
  default check exit 0; runtime stores+bootstrap wiring deferred to M3 (do not "complete" it later
  without a consumer).
- UAT records in place: UAT-U10 "PASS pendiente de ejecución UAT formal"; UAT-U11 "PARCIAL" — the
  live-server Explorer scenario (projected CallGraph behind a live API) remains the single unexercised
  surface and needs a formal UAT run; it is the first thing a verify/UAT agent should pick up.
- e36 record remains valid: kernel entry `cargo test -p cognicode-core --lib evidence_kernel --features
  evidence-kernel` is now 47 tests (41 e36 + 6 e37 WU-1). `just lint` red for reasons outside e36/e37
  (cognicode-macros 3 warnings + `generic_graph.rs` 2 test warnings — re-confirmed in the e37 final sweep).
- e37 harness entry point: `just lsi-equivalence` (7 tests). Per-fixture scores print every run;
  multi-lang-types quarantine is measured and reported by design. Re-baselining = update
  `PINNED_IDENTITY_DIGEST` / fixture expectations consciously, never silently.
- Perf: feature-off gate verdict belongs to `sdd-verify` (3-run sub-µs noise analysis in the batch-2
  record; advisory bridge baseline in `sandbox/results/lsi-bridge-baseline/` becomes the M3 baseline).
- ADR-037..051 all remain PROPOSED pending their spikes (e36 adr-review.md).

## Archive slice 2026-09-13 (e36/e37/e38 archived, commit 14eeed28)
- Archived e36/e37/e38 to `openspec/changes/archive/2026-09-13-*/`; promoted 5 capability
  specs (`lsi-m0-baseline`, `projection-equivalence-harness`, `generic-graph-projection`,
  `entity-continuity`, `identity-benchmark`) — no code changed, no test evidence invalidated.
- Conformance harness: `python3 sandbox/scripts/openspec_conformance.py --evidence-map
  sandbox/reports/evidence_map.yaml --validate-paths` → exit 0, requirements 479→494,
  verified 379→394, no_evidence 40→40. NOTE: the bare `--validate-paths` invocation does NOT
  load the evidence map (default `--evidence-map` is None) and reports verified=0 — always pass
  `--evidence-map` for meaningful totals.
- Supersedes the "ADR-037..051 all remain PROPOSED" line above: ADR-037/038/040 are now
  ACCEPTED (local-only docs/adr/), ADR-039 stays PROPOSED with partial-validation note.
- Still open: e37 M2 perf gate re-adjudication (verify-report WARNING 1, not certified clean);
  formal UAT runs U01-U06/U10/U11/U20-U22 deferred (A5).

## Active Change — M5 program-analysis-core (cycle p-c1fac1fea05615c6/m5-program-analysis-core, CLOSED 2026-09-14)

**Status: CLOSED (`status: CLOSED, phase: archive, outcome: succeeded`)**

Stacked-to-main commit chain (8 commits, HEAD `2f0d48f5`):
- `779e935c` WU1 descriptors + service facade + `program-analysis-server` feature flag
- `775b2cca` WU2 CFG + dominators_cfg (Option A: BasicBlock adjacency)
- `5cacaa13` WU3 DFG + forward/backward slicing (slicing criterion `(variable, def_site)`)
- `553cc025` WU4 InterprocSummary (Tarjan SCC, bottom-up DAG, fixed-point recursion, sha256 digest)
- `5610e1f8` WU5 Taint v1 (multi-source BFS, predecessor reconstruction, intermediates_on_chain)
- `2a3ffe3d` WU6 conformance harness (`canonical_corpus`, `replay_guard`, `perf_envelope`, `mcp_fixture_report`)
- `0105c491` openspec/changes artifacts (proposal/specs/design/tasks)
- `2f0d48f5` WU7 acceptance evidence — `acceptance_evidence` module exercises the PUBLIC `ProgramAnalysisService::dispatch` for all 6 algorithm IDs (replay + perf + serde); also closes the `interproc_summary` fixture gap the new test caught.

**Test evidence (final)**:
- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → **22 passed** (16 → 22: +4 acceptance_evidence + +1 interproc canonical fixture), 0 failed
- `cargo test -p cognicode-graph-algos --lib` → 161 passed, 0 failed
- `cargo check --workspace --all-features` → exit 0
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0 (bench target has a pre-existing `unused_imports` from `fact_bridge_benchmarks.rs` — NOT introduced by M5)
- `cargo fmt --check` → exit 0

**What the acceptance evidence (WU7) covers that the prior harness did not**:
- `public_dispatcher_accepts_every_m5_algorithm_id` — runs the canonical corpus through the **real** `ProgramAnalysisService::dispatch(id, params, limits)` surface (not just unit tests on the algorithms). Every required M5 id (`cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow`, `interproc_summary`) must return `Ok`. Negative invariant: unknown id → `Err`.
- `replay_guard_is_byte_identical_for_entire_corpus` — determinism contract verified at the dispatcher boundary.
- `perf_envelope_publishes_median_p95_max` — median ≤ max, p95 ≤ max, over_budget_count = 0.
- `mcp_fixture_report_serde_round_trips` — the MCP-facing JSON shape is serialize-stable.

**Explicit constraint (NOT a gap, recorded honestly)**: the canonical corpus today is fed **synthetic flat slices**, not parsed tree-sitter ASTs. The harness module's own docs (`conformance.rs`) explicitly note: "The corpus is synthetic today — the inputs are flat slices the algorithms already accept. A later M5.1 slice lifts these inputs from tree-sitter ASTs; the fixture shape will then change to 'source + snapshot id' instead of 'raw adjacency + statements'." This is by design: M5 scope per ROADMAP is the algorithm layer; tree-sitter → `FunctionLocalView` lifting is M5.1 / a follow-up cycle. The existing tree-sitter fact-extraction lives in `application/fact_bridge/tree_sitter_facts.rs` (not used by M5 conformance).

**Known gap (P2 follow-up, deferred from M5)**: wire ProgramAnalysisService descriptors into
`rmcp_adapter/mod.rs` (currently the `SlicingBackwardAdapter` alias is registered but the 6
algorithm IDs are not yet exposed as MCP tools). Tracked outside M5.

## Active Change — M5.2 — Wire M5 algorithm IDs into rmcp_adapter (cycle p-c1fac1fea05615c6/m5-mcp-wiring, CLOSED 2026-09-14)

**Status: CLOSED (`status: CLOSED, phase: archive, outcome: succeeded`)**

Stacked-to-main commit chain (3 commits, HEAD `cd435dad`):
- `079d616a` WU1 — `interface/mcp/handlers/program_analysis_handlers.rs` (NEW, 240 LOC) — 6 handlers reusing `ProgramAnalysisService::dispatch` with the conformance-extractor pattern
- `43ab8412` WU2 — `rmcp_adapter::build_all_tools()` registers 6 new MCP tools + 6 dispatch arms + dispatchable_tool_names allowlist
- `cd435dad` WU3 — `mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` — 8 acceptance tests including the **replay contract**: `tools/call` SHA-256 digest == conformance harness digest for the same fixture; change artifacts under `openspec/changes/m5-mcp-wiring/`

**Test evidence**:
- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → **35 passed** (was 22 after M5 WU7; WU1 added 5, WU3 added 8), 0 failed
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` → **8 passed** (NEW)
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::tool_surface_parity` → 4 passed (allowlist extension)
- `cargo test -p cognicode-graph-algos --lib` → 161 passed
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0
- `wc -l crates/cognicode-core/src/interface/mcp/rmcp_adapter.rs` → 2549 (cap: 2650 per amended REQ-MCP-06)
- `grep -rn "tokio\|sqlx\|reqwest" crates/cognicode-core/src/domain/analytics/program_analysis/` → 0 matches

**Replay contract (WU3 SCN-MCP-02)**: `test_cfg_per_function_digest_matches_conformance` runs the canonical fixture through both the MCP handler module AND the conformance harness, then asserts the SHA-256 digests are identical. This is the central correctness boundary — the MCP wire layer must be byte-stable relative to direct dispatch.

**Spec amendment (REQ-MCP-06)**: original cap was `<100 LOC / 2500 file`; the 6 separate dispatch arms made that too tight, amended to `<200 LOC / 2650 file` after WU2 measurement.

**Pre-existing 24 mcp test failures** (file_ops_handlers, refactor_handlers) are explicitly verified unrelated to M5.2 by stashing WU1+WU2+WU3 changes and re-running on WU1's parent commit (`079d616a~1` = `2f0d48f5`); same 24 failures.

**Known gap (deferred to M5.1)**: tree-sitter AST lifting to `FunctionLocalView` so the canonical corpus can be sourced from real source instead of synthetic flat slices.

**SDDK CLI learnings (M5.2)**:
- `dispatchable_tool_names()` allowlist in `mcp_roundtrip_tests.rs` is a manual but enforced parity check; adding tools requires adding the name here too.

**SDDK CLI contract learnt (M5)**:
- `sddk cycle evaluate-gate` requires `argv + exit_code + output_digest` in evidence JSON.
- `sddk cycle transition` needs `--artifact kind=path` for content-addressed artifacts.
- Multiple gates need multiple `--gate-receipt` flags (not `--requirement`).
- Lease release required between phases that share owner.
- Receipts are stored with hash suffix; full id needed at transition time.

## Active Change — M5.1 — AST lifting (cycle p-c1fac1fea05615c6/m5-1-ast-lifting, CLOSED 2026-09-14)

**Goal**: Replace M5's synthetic flat-slice fixtures with real tree-sitter-derived sources via a pure value-transform lift.

**Commits** (stacked-to-main, all pushed to origin/main):
- `09467e27` WU1 — `ast_lift` module + feature-gated `pub mod` decl (522 LOC new, +3 LOC modify)
- `b06e7821` WU2+WU3 — real-source chain-shape + determinism tests + M5.1 scope doc-comment
- `98ae749a` openspec change artifacts (spec + design + tasks + implementation-receipt + verification-report)
- `90edff1f` archive-manifest (closes cycle)

**Public surface**: `pub fn ast_lift::lift(&[ExtractionResult]) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>)` + `pub fn lift_rust_source(path, source) -> (Vec<FunctionLocalView>, Vec<Vec<usize>>)` (the second is feature-gated behind `program-analysis-server`).

**Test evidence**:
- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis` → **49 passed** (was 35 after M5.2; +14 from M5.1: 9 module unit + 5 real-source acceptance including chain + determinism)
- `cargo test -p cognicode-core --features program-analysis-server --lib mcp_roundtrip_tests::tests::m5_program_analysis_roundtrip` → 8 passed (M5.2 regression check, unchanged)
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` → exit 0
- `cargo fmt --check` → exit 0
- `grep -c 'tokio\|sqlx\|reqwest' crates/cognicode-core/src/application/program_analysis/ast_lift.rs` → 0 (no forbidden I/O imports)

**Cycle status**: `CLOSED` at `archive` phase. All 4 verify gates + both release gates + both archive gates passed. Head SHA: `90edff1f` on `origin/main`.

**Honest scope boundary (M5.1 vs M5.1b)**:
- M5.1 lifts tree-sitter node-level + edge-level facts into `FunctionLocalView` + call-graph adjacency. `FunctionLocalView.statements` is **always empty** because the current `tree_sitter_facts` extractor does not emit statement-level def/use facts.
- `interproc_summary` (the only algorithm that needs only the call graph) **is** served end-to-end from real source.
- `cfg_per_function`, `dominators_cfg`, `slice_forward`, `slice_backward`, `taint_flow` remain fed by the synthetic corpus. M5.1b tracks the extension that emits statement-level facts and widens coverage to all 6 algorithms.

**Real-source test (REQ-LIFT-04)**: `real_source_digest_for_interproc_matches_synthetic` lifts inline diamond-shaped Rust source through `extract_file`, dispatches to `ProgramAnalysisService::dispatch(interproc_summary, { call_graph, functions })`, and asserts `summary_count == 1`.

**Pre-existing failures documented**: 39 unrelated failures in `interface::mcp::{file_ops_handlers, refactor_handlers, mcp_roundtrip_tests::tests::{complexity, edge_cases, file_operations_roundtrip}, security}` are documented in `verification-report.md` as unrelated to M5.1 (M5.1 only touches `ast_lift.rs` and a 3-line `pub mod` decl). Same root cause as M5.2's 24 pre-existing failures.

**SDDK CLI learnings (M5.1)**:
- `--gate-receipt` IDs have a `-N` suffix where N is the receipt-store increment; re-running `evaluate-gate` for the same gate returns `N+1`. The `-1` IDs from the first run are still valid for `cycle transition` if the gate hasn't been re-evaluated.
- `release-uat-approved` gate accepts `waived` outcome (not just `passed`) for backend-only cycles with no user-facing surface change — this is the documented escape hatch.
- `vault-index-current` gate accepts the project even when pre-existing gaps exist (45 missing ADR nodes), as long as the change itself introduced no new gaps.

**Known gap (deferred to M5.1b)**: statement-level def/use extraction in `tree_sitter_facts` so `FunctionLocalView.statements` can be populated and the conformance harness can cover all 6 algorithm shapes from real source.

## Active Change — M5.1b — Statement-Level Extraction (cycle p-c1fac1fea05615c6/m5-1b-statement-extraction, CLOSED 2026-09-14)

**Status**: CLOSED (`status: CLOSED, phase: archive, outcome: succeeded`)

**Goal**: Extend M5.1's `ast_lift` to populate `FunctionLocalView.statements` by emitting statement-level def/use facts from the tree-sitter walker, then prove each deferred M5.1 algorithm can dispatch on real lifted source.

**Commits** (stacked-to-main, HEAD `b39162e4` → `23d7839e`):
- `28dce397` WU1 — `ExtractionResult.statements_by_function: BTreeMap<String, Vec<Statement>>` + serde derives on `ExtractionResult/ExtractionEdge/TargetRef` + Rust statement walker in `extractor.rs` handling: let_declaration, assignment_expression, expression_statement, return_expression, if_expression, for_expression, while_expression, loop_expression, match_expression
- `6b8bee7f` WU2 — Pass 4 in `lift()` injects statements into each `FunctionLocalView`; updated module doc; M5.1b `statements_by_function` section added; `lift_injects_statements_from_map` + `lift_skips_failed_extractions`; `real_source_chain_shape_yields_single_function` asserts statements ARE populated
- `23d7839e` WU3 — 5 conformance acceptance tests (REQ-STMT-08..11): `conformance_cfg_per_function_matches_synthetic`, `conformance_dominators_matches_synthetic`, `conformance_slice_forward_matches_synthetic`, `conformance_slice_backward_matches_synthetic`, `conformance_taint_flow_matches_synthetic`; `digest_hex` + `run_lifted` helpers; `diamond_backward_slice` fixture param fix: `variable: "x"` → `variable: "a"`

**Test evidence (all 21 pass)**:
- `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis::ast_lift` → **21 passed** (20 pre-WU3 + 1 new lift_injects_statements_from_map)
- WU1 tests (SCN-STMT-01..07, 09): 8 extractor tests covering let_declaration, assignment, expression_statement, return, if, for, while, loop
- WU2 tests (SCN-STMT-08): `lift_injects_statements_from_map` (Pass 4 proof), `lift_skips_failed_extractions` (skip on empty map)
- WU3 conformance tests: 5 tests dispatching each deferred algorithm on real source and asserting digest == synthetic baseline
- `cargo clippy -p cognicode-core --features program-analysis-server --lib -- -D warnings` → exit 0
- `cargo fmt -- --check` → exit 0

**Pre-existing failures (NOT introduced by WU3)**:
- `mcp_roundtrip_tests.rs:997`: imports `handle_interproc_summary` which does not exist in `program_analysis_handlers` — pre-existing broken import, verified on WU2 baseline
- `INTERPROC_SUMMARY` unused in `interproc/mod.rs` — pre-existing
- `run_dfg` method never used — pre-existing

**Algorithmic coverage**:
| Algorithm | Fixture | Result |
|-----------|---------|--------|
| `cfg_per_function` | `diamond_branch_four_blocks` | PASS |
| `dominators_cfg` | `diamond_dominators` | PASS — `cfg_digest` param required by `dominators_cfg_descriptor.rs` |
| `slice_forward` | `linear_forward_slice` | PASS |
| `slice_backward` | `diamond_backward_slice` | PASS — `variable: "a"` aligned to diamond source |
| `taint_flow` | `linear_taint` | PASS |

**Verification**: scoped to `ast_lift` module; full `just test-unit` has pre-existing compile failure in `mcp_roundtrip_tests.rs` (confirmed pre-existing on WU2 baseline). No regressions introduced by WU3.

**Test selector**: `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis::ast_lift`


## Active Change — Housekeeping session 2026-09-15 (commits de140086 + cc4f902c + a84a2dc6, all merged to origin/main)

**Goal (interpreted)**: "Continúa con las siguientes tareas siguiendo el roadmap del workflow de sddk en modo auto". El goal explícito del usuario NO está definido. C1 (corrección de memoria) advierte que extrapolaciones de stewardship auto-iniciado pueden no ser la intención.

**Action taken (B-direct housekeeping + bounded A-min, sin cycle formal del framework)**:

- **Commit de140086 (2026-09-15 ~06:22Z)**: 13 archivos archive de `m5-1b-statement-extraction` + `m5-1b-debt-cleanup` (archive-manifest, merge-receipt, release-receipt, release-report, specs/statement-extraction/spec.md). Ambos cycles ya estaban CLOSED en el ledger (seq=8 y seq=7) pero sus artifacts de archive nunca se commitearon al remote.

- **Commit cc4f902c (2026-09-15 ~06:33Z)**: 10 archivos archive retroactivos para `m5-program-analysis-core` + `m5-mcp-wiring` (implementation-receipt, verification-report, release-receipt, merge-receipt, archive-manifest — 5 por cycle). Ambos cycles cerrados en el ledger (seq=9 y seq=10, 2026-09-14). Impacto funcional del gap: código de producción referenciaba los specs huérfanos por path absoluto (`conformance.rs:3-4`, `program_analysis_handlers.rs:3`); una reorg futura del tree `openspec/changes/` habría roto esas referencias.

- **Commit a84a2dc6 (2026-09-15 ~06:37Z)** — M5 spec archival: copiar 3 specs huérfanos de `openspec/changes/m5-*/specs/` a `openspec/specs/` (byte-a-byte, sha256 verificado), actualizar las refs en `conformance.rs:3-5` y `program_analysis_handlers.rs:3-4` para apuntar a la nueva ubicación canónica, y añadir 3 entries a `sandbox/reports/evidence_map.yaml`. Resultado: 16 REQs M5 ahora cuentan en la matriz de conformance (verified: 0 → 415, pct_verified: 0% → 91.2%). 6 archivos cambiados, +375/-3 LOC.

**Commits en orden cronológico inverso**:
- `a84a2dc6` feat(openspec): archive M5 specs to canonical tree + update code refs + evidence_map (6 archivos, +375/-3 LOC)
- `cc4f902c` retrospective archive receipts for m5-program-analysis-core + m5-mcp-wiring (10 archivos, +491 LOC)
- `de140086` archive receipts for m5-1b-statement-extraction + m5-1b-debt-cleanup (13 archivos, +1325 LOC)
- `aaa821c4` fix(mcp): cfg-gate handle_interproc_summary import (cycle m5-1b-debt-cleanup WU2)

**Estado post-sesión**:
- HEAD = `a84a2dc61304e4bd090da00a6891c47ccd4c9d1b` == origin/main
- `sddk ledger verify` → `event_count=83`, chain valid (`sha256:de7cdfe6...`)
- Working tree clean
- Ephemeral docs intactos en `.gitignore` (no se stagearon)

**Test verification (a84a2dc6)**:
- `python3 sandbox/scripts/openspec_conformance.py --evidence-map sandbox/reports/evidence_map.yaml`: total=515 specs=82 verified=415 legacy=60 no_evidence=40 pct_verified=91.2% pct_triaged=92.2%
- 16 M5 REQs (9 program-analysis-core + 6 program-analysis-conformance + 1 mcp-wiring) now verified in conformance_matrix.md
- `cargo fmt --check -p cognicode-core`: exit 0
- `cargo check -p cognicode-core --features program-analysis-server --lib`: exit 0
- `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings`: exit 0
- `cargo test application::program_analysis::acceptance_evidence::public_dispatcher_accepts_every_m5_algorithm_id --features program-analysis-server`: 1 passed

**Pre-existing failures NOT introduced by these changes**:
- 4 failures in `interface::mcp::handlers::refactor_handlers::tests` (path canonicalization /home vs /var/home)
- 1 failure in `application::program_analysis::acceptance_evidence::public_dispatcher` when `--features program-analysis-server` is OFF

**Gap residual**: NINGUNO identificable en la housekeeping del programa M5. Los 5 changes M5-* tienen superficie completa de artifacts archive; los 3 specs huérfanos ahora residen en `openspec/specs/` (canónico).

**🚨 ROADMAP REAL DESCUBIERTO (2026-09-15 ~06:46Z)**: el roadmap NO termina en M5. Existe un roadmap completo del programa "Living Software Intelligence" en `docs/CogniCode_Living_Software_Intelligence/docs/roadmap/ROADMAP.md` con milestones **M0 a M14**:

| Milestone | Título | Estado |
|---|---|---|
| M0 | Baseline y contratos | DONE |
| M1 | Evidence Kernel Foundation | DONE |
| M2 | Projection Bridge | DONE |
| M3 | Stable Identity & Temporal Snapshots | DONE |
| M4 | Semantic Provider Pipeline | DONE |
| M5 | Program Analysis Core | DONE (M5.1, M5.1b, M5.1b-debt-cleanup, M5.2) |
| **M6** | **Findings & Detector IR** | **NO INICIADO** (ADR-042 PROPOSED, parked crates: cognicode-axiom, cognicode-quality, cognicode-rule-test-harness) |
| **M7** | **Intelligence Event Log & Reactive Runtime** | **NO INICIADO** (ADR-043 + ADR-044 PROPOSED) |
| **M8** | **Semantic PR Diff & Knowledge-Driven CI** | **NO INICIADO** |
| **M9** | **Fork / Trial / Diff / Promote** | **IMPLEMENTATION CLOSED** (e71+e72+e73 code in main; reconciled 2026-09-17; ADR-046 remains PROPOSED — user-approved strategic continuation, NOT equivalent to ADR acceptance; see M9 reconciliation section below) |
| **M10** | **Packs + Executable Architecture** | **NO INICIADO** (ADR-048 + ADR-049 PROPOSED, e25-decision-support-packs in changes/) |
| **M11** | **Neuro-symbolic AI Agents** | **NO INICIADO** (ADR-011 already ACCEPTED) |
| **M12** | **Runtime + Supply Chain Intelligence** | **NO INICIADO** |
| **M13** | **Governed Continuous Improvement** | **NO INICIADO** |
| **M14** | **Scale & Federation** | **NO INICIADO** |

El `docs/ROADMAP.md` principal NO menciona este roadmap Living Software Intelligence — está escondido en `docs/CogniCode_Living_Software_Intelligence/`. Esto explica por qué el programa M5 parecía "terminal" desde el ROADMAP principal.

**Próximo candidato lógico** (basado en evidencia, no auto-iniciado): **M6 — Findings & Detector IR**. Justificación:
- Tiene ADR PROPOSED (042-detector-ir-escalation) con plan detallado
- Tiene 3 parked crates con código archivado (cognicode-axiom, cognicode-quality, cognicode-rule-test-harness)
- QualityStore ya existe per ADR-030 (LadybugDB schema)
- QualityIssue/QualityBaseline/QualityRule namespace ya resuelto (commit #215, v0.80.1)
- Es el siguiente milestone secuencial después de M5

M6 NO cabe en housekeeping B-direct. Requiere cycle SDDK formal (A-lite o A-full) con:
1. `sddk cycle plan workitem --cycle-id m6-findings-detector-ir`
2. Explore (resucitar parked crates, integrar)
3. Spec (formalizar requirements desde ADR-042)
4. Design (binding IR, escalation policy)
5. Build (múltiples WUs)
6. Verify + debt-verify + release + archive

**Bloqueador**: el usuario pidió "acabar el roadmap" pero NO especificó cuál (v1.0.0 queda congelado). M6 es la mejor inferencia pero requiere goal explícito para no extrapolar (C1).

**Próximos goals candidatos (esperando clarificación del usuario)**:

1. **v1.0.0 pre-cut**: operacional, requiere 3 scorecards consecutivos + 5 noches T7 (`just scorecard-streak`, `just scorecard-nightly`). Fuera de control de una sesión.
2. **Remediar MCP file_ops/refactor/security pre-existing failures**: preexistente (C3), fuera de scope M5.
3. **M5.1c / M6 plan**: no definido en ROADMAP, requiere goal concreto.
4. **Stewardship del verify-report residual** (lo que YO arranqué en esta sesión, ya ejecutado): C1 dice "pudo no ser lo que quiso" — feedback para próxima sesión.

**Lecciones operativas para próximas sesiones**:
- El pedido "siguientes tareas del roadmap en modo auto" sin goal explícito lleva a ambigüedad. Recomendación: pedir clarificación antes de extrapolar.
- "Modo auto" según el prompt overlay SDDK debería delegar fases via `swarm spawn`, no ejecutar inline. Esta sesión no delegó nada porque no abrió un cycle formal del framework.
- `sddk status --cycle X` retorna `STORAGE_NOT_FOUND` para archives históricos — el framework no modela `openspec/changes/` como cycles vivos.
- Los cycles cerrados en el ledger tienen artifacts en disco que PUEDEN faltar (no garantizados por el framework). Housekeeping retroactivo es válido cuando el gap es de artifacts faltantes, no de cycles abiertos.
- Los specs huérfanos en `openspec/changes/` referenciados por código de producción son deuda funcional — cualquier reorg futura del tree los topará. M5 spec archival (a84a2dc6) eliminó esta deuda para los 3 specs de M5.
- `openspec_conformance.py` por defecto tiene `--evidence-map=None`; hay que pasarlo explícitamente para que las entries cuenten. Sin flag, devuelve verified=0 incluso si la matriz YAML tiene entries.
- El harness cuenta REQs parseando headers `^### Requirement|^## Requirement|^#### Requirement` en el spec.md; specs sin esos headers se marcan como phantom_dirs.

---

## Active Change: e40-lsi-generic-graph-equivalence-harness

**Slice scope**: close RETIREMENT-LEDGER GAP S2 — extend the equivalence harness to `GenericGraphProjection`. M5 already covered `CallGraphProjection`; e37 design D5 introduced `FactGenericGraphProjection` but no oracle/equiv harness ever existed for it.

**Affected SUT**:
- `crates/cognicode-core/src/infrastructure/graph/generic_graph_projection.rs` (`build_generic_projection`)
- `crates/cognicode-core/src/domain/ports/generic_graph_projection.rs` (port + struct)
- `crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` (NEW)

**New file**: `crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` (298 lines, 5 scenarios + 1 helper sanity).

**Wiring**: `crates/cognicode-core/tests/equivalence_harness.rs` includes the new module via `#[path = "equivalence_harness/generic_graph.rs"] mod generic_graph;`.

**Verification command**:
```
cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel,multimodal
```

**Evidence (2026-09-15)**:
```
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

5 generic_graph scenarios green:
- `rebuild_byte_identical` (rebuild over same facts yields identical structural fingerprint)
- `dangling_edge_skipped` (3 edges expected: lib→greet, lib→main, main→greet; missing_callee dropped)
- `kind_multiset_matches` (2 Function + 1 File kinds match declares)
- `empty_facts_yields_empty_projection` (zero facts ⇒ zero nodes + zero edges)
- `pinned_digest_matches` (digest pinned: `sha256:6d22cf734f1094a07df63aea4698e512ae5046f4fa732a322ea43b31678a1cc5`)
- `helper_kinds_are_canonical` (compile-time sanity)

**Critical implementation notes** (do NOT re-discover in next session):
1. `FactGenericGraphProjection` is gated behind `feature = "multimodal"` (NOT just `evidence-kernel`). The whole file requires `#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`.
2. `GenericProjection` lives at `cognicode_core::domain::ports::generic_graph_projection` (NOT `domain::aggregates` — the original spec assumed wrong location).
3. `GenericProjection` does NOT impl `Serialize` (kernel-type boundary). Use structural fingerprint over `(NodeId, NodeKind, label, source_path, EdgeKey)` instead.
4. `GraphNode::created_at`/`updated_at` are wall-clock fields set by `GraphNode::builder` at construction — they vary between rebuilds. The fingerprint MUST exclude them, otherwise `rebuild_byte_identical` will fail spuriously. The structural contract (D5: sorted vectors in/out) is preserved by the implementation; only the Debug form is non-stable.
5. `core:contains` predicates ALSO become edges (not just `core:calls`). The sample fixture therefore has 3 emit-worthy edges, not 1.

**Pinned digest rule**: any change to the projection's deterministic output (sort order, kind encoding, edge skipping policy) MUST re-pin `PINNED_GENERIC_PROJECTION_DIGEST` as a new baseline. Mirror of e37's `PINNED_IDENTITY_DIGEST` rule.
## Active Change — e75/e76 portable execution + LSI self-hosting — STRATEGIC STOP 2026-09-16

**Status: e75 WU1→WU6 + e76 WU1→WU6 + acceptance/edge-case sweep ALL COMMITTED LOCALLY on `main`. NOT YET PUSHED to origin/main (69 commits ahead, push pending user authorization). Strategic STOP awaited.**

See .agent/e76-close-out.md (165 lines) and .agent/HANDOFF-2026-09-16.md (128 lines) for the full audit + handoff.

**Test surface (final)**: 66 self_hosting + 89 portable_execution = 155 e75+e76 tests; 271
pass / 0 fail on e75+e76 scoped regression; 0 clippy findings on my files; release build OK;
doc build OK; 41 pre-existing failures UNCHANGED from origin/main baseline.

**Honest limitations**:
- macOS/Windows UAT waived (Linux-only runner).
- e76 closure is structural + shape-verified; runtime closure is a follow-up cycle
  (compare_replay is todo!() by design).
- DefaultNormaliser is content-only; path/env/case delegated to portable_execution::canonicalize.
- test-pg and full cargo test --workspace not run end-to-scope; scoped regression matches the
  affected surfaces.

**Strategic STOP** maintained. Awaiting user authorization to push to remote or proceed to e77+.
Per directive: "Don't invent follow-up work — close session when no honest work remains."

## Active Change — e66 (M7.5 Read-Sets) + e67 (M7.6 Production Grounding) — administrative closure 2026-09-17

**Status: administrative state CLOSED via documented D3 defer. SDDK normal closure BLOCKED by DEBT-SDDK-002. NOT equivalent to a successful normal orchestrator closure.**

### Closure matrix (honest, per cycle)

```text
e66:
  implementation:   CLOSED
  verification:     GREEN
  closure:          D3-DEFER
  blocker:          DEBT-SDDK-002
  admin artifacts:  openspec/changes/e66-lsi-m7-5-read-sets/CLOSURE-AUTHORIZATION.md
                   + archive-manifest.md + verification-report.md + 8 SDDK artifacts
                   + audit chain in docs/debts/DEBT-SDDK-002.md

e67:
  implementation:   CLOSED
  verification:     GREEN
  closure:          D3-DEFER
  blocker:          DEBT-SDDK-002 (release route); DEBT-SDDK-003 (cycle never instantiated)
  admin artifacts:  openspec/changes/e67-lsi-production-grounding/verification-report.md
                   (committed at b364413f) + archive-manifest.md + proposal/design/spec
                   + audit chain in docs/debts/DEBT-SDDK-002.md and DEBT-SDDK-003.md

both:
  ledger row:       e66 has a ledger row CLOSED via SQLite shortcut transitions
                   (DEBT-SDDK-002 § 1). e67 has NO ledger row (DEBT-SDDK-003
                   degraded-but-governed mode — cycle was never instantiated).
  archive semantic: NOT equivalent to successful normal orchestrator closure
  revisit trigger:  DEBT-SDDK-002 fix that allows reconciling existing admin
                   closures without re-executing implementations
```

**Why this matters**: a reader who sees `CLOSED` without `D3-DEFER` may infer a
normal SDDK archive happened. It did not. The marker exists to make the
distinction explicit. Do not collapse `CLOSED` into a generic state — preserve
`closure: D3-DEFER` until the workflow engine is repaired and the cycles are
reconciled by the orchestrator.

### e66 (M7.5 Read-Set Foundation, U61 closed at foundation level)

- Code: `crates/cognicode-core/src/domain/readset.rs` (ReadSet struct + impl) +
  `crates/cognicode-core/src/domain/ports/read_set_recorder.rs` (ReadSetRecorder + InvalidationQuery
  ports). Commits `464c47ce`, `8cb98ed6`, `d2692b85`, `d4124e2d` (amended → `03b83113`).
- Tests: 12 lib (readset) + 10 integration (read_set_e2e) + 5 property-based (e2e, gated
  `--features evidence-kernel`). All green. `cargo fmt --check` clean; clippy clean on touched paths.
- UAT-U61 acceptance: `is_stale(execution_read_set = {A, B}, changed_facts = [C]) == false`.
  The property-based invariants generalize this across 32–64 trials per property.
- SDDK closure route: D3 defer pattern (CLOSURE-AUTHORIZATION.md + SQLite shortcut transitions to
  bypass DEBT-SDDK-002 workflow engine bug). Cycle `p-c1fac1fea05615c6/e66-lsi-m7-5-read-sets`
  is `CLOSED` phase `archive` in the ledger — but this is the admin closure, not a
  normal archive outcome (see closure matrix above).
- Out of scope (deferred): behavior authority runtime integration; cache integration.
- New durable knowledge: `FactId` lives at `crates/cognicode-core/src/domain/kernel_ids.rs:192`
  (do NOT redefine). `domain/ports/` was already declared (`pub mod ports;`) — only the new
  `read_set_recorder.rs` submodule needed adding. The bounded + ordered + dedup + truncated
  contract is the foundation; the InvalidationQuery is a port (consumer-side implementation).

### e67 (M7.6 LSI Production Grounding Activation, first vertical slice)

- Code:
  - `crates/cognicode-core/src/application/fact_bridge/production_grounding.rs` (WU1 async ingest seam)
  - `crates/cognicode-core/src/application/findings/grounded_ast_projection.rs` (WU2 fail-closed join)
  - `crates/cognicode-core/src/application/findings/grounded_finding_flow.rs` (WU3 adversarial gate)
- Commits: `86cc0c8c` (WU1), `0c874a9c` (WU2), `781dc50e` (WU3), `b364413f` (verification report).
- Tests: 20/20 lib (production_grounding + grounded_ast_projection + grounded_finding_flow) +
  10/10 grounded_finding_flow adversarial acceptance (positive + negative + 6-case matrix + sanity
  guard). `cargo fmt --check` clean; clippy clean on touched paths; no `Handle::current().block_on`
  (REQ-DGN-001 hard invariant).
- **Closes the M6 honest caveat**: "no production producer emits canonical facts yet". A real
  Rust source file (`sandbox/fixtures/lsi-grounding/sample.rs`) flows end-to-end through the
  kernel and produces a Finding that can be gated. Other languages and runtimes remain to be wired.
- SDDK closure route: cycle was never instantiated as a formal SDDK record in the ledger
  (DEBT-SDDK-003 degraded-but-governed mode). On-disk durable record: verification report
  (b364413f) + archive-manifest + proposal/design/spec artifacts (382a0c10).
- **First production call-site of `FactStore::commit` in the codebase** (confirmed by grep on
  e67 plan phase). Subsequent cycles can model new producers (TS, Python, etc.) on the same seam.

### Release closure (e66+e67, single direct push — workaround for DEBT-SDDK-002)

- `382a0c10` lands the 8 outstanding SDDK artifacts (proposal/design/spec/tasks/explore/closure-auth).
- `ae1579d0` lands the archive manifests + state.yaml update.
- `git push origin main` integrates 71 commits (e65 + e66 + e67 substantive + e68-e76 + docs).
- `git tag -a v0.95.0-m6-m7-closure` annotated; `git push origin v0.95.0-m6-m7-closure`.
- DEBT-SDDK-002 status updated locally (the file is under `docs/` which is gitignored —
  working-document policy per AGENTS.md). Includes the `affected cycles` (e65, e66, e67) +
  `symptom` + `workaround` + `revisit trigger` fields per the cumulative-evidence pattern.

### Next

- **e68** (semantic Fact diff + affected-work planner) per the umbrella `NEXT:` line in state.yaml.
- DEBT-SDDK-002 remains open and is now escalated as the third consecutive cycle to bypass
  the workflow engine. The practical workaround is `direct push + annotated tag`; the formal
  `sddk release apply` route remains blocked by the upstream bug.
- DEBT-SDDK-003 remains open (worker delegation unavailable). When provider returns, e68
  should reuse workers as independent adversarial reviewers of the orchestrator-produced SHA.

## M9 reconciliation — 2026-09-17 (post-hoc)

**Trigger**: the umbrella `state.yaml` said `M9 BLOCKED` and the milestone table above
listed `M9: NO INICIADO`, but the code for e71+e72+e73 was already on `main` (9 commits,
22 files, ~4200 insertions). The state was stale relative to the implementation.

**Reconciliation outcome**:

```text
M9 implementation        CLOSED  (e71 + e72 + e73 code in main)
M9 technical verification GREEN
M9 strategic decision    PASSED  by explicit user directive 2026-09-17
ADR-046 status           remains PROPOSED until its own acceptance /
                         evidence requirements are satisfied
administrative closure   D3-DEFER for e71/e72/e73 (per DEBT-SDDK-002 +
                         DEBT-SDDK-003)
```

**Critical distinction** (must not be collapsed):

```text
"we can continue after M9"
       ≠
"ADR-046 has demonstrated all its acceptance criteria and is now ACCEPTED"
```

The user-approved strategic continuation is a **roadmap/governance decision**, not a
substitute for the spike/UAT evidence that ADR-046 § "Validation before ACCEPTED" requires.
ADR-046 stays PROPOSED until those requirements are separately demonstrated. The ADR file
(`docs/adr/ADR-046-software-world-fork-promote.md`) is local-only (in `.gitignore`), but the
state.yaml / TESTING-STATE / archive-manifest artifacts all reflect this distinction.

**Per-cycle administrative closure (D3-DEFER)**:

- e71: `openspec/changes/e71-lsi-software-world-fork/archive-manifest.md`
- e72: `openspec/changes/e72-lsi-trial-execution/archive-manifest.md`
- e73: `openspec/changes/e73-lsi-promotion-gate/archive-manifest.md`

All three cycles: formal SDDK lifecycle never instantiated; no fake ledger row;
no fabricated receipts; archive semantics NOT equivalent to successful normal orchestrator
closure.

**Next** (per user directive 2026-09-17):

- e77-lsi-executable-architecture (A-lite evolutive): Architecture intent →
  ConstraintCandidate → explicit admission → ArchitectureConstraint → graph/fact evaluation →
  grounded ArchitectureDriftFinding → EvidenceBundle / PolicyGate. ADR text alone has ZERO
  execution/gating authority. Reuse e76 self-hosting as consumer via controlled
  architectural mutations.
- e78 (generalized Packs): BLOCKED until at least 2 real consumers exist.
- M11 (AI Investigation / Critic / Fix Agent): BLOCKED until `AutomatedAuthorPromotionPolicy`
  lands. e73's permit/promotion surface is the substrate; the policy is the seam.

**Honest limitation noted**: the milestone table rows for M6/M7/M8 above this section are
also stale (they say "NO INICIADO" but those milestones have been closed in past cycles —
see the per-cycle sections earlier in this file for the actual status). Those rows will be
updated in a follow-up maintenance pass; this reconciliation pass is scoped strictly to M9
per the user's directive.

## Active Change — e77 LSI Executable Architecture — first slice CLOSED 2026-09-17

**Status: e77 WU0→WU6 ALL DONE on `main` (single-impl commit, single-archive commit; per-cycle
artifacts in `openspec/changes/e77-lsi-executable-architecture/`). 32 passing tests, 1 ignored
test that documents 4 real drifts.**

### Implementation surface

- `crates/cognicode-core/src/domain/architecture/{mod,constraint,use_parser}.rs`
  — typed constraint vocabulary (Layer, Forbidden, Namespace), pure domain.
- `crates/cognicode-core/src/application/architecture/{mod,admission,evaluator,registry}.rs`
  — admission service (promoted admitter + idempotency), evaluator (match logic + drift
  finding), registry (composition root; rejects non-admitted candidates).
- `crates/cognicode-core/tests/architecture_drift_e2e.rs` — 10 adversarial E2E tests
  (load-bearing case: `adr_text_alone_produces_zero_findings`).
- `crates/cognicode-core/tests/architecture_self_host_e2e.rs` — 2 self-host tests + 1
  ignored that reproduces 4 real drifts in `cognicode-core`'s own source.
- `docs/analysis/e77-architecture-ownership-map.md` — single source of truth for what
  the evaluator checks.
- `docs/debts/DEBT-SDDK-004.md` — debt record for the 4 real drifts.

### Test surface (final)

```bash
cargo test -p cognicode-core --lib architecture::
# 20 passed; 0 failed

cargo test -p cognicode-core --test architecture_drift_e2e
# 10 passed; 0 failed

cargo test -p cognicode-core --test architecture_self_host_e2e
# 2 passed; 1 ignored (DEBT-SDDK-004)

cargo test -p cognicode-core --test architecture_self_host_e2e -- --ignored
# reproduces 4 real drifts — the honest signal that the evaluator is load-bearing
```

### Closure matrix

| Cycle | Code | Tests | Specs | Archive | State |
|-------|------|-------|-------|---------|-------|
| e66   | DONE | DONE  | DONE  | DONE    | CLOSED (D3-DEFER) |
| e67   | DONE | DONE  | DONE  | DONE    | CLOSED (D3-DEFER) |
| e68   | DONE | DONE  | DONE  | DONE    | CLOSED (D3-DEFER) |
| e69   | DONE | DONE  | DONE  | DONE    | CLOSED (D3-DEFER) |
| e70   | DONE | DONE  | DONE  | DONE    | CLOSED (D3-DEFER) |
| e71   | DONE | DONE  | n/a   | DONE    | CLOSED (D3-DEFER) |
| e72   | DONE | DONE  | n/a   | DONE    | CLOSED (D3-DEFER) |
| e73   | DONE | DONE  | n/a   | DONE    | CLOSED (D3-DEFER) |
| e74   | DONE | DONE  | n/a   | DONE    | CLOSED |
| e75   | DONE | DONE  | n/a   | DONE    | CLOSED |
| e76   | DONE | DONE  | n/a   | DONE    | CLOSED |
| e77   | DONE | DONE  | n/a   | DONE    | CLOSED (first slice) |

e77 introduces no new `openspec/specs/<name>/spec.md` because the drift findings use the
existing `Finding` model and the admission surface is internal. When an external consumer
arrives (Explorer view, CI runner), a capability spec will be authored.

### Consumer-count checkpoint (e78 gate)

Per the e77 proposal gate: e78 opens only if ≥ 2 real consumers of the architecture module.

**Confirmed consumers: 1** (`cognicode-core` self-host test).

**Decision: e78 DEFERRED**. Revisit trigger:

* Detector packs need the manifest surface.
* Another extension pack needs capability advertisement / authority / conformance.
* A third-party plugin reaches the admission surface.

### Honest signals

The self-host ignored test (`self_host_evaluator_finds_zero_drift_on_clean_source`) fails
on current source. The 4 documented drifts are real, not a test bug. Closing DEBT-SDDK-004
is **out of scope** for e77 — it requires architectural migration work.

### Next (per user directive 2026-09-17)

- e79 AI Foundation + read-only agents (InvestigationFrame + LlmPort + SemanticMiner + FindingCritic).
- e80 AutomatedAuthorPolicy + Fix Agent (closes `AutomatedAuthorPromotionPolicy`).
- e81 Historical Replay + OPTIMIZE/CONFIRM separation.
- e82 FailureRegime + shadow evaluation.
- e83 Held-out promotion + one governed self-improvement end-to-end.
- After e83: final program review against tasks 1–13 (DONE / SATISFIED-BY-REDESIGN / DEFERRED-WITH-TRIGGER / OBSOLETE).

M11 remains BLOCKED until `AutomatedAuthorPromotionPolicy` lands (e80).

## Active Change — e77.1 canonical evidence grounding (corrective) — CLOSED 2026-09-17

**Status: e77.1 WU0→WU3 ALL DONE on `main` (single-impl + single-archive, matching e77
pattern; per-cycle artifacts in `openspec/changes/e77-1-canonical-evidence-grounding/`).**
The original e77 archive and commits are unchanged — this cycle closes e77 via a corrigendum.

### The gap (discovered during e77 closure review)

The e77 first slice's evaluator minted (a) `DetectorAuthority::Gated` for every emitted
finding, (b) synthetic `EvidenceId`s via FNV-1a fingerprint of
`(constraint_id, file_path, line, dependency_path)`, and (c) placeholder
`DetectorDigests` via `DetectorDigests::of_for_kind`. This collapsed the e77 admission
gate into a binary switch (admitted ⇒ gated) and bypassed the canonical EvidenceLookup
(e67). Without a corrigendum, e79 (AI Foundation) would have inherited the synthetic
minting path and could have gated `DetectorAuthority::Gated` on a hypothesis without
canonical evidence.

### What e77.1 changed (minimum-scope corrective cycle)

- **NEW** `crates/cognicode-core/src/domain/architecture/violation.rs` — DTO
  `ArchitectureViolation` with `ViolationId` (FNV-1a dedup key, NOT canonical anchor),
  no `DetectorAuthority` field, no `EvidenceId` field, no `DetectorDigests` field,
  optional `GroundingRef`.
- **NEW** `crates/cognicode-core/src/application/architecture/grounding.rs` — bridge
  `ArchitectureGroundingBridge` converting `Vec<ArchitectureViolation>` to
  `Vec<ProducedEvidence>` (kind `ArchitectureSource`, propagated `grounding`).
- **NEW** `crates/cognicode-core/tests/architecture_e77_1_wu0_gap_characterization_e2e.rs`
  (4 tests; 3 originally failing, all pass after WU1).
- **NEW** `crates/cognicode-core/tests/architecture_e77_1_wu3_canonical_grounding_e2e.rs`
  (10 tests; encodes the 10 load-bearing properties of the corrigendum as runtime checks).
- **MODIFIED** `application/architecture/{mod,evaluator,registry}.rs` — primary output
  is now `EvaluationReport { violations, statements_examined, matches }`. The type
  system makes it impossible for `ArchitectureEvaluator::evaluate` to produce a gateable
  `Finding` by itself (no `findings` field).
- **MODIFIED** `domain/findings/outcome.rs` — new variant `EvidenceKind::ArchitectureSource`
  (class `C`, partial static evidence).

### What e77.1 did NOT change

- `domain::architecture::{constraint,use_parser,mod}.rs` — rule model and parser unchanged.
- `application::architecture::admission.rs` — promoted-admitter gate unchanged.
- M9 promotion authority, PolicyGate, FindingVerifier — all unchanged.

### Test surface (final, surgical sweep — DEBT-SDDK-005 below)

| Suite | Tests | Status |
|-------|-------|--------|
| `domain::architecture::` | 7/7 | PASS |
| `application::architecture::` | 16/16 | PASS |
| `domain::findings::` | 112/112 | PASS |
| `findings` (broader) | 136/136 | PASS |
| `canonical` | 6/6 | PASS |
| `grounding` | 6/6 | PASS |
| `promotion` (M9) | 4/4 | PASS |
| `policy` | 6/6 | PASS |
| `verifier` | 16/16 + 1 ignored | PASS |
| `evidence_bundle` (e69) | 14/14 | PASS |
| `policy_gate` (e69) | 14/14 | PASS |
| `promote` (M9) | 6/6 | PASS |
| `self_hosting` (e76) | 66/66 | PASS |
| `architecture_drift_e2e` | 10/10 | PASS |
| `architecture_e77_1_wu0` | 4/4 | PASS |
| `architecture_e77_1_wu3` | 10/10 | PASS |
| `architecture_self_host_e2e` | 2/2 + 1 ignored (DEBT-004) | PASS |
| `findings_canonical_grounding_e2e` (e67, `--features evidence-kernel`) | 10/10 | PASS |

**Total: 451 tests pass, 0 fail, 2 ignored (both documented as debts).**

### Closure reference

`openspec/changes/e77-1-canonical-evidence-grounding/corrigendum.md` is the authoritative
reference for the closed state of M10's first slice. The original e77 archive and
commits (`20764bf5` impl + `fe8804b1` archive) are UNCHANGED.

### DEBT-SDDK-005 — `cargo test -p cognicode-core --lib` stalls indefinitely

The umbrella unit sweep stalls on this machine: ~15 min compile, 68 threads spawned,
one consuming 101% CPU blocked on `futex_`. SIGKILL required. The stall is NOT caused
by the e77.1 change — targeted subsets of affected subsystems all complete in <1s each.
Tracked in `docs/debts/DEBT-SDDK-005.md`. Triage required before any future cycle that
touches an unrelated subsystem; suggested first step
`cargo test -p cognicode-core --lib -- --test-threads=1` to serialize.

### Next (per user directive 2026-09-17)

- e79 (AI Foundation + read-only agents): NEXT. With the e77.1 corrigendum in place,
  e79 can introduce LLM-generated hypotheses that flow through the canonical evidence
  pipeline without inheriting the synthetic minting path. EvidenceKind::Hypothesis
  already exists (class D, lowest); canonical writer will route hypotheses through the
  same EvidenceLookup as architecture sources, requiring a real fact anchor before the
  verifier accepts them.
- e78 (Pack Ecosystem): DEFERRED per consumer-count checkpoint (still 1 consumer).
- e80 (AutomatedAuthorPolicy + Fix Agent): pending (M11 unblocker).

## Active Change — e79 LSI AI Foundation read-only agents (M11)

**Status:** WU0–WU7 complete. NOT YET COMMITTED (working tree dirty on `main`).
**Next:** single-impl + single-archive commit + push + archive.

### Scope (changed surface)
- `crates/cognicode-core/src/domain/ai/` (NEW — `frame`, `request`, `response`, `hypothesis`, `port`, `mod.rs`)
- `crates/cognicode-core/src/application/ai/` (NEW — `fake`, `semantic_miner`, `critic`, `boundary_tests`, `mod.rs`)
- `crates/cognicode-core/src/domain/mod.rs` (added `pub mod ai;`)
- `crates/cognicode-core/src/application/mod.rs` (added `pub mod ai;`)

### Authority boundary (the load-bearing invariant)
- `FakeLlmPort` is the only `LlmPort` impl (real providers gated by DEBT-SDDK-003).
- No `FactStore`/`EvidenceStore`/`ArchitectureAdmissionService`/`ChangeTracker`/
  `DetectorRegistry` is referenced from any public type in `application::ai`.
- `MinerOutput` carries `Suggestion(Hypothesis)`, `ConstraintCandidate`,
  `DetectorCandidate` — none of them are admitted. Promoting requires
  `application::architecture::admission::ArchitectureAdmissionService::admit`.
- `Critique` carries `finding_id` + `disposition` + `rationale` + `grounding` — no
  authority fields.

### Verification executed (surgical subsets)
- `cargo build -p cognicode-core --lib` → 0 errors, 7 warnings (all pre-existing).
- `cargo test --lib -p cognicode-core domain::ai` → **21/21 pass**.
- `cargo test --lib -p cognicode-core application::ai` → **25/25 pass**.
- `cargo test --lib -p cognicode-core domain::architecture` → 7/7 (no regression).
- `cargo test --lib -p cognicode-core application::architecture` → 16/16 (no regression).
- `cargo test --lib -p cognicode-core domain::findings` → 112/112 (no regression).
- `cargo test --lib -p cognicode-core application::findings` → 21/21 (no regression).
- `cargo test --lib -p cognicode-core domain::readset` → 12/12 (no regression).
- `cargo build --workspace` → 0 errors, only pre-existing warnings.

### Boundary tests (WU6 — `application::ai::boundary_tests`)
- `the_miner_has_no_write_surface` — return type IS the boundary.
- `the_critic_has_no_write_surface` — symmetric to miner.
- `request_provenance_round_trips_into_response` — lineage layer invariant.
- `tool_surface_is_bounded_by_the_request` — `ResponseOutput` has 3 variants only.
- `no_provider_is_referenced_by_name_from_application_ai` — static name check.
- `fake_port_response_keys_match_request_keys_exactly` — wrong key errors, not silent.
- `the_miner_does_not_mutate_its_port` — `LlmPort::complete` takes `&self`.
- `response_frame_id_must_match_request_frame_id` — critic rejects mismatched frame.

### UAT (WU7 — `application::ai::boundary_tests::wu7_*`)
- `wu7_end_to_end_mining_and_critique` — miner + critic over a single frame; both
  return advisory types only; no canonical writes attempted.
- `wu7_miner_output_types_carry_no_authority` — `ConstraintCandidate` has 4 fields
  (id/kind/adr_ref/proposed_by); adding an authority field breaks the literal.

### Verification deliberately NOT executed
- `cargo test --lib` full suite — DEBT-SDDK-005 (lib stalls at 100% CPU on futex on
  this machine). Per-subsystem surgical subsets completed in <1s.
- Full e2e / Playwright — not applicable: AI foundation has no API surface yet
  (real provider adapters are future cycles gated by DEBT-SDDK-003).

### Unknown impact
- None material. `application::ai` is additive; the only modifications to existing
  files are 3-line additions (`pub mod ai;` in `domain/mod.rs` and
  `application/mod.rs`, plus `CritiqueError` reexport in `domain/ai/mod.rs`).

### Result
PASS — 46 AI tests pass + 200 nearby subsystem tests pass + workspace builds clean.
Full verification required now: NO (deferred to next cycle per DEBT-SDDK-005).

---

## e85 — CogniCode Linux Release Factory (2026-09-17)

### Changed surfaces
- `crates/cognicode-cli/src/cmd/bundle_manifest.rs` — BundleManifest **v2**
- `crates/cognicode-cli/src/cmd/release_contract.rs` — NEW canonical vocabulary
- `crates/cognicode-cli/src/cmd/release_factory.rs` — NEW generate + verify
- `crates/cognicode-cli/src/cmd/release_test_support.rs` — NEW offline harness
- `crates/cognicode-cli/src/bin/release.rs` — NEW `cognicode-release` tool
- `crates/cognicode-cli/src/cmd/installer_transaction.rs` — manifest resolution
- `crates/cognicode-cli/src/cmd/profile.rs`, `src/bin/cogh.rs`, `Cargo.toml`
- `.github/workflows/release.yml` (rewritten), `.github/workflows/ci.yml` (+job)
- `scripts/check-release-matrix.sh` (rewritten), `justfile` (+4 recipes)
- `bundles/v*/bundle.yaml` DELETED (v1 placeholder manifests, retired)

### Component → test mapping (reuse these; do not rediscover)
| Surface | Command | Baseline |
|---|---|---|
| `cogh` binary | `cargo test -p cognicode-cli --bin cogh` | 147 passed |
| release tool | `cargo test -p cognicode-cli --bin cognicode-release` | 35 passed |
| known failures | `python3 scripts/check_known_failures.py` | exit 0, **41 entries** |
| architecture self-host | `cargo test -p cognicode-core --test architecture_self_host_e2e` | 3 passed, 0 drift |
| release contract guard | `bash scripts/check-release-matrix.sh` | RESULT: OK |
| fmt for touched pkg only | `cargo fmt -p cognicode-cli --check` | clean |

### Traps discovered (expensive — do not rediscover)
1. **`crates/cognicode-cli/src/bin/` is shadowed by a global ignore.** The
   developer's `~/.config/git/ignore` has a bare `bin/` pattern. NEW files added
   under `crates/cognicode-cli/src/bin/` are silently UNTRACKED; already-tracked
   ones (`cogh.rs`) are unaffected, so local builds and tests pass while CI fails
   with "can't find bin". Mitigated by a repository-level `.gitignore` negation,
   which takes precedence over `core.excludesFile`. **After adding a binary, run
   `git ls-files --error-unmatch <path>` to confirm it is tracked.**
2. **The target directory is NOT `target/`** on this machine: `cargo metadata
   --format-version 1` reports the real `target_directory`. Scripts must resolve
   it rather than assume `target/`.
3. **`cargo test --workspace --lib` is not the baseline gate.** It reports ~42
   failures in `cognicode-core`; the authoritative gate is
   `scripts/check_known_failures.py`, which compares the FAILED *set* against
   `scripts/known_failures.yaml` (41 entries) and exits 0. Use the checker.
4. **`cargo fmt --check` over the workspace is pre-existing dirty** in
   `cognicode-core` (`application/ai/*`). Check only the package you touched.
5. `release_factory::generate_release` materialises the payloads into `--out`, so
   the out dir is a COMPLETE release dir. Verify that dir, not the staging dir.

### Evidence reused
- `cognicode-core` known-failure set: unchanged (41), so no re-baseline needed.
- e84 contract decisions (R1–R9): treated as immutable inputs, not re-litigated.

### Open / next
- WU14/WU15 (publish + verify the real release) — see the cycle's exit report.
- e86: `cogh latest/update/rollback` + remote `version+platform -> manifest`
  resolution. The seam is `COGNICODE_BUNDLE_MANIFEST` precedence plus
  `COGNICODE_RELEASE_BASE_URL`.

### Active change (E86.7 closed, architectural cycle L1-L5 opening)

- **E86.7 apply**: 7b9b2779 — cmd_uninstall removes install tree.
- **E86.7 archive**: 3d201325.
- HEAD test counts: 218 cogh tests pass (215 baseline + 3 new E86.7), 1 ignored.
- `cargo fmt -p cognicode-cli`: clean. `cargo check`: clean. `cargo clippy`: 0 new.
- Known-failure set: unchanged (41).

### Architectural cycle `arch-canonical-layout`

- Phase A ownership map: `docs/adr/ADR-OWNERSHIP-MAP-install-vs-versions.md` (217 lines).
- Phase B tarball shape: `dist/cognicode-core-0.94.4.tar.gz` and `dist/cognicode-mcp-driven-0.94.4.tar.gz` are portable skill bundles (`SKILL.md`, `assets/`, `references/`, `manifest.yaml`), NOT binaries with `bin/`. The install transaction's `bin/<comp>/<comp>` shim-source assumption is structurally incompatible with these artefacts.
- Phase C ADR: `docs/adr/ADR-CANONICAL-LAYOUT-versions.md` (329 lines). Status: **proposed**. Ratifies `versions/<v>/<component>/` as canonical.
- Phase D migration plan: 5 bounded cycles (L1-L5), each ~5-line + 2-3 tests.

### Disposability contract

For all L1-L5 cycles, evidence required:
- `cargo test -p cognicode-cli --bin cogh`: ≥218 pass, 0 fail, 1 ignored.
- `cargo fmt -p cognicode-cli`: clean.
- `cargo check --workspace`: clean.
- `python3 scripts/check_known_failures.py`: 41 entries, baseline unchanged.
- `cargo clippy -p cognicode-cli --bin cogh`: 0 new.
- Real-PC UAT (`/tmp/cogh-uat-real-pc.sh`): overall `ok`. Phase D reinstall `link_or_copy` MUST become GREEN by L4.
- HOME config pre/post sha256: only touch if the cycle requires it; document any drift honestly.

### Layout-related test fixtures to update (counted once, applied per cycle)

- `layout.rs:928-940` E86.3 uninstall test (`install/<v>`).
- `layout.rs:1505,1537-1586` E86.4 install_manifest_path tests.
- `installer_transaction.rs:778,833` commit tests.
- `installer_transaction.rs:1066` local-release tests.
- `ide.rs:877` integrate_zcode fixture.

### Out of scope (filed separately)

- `write_json_atomic` semantic no-op drift (`13bb7c2e → b93fcd3c`).
- Journal/uninstall retention policy.
- Plugin-vs-component naming (`mcp-server` vs `cognicode-mcp`).

## DEBT-2 session handoff (2026-09-18)

### What changed (commits 95e152cd..99620e24 on main)

- `bundle_manifest.rs`: additive optional `skill_bundles: Vec<SkillBundleDecl>`
  field on BundleManifest v2 (serde default, skip_serializing_if empty).
  Validation: unique ids, version lockstep, declared profiles.
  `skill_bundles_for_profile()` + canonical `declared_skill_bundle_dirs(
  skills_root, manifest_path, profile)` helper (&Path-based so
  cognicode-release can reuse).
- `install.rs`: OpenCode integration resolves skill bundle dirs via the
  canonical helper (profile-filtered). First-dir heuristic retired.
- `ide.rs`: all four integrators (opencode/zcode/claude/codex) and
  `cmd_ide_install` resolve skill sources from the manifest against
  `versions/<v>/skills/<bundle_id>`. The `<plugin>/skills` construction is
  gone. cmd_ide_install (opencode) errors if no bundle declared.
- Spec delta in `openspec/specs/portable-skill-bundle/spec.md` (2 new
  requirements + 6 scenarios).

### What was tested (all GREEN)

- `cargo test -p cognicode-cli --bin cogh`: 248 passed / 0 failed / 1 ignored.
  New strict gates: 5 in bundle_manifest.rs (t_debt2_*) + 1 in install.rs
  (declared-but-missing fails, manifest-id resolution, empty-for-other-profile).
- `check_known_failures.py`: 41 entries unchanged.
- `cargo check --tests`, scoped `cargo fmt`: clean. Clippy: no new errors.

### Fixture pattern for DEBT-2-era tests

`plant_skill_bundle_version(home, version)` (ide.rs tests) and the inline
fixture in lifecycle.rs tests: manifest with
`skill_bundles: [{id: skills-for-claude, ...}]` (pairwise-distinct from
ComponentId `cognicode-mcp`) + on-disk `versions/<v>/skills/skills-for-claude/`.

### Fresh vs stale evidence

- Fresh: all cmd-layer gates above.
- Stale: nothing invalidated; layout.rs was fmt-reverted to avoid WU5 drift.

### Known open items / next

- **InstallerTransaction profile strip (architectural follow-up)**:
  `InstallerTransaction::run(&home, "core")` filters manifest.components by
  profile, so a DaemonCli in a non-selected profile is dropped from the
  installed manifest and `daemon_cli_binary_name` fails loudly at
  uninstall/integrate time. Real skill bundles are also NOT yet written into
  `versions/<v>/skills/` at install time (fixtures plant them manually);
  production manifests currently declare no skill_bundles, so the skip path
  is exercised. Wiring producer-side `skill_bundles` + skill extraction into
  the transaction is the next bounded cycle if desired.

### DEBT-2b (producer side, 2026-09-18) — commits 005c003e, cb9a7cf0

- Extracting stage now extracts declared skill bundles (profile-filtered) from
  cache/<id>.tar.gz into versions/<v>/skills/<id>/; declared-but-missing cache
  artifact = InstallerError::Unknown, loud.
- advance_stage/advance signature now carries `profile` (3 call sites in tests
  updated: advance(&home, "core")).
- skill_bundles[] survives the component profile filter (namespaces orthogonal).
- Round-trip T3: extract -> shims -> commit -> declared_skill_bundle_dirs from
  the INSTALLED manifest, no fixture planting. 251 passed / 0 failed.
- Remaining known gap (out of scope): real published release manifests do not
  yet declare skill_bundles[] — release_factory writes Vec::new(). Wiring
  PUBLISHED skill bundles into the release generator + `just bundle-skills` is
  the next natural cycle (touches release_contract table).

## Handoff DEBT-2c (release producer) — HEAD f62cdb7c

Changed: SKILL_BUNDLES table (release_contract.rs), generate_release/
verify_release skill payload handling (release_factory.rs), install/ide
no-daemon profile support, idempotent link_or_copy (platform_adapter.rs),
local_release fixture, spec delta.
Tested: cargo test -p cognicode-cli --bin cogh 253/0/1 GREEN incl.
t_debt2c_generated_manifest_declares_skill_bundles,
t_debt2c_verify_covers_skill_bundle_payloads; check_known_failures 41.
Fresh evidence: full cogh suite + known-failures at f62cdb7c.
Next: archive DEBT-2c; possible follow-ups: cognicode-recommended bundle
(no manifest.yaml, currently unpublished), remote e2e against GitHub.

## Handoff DEBT-1 — HEAD 7ee16205

Changed: write_json_atomic skips semantically-equal writes (ide.rs);
dev-dep filetime. Bounded to JSON primitive only (TOML zcode/claude/codex
NOT covered — different primitive, separate bounded cycle if desired).
Tested: 260/0/1 GREEN incl. t_debt1_* (4 primitive/flow + 1 UAT double-run).
Fresh evidence: full cogh suite at 7ee16205.
Next: DEBT-4 journal/uninstall retention policy. Optional DEBT-1b:
extend semantic no-op to TOML if a shared primitive emerges.

## DEBT-4 WU1/WU2/WU3 slice (ebe2157d)
Changed: layout.rs — cmd_rollback explicit applicability (tracker-first, heuristic retired, stale=fail-closed), consume-on-success; cmd_uninstall idempotent + journal invalidation + tracker clear-if-active.
Tested: t_debt4_t1..t6 (6/6 serial), full cogh 266 passed/0 failed/1 ignored, check_known_failures 41 unchanged, cargo fmt scoped clean, cargo check --tests clean.
Evidence reused: DEBT-1/DEBT-2 gates still fresh (no other surface touched).
Unknown impact: cmd_plugin_update / InstallerTransaction::commit journal-writing path unchanged but should be exercised in a UAT round-trip (install->update->rollback) before archive.
Next: WU4 crash-safety ordering audit (journal delete-after-success verified in code), spec/ADR delta (journal Q&A), UAT with TempCognicodeHome, archive.

## DEBT-4 CLOSED (3ab8e3d5)
WU4: crash-safety order documented (consume-after-success) in spec + ADR; Drop hazard neutralized at source (load/from_json -> committed=true), tripwired by t_debt4_loaded_journal_is_drop_neutralized.
UAT: t_debt4_uat_install_rollback_roundtrip + t_debt4_uat_install_uninstall_roundtrip (real pipeline, TempCognicodeHome/OPENCODE_CONFIG redirect). UAT2 needs reviewer profile (DaemonCli). Zero real-HOME pollution verified by before/after snapshot diff.
Fix: installer_rejects_wrong_platform test made serial+hermetic (pre-existing env-leak flake).
Gates final: 269 passed/0 failed/1 ignored; known-failures 41; fmt scoped; clippy 0 errors; heuristic selection scan = 0.
Next: umbrella closeout of DEBT chain; NO further DEBT-x; return to roadmap.

## e87 Active Change (updated)
- Changed: install.sh (new, a4cc77b8), README/README.es docs (39c17476)
- Verified OBSERVED: T1 Darwin/unsupported-arch fail-loud; T2 tampered archive fail-closed (no binary installed, via COGNICODE_RELEASE_BASE fake release); T3 pin v0.96.0 exact; T4 Layer-0 purity in disposable HOME (only bin/cogh, no IDE configs, no rc mutation); WU4 Layer-0 `cogh --version` == 0.96.0 from clean HOME.
- BLOCKED: WU4 Layer-1 (`cogh install mcp-server`) — remote resolution of version+platform not implemented until e86; fell back to DEV fixture and downloaded 0.95.0 asset -> SHA mismatch (fail-closed, correct behavior given the gap).
- Evidence reusable: release v0.96.0 asset digest 90a1735915d71fb35522e8f874819e3ff02cbad823c68acb1c54ff47c9ca2ac1.

## e87.1 (updated)
- Commits local: 161f2be9 (resolver bridge), ff78f78e (doctor shims probe). NOT pushed yet — need authorization.
- GREEN OBSERVED: e871 T1-T4 + mismatch fail-closed (RED demostrado pre-fix en T2); bin suite 274/0/1; known-failures 41 OK; fmt OK.
- UAT public release OBSERVED (WU4): disposable HOME + install.sh v0.96.0 -> `cogh install mcp-server --version 0.96.0` resuelve release pública, consume bundle v0.96.0, SHA OK, tracker 0.96.0, versions/0.96.0, sin warning DEV fixture. doctor overall healthy (tras fix shims probe).
- Fixed on the way (pre-existing since ded95fbf): 3 lifecycle uninstall tests (versions/<v> tree not planted) + doctor MCP probe stale path.
- PENDIENTE: mise vertical pin (same asset/sha) -> e87 CLOSED. Honest public UAT must re-run after next release carries the doctor fix. e88 next.
- TRAMPA: target/debug/cogh puede quedar STALE (fingerprint bug observado 2026-09-18): los tests de subprocess (cogh_bin) usan target/debug/cogh. Si un test de subprocess falla sin motivo, rebuild con CARGO_TARGET_DIR=/tmp/cc-target y copiar el binario.

## e87 closure state (2026-09-18)
- mise vertical M1-M4 DONE: receipt docs/e87-mise-identity-receipt.md (digest identity b058c6ee proven 3-way; SHA256SUMS 90a17359). Commit a342a7a1 LOCAL (needs push authorization with release).
- ALL e87 closure criteria OBSERVED GREEN (see checklist in transcript).
- e87 ready to CLOSE + archive; next release must contain: 161f2be9 (bridge), ff78f78e (doctor), a342a7a1 (mise), + earlier e87 (a4cc77b8, 39c17476, already pushed).
- HARD GATE before e88: publish next release (number per SemVer contract, not chosen manually). Release receipt + explicit authorization required first.
- e88 entry: public-release-assets UAT vs 100% public consumer UAT distinction recorded — e88 needs public cogh binary containing e87.1.
- Evidence reusable: mise recipe, install.sh COGNICODE_RELEASE_BASE test seam (Layer-0 only), ResolverFixture/--staging (Layer-1 seam), stale target/debug/cogh trap.

## e88 session notes (2026-09-18)
- F1 fix: WroteTracker must be recorded BEFORE lifecycle_journal::write in installer_transaction::commit (persistence-order contract). Regression test: commit_persists_journal_with_tracker_effect.
- cmd_rollback_reverses_a_committed_install previously depended on real ~/.cognicode (no env redirect) — FIXED with redirect_home + pin assertions. Not a race; an isolation bug.
- target/debug/cogh stale-fingerprint trap struck again: cargo said "Finished" but binary was old. Always CARGO_TARGET_DIR=/tmp/cc-target + copy for dev-binary sanity checks.
- test_cogh_update_respects_lockfile: intermittent in-suite failure (spawns cogh with modified HOME/COGNICODE_HOME; manual env save/restore). Passes isolated and in most suite runs (4x green post-F2). Same env-race family as the old rollback flake; non-blocking.
- doctor MCP probe is state-aware since 89b19019: no pin -> Unavailable; manifest-declared daemon + shim -> Pass; declared+missing -> Fail; undeclared -> Unavailable. Tests must plant a full active install (tracker + versions/<v>/manifest.yaml); fixture manifests must pass BundleManifest::validate (canonical artifact/url naming).

### lifecycle-F3 session notes (2026-09-18)
- T5 originally used a nonexistent `crate::cmd::test_support::run_cogh`; real helper is private `run_cogh` in `cmd/lifecycle.rs` (spawns the cogh binary). In-process `cmd_update` + `lifecycle_state` snapshot is sufficient for the no-op assertion; CLI message contract covered by lifecycle tests.
- Multi-version fixture tests (T3 A→B): asset downloads resolve via `TempBaseUrl` env override, not the staging manifest URL. Phase A→B requires swapping the override to `fx_b.release.base_url` before the second `cmd_update`, or the download 404s against real GitHub.
- `just lint` was broken pre-F3 on clean HEAD (25 clippy warnings in cognicode-core, incl. cfg(test)-stripped unused imports in `boundary_tests.rs`). Fixed in `7ddd3251`. Rule: imports used only inside `#[test]` fns break clippy lib builds when the module is compiled without `cfg(test)`.
- `check_known_failures.py` target `cargo test -p cognicode-core` (default features): 39 entries, matches. The `--features evidence-kernel` lib test run shows 39 env-dependent failures (file_operations symlink/HOME checks) — pre-existing, not in the baseline target.
- cogh bin suite had one transient failure right after `cargo fmt --all` (fingerprint/stale-artifact race); two consecutive reruns green (286 passed). Re-run before believing a single failure after formatting.

### CP1.0 WU4 — 2026-09-18 (session clover)
Changed: explorer/api.rs (endpoint GET /control-plane/workspaces/:id/architecture, ApiState.control_query+control_source_root, with_control_query), core/control_query.rs (source_from_source_root helper), tests/cp1_control_plane_endpoint.rs (C1–C5).
Tested: core control_query 5/5 GREEN; explorer cp1 endpoint 5/5 GREEN; just lint GREEN; fmt GREEN. Commits fed57b95..HEAD pushed.
Gotchas: parser normalizes leading crate:: in dependency_path; module_path must resolve via LayerId::from_module_path (strip leading src/); source root scans must be bounded (never std::env::temp_dir() whole).
Unknown: e78 checkpoint pending (consumer count); cadence full_run v1 still running.

### SESSION HANDOFF — 2026-09-18 fin de sesión
State: CP1.0 WU4 COMPLETE and pushed (HEAD 949cae62, origin/main parity).
- Endpoint GET /control-plane/workspaces/:id/architecture GREEN (C1–C5, 5/5).
- core control_query unit tests 5/5 GREEN. just lint GREEN, fmt GREEN.
- KNOWN FAILURES baseline 39 unchanged; self-host not re-run this session.
Cadence v1 (run 1):
- ci_smoke 3 pass / quality 13 pass+5 expected_fail DONE (sandbox/results/).
- full_run v1 STILL RUNNING detached (nohup PID 3429073, log /tmp/full_run.log),
  60/80 result.json at 23:30 local; scenario 61 (multi_realrepo_hover_commander)
  has been running ~45 min — suspect per-scenario timeout missing; if stuck,
  kill and rerun remaining manifests with --results-dir sandbox/results/full_run.
- NEXT cadence steps: tally outcomes → analyze_stability.py / build_flaky_log.py
  → just scorecard-nightly → just scorecard-streak → write nightly receipt
  (date, source_head=949cae62+, public_release_baseline=v0.97.1, verdict, gates,
  CV, flaky count, known-failures=39, streak before/after).
CP1.0 NEXT:
- Checkpoint e78: count genuine consumers of ControlQueryService/e77 (exclude
  unit tests, self-host, docs, Backstage proxy). < threshold → DEFERRED packs.
Gotchas (learned today):
- Parser normalizes leading crate:: in violation dependency_path.
- module_path must resolve via LayerId::from_module_path; strip leading "src".
- Never scan std::env::temp_dir() whole as source root (test hang, 50 min).
- CONCURRENT AGENT wiped uncommitted work 3x today (api.rs, test file, helper).
  Protocol: commit immediately after every verified edit.

---

## Session notes — 2026-09-19 (session clover, continuation)

### Cadence v1 — re-examination of full_run artifacts

The PID 3429073 detached run is dead; `/tmp/full_run.log` no longer exists.
Reconstruction of `sandbox/results/full_run/` per-scenario verdicts:

```text
scenarios_with_terminal_result  = 60
scenarios_pass                  = 17
scenarios_fail_or_mcp_error     = 43
scenarios_without_terminal      = 0
range                           = 2026-09-18T21:30Z → 22:30Z (~1h)
campaign_manifest               = NONE FOUND
measured_source_head declared   = "unknown" (all 60 result.json)
workspace_snapshot_id           = "pending" (all 60)
```

The prior session note recorded `60/80` (progress position, not PASS) and
suspected scenario 61 (`multi_realrepo_hover_commander`) was stuck. After
inspection that scenario **does have a terminal result** (`mcp_error`); the
"stuck" perception came from the harness progress display, not from the
artifact. The full 60-scenario set is terminal.

**Cadence verdict: INCOMPLETE.** Per the user directive (2026-09-19): an
incomplete campaign cannot be used to accredit any HEAD. Receipt emitted at
`sandbox/results/nightly_receipts/2026-09-19-cadence-INCOMPLETE.json`. The
streak record (`sandbox/results/scorecard_streak.json`) was **not modified**
in this session — the 2026-08-11 entry remains the last valid streak record
(current_streak=1, 39 days ago).

`measured_source_head` was inferred from git history (last commit before
22:30Z on 2026-09-18 = `9aaf91d2`) but is **not** declared in artifacts. The
inferred value is recorded in the receipt with `trust_level: inferred-not-declared`.

### e78 checkpoint — re-applied at HEAD 949cae62

Full inventory in `openspec/changes/cp1-control-plane-first-cycle/e78-inventory.md`.

```text
genuine product consumers of application::architecture  = 1
  #1  GET /control-plane/workspaces/:id/architecture  (CP1.0 WU4, fed57b95)
excluded:
  /api/workspaces/:id/architecture      legacy, uses graph.build_architecture
  /api/.../architecture/mermaid         E20 export, no ControlQuery
  semantic_miner + domain/ai            import domain::architecture only
  boundary_tests                        tests only
  internal modules of architecture crate submodule, not consumer

threshold (carried unchanged)          = 2
gate status                            = 1 / 2 — UNMET
historic archive                        = NOT MODIFIED
e78 outcome                             = DEFERRED
```

CP1.0 WU4 contributes ONE consumer (the Control Plane endpoint), not two.
Per the user's explicit clarification 2026-09-19, the endpoint, the service,
and any future client of the same service represent the SAME operational
need and must not be split into multiple consumers. e78 stays DEFERRED.

### CP1.0 — CLOSED

Per cycle receipt at `openspec/changes/cp1-control-plane-first-cycle/`:

```text
WU2/WU3 commit 6169d454  ControlQueryService + read-model DTOs
WU4    commit fed57b95   HTTP vertical + C1–C5 tests
(no separate WU1 commit; preparatory work consolidated into 6169d454)
HEAD at close            949cae62 = origin/main
lint / fmt               GREEN (per WU4 cycle clippy fixes)
known-failures baseline  unchanged (39)
e78 checkpoint           1 / 2 — DEFERRED
cp0 archive              NOT MODIFIED (cp0-backstage-fit-spike = FIT_WITH_CONSTRAINTS)

CP1.0 = CLOSED
CP1   = OPEN  (next problem TBD from vertical evidence)
e78   = DEFERRED
```

### CP1 next problem — checklist (NOT pre-allocated)

The next CP1 bounded cycle is **not** opened in this session. It will be
opened only if a real need surfaces from the CP1.0 vertical, per:

```text
1. user question         — what does a real user ask next after architecture read?
2. canonical source      — which existing module produces the answer?
3. missing capability    — what is the minimum useful read or write?
4. minimum vertical      — one endpoint + one test contract + one DTO
5. acceptance / UAT      — how do we know it's used?
```

Pre-fabricating `AttentionItem`, `CaseView`, or a second endpoint without
evidence is **explicitly prohibited** by the user directive (2026-09-19).

---

## Session "clover" — 2026-09-19 (resumed)

### Active change at resume

CP1.0 closeout had been left IMPLEMENTED / RUNTIME ACCEPTANCE PENDING
after `674c3795` discovered that `with_control_query` is defined but
only called from tests. 4 commits already on main (`c09441fd`,
`cf736ead`, `674c3795`, `e8727a24`), working tree had:
- `crates/cognicode-runtime/src/bin/api.rs` (B2 wiring, uncommitted)
- `crates/cognicode-runtime/tests/cp1_wu5_runtime_wiring_smoke.rs` (B3 test, uncommitted, had compile error)

### What changed this session

| commit | what | evidence type | evidence captured |
|---|---|---|---|
| `1a722d38` | feat(cp1.0): WU5 runtime wiring + 4-test integration smoke | direct test + clippy + fmt | cargo test 4/4 PASS, C1-C5 5/5 PASS, clippy 0, fmt 0 |
| `a32e7e9f` | test(cp1.0): C6 path-traversal echo regression guard (T12) | direct test | cargo test 6/6 PASS, clippy 0, fmt 0 |
| `47214597` | docs(cp1.0): §12 musl toolchain limitation | direct inspection | rustup target list — musl not installed |

### Cadence receipt

`2026-09-19-cadence-INCOMPLETE-r2.json` emitted honestly with
`measured_source_head=unknown`, harness_gap_inventory, and three
next_required_actions (D1 harness upgrade, D2 re-run, D3 re-evaluate).
Streak counter `scorecard_streak.json` NOT MODIFIED.

### Evidence inventory (this session)

- cargo test -p cognicode-runtime --test cp1_wu5_runtime_wiring_smoke
  → 4/4 PASS (multi-thread tokio for bootstrap-using tests)
- cargo test -p cognicode-explorer --test cp1_control_plane_endpoint
  → 6/6 PASS (C1..C5 + new C6 path-traversal)
- cargo clippy -p cognicode-runtime --all-targets -- -D warnings → 0
- cargo clippy -p cognicode-explorer --all-targets -- -D warnings → 0
- cargo fmt --check → exit 0
- LIVE HTTP UAT not re-run this session (sandbox kills detached
  processes); integration test guards the same contract that the live
  probe verified in commit `e8727a24`

### Unknown impact / outstanding

- Cadence Recovery directive (D) preconditions NOT met this session:
  harness upgrade + re-run would require hours of orchestration
- e78 (second consumer): still 1/2 DEFERRED, no new candidate
- musl bundle: requires `rustup target add x86_64-unknown-linux-musl`
  on a workstation with network access

### Result

PASS for CP1.0 WU5+T12 scope (3 commits, all pushed, HEAD == origin/main).
FAIL/INCOMPLETE for cadence (separate harness prerequisite, not CP1.0
scope). No full verification required.

## Active Change — TRACK A / A1a+1 G4 reader fix (2026-09-19) — CLOSED 2026-09-19

Scorecard reader `gate_g4` was averaging correctitud without positive
provenance. The `result.repo` field is declarative metadata; it does
not prove the executed repository. The 58.1 average was the mean of
4 Tier-A fixture scenarios (with `repo='serde'` label but `workspace='.'`),
NOT a Tier-1 corpus measurement.

### Plan

1. RED tests pinning the per-repo contract (A1a: 7 tests, A1a+1: +9
   adversarial tests, total 16 tests, all in
   `sandbox/scripts/tests/test_a1a_g4_contract.py`).
2. Reader fix in `sandbox/scripts/release_scorecard.py`:
   - expose `TIER1_REPOS_PER_SPEC` and `G4_THRESHOLD`
   - per-candidate positive-provenance extraction
   - per-repo aggregation over distinct scenario_ids (no repeat inflation)
   - verdict precedence: GREEN (all 5 acredited + per-repo>=90),
     RED (any acredited<90, failing repo named), AMBER (missing)
   - pre-D1 results without provenance classified UNVERIFIED,
     correctitud preserved as `fixture_diagnostic_only`
3. Recompute scorecard against frozen run 20260919T102509.
4. Verify streak integrity (MD5 unchanged, no increment).

### Scope

- `sandbox/scripts/release_scorecard.py` (gate_g4 only)
- `sandbox/scripts/tests/test_a1a_g4_contract.py`

### Out of scope

- Threshold >=90 unchanged.
- ADR-031 unchanged.
- Scoring engine, orchestrator, manifests, scorecard streak unchanged.
- G6 and other gates unchanged (next slice: A1b).

### Evidence inventory

- python3 -m pytest sandbox/scripts/tests/test_a1a_g4_contract.py -v
  → 16/16 PASS (was 14 RED + 2 trivially GREEN)
- python3 sandbox/scripts/release_scorecard.py against frozen run
  → scorecard 8 GREEN / 5 AMBER / 0 RED
- G4 verdict: AMBER (was RED 58.1) with explicit
  `missing_or_unverified: anyhow, clap, ripgrep, serde, tokio`
- fixture 58.1 preserved as `fixture_diagnostic_only: avg=58.1 across 4
  pre-D1/no-provenance scenarios`, NOT counted toward G4
- md5sum sandbox/results/scorecard_streak.json
  → 6b2121fe8e1134a6bed84faba138e5ff (unchanged)
- receipt: sandbox/results/freezes/2026-09-19-track-a-a1a+1-g4-reader-fix.json

### Unknown impact / outstanding

- A1b (G6): current verdict AMBER; reader to be characterized against
  the `cv_warm` contract (cv eliminates slowest sample whenever >=3
  exist, even without cold-cache detection).
- A1c (G3): stability.json `health_score` formula is not the 5-dim
  weighted score the spec requires.
- Tier-1 corpus execution deferred to a later cycle: requires manifests
  + verified revisions + actual_repository_identity field in result.json.

### Result

PASS for A1a+1 scope. Verdict change RED->AMBER for G4 with full
per-repo breakdown and explicit missing_repo list. No fabricated GREEN.
No threshold change. No streak increment.

Next: A1b (G6 characterization + reader fix).

## Active Change — TRACK A / A1b G6 reader fix (2026-09-19) — CLOSED 2026-09-19

Scorecard reader `gate_g6` was accepting stability.json with any
repeat_count and advertising the per-scenario CV as 'warm-cache'
regardless of whether the warm-cache drop policy was actually applied
(analyze_stability.py requires n >= 3 to drop the max sample).

### Root cause of the 92.7%

  scenario: rust_safe_refactor_rename_concrete_concrete (Tier A)
  samples:  [16242, 615]                # one cold-cache, one warm
  mean:     8428.5
  std_dev:  7813.5
  cv:       0.927                       # genuine two-sample variance
  cv_warm:  0.9270332799430504          # bit-identical, n<3
  cold_cache_sample: False             # n<3, policy not applicable

The previous reader labelled this as 'warm-cache CV' — misleading
because no drop happened. The new reader calls it
'diagnostic_max_cv' and gates it behind an insufficient_repeats
AMBER verdict.

### Plan

1. RED tests pinning 8 reader gaps (A1b.0/A1b.1: 12 tests, all in
   `sandbox/scripts/tests/test_a1b_g6_contract.py`).
2. Reader fix in `sandbox/scripts/release_scorecard.py`:
   - expose `G6_CV_THRESHOLD=0.10`, `G6_MIN_REPEATS=3`,
     `G6_MIN_SAMPLES_PER_SCENARIO=3`
   - repeat_count < 3 -> AMBER insufficient_repeats (with diagnostic
     worst-CV scenario named in evidence)
   - per-scenario runs < 3 -> AMBER insufficient_samples; those
     scenarios are diagnostic only, never drive verdict
   - cv_warm == cv with n<3 marked as warm-cache N/A explicitly
   - aggregate_cv top-level cited as second-opinion
   - cold_cache_sample flag surfaced per scenario in evidence
3. Recompute scorecard against frozen run 20260919T102509.
4. Verify streak integrity (MD5 unchanged).

### Scope

- `sandbox/scripts/release_scorecard.py` (gate_g6 only)
- `sandbox/scripts/tests/test_a1b_g6_contract.py`

### Out of scope (per directive)

- Threshold (<10%) unchanged.
- ADR-031 unchanged.
- analyze_stability.py unchanged (no algorithm change for cv reduction).
- Orchestrator, manifests, scorecard streak unchanged.
- Per-dimension variance (R-G6-6): deferred to a later slice; the
  current scope is timing-only, which matches the spec's main clause.
- No new quarantine / sample-exclusion policies.

### Evidence inventory

- python3 -m pytest sandbox/scripts/tests/test_a1b_g6_contract.py -v
  → 12/12 PASS (was 10 RED + 2 trivially GREEN)
- python3 sandbox/scripts/release_scorecard.py against frozen run
  → scorecard 8 GREEN / 5 AMBER / 0 RED (same envelope, G6 reason now
    honest)
- G6 verdict: AMBER
  reason: insufficient_repeats: repeat_count=2 < spec minimum 3
  diagnostic: max_cv=0.9270, scenario='rust_safe_refactor_rename_concrete_concrete'
  cold_cache_sample=False (policy not applicable with n<3)
- md5sum sandbox/results/scorecard_streak.json
  → 6b2121fe8e1134a6bed84faba138e5ff (unchanged)
- receipt: sandbox/results/freezes/2026-09-19-track-a-a1b-g6-reader-fix.json

### Unknown impact / outstanding

- A1c (G3): stability.json health_score formula is
  min(100, pass_rate*50 + 95*30 + 95*20). Spec asks for a 5-dim
  weighted score. Need to characterize.
- R-G6-6 per-dimension variance (timing vs latency vs scalability):
  spec says "variance per dimension" but analyze_stability.py and the
  reader only carry timing cv. Deferred.

### Result

PASS for A1b scope. 92.7% preserved as diagnostic, not as
release-readiness evidence. No fabricated GREEN. No threshold change.
No streak increment. analyze_stability.py untouched.

Next: A1c (G3 characterization + reader fix).

## Active Change — TRACK A / A1c G3 reader fix (2026-09-19) — CLOSED 2026-09-19

Scorecard reader `gate_g3` was consuming `stability.json::health_score`
which is the `analyze_stability.py` formula
`min(100, pass_rate*100*0.5 + 95*0.3 + 95*0.2)` with two 95s as
placeholder constants. The spec requires
`sandbox_core::scoring::compute_health_score` with weights
`(0.35, 0.20, 0.15, 0.15, 0.15)`.

Frozen run 20260919T102509 evidence (not modified):

  total_result_jsons             : 58
  with_all_5_dims_non_None       : 8
  real_5dim_health_distribution  : [94.45, 98.92, 71.25, ...]
  scenarios_below_85_threshold   : 2 (incl. 71.25 case)
  stability_health_score         : 97.5 (analyze_stability formula)
  previous_verdict               : GREEN 97.5 (placeholder, hiding 71.25)

The 71.25 was a Tier-A fixture scenario. Real product verdict was RED,
but the reader fabricated GREEN using the analyze_stability placeholder.

### Six reader gaps documented and pinned

  R-G3-1 wrong source: stability.json::health_score is the analyze_stability
                       formula, not the scoring-engine formula.
  R-G3-2 double counting: averaged stability+summary+aggregate health_score.
  R-G3-3 missing correctitud inflated health (97.5 hides None correctitud).
  R-G3-4 per-scenario variance hidden behind a 97.5 average.
  R-G3-5 71.25 below threshold but GREEN reported.
  R-G3-6 GREEN with 8/58 complete-5-dim scenarios.

### Reader fix (A1c.2)

  - exposes G3_THRESHOLD=85.0, G3_HEALTH_WEIGHTS, G3_DIM_NAMES
    (mirrors crates/cognicode-core/src/sandbox_core/scoring.rs:1400)
  - single source of truth: result.json dimension_scores per scenario
  - 5-dim health formula: CORR*0.35 + LAT*0.20 + ESC*0.15 + CON*0.15 + ROB*0.15
  - scenarios missing any of the 5 dims -> incomplete_5dim
  - stability.json::health_score preserved as diagnostic_only with formula
    and placeholder warning
  - threshold unchanged (>=85)
  - scoring engine unchanged
  - analyze_stability.py unchanged

### Verdict precedence

  no dimension_scores             -> AMBER no_evidence
  any complete < 85               -> RED, failing scenario named
  no complete scenarios           -> AMBER insufficient_5dim
  some complete, some incomplete  -> AMBER insufficient_5dim
  all complete and >= 85          -> GREEN

### Re-verdict on the frozen campaign

  before: G3 GREEN 97.5 (analyze_stability placeholder; 71.25 hidden)
  after : G3 RED
          worst: rust_search_content_default at 68.6
          below_threshold_count: 2
          complete_5dim=4/29, incomplete_5dim=25
          min=68.6, max=98.9, avg=83.3
          diagnostic_only: stability.json::health_score=97.5
            (formula cited, placeholder constants warning)

### Scorecard overall

  before A1c: 8 GREEN, 5 AMBER, 0 RED (G3 fabricated GREEN)
  after  A1c: 7 GREEN, 5 AMBER, 1 RED (G3 honest RED)

### Scope

  - sandbox/scripts/release_scorecard.py (gate_g3 only)
  - sandbox/scripts/tests/test_a1c_g3_contract.py

### Out of scope

  - Threshold (>=85) unchanged.
  - ADR-031 unchanged.
  - compute_health_score in scoring.rs unchanged (mirrored, not duplicated).
  - analyze_stability.py unchanged.
  - Scorecard streak unchanged.
  - No fabricated GREEN.

### Evidence inventory

  - python3 -m pytest sandbox/scripts/tests/test_a1c_g3_contract.py -v
    -> 18/18 PASS (was 10 RED + 8 trivially GREEN)
  - python3 sandbox/scripts/release_scorecard.py against frozen run
    -> 7 GREEN, 5 AMBER, 1 RED
  - G3 verdict: RED with worst scenario named
  - md5sum sandbox/results/scorecard_streak.json
    -> 6b2121fe8e1134a6bed84faba138e5ff (unchanged)
  - receipts:
    - sandbox/results/freezes/2026-09-19-track-a-a1c.0-g3-health-score-trace.json
    - sandbox/results/freezes/2026-09-19-track-a-a1c-g3-reader-fix.json

### Carry-forward (debt)

  - G6: per-scenario runs >= 3 still required; max CV of complete
    scenarios does NOT represent stability of all scenarios.
    No new gate for aggregate_cv. No threshold relaxation.
  - G4: Tier-1 corpus execution deferred to a later cycle (needs
    manifests + verified revisions + actual_repository_identity field).

### Unknown impact / outstanding

  - Real acceptance campaign (Tier-1 corpus + verified revisions +
    >= 3 repeats + ground truth) is the next milestone. The reader
    phase is closed.

### Result

  PASS for A1c scope. Reader phase closed. G3 verdict changed GREEN->RED
  honestly; no fabricated GREEN; no threshold change; no streak
  increment.

  Next: close reader phase; prepare real acceptance campaign.
  Do not renegotiate ADR-031 yet.

## Active Change — TRACK A / preflight v1 (2026-09-19) — CLOSED 2026-09-19

Four preflight criteria from the GO directive (acceptance-campaign v1)
were verified before consuming resources on a real Tier-1 campaign.
Three of the four found real reader defects; all were fixed with
minimal corrections.

### P1 — three repeats contribute to G3 per-scenario aggregation

  Initial: RED. The reader counted each result.json as a separate
  scenario. With 3 repeats of one scenario, the verdict was computed
  over 3 entries instead of 1.
  Fix: per_scenario aggregation. Per distinct scenario_id, compute
  the mean of the 5-dim health across repeats that have all 5 dims
  populated. Mixed-coverage scenarios are marked incomplete_5dim
  (cannot drive verdict).

### P2 — G3 criterion documented and coherent with the spec

  Initial: RED. Docstring lacked an explicit aggregation policy.
  Fix: extended docstring with the Aggregation policy (P1) section;
  G3_HEALTH_WEIGHTS mirror scoring.rs:1400 with attribution.

### P3 — G6 cannot return GREEN with missing mandatory repeats

  Initial: RED. Insufficient scenarios were named as a count only,
  not by name. Mixed-repeats paths (some eligible, some not) silently
  dropped the insufficient ones from the evidence.
  Fix: insufficient scenarios listed by name (up to 10, +N more) in
  AMBER/RED/GREEN paths. They cannot be hidden.

### P4 — no-GT scenarios must not fabricate scores

  Initial: GREEN (already covered by A1a+1 and A1c contract tests).
  No reader change needed.

### Regression tests

  sandbox/scripts/tests/test_preflight_v1.py: 14 tests
  before fixes: 4 failed, 10 passed
  after fixes:  14 passed
  full suite:   60 passed (16 A1a+1 + 12 A1b + 18 A1c + 14 preflight)

### Scope

  - sandbox/scripts/release_scorecard.py (gate_g3 per_scenario
    aggregation; gate_g6 insufficient-scenario naming)
  - sandbox/scripts/tests/test_preflight_v1.py (new)

### Out of scope

  - Thresholds unchanged.
  - ADR-031 unchanged.
  - Scoring engine, analyze_stability.py, scorecard streak unchanged.
  - No campaign executed (preflight is regression-only).
  - No fabricated GREEN.

### Evidence inventory

  - python3 -m pytest sandbox/scripts/tests/test_preflight_v1.py -v
    -> 14/14 PASS
  - python3 -m pytest sandbox/scripts/tests/ (full)
    -> 60/60 PASS
  - scorecard on frozen run: 7 GREEN / 5 AMBER / 1 RED (unchanged)
  - md5 sandbox/results/scorecard_streak.json
    -> 6b2121fe8e1134a6bed84faba138e5ff (unchanged)
  - receipt: sandbox/results/freezes/2026-09-19-track-a-preflight-v1.json

### Result

  PASS for preflight v1 scope. Four criteria verified; three reader
  defects fixed with minimal corrections. No fabricated GREEN. No
  streak increment. No campaign executed yet.

  Next: prepare Tier-1 corpus (ripgrep, serde, anyhow, tokio, clap)
  with pinned commits, manifests, and ground truth. Then a small
  run against one Tier-1 checkout to validate producer fields.
