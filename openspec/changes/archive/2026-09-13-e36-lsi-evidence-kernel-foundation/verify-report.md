```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:3db040b657b79f04a2ac526096dd1b769084e1d4e56b6bcaf4e9f33582351cbe
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 5/5
scenarios: 9/9
test_command: cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel
test_exit_code: 0
test_output_hash: sha256:bb75daf142ac1bc5a013df93cb2d82b3ba8686a601c2c0bddcd04c064b64d933
build_command: cargo check -p cognicode-core --features evidence-kernel
build_exit_code: 0
build_output_hash: sha256:12999f19dc0b3af5ca3d54561f88b189a092e845f9b53f5b42277622d2f214ec
```

## Verification Report

**Change**: e36-lsi-evidence-kernel-foundation
**Version**: N/A (no spec version declared on either spec surface)
**Mode**: Standard (strict_tdd inactive; strict-tdd-verify.md not loaded)

Independent final verification: all runtime evidence below was generated fresh by this
verify run on the current working tree (branch `main`, HEAD `4e1d39eb`, e36 uncommitted),
NOT reused from apply evidence. `evidence_revision` is the SHA-256 over the fixed-order
manifest of the ten per-command output hashes listed in this report.

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 25 |
| Tasks complete | 25 |
| Tasks incomplete | 0 |

Task-completeness spot-check (all claimed artifacts verified to exist): 1.1
`sandbox/fixtures/lsi-baseline/inventory.json` (48 surfaces with criticality weights);
1.3 `sandbox/scripts/capture_lsi_fixtures.py` (1098 lines, fixed argv, `shell=False`);
2.2 `sandbox/scripts/lsi_bench_baseline.py` (497 lines); 2.3
`sandbox/results/lsi-baseline/baseline.json` (schema_version, commit, env, 24 benchmarks);
2.5 `justfile:573` (`lsi-fixtures`) + `justfile:586` (`lsi-baseline`) +
`sandbox/scripts/release_scorecard.py:617` (`gate_g13_lsi`, optional non-blocking);
3.2 `crates/cognicode-core/Cargo.toml:35` (`evidence-kernel = []`) + cfg gates at
`domain/mod.rs:17-18` and `infrastructure/mod.rs:10-11`; 3.3–4.4 kernel modules
(7 domain files + `infrastructure/evidence_kernel/{mod,in_memory}.rs`) with 41 green
tests; 5.1 UAT-U05/U06 in
`docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md` + change-folder
`adr-review.md` (ADR-037..051 verdicts); 5.2 `.agent/TESTING-STATE.md` updated.

### Build & Tests Execution

**Build**: ✅ Passed (both feature-off and feature-on)
```text
$ cargo check -p cognicode-core                                  # consumer build, feature off
exit=0  output sha256:7c06affc159418ac1b684efa1eb8e2483018d0036a77f26f7818644760c5967a
$ cargo check -p cognicode-core --features evidence-kernel        # kernel build
exit=0  output sha256:12999f19dc0b3af5ca3d54561f88b189a092e845f9b53f5b42277622d2f214ec
```

**Tests**: ✅ 41 passed / ❌ 0 failed / ⚠️ 0 skipped (1689 filtered out by the kernel filter)
```text
$ cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel
exit=0  output sha256:bb75daf142ac1bc5a013df93cb2d82b3ba8686a601c2c0bddcd04c064b64d933
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 1689 filtered out
```
```text
$ python3 sandbox/scripts/capture_lsi_fixtures.py --self-test
exit=0  output sha256:4de727a118b057b7439143110ac73ab996be0d82a2e2a6ca405e0f900af1b976
9/9 passed (argv_fixed_no_shell_inrepo_roots, canonicalizer_sorted_scrubbed, capture_all=42,
known_unstable_excluded_from_goldens [3 registered, 0 captured],
check_without_goldens_fails_clean_and_writes_nothing, check_reports_diff_without_overwrite,
accept_rebaselines_explicitly, regeneration_byte_identical, coverage_reports_per_surface)
$ python3 sandbox/scripts/lsi_bench_baseline.py --self-test
exit=0  output sha256:8adf9c5c17fe6f50fef9148bfa9304abfd1a902fa9096738dc5862ef03a22c64
5/5 passed (bencher_parse_normalizes_units, compare_emits_delta_report_and_flags_regression,
compare_without_baseline_fails_clean, bench_argv_fixed_no_shell, baseline_artifact_schema_complete)
```

