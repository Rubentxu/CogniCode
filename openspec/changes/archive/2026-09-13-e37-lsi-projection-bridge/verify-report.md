```yaml
schema: gentle-ai.verify-result/v1
evidence_revision: sha256:6aac01560b92fc81daded01ba769cb5b20cbfc260b286aac9a6eea3f31baa528
verdict: pass_with_warnings
blockers: 0
critical_findings: 0
requirements: 6/6
scenarios: 12/12
test_command: cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel -- --nocapture
test_exit_code: 0
test_output_hash: sha256:9ef4df4fab08b69b7fb6a763b76a05823dd657204d65f21fbd3ae47a6210664d
build_command: cargo check -p cognicode-core --features evidence-kernel,multimodal
build_exit_code: 0
build_output_hash: sha256:12999f19dc0b3af5ca3d54561f88b189a092e845f9b53f5b42277622d2f214ec
```

## Verification Report

**Change**: e37-lsi-projection-bridge
**Version**: N/A (no spec version declared on either delta spec surface)
**Mode**: Standard (strict TDD inactive; strict-tdd-verify.md not loaded)

Independent final verification: all runtime evidence below was generated fresh by this
verify run on the current working tree (branch `main`, HEAD `4e1d39eb`, e36+e37 uncommitted),
NOT reused from apply evidence. `evidence_revision` is the SHA-256 over the fixed-order
manifest of the sixteen per-command `command|exit|output-hash` rows listed in this report.

Owned spec surface: delta `specs/projection-equivalence-harness/spec.md` (4 requirements /
8 scenarios) + delta `specs/generic-graph-projection/spec.md` (2 requirements / 4 scenarios)
= 6 requirements / 12 scenarios. Umbrella `cognicode-living-software-intelligence` →
`projection-architecture` (R1 equivalence / R2 rebuildability) is implemented by e37 and is
verified through the same harness runs as supplementary rows (not counted in the 6/12).

### Completeness
| Metric | Value |
|--------|-------|
| Tasks total | 21 |
| Tasks complete | 21 |
| Tasks incomplete | 0 |

Task-completeness spot-check (claimed artifacts verified to exist and inspected): 1.2/1.3
`domain/evidence_kernel/{ports.rs (additive facts_in_snapshot), bootstrap.rs (six `core:*`,
idempotent)}` + `infrastructure/evidence_kernel/in_memory.rs` impl; 2.2–2.4
`application/fact_bridge/{mod,entity_table,batch_builder,tree_sitter_facts,lsp_facts}.rs`;
3.2 `CallGraphProjection::from_facts` (cfg-gated, lexicographic tie-break, unresolved
dropped+counted); 3.3 dual-gated `domain/ports/generic_graph_projection.rs` +
`infrastructure/graph/generic_graph_projection.rs`; 4.1–4.4
`tests/equivalence_harness{.rs,/*.rs}` with `EQUIVALENCE_THRESHOLD=0.99`,
`KNOWN_UNSTABLE_SURFACES=["multi-lang-types"]`, `PINNED_IDENTITY_DIGEST`
`fnv1a64:efccc22e912913fe`, `justfile` `lsi-equivalence` recipe (fixed argv); 4.5
`benches/fact_bridge_benchmarks.rs` + `Cargo.toml` `[[bench]]` entry; 5.1 honest no-op
determination recorded in tasks.md; 5.2 UAT-U10/U11 records in
`docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md:44-56`; 5.3
`.agent/TESTING-STATE.md` handoff present.

### Build & Tests Execution

**Build**: ✅ Passed (all four states)
```text
$ cargo check -p cognicode-core                                        # gate off
exit=0  output sha256:b8305cd8446a78edda323602581d2949138fdf09a7d2f6434e21edf6b69989e5
$ cargo check -p cognicode-core --features evidence-kernel              # kernel on
exit=0  output sha256:b8305cd8446a78edda323602581d2949138fdf09a7d2f6434e21edf6b69989e5
$ cargo check -p cognicode-core --features evidence-kernel,multimodal   # dual-gated port
exit=0  output sha256:12999f19dc0b3af5ca3d54561f88b189a092e845f9b53f5b42277622d2f214ec
$ cargo check -p cognicode-runtime                                     # consumer crate
exit=0  output sha256:e052f08151a523a439df57c09c4c86dbe512e3beeaa2976d416a3a99de3a4f62
$ cargo fmt -p cognicode-core --check
exit=0  output sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 (empty)
```
The two first check logs hash identically because both builds were fully cached and cargo
emitted the same 4-line output (workspace profile warning + `Finished` line); both exit 0.

