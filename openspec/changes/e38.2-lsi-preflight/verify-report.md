```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:3432fabf072fa5e5bd83706a60fe32e2d3fefde53a9f1b4b2c5991410ca844a4
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 15/15
scenarios: 27/27
test_command: cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel
test_exit_code: 0
test_output_hash: sha256:f2c67a58b7d73fa7513c144bf6a1b1fa515b6bcb0481e48e9f821b89dd536157
build_command: cargo check -p cognicode-core --features evidence-kernel
build_exit_code: 0
build_output_hash: sha256:9d7489b523d1eb18492405c8f158bf92a25bcb3fdeef18315776e1273f8da19e
```

## Verification Report

**Change**: e38.2-lsi-preflight
**Version**: N/A (no spec version declared on the five promoted main specs)
**Mode**: Standard (strict TDD inactive; strict-tdd-verify.md not loaded)

Independent final verification: all runtime evidence below was generated fresh by this
verify run on the current working tree (branch `main`, HEAD `6977d816`, e38.2 uncommitted,
9 modified files + new change dir), NOT reused from apply evidence — including re-runs of
the harness commands whose e38.1-era evidence had gone stale (tree changed since).
`evidence_revision` is the SHA-256 over the fixed-order manifest of the twenty-six
per-command `id|command|exit|output-hash` rows listed in this report.

Spec surface verified: the five promoted main specs `openspec/specs/{lsi-m0-baseline,
projection-equivalence-harness, generic-graph-projection, entity-continuity,
identity-benchmark}/spec.md` = 15 requirements / 27 scenarios (recounted from the files
by this verify run: 3/6 + 4/8 + 2/4 + 4/6 + 2/3). No spec deltas exist for this change
(Capabilities: None/None — preflight hardening). Supplementary (not counted in 15/27):
the NEW CP-4 guard behavior, covered by two new unit tests inside the kernel suite.

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 21 |
| Tasks complete | 21 |
| Tasks incomplete | 0 |

All 21 tasks checked `[x]` in `openspec/changes/e38.2-lsi-preflight/tasks.md`. Spot-check
of claimed artifacts (verified to exist and inspected): 1.2/1.3 `KernelError::FactIdSpaceCollision(FactId,
SnapshotId)` in `domain/evidence_kernel/ports.rs` + atomic guard in
`infrastructure/evidence_kernel/in_memory.rs` `commit` (rejects when any existing `(ws, snap)`
fact id ≤ batch max, spaces start at 1; carries the smallest colliding id; error raised under
the lock BEFORE any state change — atomic); 1.1 RED-first test present and passing
(`commit_rejects_second_batch_reusing_id_space_in_same_snapshot`); 1.4 `FactStore::commit`
port doc states the id-space rule; 2.4 `delta_u382_run{1,2}.json` exist with the recorded
deltas and `baseline.json` mtime unchanged (2026-09-12 11:15 — before the e38.2 session);
3.1 determinism hunks in `analysis_service.rs` `build_project_graph` (sorted-by-`file_path`
fold + smallest-FQN-wins duplicate rule aligned with `resolve_callee_identity`); 4.1–4.3
the five clippy fixes present in `aix_tool.rs` / `newtype.rs` / `generic_graph.rs`; 5.4
`.agent/TESTING-STATE.md` Active Change + RETIREMENT-LEDGER CP-4 row updated.

### Build & Tests Execution
**Build**: ✅ Passed (6/6 configurations, exit 0)
```text
cargo check -p cognicode-core                                                    -> exit 0
cargo check -p cognicode-core --features evidence-kernel                         -> exit 0
cargo check -p cognicode-core --features evidence-kernel,multimodal              -> exit 0
cargo check -p cognicode-runtime                                                 -> exit 0
cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel         -> exit 0
cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel,multimodal -> exit 0
```

