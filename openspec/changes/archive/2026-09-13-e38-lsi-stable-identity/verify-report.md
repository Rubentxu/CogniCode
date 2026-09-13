```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:dd94dc9b111ee504007288d520fb4dc80308b8b10849350283a62d0135f2eab2
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 6/6
scenarios: 9/9
test_command: cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel -- --nocapture
test_exit_code: 0
test_output_hash: sha256:820ef4c359c207f129a7f4a9c38f67937a5be06368722591931ba18bb9e478a7
build_command: cargo check -p cognicode-core --features evidence-kernel
build_exit_code: 0
build_output_hash: sha256:70cc385b6fb9223400631c2f358ee0a91fc5e71f0846d98e2405a3c4a441128f
```

## Verification Report

**Change**: e38-lsi-stable-identity
**Version**: N/A (no spec version declared on either delta spec surface)
**Mode**: Standard (strict TDD inactive; strict-tdd-verify.md not loaded)

Independent final verification: all runtime evidence below was generated fresh by this
verify run on the current working tree (branch `main`, HEAD `4e1d39eb`, e36+e37+e38
uncommitted, 33 status entries), NOT reused from apply evidence. `evidence_revision` is
the SHA-256 over the fixed-order manifest of the thirteen per-command
`command|exit|output-hash` rows listed in this report.

Owned spec surface: delta `specs/entity-continuity/spec.md` (4 requirements /
6 scenarios) + delta `specs/identity-benchmark/spec.md` (2 requirements / 3 scenarios)
= 6 requirements / 9 scenarios. Umbrella `cognicode-living-software-intelligence` →
`stable-entity-identity` (R1 entity/occurrence separation: line shift preserves identity;
ambiguous continuity fails closed) is implemented by e38 via the continuity layer and is
verified through the same benchmark/matcher runs as supplementary rows (not counted in
the 6/9).

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 16 |
| Tasks complete | 16 |
| Tasks incomplete | 0 |

Task-completeness spot-check (claimed artifacts verified to exist and inspected): 1.1
`ids.rs:201-248` (`StableEntityId(u64)` Display `stable:N`, sibling derives, const
`OccurrenceId::from_entity`/`to_entity`, display/serde/copy tests); 1.2–1.3
`domain/evidence_kernel/continuity/{mod,view,fingerprint}.rs` + additive
`evidence_kernel/mod.rs:28,45` re-exports; 2.1–2.3 `infrastructure/git/rename_evidence.rs`
(fixed argv `.arg()` composition, `R<nnn>` parse, warn+empty on every failure mode) +
additive `ports.rs:177-211` (`FileRename`, sync `RenameEvidencePort: Send + Sync`) +
cfg-gated `infrastructure/git/mod.rs` wiring; 3.1–3.3 `continuity/matcher.rs`
(pinned 0.6/0.05/0.5, T0–T3, fail-closed ambiguity, no-resurrection, permutation
invariance); 4.1–4.3 `tests/identity_benchmark{.rs,/harness.rs}` with gates
PRECISION=0.95/RECALL=0.90/LINE_SHIFT=1.00/MOVE=0.99 declared before scoring,
`MATCHER_CONVENTION` text + `PINNED_MATCHER_DIGEST` `fnv1a64:e79f623705344f98`
(harness.rs:114-126), quarantine + inverted fail-closed check; 4.2 seven fixture cases
under `sandbox/fixtures/lsi-identity/{control,pure-rename,rename-edit,move,move-edit,
line-shift,colliding-names}/`; 4.4 `tests/workspace_isolation.rs` + `justfile:619`
`lsi-identity` recipe (fixed argv); 5.1–5.3 e37-reuse records, guards, UAT deferral
records, `.agent/TESTING-STATE.md` handoff.

### Build & Tests Execution