**Tests**: ✅ 118 passed / ❌ 0 failed / ⚠️ 0 skipped (plus gate-off run: 0 tests compiled)
```text
$ cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel
exit=0  output sha256:6098674ccac65a941ff01c13fd8a024b1225d3afed949dd3768039fc8af863f0
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 1712 filtered out
$ cargo test -p cognicode-core --lib fact_bridge --features evidence-kernel
exit=0  output sha256:0f1638e7f4d18d8402967c07eb0e48b32c1698df50b4834ad3199d9db2d109af
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 1745 filtered out
$ cargo test -p cognicode-core --lib call_graph_projection --features evidence-kernel
exit=0  output sha256:1b0960ab5e02e0672b3b3cbe48fabc01a7083865bd2a67dbc8d5aed5d0ffa90e
test result: ok. 44 passed; 0 failed; 0 ignored; 0 measured; 1715 filtered out
$ cargo test -p cognicode-core --lib generic_graph_projection --features evidence-kernel,multimodal
exit=0  output sha256:60d2a879bf2bd2c2ffe057092dc9471a6625ce9ec1048189930d9460d33d5c73
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 1972 filtered out
$ cargo test -p cognicode-core --test equivalence_harness --features evidence-kernel -- --nocapture
exit=0  output sha256:9ef4df4fab08b69b7fb6a763b76a05823dd657204d65f21fbd3ae47a6210664d
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
$ cargo test -p cognicode-core --test equivalence_harness          # GATE OFF (feature absent)
exit=0  output sha256:fc6b14d312d8ef4ec4fcd89e867b4d06758f715ef5eced0ec5d72fc39025743e
running 0 tests — the bridge surface compiles to nothing without the feature
```

**Equivalence harness per-fixture scores (fresh run, threshold 0.99 declared before comparison)**:
```text
fixture 'python-hello'      [scored]:      node_score=1.0000 edge_score=1.0000 legacy(nodes=7,  edges=3) fact(nodes=7,  edges=3,  unresolved=0)
fixture 'rust-hello'        [scored]:      node_score=1.0000 edge_score=1.0000 legacy(nodes=5,  edges=0) fact(nodes=5,  edges=0,  unresolved=3)
fixture 'multi-lang-types'  [QUARANTINED]: node_score=0.5969 edge_score=0.0714 legacy(nodes=94, edges=7) fact(nodes=112,edges=23, unresolved=40)
identity-convention digest matches pinned baseline fnv1a64:efccc22e912913fe; run passed
```

**M0 goldens (legacy byte-stability gate, fresh)**:
```text
$ python3 sandbox/scripts/capture_lsi_fixtures.py --check
exit=0  output sha256:24ef2485d48d112baab546bcb53e0e21d6d6de4bd3f15e574019f70eacaaf3bc
RESULT: PASS — all 42 goldens byte-identical; covered 14/48 surfaces (weighted 37/111 = 33.3%)
```

**Clippy delta (warnings non-fatal; zero findings required on e37 paths)**:
```text
$ cargo clippy -p cognicode-core --lib --tests --features evidence-kernel,multimodal
exit=0  output sha256:b6eedd3bab3c6261ba9c0cbf096b8fcc57f720ee0356d421076f2b2d7fd26ddb
findings touching fact_bridge / generic_graph_projection / evidence_kernel /
equivalence_harness / call_graph_projection: 0. All 3 warnings in cognicode-macros
(aix_tool.rs:154, newtype.rs:59, newtype.rs:119).
$ cargo clippy -p cognicode-core --tests    # default features, attribution check
exit=0  output sha256:16e30c6785f97ea4ba3795bc1bf8ec123aa51d1e5f4d6033197f6b6bcb194565
same 3 macro warnings + 2 pre-existing `generic_graph.rs` test-lint warnings (467:9, 479:8).
git diff HEAD --stat on cognicode-macros/ and generic_graph.rs: EMPTY (untouched by e36/e37).
```

