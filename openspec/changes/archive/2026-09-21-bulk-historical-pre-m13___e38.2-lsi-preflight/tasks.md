# Tasks: e38.2 LSI Preflight

## Review Workload Forecast

Estimated changed lines: ~130–190 authored. Delivery strategy: auto-chain — single PR, 4 independent units, one slice.

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: stacked-to-main
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Focused test command | Runtime harness | Rollback boundary |
|---|---|---|---|---|
| 1 | CP-4 commit guard | `cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel` | `just lsi-identity` | Revert guard + variant |
| 2 | PERF A/B | `lsi_bench_baseline.py compare --fail-above 10` | compare ×2 + A/B | stash, no commit |
| 3 | ENGINE-DET | `just lsi-fixtures check` + `just lsi-equivalence` | equivalence double-run | Revert service hunk |
| 4 | CLIPPY | `just lint` | N/A (mechanical) | Revert 3 file hunks |

## Phase 1: CP-4 Guard (RED-first)

- [x] 1.1 RED: `in_memory.rs` test — second commit reusing ids in the same `(ws, snap)` returns `KernelError`; prove FAILS today.
- [x] 1.2 Add `KernelError` variant (`ports.rs`) carrying `(FactId, SnapshotId)` — `SnapshotMismatch`-style caller violation; `Store` stays I/O-reserved.
- [x] 1.3 GREEN: guard in `InMemoryFactStore::commit` — reject if any existing `(ws, snap)` fact id ≤ batch max (spaces start at 1); atomic, pre-extend.
- [x] 1.4 Update `FactStore::commit` port doc; kernel suite green.

## Phase 2: PERF Re-Adjudication (A/B)

- [x] 2.1 Run `python3 sandbox/scripts/lsi_bench_baseline.py compare --fail-above 10` ×2; record deltas. None >10% both runs → close WARNING as machine noise; STOP.
- [x] 2.2 If any reproduces >10%: stash-revert `unresolved_edges` field+accessor in `call_graph_projection.rs` (DO NOT commit), rebuild bench, re-run. *(condition false — no benchmark >10% in both runs; `hot_path_outgoing_calls` +12.38% run 1 → −7.13% run 2)*
- [x] 2.3 Restore; re-run. Persists → NOISE-ATTRIBUTED (same code path); flips → field behind `evidence-kernel` gate + re-run. *(condition false — nothing reproduced; NOISE-ATTRIBUTED verdict from 2.1 rule)*
- [x] 2.4 Record evidence + verdict; `baseline.json` UNTOUCHED if noise. *(NOISE-ATTRIBUTED; `search` +0.00%/+2.53%; deltas in delta_u382_run{1,2}.json)*

## Phase 3: ENGINE-DET Determinism (Minimal)

- [x] 3.1 `build_project_graph` (`analysis_service.rs:349`): sort per-file results by `file_path` before name-index insertion; deterministic duplicate-name handling (no first-file-wins).
- [x] 3.2 Alignment: shared `resolve_callee_identity` rule (exact → lowercase → smallest-FQN); `just lsi-fixtures check` — goldens MUST stay byte-identical. *(PASS 42/42; exact-identity stage documented inapplicable — legacy callees are bare names)*
- [x] 3.3 If goldens break: determinism-only rule (sorted walk + lexicographic tie-break), document; `--accept` ONLY with double-run proof + justification. *(condition false — goldens passed on first attempt; no re-baseline)*
- [x] 3.4 Re-run `just lsi-equivalence`; REPORT multi-lang-types; un-quarantine ONLY if ≥0.99 (expected: stays); double-run proves determinism. *(multi-lang-types 0.5969/0.0714 both runs — identical, stays QUARANTINED; scored 1.0000 ×6 both runs)*
- [x] 3.5 Fix exceeds minimal (deep entanglement) → STOP item, record blocker, ship with 1/2/4. *(condition false — fix stayed minimal: 2 hunks in one function, goldens passed first attempt; no STOP)*

## Phase 4: CLIPPY → GREEN

- [x] 4.1 `aix_tool.rs:154` let_and_return → direct return. *(actual lint at :154 was collapsible_if — fixed via let-chain merge; no let_and_return existed in aix_tool.rs)*
- [x] 4.2 `newtype.rs:59` + `:119` collapsible_if → merge nested ifs. *(:59 collapsible_if fixed via let-chain; :119 was actually let_and_return — `let expanded = ...; expanded` → direct if-expression)*
- [x] 4.3 `generic_graph.rs:467` drop unused `chrono::TimeZone` import; `:479` drop dead `doc_id`. *(both used only by multimodal-gated tests — gated with `#[cfg(feature = "multimodal")]` instead of dropped, so BOTH feature sets stay warning-free)*
- [x] 4.4 `just lint` exit 0 (criterion); fmt clean; clippy delta zero.

## Phase 5: Verification + Handoff

- [x] 5.1 Lib suites green: `--lib evidence_kernel`, `--lib call_graph_projection` (`--features evidence-kernel`). *(evidence_kernel 93/0 incl. 2 new guard tests; call_graph_projection 46/0; also fact_bridge 15/0, continuity 36/0, analysis_service 25/0, generic_graph_projection 6/0 (e-k,mm))*
- [x] 5.2 Harnesses: `just lsi-fixtures check` (42/42 or justified `--accept`), `just lsi-equivalence` (scored 1.0/1.0), `just lsi-identity` (gates 1.0000). *(42/42 byte-identical; python-hello/rust-hello 1.0000 ×3 runs, multi-lang-types 0.5969/0.0714 QUARANTINED deterministic; identity gates all 1.0000, isolation 2/0 collisions=0)*
- [x] 5.3 `cargo check -p cognicode-core` ±features and `-p cognicode-runtime` exit 0. *(all 4 exit 0; bench fact_bridge_benchmarks check exit 0 in both feature states; workspace `cargo fmt --check` exit 0)*
- [x] 5.4 Update `.agent/TESTING-STATE.md` Active Change + RETIREMENT-LEDGER CP-4 row.