**Tests**: ✅ 247 passed / 0 failed / 3 ignored (analysis_service slow-path tests)
```text
cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel            93 passed / 0 failed
cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel                15 passed / 0 failed
cargo test -p cognicode-core --lib continuity --features evidence-kernel                 36 passed / 0 failed
cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel      46 passed / 0 failed
cargo test -p cognicode-core --lib analysis_service --features evidence-kernel           25 passed / 0 failed / 3 ignored
cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal   6 passed / 0 failed
cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel -- --nocapture   7 passed / 0 failed
cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel -- --nocapture    7 passed / 0 failed
cargo test -p cognicode-core --test workspace_isolation --features evidence-kernel -- --nocapture   2 passed / 0 failed
cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel            10 passed / 0 failed
```

**Harnesses & gates** (all exit 0):
```text
just lsi-fixtures check        -> RESULT: PASS — all 42 goldens byte-identical; covered 14/48 surfaces (weighted 37/111 = 33.3%) with per-surface listing
python3 sandbox/scripts/capture_lsi_fixtures.py --self-test -> 9/9 PASS (incl. check_reports_diff_without_overwrite exit=1 tampered_intact=True; accept_rebaselines_explicitly exit=0; regeneration_byte_identical)
just lsi-equivalence  (run 1)  -> python-hello 1.0000/1.0000/1.0000; rust-hello 1.0000/1.0000/1.0000; multi-lang-types QUARANTINED 0.5969/0.0714 kind 0.5259
just lsi-equivalence  (run 2)  -> scores byte-identical to run 1 (determinism double-run proof on the e38.2 tree)
just lsi-identity              -> gates: precision=1.0000 (>= 0.95) recall=1.0000 (>= 0.9) line_shift_retention=1.0000 (== 1) move_retention=1.0000 (>= 0.99); 7 cases PASS; colliding-names ambiguous=1 quarantined, reported separately; workspace isolation: 7/7 outcomes, occurrences disjoint, collisions=0
just lint                      -> exit 0 — GREEN (first green since E30.1), the headline regression-proof re-proven fresh
cargo fmt --check              -> exit 0 (no output)
cargo clippy -p cognicode-core --lib --tests --features evidence-kernel -- -D warnings            -> exit 0, zero lint findings
cargo clippy -p cognicode-core --lib --tests --features evidence-kernel,multimodal -- -D warnings -> exit 0, zero lint findings
cargo clippy -p cognicode-macros --all-targets -- -D warnings                                    -> exit 0, zero lint findings
```

**Digest pins** (verified unchanged by grep + green harnesses):
`PINNED_IDENTITY_DIGEST = fnv1a64:ec9546e35a003ed6` (equivalence harness) and
`PINNED_MATCHER_DIGEST = fnv1a64:e79f623705344f98` (identity benchmark) — both UNTOUCHED;
no re-baseline occurred anywhere in e38.2 (goldens 42/42 on first attempt).

**Coverage**: ➖ Not available (no coverage threshold configured for this surface;
openspec config `verify.coverage_threshold: 0`)