**Coverage**: ➖ Not available (no coverage tooling configured for this change;
`openspec/config.yaml` declares no coverage threshold). Coverage proxy = scenario matrix below.

### M2 Perf-Gate Re-Adjudication (key open item)

Apply batch 2 flagged that `lsi_bench_baseline.py compare --fail-above 10` fails on a
DIFFERENT sub-µs benchmark each run. This verify ran the gate twice more, fresh (each a full
`cargo bench` re-run, not parsed from cache):

```text
$ python3 sandbox/scripts/lsi_bench_baseline.py compare --fail-above 10   # verify run 4
exit=1  output sha256:b134a20d18da98225c9927373725d12b66e4457f3f2683d2ecb27343114bb83b
RESULT: FAIL — 2 benchmark(s) regressed beyond 10.0%: add_node, search
$ python3 sandbox/scripts/lsi_bench_baseline.py compare --fail-above 10   # verify run 5
exit=1  output sha256:35e9c90ac945813870d1a5436e399e39d52ec6654efefc3f415013ddfc6681db
RESULT: FAIL — 1 benchmark(s) regressed beyond 10.0%: search
```

Per-benchmark deltas (apply-era persisted run 3 vs the two fresh verify runs; % vs
`baseline.json` captured 11:15):

| benchmark | apply run3 | verify run4 | verify run5 |
|-----------|-----------:|------------:|------------:|
| search (0.079 µs baseline) | +2.53% | **+124.05%** | **+124.05%** |
| add_node (2.19 µs) | −2.69% | **+23.84%** | +6.67% |
| symbol_index_build_1000_files | −0.80% | +4.02% | −0.05% |
| bfs_traversal_100_nodes | +1.57% | +3.29% | +1.43% |
| p99_graph_cache/update | +0.25% | +3.00% | +3.38% |
| call_graph_10k_lines_python | +0.72% | +2.32% | +1.25% |
| lightweight_index_build_1000_files | +0.31% | +1.98% | +0.51% |
| get_neighbors | +1.29% | +1.68% | +2.17% |
| p99_call_graph/construction | +1.12% | +1.37% | +1.00% |
| all remaining 16 benchmarks | within ±7.2% | within ±5.1% | within ±10.2% |
| hot_path_incoming_calls | −12.98% | −12.21% | −10.16% (consistently FASTER) |

Within threshold: 22/24 (run 4) and 23/24 (run 5). Both flagged benchmarks are tiny;
`search` measured 176.33 ns ± 0.44 (criterion, in-window) vs 79 ns baseline — precise and
reproducible inside this verify window, but it swung +15.19% → −1.27% → +2.53% across the
three batch-2 windows on the same morning.

Attribution evidence gathered by this verify (no code was modified):
1. **One-usize-field claim VERIFIED in code**: `git diff HEAD` over
   `infrastructure/graph/call_graph_projection.rs` (+563 lines) shows the only non-cfg-gated
   additions are `unresolved_edges: usize` (field), its `unresolved_edges: 0` initializer,
   and the trivial `unresolved_edges()` accessor. Everything else (from_facts, helpers,
   tests) is behind `#[cfg(feature = "evidence-kernel")]` / `#[cfg(all(test,
   feature = "evidence-kernel"))]`. All other e37 surfaces are cfg-gated modules (verified
   by diff over the six wiring files).
2. The `search` benchmark exercises `LightweightIndex::find_symbol` over a 100-file index
   (`benches/graph_benchmarks.rs:820-844`) — code e37 never touched, on a struct that
   received no fields.
3. Binary timeline: the release bench binary was built 15:48:38 (after the last source
   change; batch 3 was records-only) and was NOT rebuilt between verify runs 4 and 5; the
   baseline was captured 11:15 on an earlier binary whose default-path surface differs from
   the final one by at most that one usize field. ms-scale, memory-heavy benchmarks are all
   within ±4.02% in both verify runs, so the machine is not globally slower — but an
   L1-resident ~80 ns loop is exactly the kind of workload that a CPU-frequency/governor
   window (or a code-layout shift from the single added field) moves by large factors
   without affecting ms-scale benchmarks.
