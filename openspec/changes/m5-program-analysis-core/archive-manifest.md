# Archive Manifest — M5 — Program Analysis Core

> Cycle: `p-c1fac1fea05615c6/m5-program-analysis-core`
> Path: **A-lite**
> Phase: **archive** (closes cycle)
> Date: 2026-09-14 (cycle closed in ledger); housekeeping artifact date 2026-09-15

## Cycle summary

| Property | Value |
|---|---|
| Cycle ID | `p-c1fac1fea05615c6/m5-program-analysis-core` |
| Path | A-lite |
| Phases completed | explore → specify → build (7 WUs) → verify → release → archive |
| Final status | CLOSED in ledger (sequence 9 transition, 2026-09-14) |
| Final HEAD | `2f0d48f5` (acceptance evidence commit) |
| Tag | none annotated (release via direct push) |
| Verify verdict | PASS_WITH_WARNINGS (22/22 program_analysis tests, 161/161 graph-algos tests) |

## Deliverables shipped (cycle retrospective)

### Production code (5 new algorithm modules + 5 descriptors + 1 facade + 1 harness)
- 5 algorithm modules in `crates/cognicode-graph-algos/src/algorithms/`: cfg, dominators, slicer, interproc, taint
- 5 `AlgorithmDescriptor` files in `crates/cognicode-core/src/domain/analytics/`
- `ProgramAnalysisService::dispatch(id, params, limits)` facade in `crates/cognicode-core/src/application/program_analysis.rs` (+101 LOC)
- Conformance harness in `crates/cognicode-core/src/application/program_analysis/conformance.rs` (+14 LOC)

### Specs (NEW)
- `openspec/changes/m5-program-analysis-core/specs/program-analysis-core/spec.md`
- `openspec/changes/m5-program-analysis-core/specs/program-analysis-conformance/spec.md`

### Tests (delta)
- 16 → 22 program_analysis lib tests (+6 from M5: 4 acceptance_evidence + 1 interproc canonical fixture + 1 from initial harness)
- New `acceptance_evidence` module proving public-dispatcher contract for all 6 algorithm IDs

## Acceptance contract — final state

| Capability | Status |
|---|---|
| All 6 M5 algorithm IDs reachable via `ProgramAnalysisService::dispatch` | COMPLIANT (`public_dispatcher_accepts_every_m5_algorithm_id`) |
| Replay determinism at the dispatcher boundary | COMPLIANT (`replay_guard_is_byte_identical_for_entire_corpus`) |
| Perf envelope emits median/p95/max with `over_budget_count == 0` | COMPLIANT (`perf_envelope_publishes_median_p95_max`) |
| MCP-facing JSON shape serialize-stable | COMPLIANT (`mcp_fixture_report_serde_round_trips`) |
| Domain purity: no I/O in domain/analytics | COMPLIANT (`grep -rn 'tokio\|sqlx\|reqwest' crates/cognicode-core/src/domain/analytics/program_analysis/` → 0) |

**Result: all 5 acceptance capabilities COMPLIANT.**

## Pre-existing failures documented (out of M5 scope)

- `fact_bridge_benchmarks.rs` — pre-existing `unused_imports` warning on bench target (NOT introduced by M5).
- 39 unrelated MCP test failures in `file_ops_handlers`, `refactor_handlers`, `mcp_roundtrip_tests::{complexity, edge_cases, file_operations_roundtrip}`, `security` — pre-existing path canonicalization issues between `/home` and `/var/home`, NOT introduced by M5.

## Warnings (non-blocking)

- W-1: `fact_bridge_benchmarks.rs` unused_imports — pre-existing, NOT introduced by M5.
- W-2: canonical corpus is synthetic flat slices, not parsed tree-sitter ASTs — by design; M5 scope is the algorithm layer. Tree-sitter → `FunctionLocalView` lifting is M5.1 (deferred to a follow-up cycle).

## Linked receipts (retrospective)

- `implementation-receipt.md` — production code shipped, test evidence
- `verification-report.md` — PASS_WITH_WARNINGS verdict
- `merge-receipt.md` — 8 commits pushed as fast-forward
- `release-receipt.md` — release evidence (no annotated tag)
- `archive-manifest.md` (this file)

## Spec delta sync

The 2 specs in this change (`program-analysis-core`, `program-analysis-conformance`) were never archived to `openspec/specs/` (the canonical tree). They remain in `openspec/changes/m5-program-analysis-core/specs/` and are linked from production code by absolute path:

- `crates/cognicode-core/src/application/program_analysis/conformance.rs:3-4` references `openspec/changes/m5-program-analysis-core/specs/program-analysis-conformance/spec.md`
- `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs:3` references design D1 of `m5-mcp-wiring`

The harness `openspec_conformance.py` does NOT include these specs in its matrix because they live in `openspec/changes/`, not `openspec/specs/`. Any future re-organization of the `openspec/changes/` tree must update these code references or archive the specs to `openspec/specs/`.

## Cycle outcome

**CLOSED** at archive phase. All A-lite path gates passed (exploration, specification, implementation, tests, policy, debt severity, debt priority, no-pending-effects, release-uat-approved waived, ledger-valid, vault-index-current).

This archive-manifest is a retrospective document created at housekeeping time (2026-09-15). The cycle was already CLOSED in the ledger at sequence 9 (2026-09-14); the gap was that the on-disk artifact was missing.