### Spec Compliance Matrix
| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| lsi-m0-baseline / Golden fixture byte-stability | Regeneration is byte-identical | `capture_lsi_fixtures.py --check` (h1: PASS 42/42) + self-test `regeneration_byte_identical` | ✅ COMPLIANT |
| lsi-m0-baseline / Golden fixture byte-stability | Kernel addition does not change consumer outputs | `just lsi-fixtures check` (h1: 42/42 with e38.2 kernel/service edits in tree) | ✅ COMPLIANT |
| lsi-m0-baseline / Golden fixture byte-stability | Intended behavior change requires explicit re-baseline | self-test `check_reports_diff_without_overwrite` (exit=1, tampered_intact=True) + `accept_rebaselines_explicitly` (exit=0) | ✅ COMPLIANT |
| lsi-m0-baseline / Baseline coverage of critical surfaces | Inventory surfaces report coverage | h1 per-surface report: "covered 14/48 surfaces (weighted 37/111 = 33.3%)" + [x]/[ ] listing with fixture paths | ✅ COMPLIANT |
| lsi-m0-baseline / Benchmark baseline artifact | Baseline artifact is captured | `sandbox/results/lsi-baseline/baseline.json` (24 benchmarks; env: cargo/os/rustc/commit; schema_version 1) | ✅ COMPLIANT |
| lsi-m0-baseline / Benchmark baseline artifact | Kernel change is compared to baseline | `delta_u382_run1.json` + `delta_u382_run2.json` (per-benchmark deltas vs baseline; fail_above_pct 10.0) | ✅ COMPLIANT |
| projection-equivalence-harness / Declared equivalence contract | All fixtures meet the threshold | `equivalence_harness > all_fixtures_meet_the_threshold` (python-hello 1.0000, rust-hello 1.0000 ≥ 0.99) | ✅ COMPLIANT |
| projection-equivalence-harness / Declared equivalence contract | Fixture below threshold fails the run | `equivalence_harness > fixture_below_threshold_fails_the_run` | ✅ COMPLIANT |
| projection-equivalence-harness / Quarantine of known-unstable surfaces | Quarantined divergence is excluded and reported | `equivalence_harness > quarantined_divergence_is_excluded_and_reported` (multi-lang-types QUARANTINED 0.5969/0.0714 printed) | ✅ COMPLIANT |
| projection-equivalence-harness / Quarantine of known-unstable surfaces | Unquarantined divergence fails | `equivalence_harness > unquarantined_divergence_fails` | ✅ COMPLIANT |
| projection-equivalence-harness / Deterministic fact identity | Repeated extraction is identical | `fact_bridge > batch_builder::repeated_extraction_is_identical` + `lsp_facts::repeated_provider_collection_is_identical`; fresh double-run `just lsi-equivalence` ×2 scores byte-identical | ✅ COMPLIANT |
| projection-equivalence-harness / Deterministic fact identity | Convention change requires explicit re-baseline | `equivalence_harness > convention_change_requires_explicit_re_baseline` + `identity_convention_states_the_declared_rules` (pin `ec9546e35a003ed6` unchanged) | ✅ COMPLIANT |
| projection-equivalence-harness / No silent cutover | Gate off serves the legacy path | `cargo check -p cognicode-core` (default: zero kernel code compiled, gate is crate-level) + h1 goldens byte-stable | ✅ COMPLIANT |
| projection-equivalence-harness / No silent cutover | Failing harness keeps legacy default | `cargo check -p cognicode-runtime` (default features: no fact-path wiring; bridge reachable only behind off-by-default `evidence-kernel` gate) + structural port inspection | ✅ COMPLIANT |
| generic-graph-projection / Fact-derived generic graph emission | Facts map to nodes and edges | `generic_graph_projection > facts_map_to_nodes_and_edges` | ✅ COMPLIANT |
| generic-graph-projection / Fact-derived generic graph emission | Empty snapshot yields empty projection | `generic_graph_projection > empty_snapshot_yields_empty_projection` | ✅ COMPLIANT |
| generic-graph-projection / Fact-derived generic graph emission | Clear and rebuild is equivalent | `generic_graph_projection > clear_and_rebuild_is_equivalent` (+ `equivalence_harness > rebuild_from_reread_facts_equals_prior_projection`) | ✅ COMPLIANT |
| generic-graph-projection / Consumers remain FactStore-ignorant | Consumer needs only emitted types | `generic_graph_projection > consumer_needs_only_emitted_types` | ✅ COMPLIANT |
| entity-continuity / Tiered deterministic continuity matching | Line shift keeps stable identity | `continuity > matcher::t1_line_shift_keeps_stable_identity` + identity_benchmark case 'line-shift' PASS (retention 1.0000) | ✅ COMPLIANT |
| entity-continuity / Tiered deterministic continuity matching | File move keeps stable identity | `continuity > matcher::t2_file_move_keeps_stable_identity` + cases 'move'/'move-edit' PASS (retention 1.0000) | ✅ COMPLIANT |
| entity-continuity / Tiered deterministic continuity matching | Fingerprint is fact-deterministic | `continuity > fingerprint::fingerprint_is_fact_deterministic` | ✅ COMPLIANT |
| entity-continuity / Ambiguous continuity fails closed | Colliding names fail closed | identity_benchmark case 'colliding-names' PASS (ambiguous=1, candidates listed, reported separately) + `matcher::ambiguous_is_terminal_and_never_rescued_by_a_lower_tier` | ✅ COMPLIANT |
| entity-continuity / Fail-closed rename evidence port | Version control unavailable degrades safely | `rename_evidence` suite 10/0 (fail-closed on git-absent/non-repo/unborn-rev/malformed rows) | ✅ COMPLIANT |
| entity-continuity / Workspace isolation of continuity | Identical workspaces do not collide | `workspace_isolation > identical_workspaces_do_not_collide` + `differing_workspaces_do_not_contaminate_each_other` (collisions=0) | ✅ COMPLIANT |
| identity-benchmark / Pinned scoring gates and fixture coverage | Missed gate fails the run | `identity_benchmark > missed_gate_fails_the_run` + `precision_gate_failure_names_gate_and_value` (live gates all 1.0000) | ✅ COMPLIANT |
| identity-benchmark / Pinned scoring gates and fixture coverage | Ambiguous outcome reported separately | `identity_benchmark > ambiguous_outcome_reported_separately` (colliding-names quarantined, excluded from precision/recall) | ✅ COMPLIANT |
| identity-benchmark / Separate pinned convention digest | Convention change requires explicit re-pin | `identity_benchmark > convention_change_requires_explicit_re_pin` + `matcher_convention_states_the_declared_rules` (pin `e79f623705344f98` unchanged) | ✅ COMPLIANT |