4. `add_node` (2.19 µs, criterion SE ±37% in-window) flagged in run 4 and recovered in
   run 5 on the SAME binary — direct in-window proof that these micro measurements are
   noise-sensitive.

Verdict on the gate: **NOT certified clean** — the gate command exits 1 on today's machine
window, and `search` reproduced >10% in both verify runs, so option (a)'s "flagging
benchmarks are unstable across runs" premise does not hold for `search`. At the same time
the regression is demonstrably outside e37's compiled default-path surface (points 1–2),
and the residual attribution uncertainty (frequency window vs one-field layout butterfly)
cannot be resolved inside verify, which is forbidden from remediating. Flagged honestly as
WARNING 1 with a concrete re-adjudication path for archive/M3. Committed
`sandbox/results/lsi-baseline/delta.json` was left untouched (fresh deltas written to
`/tmp/e37-verify/delta-run{4,5}.json`).

### Spec Compliance Matrix

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Harness REQ-1: Declared equivalence contract | All fixtures meet the threshold | `tests/equivalence_harness.rs > all_fixtures_meet_the_threshold` (fresh; threshold 0.99 declared pre-comparison; per-fixture report printed; digest pin checked) | ✅ COMPLIANT |
| Harness REQ-1: Declared equivalence contract | Fixture below threshold fails the run | `tests/equivalence_harness.rs > fixture_below_threshold_fails_the_run` (edge 0.5 → run fails, names fixture + score) | ✅ COMPLIANT |
| Harness REQ-2: Quarantine of known-unstable surfaces | Quarantined divergence is excluded and reported | `tests/equivalence_harness.rs > quarantined_divergence_is_excluded_and_reported` (fresh multi-lang-types measured 0.5969/0.0714 and reported; synthetic zero-score quarantined does not fail the run) | ✅ COMPLIANT |
| Harness REQ-2: Quarantine of known-unstable surfaces | Unquarantined divergence fails | `tests/equivalence_harness.rs > unquarantined_divergence_fails` (0.75 surface named in failures) | ✅ COMPLIANT |
| Harness REQ-3: Deterministic fact identity | Repeated extraction is identical | `tests/equivalence_harness.rs > repeated_extraction_is_identical` (byte-identical bincode fact sets) + `fact_bridge > batch_builder::tests::repeated_extraction_is_identical` (reversed walk order) + `finish_assigns_canonical_ids_regardless_of_insertion_order` | ✅ COMPLIANT |
| Harness REQ-3: Deterministic fact identity | Convention change requires explicit re-baseline | `tests/equivalence_harness.rs > convention_change_requires_explicit_re_baseline` (current digest matches pin; changed convention → error instructing explicit re-baseline; pin also asserted in the live run) | ✅ COMPLIANT |
| Harness REQ-4: No silent cutover | Gate off serves the legacy path | Gate-off harness run (0 tests compiled) + `cargo check -p cognicode-core` default exit 0 + cfg-gating verified by diff + `capture_lsi_fixtures.py --check` PASS 42/42 byte-identical (fresh, with all e37 edits in tree) | ✅ COMPLIANT |
| Harness REQ-4: No silent cutover | Failing harness keeps legacy default | Structural proof: no default-source-selection surface exists in M2 — `from_facts`, `fact_bridge`, and the harness are feature-gated (gate-off run compiles 0 tests); no runtime wiring (task 5.1 determination, `cargo check -p cognicode-runtime` exit 0 fresh); goldens 42/42 prove consumer outputs still come from the legacy path | ✅ COMPLIANT |
| Generic REQ-5: Fact-derived generic graph emission | Facts map to nodes and edges | `generic_graph_projection.rs > facts_map_to_nodes_and_edges` (fresh; file node + symbol nodes, contains/resolved-call edges, unresolved skipped) | ✅ COMPLIANT |
| Generic REQ-5: Fact-derived generic graph emission | Empty snapshot yields empty projection | `generic_graph_projection.rs > empty_snapshot_yields_empty_projection` (fresh; no nodes, no edges, no error) | ✅ COMPLIANT |
| Generic REQ-5: Fact-derived generic graph emission | Clear and rebuild is equivalent | `generic_graph_projection.rs > clear_and_rebuild_is_equivalent` + harness `rebuild_from_reread_facts_equals_prior_projection` (exact node/edge multiset equality from re-read facts) | ✅ COMPLIANT |
| Generic REQ-6: Consumers remain FactStore-ignorant | Consumer needs only emitted types | `generic_graph_projection.rs > consumer_needs_only_emitted_types` (fresh; consumes `Vec<GraphNode>`/`Vec<GraphEdge>` only) | ✅ COMPLIANT |
| Umbrella R1: CallGraph projection equivalence (supplementary, not counted) | Golden equivalence | Harness fresh run: python-hello 1.0000/1.0000, rust-hello 1.0000/1.0000 ≥ declared 0.99; `just lsi-equivalence` recipe fixed argv | ✅ COMPLIANT |
| Umbrella R2: Derived projections are rebuildable (supplementary, not counted) | Projection rebuild | `rebuild_from_reread_facts_equals_prior_projection` (exact multiset equality) + `clear_and_rebuild_is_equivalent` (generic port) | ✅ COMPLIANT |