**Build**: ✅ Passed (all four feature states)
```text
$ cargo check -p cognicode-core                                        # gate off
exit=0  output sha256:55c652ff6feee26f49676cfade4efe2a371acc7e386340dfa71df491c8901491
$ cargo check -p cognicode-core --features evidence-kernel              # kernel on
exit=0  output sha256:70cc385b6fb9223400631c2f358ee0a91fc5e71f0846d98e2405a3c4a441128f
$ cargo check -p cognicode-core --features evidence-kernel,multimodal   # dual-gated surface
exit=0  output sha256:12999f19dc0b3af5ca3d54561f88b189a092e845f9b53f5b42277622d2f214ec
$ cargo check -p cognicode-runtime                                     # consumer crate
exit=0  output sha256:3eb9cc8bd53e5cfe9e865e21e4096d3b1560c7e4acdb2ed5f191e787fb1f8cf0
$ cargo fmt -p cognicode-core --check
exit=0  output sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 (empty)
```

**Tests**: ✅ 141 passed / ❌ 0 failed / ⚠️ 0 skipped (e38 scope; plus 7 e37-reuse harness
tests and 42/42 goldens below)
```text
$ cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel
exit=0  output sha256:5426630bb7ba72ff04d8fd08d4c0e3b59be496903f2253a925daf64cb295d803
test result: ok. 87 passed; 0 failed; 0 ignored; 0 measured; 1722 filtered out
$ cargo test -p cognicode-core --lib continuity --features evidence-kernel
exit=0  output sha256:d20f5945b54b026519339aa7cc0238d784139d5fc1a59a8f7449eeb784f78ce0
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 1773 filtered out
$ cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel
exit=0  output sha256:4925270077bc29f79ce8236497360868e009a0c6f6eabb22e61fcb5a97b03778
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 1799 filtered out
$ cargo test -p cognicode-core --test identity_benchmark --features evidence-kernel -- --nocapture
exit=0  output sha256:820ef4c359c207f129a7f4a9c38f67937a5be06368722591931ba18bb9e478a7
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ cargo test -p cognicode-core --test workspace_isolation --features evidence-kernel -- --nocapture
exit=0  output sha256:76781e7566c5fd266c3f6b0cb69078417d7b40c59443030989f5f0add4dc8cd9
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Identity benchmark gate report (fresh run, printed live)**:
```text
== E38 identity benchmark ==
matcher convention digest: fnv1a64:e79f623705344f98
case 'control' [PASS]: matched=2/expected=2 ambiguous=0 failures=
case 'colliding-names' [PASS]: matched=1/expected=1 ambiguous=1 failures=
case 'line-shift' [PASS]: matched=1/expected=1 ambiguous=0 failures=
case 'move' [PASS]: matched=2/expected=2 ambiguous=0 failures=
case 'move-edit' [PASS]: matched=2/expected=2 ambiguous=0 failures=
case 'pure-rename' [PASS]: matched=2/expected=2 ambiguous=0 failures=
case 'rename-edit' [PASS]: matched=1/expected=1 ambiguous=0 failures=
gates: precision=1.0000 (>= 0.95) recall=1.0000 (>= 0.9) line_shift_retention=1.0000 (== 1) move_retention=1.0000 (>= 0.99)
quarantined ambiguous: case 'colliding-names' after='src/c.rs:handle:1' candidates=["src/a.rs:handle:1", "src/b.rs:handle:1"]
```
M3 exit gates all met at benchmark level: precision 1.0000 ≥ 0.95, recall 1.0000 ≥ 0.90
(UAT-U22 related), line_shift_retention 1.0000 == 1.00 (UAT-U20 related), move_retention
1.0000 ≥ 0.99 (UAT-U21 related); 7/7 cases PASS; the expected-Ambiguous case is
quarantined and reported separately with candidates listed and no stable id.

**Workspace isolation (fresh)**:
```text
workspace isolation: 7 ws-a outcomes, 7 ws-b outcomes, occurrences disjoint, collisions=0
```

**e37 evidence-reuse validation (fresh, e38 additive-only proof)**:
```text
$ just lsi-equivalence
exit=0  output sha256:c035aa1d6c6673308904c22dd7b80bdcfb510ba4b34e7be36816df3c40ace113
7/7 tests pass; python-hello 1.0000/1.0000, rust-hello 1.0000/1.0000,
multi-lang-types [QUARANTINED] 0.5969/0.0714 (measured and reported every run);
e37 harness green with `PINNED_IDENTITY_DIGEST` byte-untouched
(grep: equivalence_harness/harness.rs:91 = "fnv1a64:efccc22e912913fe";
file mtime 16:03 predates all e38 edits 22:52–23:25)
$ just lsi-fixtures check
exit=0  output sha256:24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc
RESULT: PASS — all 42 goldens byte-identical
```

**Clippy delta (warnings non-fatal; zero findings required on e38 paths)**:
```text
$ cargo clippy -p cognicode-core --lib --tests --features evidence-kernel
exit=0  output sha256:7ae9d7557e2a3528a40b14f4f0e930b5036fac4cc860f717ab13632cd2c7f0ec
5 findings, ALL pre-existing documented red: cognicode-macros 3
(collapsible_if ×2, let_and_return — aix_tool.rs/newtype.rs) + core lib-test 2
(generic_graph.rs:467 unused `chrono::TimeZone` import, :479 dead `doc_id`).
grep over continuity/|rename_evidence|identity_benchmark|workspace_isolation: 0 hits.
git diff HEAD on cognicode-macros/ and domain/aggregates/generic_graph.rs: EMPTY
(the status entries matching "generic_graph" are e37's NEW projection files, not this file).
```

**Coverage**: ➖ Not available (no coverage tooling configured for this change;
`openspec/config.yaml` declares no coverage threshold). Coverage proxy = scenario matrix
below plus the 7-case fixture multiset.

### Spec Compliance Matrix

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Continuity REQ-1: Tiered deterministic continuity matching | Line shift keeps stable identity | `continuity/matcher.rs > t1_line_shift_keeps_stable_identity` (fresh; same path+name+kind, line segment shifted, T1 PathNameKind) + benchmark `line-shift` case PASS (line 1→6, `line_shift_retention=1.0000`) | ✅ COMPLIANT |
| Continuity REQ-1: Tiered deterministic continuity matching | File move keeps stable identity | `continuity/matcher.rs > t2_file_move_keeps_stable_identity` (fresh; declared move evidence ≥ floor, equal name+kind) + benchmark `move`/`move-edit` cases PASS (`move_retention=1.0000`) | ✅ COMPLIANT |
| Continuity REQ-1: Tiered deterministic continuity matching | Fingerprint is fact-deterministic | `continuity/fingerprint.rs > fingerprint_is_fact_deterministic` (fresh; same committed fact set → identical fingerprints) + `matcher.rs > result_is_identical_under_shuffled_fact_and_rename_input` + `view.rs > view_is_identical_under_shuffled_fact_input` (matcher sorts itself; permutation-invariant) | ✅ COMPLIANT |
| Continuity REQ-2: Ambiguous continuity fails closed | Colliding names fail closed | `continuity/matcher.rs > t3_epsilon_tie_fails_closed` + `t2_candidates_within_epsilon_fail_closed` + `multiple_t1_candidates_fail_closed` + `ambiguous_is_terminal_and_never_rescued_by_a_lower_tier` (fresh; Ambiguous lists candidates, NO StableEntityId, terminal, never merged) + benchmark `colliding-names` case: Ambiguous with candidates, quarantined, no merge | ✅ COMPLIANT |
| Continuity REQ-3: Fail-closed rename evidence port | Version control unavailable degrades safely | `rename_evidence.rs > adapter_returns_empty_when_git_is_absent` + `adapter_returns_empty_outside_a_repository` + `adapter_returns_empty_for_unborn_repository` + `parser_degrades_to_none_on_malformed_rows` (fresh; every failure mode → warn + EMPTY evidence, never invented) | ✅ COMPLIANT |
| Continuity REQ-4: Workspace isolation of continuity | Identical workspaces do not collide | `workspace_isolation.rs > identical_workspaces_do_not_collide` (fresh; "7 ws-a outcomes, 7 ws-b outcomes, occurrences disjoint, collisions=0") + `differing_workspaces_do_not_contaminate_each_other` | ✅ COMPLIANT |
| Benchmark REQ-1: Pinned scoring gates and fixture coverage | Missed gate fails the run | `identity_benchmark.rs > missed_gate_fails_the_run` + `precision_gate_failure_names_gate_and_value` (fresh; one unmet gate → run fails naming gate + measured value; gates declared in harness.rs before scoring) | ✅ COMPLIANT |
| Benchmark REQ-1: Pinned scoring gates and fixture coverage | Ambiguous outcome reported separately | `identity_benchmark.rs > ambiguous_outcome_reported_separately` (fresh; colliding-names excluded from precision/recall, reported with candidates) + `expected_ambiguous_returning_matched_fails_the_case` (inverted fail-closed check) | ✅ COMPLIANT |
| Benchmark REQ-2: Separate pinned convention digest | Convention change requires explicit re-pin | `identity_benchmark.rs > convention_change_requires_explicit_re_pin` (fresh; current digest matches `fnv1a64:e79f623705344f98` printed live by the run; changed convention → error instructing explicit re-pin; digest separate from e37's `fnv1a64:efccc22e912913fe`, verified byte-untouched) | ✅ COMPLIANT |
| Umbrella R1: Entity and occurrence separation (supplementary, not counted) | Line shift preserves identity | `t1_line_shift_keeps_stable_identity` + benchmark `line-shift` case (new occurrence maps to the same `StableEntityId`; occurrence = snapshot-scoped `EntityId` + `OccurrenceId`) | ✅ COMPLIANT |
| Umbrella R1: Entity and occurrence separation (supplementary, not counted) | Ambiguous continuity fails closed | `t3_epsilon_tie_fails_closed`/`multiple_t1_candidates_fail_closed` + benchmark `colliding-names` case (matcher returns Ambiguous, creates no forced identity merge) | ✅ COMPLIANT |

**Compliance summary**: 9/9 scenarios compliant (6/6 requirements); umbrella R1 both
scenarios additionally verified through the matcher/benchmark runs.

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| Continuity REQ-1: Tiered deterministic continuity matching | ✅ Implemented | `matcher.rs` runs T0 exact FQN → T1 path+name+kind → T2 declared evidence + name/kind equality + 0.5 floor → T3 tagged-multiset Jaccard ≥ 0.6 with kind hard pre-filter; fingerprint from facts only (`fingerprint.rs`, no extractor change — `grep -rn continuity application/` = 0 hits); matcher sorts via `SnapshotEntityView::from_facts` and indexes renames order-freely (duplicate rows collapse to max similarity, sorted deterministically) |
| Continuity REQ-2: Ambiguous continuity fails closed | ✅ Implemented | Statuses exactly `Matched{tier,confidence}/New/Terminated/Ambiguous{candidates}`; >1 candidate separated by ≤ `PINNED_AMBIGUITY_EPSILON=0.05` → Ambiguous listing every candidate FQN with `stable_id: None` (None ONLY for Ambiguous, doc-enforced and tested); terminal per pair, never rescued by a lower tier, never retroactively force-matched; candidate ordering only enumerates deterministically |
| Continuity REQ-3: Fail-closed rename evidence port | ✅ Implemented | Port in `domain/evidence_kernel/ports.rs` (no I/O in the trait; domain purity preserved); adapter shells out `git -C <root> diff --name-status --find-renames=50%` with fixed `.arg()` argv; spawn error / non-zero exit / non-UTF-8 / malformed row (strict whole-result parse) each degrade to warn + empty Vec — no evidence, never invented evidence |
| Continuity REQ-4: Workspace isolation of continuity | ✅ Implemented | Continuity derives from per-workspace `facts_in_snapshot` reads keyed `(WorkspaceId, SnapshotId)`; isolation suite proves identical symbol sets across two workspaces produce disjoint occurrences, zero mapping/candidate crossings, collisions = 0 |
| Benchmark REQ-1: Pinned scoring gates and fixture coverage | ✅ Implemented | Gates declared as constants before scoring (`PRECISION_THRESHOLD=0.95`, `RECALL_THRESHOLD=0.90`, `LINE_SHIFT_RETENTION=1.00`, `MOVE_RETENTION=0.99`); 7 ground-truth fixtures cover rename(+edit), move(+edit), colliding names, line shift, control; missed gate fails naming gate+value; Ambiguous counts as neither match nor miss and is reported separately |
| Benchmark REQ-2: Separate pinned convention digest | ✅ Implemented | `MATCHER_CONVENTION` text (tier order, fingerprint composition, pool order, fail-closed rules, pinned constants) hashed FNV-1a 64 into `PINNED_MATCHER_DIGEST fnv1a64:e79f623705344f98` — separate constant in a separate harness file from e37's `PINNED_IDENTITY_DIGEST fnv1a64:efccc22e912913fe`; convention change fails the run until explicit re-pin |
| Umbrella R1: Entity and occurrence separation | ✅ Implemented | `StableEntityId` (logical, cross-snapshot) modeled separately from occurrence location (snapshot-scoped `EntityId` + `OccurrenceId::from_entity`); facts, goldens, store schemas untouched (Option B) |

### Coherence (Design)
| Decision | Followed? | Notes |
|----------|-----------|-------|
| D1 Continuity in `domain/evidence_kernel/continuity/{mod,view,fingerprint,matcher}.rs` (cfg `evidence-kernel`); `StableEntityId` additive in `ids.rs` Display `stable:N`; `OccurrenceId` const wiring; output types as declared; no read port/application service in M3 | ✅ Yes | Exact module map; `ContinuityOutcome{fqn,occurrence,snapshot,stable_id,status}` matches the design interface; tests wire store→matcher directly; no production consumer (`application/` grep = 0) |
| D2 Matcher consumes rename evidence as DATA (`match_snapshots(before, after, renames, thresholds)`); caller resolves the port | ✅ Yes | Signature exact; fixture-declared `evidence.json` injects identically to adapter output; port exists for cutover wiring |
| D3 Tier pipeline T0→T3 over recovered views (FQN from `core:defines` object, kind from `provenance.detail`, callees/refs from facts); entities = defines subjects only; fail-closed ambiguity (margin/epsilon/double-claim) with no id; Terminated retains id; New = K+1.. sorted-FQN; no-resurrection (strictly pairwise); Ambiguous terminal | ✅ Yes | All mechanics verified by 36 fresh continuity tests incl. `reintroduced_symbol_is_new_not_resurrected`, `ambiguous_is_terminal_and_never_rescued_by_a_lower_tier`, `stable_ids_are_pooled_in_sorted_fqn_order`, `outcomes_are_sorted_by_snapshot_then_fqn` |
| D4 Fingerprint v1 = one tagged multiset `name:`/`call:`/`ref:` (sorted BTreeMap), `similarity` = 0.0 unless kind equal else Jaccard; bodyless entities score 0 and never T3-match; pure rename costs exactly the two `name:` elements | ✅ Yes | `similarity_of_a_pure_rename_costs_exactly_the_two_name_elements`, `t3_bodyless_entities_never_match`, `bodyless_entities_score_zero_fail_closed`, `similarity_is_zero_unless_kinds_are_equal` all fresh-pass |
| D5 `RenameEvidencePort` sync Send+Sync additive in kernel `ports.rs`; adapter `git -C` fixed argv; ANY failure → empty Vec + warn | ✅ Yes (+ deviation b) | Strict parse discards the WHOLE result on the first malformed row (design D5 "degrades to empty rather than partial evidence"); `port_and_adapter_are_send_sync` verified |
| D6 Thresholds pinned constants 0.6/0.05/0.5 in `matcher.rs`; separate `MATCHER_CONVENTION` + `PINNED_MATCHER_DIGEST`; change fails run until re-pin | ✅ Yes (+ deviation d) | `thresholds_default_to_the_pinned_constants` fresh-pass; digest printed live `fnv1a64:e79f623705344f98`; e37 digest confirmed byte-untouched |
| D7 Benchmark mirrors e37 pattern: gates declared before scoring, 7 fixtures (not git repos; T2 evidence declared; git adapter tested on throwaway `git init` repos), Ambiguous quarantine, inverted fail-closed check, `just lsi-identity` fixed argv | ✅ Yes (+ deviation c) | All four gate mechanisms and both quarantine tests fresh-pass; recipe at `justfile:619-623` |
| D8 No bench — honest N/A (no production consumer in M3; fixture-scale matcher; avoids a new sub-µs noise surface) | ✅ Yes | No `[[bench]]` added; consistent with the e37 perf-gate WARNING precedent |
| Deviation (a): git-absent test injects via cfg(test) `with_git_program("/nonexistent-e38-test/git")` instead of a PATH override | ⚠️ Deviation (benign, accepted) | `std::env::set_var` is unsafe in Rust 2024 (task 2.1's "PATH override" premise); the cfg(test)-only constructor mutates no process state and makes the spawn failure deterministic. Scenario "Version control unavailable degrades safely" fully covered (4 fresh tests). Not spec-breaking |
| Deviation (b): malformed row → fail-closed WHOLE result (task said "skip" the row) | ⚠️ Deviation (benign, accepted) | Design D5 explicitly chose "strict parse degrades to empty rather than partial evidence"; the whole-result discard is strictly safer than skipping (never partial evidence). Task 2.1's testable outcome ("malformed `R<nnn>` row → empty evidence") is what the fresh test asserts. Not spec-breaking |
| Deviation (c): benchmark harness resolves matched before-FQNs by indexing the sorted-FQN pool (`StableEntityId(k)` ↔ `pool[k-1]`, `before_pool_fqns` in harness.rs:385-391) because `ContinuityOutcome` carries only the stable id | ⚠️ Deviation (benign, accepted) | The design interface has no before-FQN field on the outcome; the pool order is itself part of the pinned `MATCHER_CONVENTION` text, so the resolution is deterministic and digest-protected. Harness-side scoring only — no matcher semantics change. Not spec-breaking |
| Deviation (d): thresholds landed at the design drafts (0.6/0.05/0.5) with fixture-body sizing instead of threshold tuning | ⚠️ Deviation (benign, accepted) | Design Open Questions sanctioned "confirmed during WU-3/4 fixture tuning; any change re-pins"; matcher.rs documents the WU-4 tuning note (fixtures sized so bodies carry the structural evidence — e.g. pure-rename bodies with 8 call elements). Digest pinned over the final values; re-pin duty recorded in TESTING-STATE. Not spec-breaking |
| Deviation (e): extractor emits local let-bindings as Variable `core:defines` facts, and the ground truth includes them honestly | ⚠️ Deviation (benign, accepted) | Pre-existing e37 extractor behavior, NOT an e38 change (proposal forbids extractor changes — none made). `pure-rename` expected-mapping includes `src/lib.rs:total:2` Matched (ExactIdentity) alongside the renamed function; matched=2/expected=2 counts it honestly. Not spec-breaking |
| Deviation (f): UAT-U20's "conserva EntityId" maps to `StableEntityId` retention | ⚠️ Deviation (benign, accepted) | The terminology pin (delta spec header: spec "EntityId" = `StableEntityId`; occurrence = snapshot `EntityId` + `OccurrenceId`) was authored exactly for this drift; the UAT record states the mapping explicitly. Not spec-breaking |

None of the six deviations breaks a spec requirement or scenario.

### Issues Found

**CRITICAL**: None.

**WARNING**:
1. **UAT-U20/U21/U22 formal runs deferred (A5 pattern, e37 verify precedent).** The
   benchmark evidence comes from synthetic micro-fixtures with declared evidence, NOT the
   e36 golden repos, and no production/Explorer surface exists in M3 to exercise
   continuity end-to-end. The deferral is honestly recorded in the UAT catalogue
   ("PASS pendiente de ejecución UAT formal") and in `.agent/TESTING-STATE.md`, and the
   design/tasks declared it non-blocking; the M3 exit-gate numbers (precision/recall/
   retentions all 1.0000) are nonetheless benchmark-scale claims, not golden-repo-scale
   or user-surface claims. A formal UAT pass on golden-repo scale remains the single
   unexercised step.
2. **No production consumer of continuity in M3 (cutover deferred).** The matcher,
   fingerprint, and stable ids are exercised only by tests/harness — `application/`
   contains zero references to continuity, and the design records this as an honest
   no-op (proposal lists consumer cutover as out of scope). Consequence: the identity
   layer's real-world integration (store wiring in the runtime, any read path) is
   completely unexercised by construction in this milestone; first wiring happens at a
   later cutover and must not be assumed verified by this report.
3. **Git rename evidence in the benchmark is fixture-declared, not adapter-produced.**
   The `move`/`move-edit` cases inject T2 evidence from `evidence.json` because the
   fixtures are not git repositories (design D7); the real `git diff -M` adapter is
   exercised only on throwaway `git init` repos in the 10 unit tests. The two halves
   have never been run end-to-end together (adapter output feeding a benchmark case).
   Design-sanctioned, but the composition remains unproven until fixtures or a formal
   UAT run use live adapter evidence.
4. **Pre-existing clippy red outside e38 (carried from e36/e37 records).** 5 findings in
   the feature-on lib+tests run: cognicode-macros ×3 (`collapsible_if`/`let_and_return`)
   + `domain/aggregates/generic_graph.rs:467,479` ×2 test lints. `git diff HEAD` over
   both locations is EMPTY — untouched by e36/e37/e38. Zero findings on any e38 path
   (required gate met).

**SUGGESTION**:
1. `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md` (M3 section)
   has no `### UAT-U21` heading: the move-scenario coverage block (move/move-edit,
   T2 evidence, collisions) was recorded as prose under UAT-U20's heading. The substance
   is present and honest, but a formal UAT executor scanning the catalogue will not find
   a U21 entry. Archive should add the `### UAT-U21 [P0] — File move` heading before the
   second e38 coverage block (verify must not edit it).