**Compliance summary**: 27/27 scenarios compliant

Supplementary (outside the 15/27 count) — NEW CP-4 guard behavior:
`evidence_kernel > in_memory::commit_rejects_second_batch_reusing_id_space_in_same_snapshot`
(asserts `KernelError::FactIdSpaceCollision(1, snap)` and atomic non-mutation) and
`commit_allows_reusing_id_space_in_a_different_snapshot` — both pass; apply's RED-first
claim is corroborated by the test's `expect_err` structure and the guard's placement after
`validate`/before mutation. ENGINE-DET determinism is behaviorally proven by the byte-identical
double equivalence runs and 42/42 goldens on the U3-edited service (`analysis_service` suite
25/0 3-ignored).

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| lsi-m0-baseline (3 reqs) | ✅ Implemented | capture/bench scripts unchanged by e38.2 and re-proven: 42/42 byte-identical, per-surface coverage report printed, baseline.json + two fresh delta artifacts |
| projection-equivalence-harness (4 reqs) | ✅ Implemented | threshold 0.99 declared in harness; quarantine measured+reported; pins guard the conventions; gate-off proven by default-build check + runtime check |
| generic-graph-projection (2 reqs) | ✅ Implemented | `FactGenericGraphProjection` emits `GraphNode`/`GraphEdge` only; 1:1 covering unit tests under dual gate |
| entity-continuity (4 reqs) | ✅ Implemented | tier T0–T3 matcher with fail-closed ambiguity; git adapter fail-closed; isolation collisions=0 |
| identity-benchmark (2 reqs) | ✅ Implemented | pinned gates declared before scoring (0.95/0.90/1.00/0.99), all 1.0000; matcher digest separate from identity digest |
| CP-4 (supplementary, no spec surface yet) | ✅ Implemented | `KernelError::FactIdSpaceCollision(FactId, SnapshotId)` — a `SnapshotMismatch`-style caller violation; does NOT reuse the I/O-reserved `Store(String)` variant; atomic pre-extend guard in `InMemoryFactStore::commit`; port doc updated |

### Coherence (Design)
| Decision | Followed? | Notes |
|----------|-----------|-------|
| (no design.md artifact for this change) | ➖ Skipped | Change carries proposal.md + tasks.md only (preflight hardening, Capabilities None/None); design coherence not assessable and not required — proposal approach ("four independent work units, one stacked slice") was followed as tasked |