**Compliance summary**: 12/12 scenarios compliant (6/6 requirements); umbrella R1/R2 additionally verified through the harness runs.

### Correctness (Static Evidence)
| Requirement | Status | Notes |
|------------|--------|-------|
| Harness REQ-1: Declared equivalence contract | ✅ Implemented | `EQUIVALENCE_THRESHOLD=0.99` and multiset-Jaccard method declared in `tests/equivalence_harness/harness.rs:63-65` before any comparison; `evaluate_reports` fails naming fixture + score; per-fixture `describe()` report printed every run |
| Harness REQ-2: Quarantine of known-unstable surfaces | ✅ Implemented | `KNOWN_UNSTABLE_SURFACES=["multi-lang-types"]` measured and reported (0.5969/0.0714 fresh) but excluded from the verdict; nothing outside the quarantine is tolerated |
| Harness REQ-3: Deterministic fact identity | ✅ Implemented | D3 convention pinned via `IDENTITY_CONVENTION` + `PINNED_IDENTITY_DIGEST` (FNV-1a64); `EntityIdTable` sorted strings → `EntityId(1..N)`, no hashing; `finish()` canonical sort; only registered `core:*` predicates reach the store (commit rejects unregistered) |
| Harness REQ-4: No silent cutover | ✅ Implemented | Bridge reachable only behind off-by-default `evidence-kernel` (verified by gate-off run + cfg diff); legacy goldens byte-stable (42/42 fresh); cutover machinery does not exist in M2 |
| Generic REQ-5: Fact-derived generic graph emission | ✅ Implemented | `FactGenericGraphProjection` reads `facts_in_snapshot` and maps defines/contains/calls/imports/inherits/references onto `GraphNode`/`GraphEdge`; dangling-free and self-loop-free via `GraphEdge::new` error; deterministic by construction (BTreeMaps, sorted output) |
| Generic REQ-6: Consumers remain FactStore-ignorant | ✅ Implemented | `GenericProjection` contains only `GraphNode`/`GraphEdge`; kernel types stop at the port boundary |
| Umbrella R1: CallGraph projection equivalence | ✅ Implemented | `CallGraphProjection::from_facts` equals `from_call_graph` output on scored golden fixtures (Jaccard 1.0/1.0 fresh) |
| Umbrella R2: Derived projections are rebuildable | ✅ Implemented | Rebuild from re-read committed facts equals the prior projection exactly (multiset equality, fresh) |