2. `openspec/changes/e38-lsi-stable-identity/state.yaml` still shows
   `apply: {status: pending}` and `verify: {status: pending}` although tasks.md is 16/16
   and apply records report completion; the orchestrator owns state.yaml and should
   advance the markers before archive.
3. The proposal's six success-criteria checkboxes are unchecked; archive should reconcile
   them against this report: line-shift retention 1.0000 (criterion 1), move retention
   1.0000 ≥ 0.99 (criterion 2), precision/recall 1.0000 ≥ 0.95/0.90 (criterion 3),
   collisions = 0 (criterion 4), Ambiguous never force-matched (criterion 5),
   `PINNED_IDENTITY_DIGEST` unchanged — e37 harness green (criterion 6).
4. Same-file overloads (same path+name+kind, different lines) resolve to `Ambiguous` in
   v1 — accepted fail-closed per design Open Questions; order-aware disambiguation is a
   conscious future extension (re-pin duty applies).

### Verdict
PASS WITH WARNINGS
All 9/9 scenarios across both owned spec surfaces are COMPLIANT with fresh runtime
evidence (141 e38 tests + 7 e37-reuse harness tests + 42/42 goldens, every declared
command exit 0 with recorded hashes; M3 exit gates all met at 1.0000 and the convention
digest printed live by the run); the four warnings are documented, honestly-bounded gaps
— the A5-deferred formal UAT runs, the absent production consumer, the fixture-declared
benchmark evidence for the git adapter, and the pre-existing clippy red — none of which
breaks a spec scenario.
