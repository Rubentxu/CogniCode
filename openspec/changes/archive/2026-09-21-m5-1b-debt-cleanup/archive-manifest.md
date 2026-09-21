# M5.1b-debt-cleanup Archive Manifest

> Cycle: `p-c1fac1fea05615c6/m5-1b-debt-cleanup`
> Phase: archive (closes cycle)
> Date: 2026-09-14

## Cycle summary

| Property | Value |
|---|---|
| Cycle ID | `p-c1fac1fea05615c6/m5-1b-debt-cleanup` |
| Path | A-min |
| Phases completed | explore → specify → build (2 WUs) → verify → release → archive |
| Final status | RELEASED → **CLOSED** (after `archive.complete` transition) |
| Head SHA | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` |
| Tag | `m5-1b-debt-cleanup` (annotated) |
| Tag peel SHA | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` (= HEAD, verified remotely via `git ls-remote origin`) |
| Merge receipt SHA | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` |
| Release receipt SHA | `aaa821c4d8f148bfdb19c1c334dadc6cc51c9f0d` |
| Verify verdict | **PASS** (3 / 3 REQ-DC-* COMPLIANT, 9 / 9 commands exit 0, 0 critical, 0 warnings) |
| Diff stat | **+671 / -670** across **3 files** (2 commits) |
| Diff digest (sha256 of `git diff --stat 6bd72e78..HEAD`) | `776d35ce17d780289ad81cb3b1dfa11fb77d002fe552f12e14512a77943d8e4e` |
| **Inherited findings closed** | 2 of 3 (ast-lift-loc-cap, pre-existing-feature-gate) |
| **Inherited findings unchanged (out of scope)** | 1 of 3 (unused-import-conformance, pre-existing false_positive) |
| **New findings introduced** | 0 |

## Deliverables shipped

### Production code (1 modified file)

| File | LOC delta | Role |
|---|---:|---|
| `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs` | +3 / -2 | Cfg-gate `handle_interproc_summary` import; otherwise byte-identical to m5-1b |

### Test infrastructure (1 new file, 1 modified file)

| File | LOC delta | Role |
|---|---:|---|
| `crates/cognicode-core/src/application/program_analysis/ast_lift_tests.rs` | +664 | NEW — sibling of `ast_lift.rs`, holds all 21 ast_lift tests (5 conformance + 16 unit) |
| `crates/cognicode-core/src/application/program_analysis/ast_lift.rs` | +4 / -668 | Production code now 181 LOC (cap: 545); tests relocated to sibling file via `#[cfg(test)] #[path = "ast_lift_tests.rs"] mod tests;` |

### Commit chain (2 stacked-to-main commits)

| # | SHA | Subject |
|---:|---|---|
| 1 | `5cb4a644` | `refactor(ast_lift): split tests into ast_lift_tests.rs to meet REQ-LIFT-06 LOC cap` |
| 2 | `aaa821c4` | `fix(mcp): cfg-gate handle_interproc_summary import for non-feature builds` |

## Inherited findings disposition

| Finding ID | Rule | Severity | Disposition | Evidence |
|---|---|---|---|---|
| `1675d4ec…` | ast-lift-loc-cap | low | **CLOSED** by commit `5cb4a644` | `wc -l ast_lift.rs` = 181 (was 845) |
| `caf0a0f3…` | pre-existing-feature-gate | medium | **FIXED** by commit `aaa821c4` | `cargo test --lib -p cognicode-core --no-run` exits 0 (was E0432) |
| `3088ee9e…` | unused-import-conformance | low | UNCHANGED (intentionally out of scope) | pre-existing `INTERPROC_SUMMARY` import in `conformance.rs:30`; documented as `false_positive` in m5-1b verify-report (the symbol IS used with `--features program-analysis-server`) |

## Spec amendment note (no spec change)

The m5-1b spec's REQ-LIFT-06 cap (≤ 545 LOC for `ast_lift.rs`) is now
structurally enforced via the `ast_lift_tests.rs` split. The cap itself
remains 545 LOC; the structural enforcement is internal to the test
organization, not a spec change.

## Vault spec sync

`openspec/changes/m5-1b-debt-cleanup/` is itself the change artifact; no
new capability spec is promoted (this cycle's changes are mechanical
refactors of `ast_lift.rs` and a single-cfg import — no new public surface).

The upstream `openspec/specs/statement-extraction/spec.md` (created during
the m5-1b archive phase) remains the canonical spec for the statement
extraction capability; this debt-cleanup cycle does not amend it.

## Linked artifacts

- `openspec/changes/m5-1b-debt-cleanup/exploration-report.md`
- `openspec/changes/m5-1b-debt-cleanup/specs/debt-cleanup/spec.md`
- `openspec/changes/m5-1b-debt-cleanup/implementation-receipt.md`
- `openspec/changes/m5-1b-debt-cleanup/verify-report.md` (PASS)
- `openspec/changes/m5-1b-debt-cleanup/verify-findings.json`
- `openspec/changes/m5-1b-debt-cleanup/release-receipt.md`
- `openspec/changes/m5-1b-debt-cleanup/merge-receipt.md`
- `openspec/changes/m5-1b-debt-cleanup/archive-manifest.md` (this file)

## Cycle outcome

**CLOSED** at archive phase. All A-min path gates passed (exploration,
specification, implementation, tests, policy, debt severity, debt priority,
no-pending-effects, release-uat-approved waived, ledger-valid, vault-index-current).