### Coherence (Design)
| Decision | Followed? | Notes |
|----------|-----------|-------|
| D1 Fact adapters in `application/fact_bridge/` (tree_sitter + LSP, producers per authority table, 5 ReferenceKind variants, `ref=<Kind>` detail) | ✅ Yes | Both adapters present and exhaustive over `SymbolKind`/`ReferenceKind`; producers `DeterministicAnalyzer`/`RuntimeObserver` as declared |
| D2 Six-predicate idempotent `bootstrap_registry` | ✅ Yes (+ deviation d accepted) | Exactly six `core:*`; identical re-registration succeeds; `UsesGeneric`/`AnnotatedBy` stay unregistered (tested) |
| D3 Entity identity = raw FQN `Text`, `EntityIdTable` 1..N per snapshot, canonical sort, `kind=<SymbolKind>` in provenance detail | ✅ Yes (+ deviation e accepted) | No hashing; cross-entity objects stay `Text`; double-run byte-identity proven fresh at two layers |
| D4 `from_facts` — defines→nodes, calls-only edges, lowercase resolution with lexicographic tie-break, unresolved dropped+counted | ✅ Yes (+ deviation c accepted) | Order-independent (BTreeMap/BTreeSet internals); self-loops and orphans skipped; edge weight `(Calls, 1.0)` |
| D5 Dual-gated `GenericGraphProjectionPort` + `FactGenericGraphProjection`, dangling-free | ✅ Yes | Gate is `all(evidence-kernel, multimodal)` on port, adapter, and both mod wirings; no dangling NodeIds |
| D6 Additive `facts_in_snapshot` on `FactStore`, no default body, in-memory impl commit order | ✅ Yes | Port method has no default body; only the in-memory kernel store implements it; consumers sort |
| D7 Rust integration-test harness, Jaccard ≥ 0.99, quarantine reported, R2 rebuild, goldens stay e36's gate, `lsi-equivalence` recipe | ✅ Yes (+ deviation b accepted) | Comparator, quarantine constants, digest pin, and recipe all as designed |
| D8 Perf gate = e36 `compare` over 24 default-path benchmarks + advisory e37 bridge bench (no threshold in M2) | ✅ Yes | `benches/fact_bridge_benchmarks.rs` + `[[bench]]` entry exist; bridge baseline in `sandbox/results/lsi-bridge-baseline/`; gate outcome adjudicated above (WARNING 1) |
| Deviation (a): task 5.1 "nothing to wire in M2" (design file-table row `cognicode-runtime/src/lib.rs Modify`) | ⚠️ Deviation (benign, accepted) | Design INTENT honored: wiring must not change the default path. Gating is crate-level in `cognicode-core`; `cargo check -p cognicode-runtime` exit 0 (fresh); wiring stores+bootstrap now would be dead code (no consumer; cutover forbidden in M2). Determination recorded honestly in tasks.md 5.1. Not spec-breaking: REQ-4 scenarios are structural and are proven by the gate-off run + goldens |
| Deviation (b): harness normalizes the LEGACY side onto the extractor's 1-based line convention (`normalize_legacy_fqn`) | ⚠️ Deviation (benign, accepted) | Not in design D7's comparator description, but declared in harness docs as part of the comparison contract; mechanical, deterministic, and unavoidable — the same symbol is `f:name:N` in the legacy engine (0-based `start.row`) and `f:name:N+1` in the fact-side FQN convention. Spec language ("normalized ... multisets") covers it. Re-pin duty documented in TESTING-STATE |
| Deviation (c): ungated `unresolved_edges: usize` field + accessor on `CallGraphProjection` | ⚠️ Deviation (benign, accepted) | Design D4 mandates "dropped and counted" but its interface block did not declare the accessor; the count must be observable for the harness report (`fact_unresolved`). This is the ONLY compiled-in default-path delta of e37 (one usize + inlinable accessor) — verified by diff; relevance to the perf gate noted in WARNING 1 |
| Deviation (d): conflicting bootstrap spec fails with `SchemaError::AlreadyRegistered` | ⚠️ Deviation (benign, accepted) | Design D2 did not name the error variant; reusing the append-only registry's existing error avoids a new type and is asserted by `bootstrap_rejects_conflicting_specs` |
| Deviation (e): public `FactBatchBuilder::add_observation` + extended sort tie-breakers `(detail, producer_rank)` | ⚠️ Deviation (benign, accepted) | Design interface listed only `add_extraction`/`add_provider`/`finish`; `add_observation` is the adapters' insertion point and enforces LlmAgent rejection at the bridge boundary (defense-in-depth, e36-deviation-2 precedent); extended tie-breakers STRENGTHEN D3 byte-identity when two authorities observe the same triple |

None of the five deviations breaks a spec requirement or scenario.

### Issues Found

**CRITICAL**: None.

