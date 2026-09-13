# Tasks: E36 LSI Evidence Kernel Foundation

## Review Workload Forecast

|Field|Value|
|---|---|
|Estimated changed lines|~1300–1600 (goldens excluded)|
|400-line budget risk|High|
|Chained PRs recommended|Yes|
|Suggested split|PR1 goldens → PR2 baseline → PR3 types → PR4 stores|
|Delivery strategy|auto-chain|
|Chain strategy|stacked-to-main (commits/PRs await approval; WUs land in-tree)|

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

|Unit|Goal|Likely PR|Focused test command|Runtime harness|Rollback boundary|
|---|---|---|---|---|---|
|1|M0 goldens|PR 1|`capture_lsi_fixtures.py --check`|double-run byte-compare|rm script + goldens dir|
|2|M0 baseline + wiring|PR 2|`lsi_bench_baseline.py compare`|`cargo bench --bench graph_benchmarks`|revert 2 edits, rm artifacts|
|3|M1 kernel types|PR 3|`cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`|N/A (domain)|rm module+feature|
|4|M1 ports/adapters|PR 4|same + `just lint`|N/A (in-memory)|rm adapters+ports|
|5|UAT/ADR + handoff|PR 4|N/A|N/A|revert docs|

## Phase 1: M0 goldens (WU-1 → umbrella 1.1–1.3)

- [x] 1.1 Author `sandbox/fixtures/lsi-baseline/inventory.json`: MCP/CLI/Explorer graph/symbol/impact surfaces + criticality weights.
- [x] 1.2 RED: `--check` pre-goldens → non-zero exit + report, zero writes (no overwrite).
- [x] 1.3 Create `sandbox/scripts/capture_lsi_fixtures.py`: sorted, scrubbed, timestamp-free, `--check`/`--accept`; fixed argv, no shell=True.
- [x] 1.4 Double-run byte-identical ("Regeneration is byte-identical").
- [x] 1.5 Tamper golden: `--check` fails without `--accept` ("Intended behavior change requires explicit re-baseline"); `--accept` re-baselines, restore.
- [x] 1.6 Coverage: per-surface covered/uncovered + fixture paths ("Inventory surfaces report coverage").

## Phase 2: M0 baseline + wiring (WU-2 → umbrella 1.4)

- [x] 2.1 RED: `compare` without baseline → non-zero exit + report (argv guard).
- [x] 2.2 Create `sandbox/scripts/lsi_bench_baseline.py`: fixed-argv `cargo bench --bench graph_benchmarks` (no shell=True).
- [x] 2.3 Write `sandbox/results/lsi-baseline/baseline.json` (schema_version, commit, env, benchmarks) ("Baseline artifact is captured").
- [x] 2.4 `compare` emits `{name, baseline_mean_us, current_mean_us, delta_pct}` ("Kernel change is compared to baseline").
- [x] 2.5 LSI gate row in `release_scorecard.py`; `lsi-fixtures`/`lsi-baseline` justfile recipes.

## Phase 3: M1 kernel types (WU-3 → umbrella 2.1–2.5, 2.7)

- [x] 3.1 RED: exhaustive FactValue/ProvenanceRecord round-trips, `snap:N`↔`rev:N`, LlmAgent rejection ("LLM output stays a hypothesis"); compile fails.
- [x] 3.2 `evidence-kernel = []` in core `Cargo.toml`; cfg-gate module in `domain/mod.rs`.
- [x] 3.3 `ids.rs`: EntityId, OccurrenceId, SnapshotId(u64), FactId, EvidenceId; `relation.rs`: namespaced `RelationKind`, `RelationSpec`.
- [x] 3.4 `fact.rs`: `FactValue`, `ProvenanceRecord` (wraps legacy `Provenance`, D3), `Fact`; `evidence.rs`: Evidence/EvidenceGrade; `snapshot.rs`: SnapshotDescriptor (D4).
- [x] 3.5 GREEN: focused command passes ("Fact round-trip preserves provenance").
- [x] 3.6 Guard: `cargo check -p cognicode-core` feature-off unchanged.

## Phase 4: M1 ports + adapters (WU-4 → umbrella 2.6–2.7)

- [x] 4.1 RED: pinning — publish B; pinned A = pre-B, zero B facts ("Historical read remains stable").
- [x] 4.2 `ports.rs`: FactStore, kernel-namespaced EvidenceStore (D2), SnapshotStore (D4), sync SchemaRegistry (D6).
- [x] 4.3 `infrastructure/evidence_kernel/{mod,in_memory}.rs` + cfg-gated `infrastructure/mod.rs`: in-memory adapters; commit rejects unregistered predicates/LlmAgent.
- [x] 4.4 GREEN: feature tests + `just lint` (I/O-free); round-trip preserves provenance (scenario 3.5).
- [x] 4.5 `capture_lsi_fixtures.py --check` green with kernel on ("Kernel addition does not change consumer outputs").

## Phase 5: Verification / records (umbrella 1.5)

- [x] 5.1 Author UAT-U05/U06 in `docs/CogniCode_Living_Software_Intelligence/docs/uat/UAT-MILESTONES.md` (A5); write change-folder `adr-review.md` (ADR-037..051 verdicts).
- [x] 5.2 Update `.agent/TESTING-STATE.md` (fresh/stale handoff).
- [x] 5.3 Sweep: Units 1–4, `cargo check -p cognicode-core` ± feature, `just lint`.