### Four Resolutions Verified in Code (fresh inspection this run)
| Resolution | Evidence | Verdict |
|-----------|----------|---------|
| CP-4 guard | `ports.rs`: new `FactIdSpaceCollision(FactId, SnapshotId)` variant with `SnapshotMismatch`-style doc; `Store` untouched (I/O-reserved). `in_memory.rs`: guard rejects when any existing `(ws,snap)` id ≤ batch max (spaces start at 1), carries smallest colliding id, fires atomically before state change. Two new tests green in the 93-test kernel suite | ✅ Correct |
| ENGINE-DET | `analysis_service.rs` `build_project_graph`: results sorted by `file_path` after the parse stage; name-index duplicate rule = lowercase name → lexicographically smallest FQN wins (aligned with `resolve_callee_identity`), no first-file-wins; exact-identity stage documented inapplicable (legacy callees are bare names). Goldens 42/42 + equivalence scores byte-identical across double runs | ✅ Correct (minimal) |
| CLIPPY | 5 lints fixed idiomatically: `aix_tool.rs` collapsible_if → let-chain; `newtype.rs` collapsible_if → let-chain + let_and_return → direct if-expression; `generic_graph.rs` chrono import + `doc_id` gated `#[cfg(feature = "multimodal")]`. `just lint` exit 0; clippy -D warnings zero findings in all three configurations | ✅ Correct |
| PERF adjudication | `delta_u382_run1.json`: regression `hot_path_outgoing_calls` +12.38% (2.456→2.760µs), `search` +0.00%. `delta_u382_run2.json`: regressions=[] (`hot_path_outgoing_calls` −7.13% recovered; `hot_path_incoming_calls` −16.96% improvement), `search` +2.53%. No benchmark >10% in BOTH runs → per tasks 2.1 rule: NOISE-ATTRIBUTED; `baseline.json` mtime 2026-09-12 11:15 (untouched); A/B (2.2/2.3) correctly not triggered (condition false) | ✅ Rule applied correctly |

### Adjudicated Deviations
| Deviation | Adjudication |
|-----------|--------------|
| Task-text lint-name swap (4.1 said `let_and_return` at aix_tool.rs:154 — actual `collapsible_if`; 4.2 said `collapsible_if` at newtype.rs:119 — actual `let_and_return`) | Accepted: fixes are semantically correct and idiomatically idiomatic; actual lint names documented in tasks.md and TESTING-STATE; the task's criterion (`just lint` exit 0) is met |
| Feature-gating instead of deletion for `generic_graph.rs:467/479` (tasks said "drop") | Accepted (strict improvement): items are used by multimodal-gated tests only; `#[cfg(feature = "multimodal")]` keeps BOTH feature sets warning-free, verified by clippy -D warnings under `evidence-kernel` AND `evidence-kernel,multimodal` |
| `build_project_graph_filtered`/`_async` left with the legacy unordered name-index (ENGINE-DET covers sync variant only) | Accepted: out of tasked scope (task 3.1 names `build_project_graph` only); variants are on no golden/harness path; documented as known limitation for a future cycle |

### Issues Found
**CRITICAL**: None
**WARNING**: 1 — ENGINE-DET determinism covers only `build_project_graph`; `build_project_graph_filtered`
and `build_project_graph_async` retain the nondeterministic lowercase name-keyed first-file-wins
collapse. Out of tasked scope and on no golden/harness path today (aix_handlers async is outside the
fixture surface set), but any future consumer of those variants feeding goldens would reintroduce
walk-order dependence.
**SUGGESTION**: 1 — the CP-4 guard behavior has no capability-spec surface yet (per proposal, fold the
guard requirement into umbrella `cognicode-living-software-intelligence` `evidence-kernel` at archive);
sdd-archive must perform that fold.

### Verdict
PASS WITH WARNINGS
All 21 tasks complete; 27/27 scenarios compliant with fresh runtime evidence; all 26 commands exit 0;
`just lint` GREEN re-proven; digest pins and baseline untouched; single non-blocking WARNING documents
the out-of-scope filtered/_async determinism gap.