**WARNING**:
1. **M2 perf gate NOT certified clean (honest flag, attribution bounded).** Fresh
   `lsi_bench_baseline.py compare --fail-above 10` exited 1 in both verify runs (run 4:
   `add_node` +23.84% AND `search` +124.05%; run 5: `search` +124.05%). `search`
   reproduced at 176.3 ns (±0.44, criterion) vs the 79 ns baseline in both runs, so the
   "different sub-µs benchmark each run" noise pattern does NOT fully hold for it this
   window. Evidence bound: (i) the only compiled-in default-path e37 delta is one `usize`
   field + trivial accessor (diff-verified); (ii) `search` exercises
   `LightweightIndex::find_symbol`, untouched by e37; (iii) ms-scale benchmarks are all
   within ±4.02% in both runs; (iv) `add_node` flagged run 4 and recovered run 5 on the
   SAME binary, proving in-window measurement noise; (v) 22/24 and 23/24 benchmarks within
   threshold, and `hot_path_incoming_calls` is consistently ~12% FASTER than baseline
   (baseline-side noise). Residual uncertainty — CPU-frequency/governor window vs a
   one-field code-layout butterfly — cannot be resolved without editing code, which verify
   must not do. The M2 exit gate "no >10% regression on full build baseline" therefore
   remains UNADJUDICATED-CLEAN: archive/M3 should re-run the gate on a quiet machine
   (ideally as a controlled A/B reverting the single field) before claiming it. This is
   WARNING, not CRITICAL, because no spec scenario covers the perf gate, the flagged
   benchmark's code path is untouched by the change, and the gate had already failed 3×
   during apply on different benchmarks each time (documented, unattributed).
2. **`multi-lang-types` quarantine (pre-existing upstream defect, carried from e36).** The
   legacy graph engine's directory-level node set is walk-order unstable for same-named
   symbols across languages; the fixture is measured fresh every run (0.5969/0.0714, 94
   legacy vs 112 fact nodes, 40 unresolved) and reported, excluded from scoring by the
   declared quarantine. Not owned or fixable by e37; reported in every harness run.
3. **Pre-existing clippy red outside e37.** `cargo clippy -p cognicode-core --tests`
   (default features) shows 3 `cognicode-macros` warnings (aix_tool.rs:154 let_and_return;
   newtype.rs:59, newtype.rs:119 collapsible_if) + 2 `generic_graph.rs` test warnings
   (467:9, 479:8). `git diff HEAD --stat` on both is EMPTY — untouched by e36/e37. Zero
   findings on any e37 path in the fresh feature-on lib+tests run (required gate met).

**SUGGESTION**:
1. UAT-U10 formal MCP dual-path run remains deferred (A5, honestly recorded as "PASS
   pendiente de ejecución UAT formal"): the harness compares projections directly, not MCP
   tool responses; a formal run should execute call-graph MCP tools via both paths on a
   live golden repo.
2. UAT-U11 is PARCIAL by its own record: the live-server Explorer scenario (projected
   CallGraph behind a running API) was never exercised. It is the first item a formal UAT
   pass should pick up.
3. Rust `use` declarations emit no import facts (tree-sitter field mismatch, documented in
   the harness docs and TESTING-STATE). Harmless to the Calls-only edge multiset (legacy
   carries no import edges either; rust-hello edge multisets are empty on both sides and
   score 1.0), but any future extractor fix must consciously re-pin
   `PINNED_IDENTITY_DIGEST` / expectations.
4. The proposal's four success-criteria checkboxes are unchecked; archive should reconcile
   them against this report: criteria 1 (≥99% equivalence: 1.0/1.0 on both scored
   fixtures), 2 (R2 rebuild: exact multiset equality) and 4 (UAT deferral honestly stated)
   are met with fresh evidence; criterion 3's perf half is bounded by WARNING 1 while its
   MCP/CLI half (goldens byte-stable, 42/42) is met.

### Verdict
PASS WITH WARNINGS
All 12/12 scenarios across both owned spec surfaces are COMPLIANT with fresh runtime
evidence (118 tests + gate-off 0-test observation + goldens 42/42 + clippy delta zero on
e37 paths; every declared command exit 0 with recorded hashes); the three warnings are
documented, honestly-bounded gaps — the twice-reproduced sub-µs `search` flag on the M2
perf gate (demonstrably outside e37's compiled default-path surface but unresolvable
inside verify), the quarantined upstream engine defect, and the pre-existing clippy red —
none of which breaks a spec scenario.