**M0 harness runtime evidence (fresh)**:
```text
$ python3 sandbox/scripts/capture_lsi_fixtures.py --check   # run 1
exit=0  output sha256:24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc
$ python3 sandbox/scripts/capture_lsi_fixtures.py --check   # run 2 (double-run identity)
exit=0  output sha256:24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc
runs 1 and 2 byte-identical (cmp IDENTICAL); check mode writes nothing
$ python3 sandbox/scripts/lsi_bench_baseline.py compare --fail-above 25
exit=0 (317 s, full fresh cargo bench re-run — NOT cached)
output sha256:56fa223e620b992d729c4d6e23a598360b5a72606b72b328c36d5343d0c4c791
24 benchmarks compared; worst delta +14.18% (symbol_index_build_1000_files);
RESULT: OK — no benchmark regressed beyond 25.0%
```
```text
$ cargo fmt -p cognicode-core --check
exit=0  output sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 (empty)
$ cargo clippy -p cognicode-core --lib --features evidence-kernel   # delta check, warnings non-fatal
exit=0  output sha256:467904898513a7c6dd6948b81c3b93c416bd0e3f7d4316e864cd6015f17924d9
evidence_kernel findings: 0 (grep count 0); 3 warnings all in cognicode-macros
(aix_tool.rs:154, newtype.rs:59, newtype.rs:119 — pre-existing, git-confirmed untouched)
```