---
Per-command evidence manifest (id|command|exit|output-sha256; `evidence_revision` = SHA-256 of this
manifest in this fixed order):
```text
s1_evidence_kernel|cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel|0|f2c67a58b7d73fa7513c144bf6a1b1fa515b6bcb0481e48e9f821b89dd536157
s2_fact_bridge|cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel|0|728c9c65e288b35ab119d6c063b6a5a3c7e9f93c9accf1200355bbdbadb401ef
s3_continuity|cargo test -p cognicode-core --lib continuity --features evidence-kernel|0|070dc45f799717d7c41484b541968778248954318d8c277c2339aacbee315060
s4_call_graph_projection|cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel|0|41ce49f155ddd9163e0b53901d81f49096523dd74bf0a5aabd6c17cc7b4e2cd8
s5_analysis_service|cargo test -p cognicode-core --lib analysis_service --features evidence-kernel|0|820994a1ac5de03e003b0a5546b7ffcd757348e07741eac83cfe33cbb56bc775
s6_generic_graph_projection|cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal|0|3f95ba70b8e2ae6cc1e6c5ec0f47dc1d2900f849c4d4cb466baea3355dea34b2
s7_equivalence_harness|cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel -- --nocapture|0|8c47bc64a7ad50fbb40480c70569a2fc1ce0ee4562353bb39d6f8b51cf8d1004
s8_identity_benchmark|cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel -- --nocapture|0|533eca387d31d01533a4224570dc39025600bc328f60da5f99c84fa85523841d
s9_workspace_isolation|cargo test -p cognicode-core --test workspace_isolation --features evidence-kernel -- --nocapture|0|999b578d04e223cdb3bed071c5d7e606084f3ba69cc613bfcbb30bf49fc65fef
s10_rename_evidence|cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel|0|1ac9de8187717f52b6b20e22c54624999135c185827a60942bdfcaaf6d47943b
b1_check_default|cargo check -p cognicode-core|0|439f0e9693e16dc95eda0e42d51a4dab25c33c832c9f4fa050357bd883af2be6
b2_check_evidence_kernel|cargo check -p cognicode-core --features evidence-kernel|0|9d7489b523d1eb18492405c8f158bf92a25bcb3fdeef18315776e1273f8da19e
b3_check_evidence_kernel_multimodal|cargo check -p cognicode-core --features evidence-kernel,multimodal|0|8ec3deee86419e48cd153a826f66398d8e1d30f9d54c4403d077553a6d4346ca
b4_check_runtime|cargo check -p cognicode-runtime|0|2296aebce0a193411870c21d1407d6c88c95e59052094d3b3b3ae2479de44f34
b5_check_bench_evidence_kernel|cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel|0|39f5ffa9a06c5bf205aa77aa288e051cb5e60cbab599b1a81c40ea7838e92153
b6_check_bench_both|cargo check -p cognicode-core --bench fact_bridge_benchmarks --features evidence-kernel,multimodal|0|ca7e49faf375d39f27e4fe755113b4c6c279ba53418fb6223e4838dbeecca988
h1_lsi_fixtures_check|just lsi-fixtures check|0|24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc
h2_lsi_equivalence_run1|just lsi-equivalence|0|1991e205014b60124da112745af8419c44ccf6db62dc6764420493ee438c45b9
h3_lsi_equivalence_run2|just lsi-equivalence|0|0f1e31f3bf97afeb2c6af2d95d5d0bbabf3aa810223acbde7b3a5f296589959c
h4_lsi_identity|just lsi-identity|0|c137e84db975b149bf03ab184aec1ed2f81735d9a740f68598cffc280f7a4b3e
h5_fixture_self_test|python3 sandbox/scripts/capture_lsi_fixtures.py --self-test|0|4de727a118b057b7439143110ac73ab996be0d82a2e2a6ca405e0f900af1b976
g1_just_lint|just lint|0|a6ad820cec785b81075fc08335fe965549ec2f57031a4086b2b5884248e3729b
g2_fmt_check|cargo fmt --check|0|e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
g3_clippy_core_ek|cargo clippy -p cognicode-core --lib --tests --features evidence-kernel -- -D warnings|0|031f97d57de3a42e5162c96f4721fb794c85f82d908d42e6fdd284086e285f20
g4_clippy_core_ek_mm|cargo clippy -p cognicode-core --lib --tests --features evidence-kernel,multimodal -- -D warnings|0|282acfc3f430e66e2e0abcc0952335b7ab18a0d00e613c17f1b15101f6ba5c12
g5_clippy_macros|cargo clippy -p cognicode-macros --all-targets -- -D warnings|0|326bdeae3d5abea7586519353759206674b064be253f773a819bc6f90c827053
```