**Coverage**: 33.3% (weighted 37/111; 14/48 surfaces) / threshold: 95% (proposal M0 success
criterion, not a spec scenario) → ⚠️ Below. The spec's coverage requirement is SHOULD-level
("SHOULD cover every critical surface") with a MUST-level reporting duty — the reporting duty
is met (per-surface covered/uncovered list with fixture paths, fresh in both `--check` runs).
Deferred surfaces carry explicit reasons in the coverage report (Explorer API needs a running
server; most MCP surfaces marked "add in a follow-up golden pass" or "pin once ordering is
canonical"; mutating/incremental surfaces excluded from static goldens).

### Spec Compliance Matrix

Both spec surfaces this change owns are verified. Counts: delta
`specs/lsi-m0-baseline/spec.md` = 3 requirements / 6 scenarios; umbrella
`cognicode-living-software-intelligence` → `evidence-kernel/spec.md` = 2 requirements /
3 scenarios implemented by this change. Totals 5 requirements / 9 scenarios.

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Delta REQ-1: Golden fixture byte-stability | Regeneration is byte-identical | `capture_lsi_fixtures.py --check` × 2 (exit 0, outputs byte-identical) + self-test `regeneration_byte_identical` | ✅ COMPLIANT |
| Delta REQ-1: Golden fixture byte-stability | Kernel addition does not change consumer outputs | Fresh `--check` × 2 green with kernel modules in tree + `cargo check -p cognicode-core` (feature off) identical — kernel is cfg-gated off in consumer binaries, so consumer outputs cannot change; goldens prove it | ✅ COMPLIANT |
| Delta REQ-1: Golden fixture byte-stability | Intended behavior change requires explicit re-baseline | self-test `check_reports_diff_without_overwrite` (tampered golden → exit 1, no overwrite) + `accept_rebaselines_explicitly` (--accept is the only write path) | ✅ COMPLIANT |
| Delta REQ-2: Baseline coverage of critical surfaces | Inventory surfaces report coverage | Fresh `--check` coverage report (each surface listed covered/uncovered with fixture paths) + self-test `coverage_reports_per_surface` | ✅ COMPLIANT |
| Delta REQ-3: Benchmark baseline artifact | Baseline artifact is captured | `baseline.json` schema inspection (schema_version/commit/env/benchmarks) + fresh self-test `baseline_artifact_schema_complete` | ✅ COMPLIANT |
| Delta REQ-3: Benchmark baseline artifact | Kernel change is compared to baseline | FRESH `lsi_bench_baseline.py compare --fail-above 25` (per-benchmark {name, baseline_mean_us, current_mean_us, delta_pct} × 24, RESULT OK) + self-test `compare_emits_delta_report_and_flags_regression` | ✅ COMPLIANT |
| Umbrella REQ: Canonical fact provenance | Fact round-trip preserves provenance | `domain/evidence_kernel/fact.rs > fact_round_trip_preserves_everything` (bincode + JSON; id, subject, predicate, object, snapshot, provenance class, producer all asserted) | ✅ COMPLIANT |
| Umbrella REQ: Canonical fact provenance | LLM output stays a hypothesis | `fact.rs > fact_new_rejects_llm_agent_producer` + `infrastructure/evidence_kernel/in_memory.rs > commit_rejects_llm_agent_provenance` (store re-check catches struct-literal smuggling) | ✅ COMPLIANT |
| Umbrella REQ: Snapshot-pinned reads | Historical read remains stable | `in_memory.rs > pinned_read_is_stable_after_newer_snapshot_is_published` (A-read identical pre/post B, zero B facts) + `evidence_for_fact_is_stable_across_snapshots` | ✅ COMPLIANT |

**Compliance summary**: 9/9 scenarios compliant

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| Delta REQ-1: Golden fixture byte-stability | ✅ Implemented | Canonicalizer sorts elements/keys, scrubs timestamps+paths; `--check` writes nothing; `--accept` is the only write path; double-run identity proven fresh |
| Delta REQ-2: Baseline coverage of critical surfaces | ✅ Implemented (reporting) / ⚠️ SHOULD coverage at 33.3% | `compute_coverage` against `inventory.json`; every surface listed with fixture paths or a deferral reason |
| Delta REQ-3: Benchmark baseline artifact | ✅ Implemented | `baseline.json` machine-readable with env metadata; `compare` emits per-benchmark deltas and honors `--fail-above` |
| Umbrella REQ: Canonical fact provenance | ✅ Implemented | `ProvenanceRecord` wraps legacy `Provenance` (D3, extend-never-mutate); `Fact::new` + store commit both reject `ProducerKind::LlmAgent`; no LLM-originated fact can be persisted (Hypothesis/AgentEvidence path is the only LLM carrier — type-level contract) |
| Umbrella REQ: Snapshot-pinned reads | ✅ Implemented | `FactStore` reads take `(&WorkspaceId, &SnapshotId)`; in-memory rows keyed by `(ws, snap)` — snapshot isolation by construction (D5) |

### Coherence (Design)
| Decision | Followed? | Notes |
|----------|-----------|-------|
| D1 kernel module `domain/evidence_kernel/*` cfg `evidence-kernel` | ✅ Yes | 7 files exactly as planned; `multimodal` pattern reused |
| D2 kernel ports namespaced, no cross re-exports | ✅ Yes | Legacy `ports::EvidenceStore` untouched; no re-exports in `mod.rs` |
| D3 `ProvenanceRecord` wraps legacy enum; LlmAgent rejected at `Fact::new` | ✅ Yes (+ deviation 2 below, strengthening) | Legacy enum has no new variants; exhaustive round-trip over all 5 classes × 5 producers |
| D4 `SnapshotId(u64)` bijective with `RevisionId`; `from_revision` facade | ✅ Yes (+ deviation 3 below) | `snap:N`↔`rev:N` tests; `RevisionId::NONE` rejected |
| D5 pinned reads `(&WorkspaceId, &SnapshotId)` | ✅ Yes | Structural isolation via `(ws, snap)` keying |
| D6 sync `SchemaRegistry`; commit rejects unregistered predicates | ✅ Yes (+ deviation 2) | BTreeMap-ordered list; append-only vocabulary |
| D7 in-memory adapters only, Ladybug deferred | ✅ Yes | `infrastructure/evidence_kernel/{mod,in_memory}.rs`; no SQL path |
| D8 M0 harness reuses scorecard infra + existing bench | ✅ Yes | `graph_benchmarks` bencher parse path; `release_scorecard.py` G13 optional gate |
| Deviation 1: `SchemaRegistry::list()` shape | ⚠️ Deviation (benign) | Design interface says `Vec<RelationSpec>`; implementation returns `Vec<(RelationKind, RelationSpec)>`, deterministically ordered — richer, no spec scenario constrains the shape |
| Deviation 2: store-level double validation | ⚠️ Deviation (benign) | `InMemoryFactStore::validate` re-checks LlmAgent + snapshot mismatch at commit beyond `Fact::new` (design D6 named only predicate rejection) — defense-in-depth against struct-literal bypass; strengthens the umbrella LLM scenario |
| Deviation 3: `from_revision` allow attribute | ⚠️ Deviation (benign) | `#[allow(clippy::wrong_self_convention)]` on the port method (design D4 mandates the name; it is a store op, not a constructor), documented inline in `ports.rs` |
| Deviation 4: `just lint` scope | ⚠️ Deviation (benign) | Design guard cites `just lint`; executed instead as scoped clippy with warnings non-fatal + `evidence_kernel` grep because `just lint` is pre-existing red from `cognicode-macros` (see WARNING 3). Git confirms e36 touched neither `cognicode-macros` nor `generic_graph.rs` |

None of the four deviations breaks a spec requirement or scenario.

### Issues Found

**CRITICAL**: None.

**WARNING**:
1. M0 weighted coverage is 33.3% (14/48 surfaces, 37/111 weight) versus the proposal success
   criterion "≥95% critical surfaces under contract/golden fixtures". The delta spec's MUST
   (report per-surface coverage) is met; its SHOULD (cover every critical surface) is not.
   Deferrals are explicit and reasoned (Explorer needs a live server; MCP analytics surfaces
   pinned once engine ordering is canonical; mutating incremental surfaces excluded from
   static goldens). Milestone aspiration remains open, not hidden.
2. Graph-engine nondeterminism defect quarantined in `KNOWN_UNSTABLE_SURFACES`:
   `multi-lang-types` declares the same symbol names in several languages; directory-level
   graph surfaces collapse same-named symbols first-file-wins and the file walk order is not
   sorted, so the captured NODE SET (not just ordering) varies run-to-run. 3 surfaces are
   registered and 0 captured (fresh self-test `known_unstable_excluded_from_goldens` PASS).
   This is a pre-existing upstream engine defect, not introduced or owned by e36; it is
   explicitly reported in every harness run until the engine walk is order-stable.
3. Pre-existing clippy red outside this change: `just lint` / workspace
   `clippy -- -D warnings` fails on `cognicode-macros` (`aix_tool.rs:154` let_and_return;
   `newtype.rs:59` and `newtype.rs:119` collapsible_if) plus `generic_graph.rs` test lints.
   Attribution verified in this run: `git diff --stat HEAD -- crates/cognicode-macros/` is
   empty, no untracked macros files, and the scoped clippy output contains zero
   `evidence_kernel`/e36-surface findings. Not attributable to e36; documented in
   `.agent/TESTING-STATE.md`.

**SUGGESTION**:
1. Formal UAT execution for U01..U06 is still pending; U05/U06 are authored with their
   automated kernel coverage cited ("PASS pendiente de ejecución UAT formal").
2. The committed `sandbox/results/lsi-baseline/delta.json` is apply-era evidence (max delta
   +1.57%). The fresh verify compare (max +14.18%, still within threshold) deliberately did
   not rewrite it (no `--output` passed; committed artifacts left untouched). If archive
   should record final-gate deltas, regenerate with `just lsi-baseline compare threshold=25`.
3. The four proposal success-criteria checkboxes are unchecked; archive should reconcile them
   against this report (M1 criteria met with test evidence; ADR review recorded; M0 ≥95% NOT
   met; UAT formal run pending).

### Verdict
PASS WITH WARNINGS
All 9/9 scenarios across both owned spec surfaces are COMPLIANT with fresh runtime evidence
(41 kernel tests, 2× byte-identical capture runs, 9/9 + 5/5 harness self-tests, full fresh
bench delta run 317 s); all commands exit 0; no CRITICAL findings. The three warnings are
documented, honestly-bounded gaps (SHOULD-level coverage aspiration, pre-existing quarantined
upstream defect, pre-existing clippy red outside the change) that break no spec scenario.
