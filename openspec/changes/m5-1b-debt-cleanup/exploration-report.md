# Exploration Report — m5-1b-debt-cleanup

> Cycle: `p-c1fac1fea05615c6/m5-1b-debt-cleanup`
> Phase: explore
> Date: 2026-09-14
> Path: **A-min** (scope inherited from upstream cycle)

## Problem statement

The m5-1b-statement-extraction verify report (PASS_WITH_WARNINGS, 2026-09-14)
flagged 2 follow-up items:

1. **ast-lift-loc-cap (low, warning)**: `crates/cognicode-core/src/application/program_analysis/ast_lift.rs`
   is **845 LOC** vs the ~545 LOC cap stated in the m5-1b `tasks.md`
   acceptance gate. The breach is driven by the WU3 conformance test bodies
   (5 tests + 2 helpers in `#[cfg(test)]` submodules, ~330 LOC). The cap
   sized production code only and M5.1b did not split tests into a separate
   file.

2. **pre-existing-feature-gate (medium, false_positive at m5-1b)**:
   `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs:997`
   imports `handle_interproc_summary` which is feature-gated behind
   `#[cfg(feature = "program-analysis-server")]` at
   `crates/cognicode-core/src/interface/mcp/handlers/program_analysis_handlers.rs:166`.
   Without the feature flag, the lib test binary fails to compile. This was
   pre-existing on `90edff1f` (origin/main before m5-1b) but the m5-1b
   scope note explicitly excluded it.

## Scope (this cycle)

A-min path: **2 small fixes, ~150 LOC**, no architectural change.

### Fix 1 — Split `ast_lift.rs` test bodies into `ast_lift_tests.rs`

- Move the `mod tests` block (~330 LOC: `lift_*` unit tests + `real_source`
  conformance bodies + 2 helpers `digest_hex`, `run_lifted`) out of
  `ast_lift.rs` into a sibling `ast_lift_tests.rs`.
- `ast_lift_tests.rs` is included from `ast_lift.rs` via
  `#[cfg(test)] mod tests;` (file-level `#[path]` attribute or
  `include!("ast_lift_tests.rs")`).
- After split: `ast_lift.rs` production code ≤ 545 LOC; tests in a separate
  file.

### Fix 2 — Feature-gate the `mcp_roundtrip_tests::tests` submodule

- Wrap the feature-gated imports in `mcp_roundtrip_tests.rs` (or the
  sub-module that imports `handle_interproc_summary`) with
  `#[cfg(feature = "program-analysis-server")]`.
- Verify both configurations:
  - `cargo test --lib` (no feature) → compiles cleanly.
  - `cargo test --lib --features program-analysis-server` → 8/8
    `m5_program_analysis_roundtrip` tests pass (regression check).

## Out of scope

- New domain types, new ports, new dependencies.
- Algorithm changes — only file moves + cfg gates.
- Pre-existing failures documented in m5-1b verify report that are NOT
  introduced by this cycle's changes.

## Risk analysis

| Risk | Severity | Mitigation |
|---|---|---|
| Test module visibility after split | low | `#[cfg(test)] mod tests;` at the bottom of `ast_lift.rs` keeps all `super::*` paths valid |
| Hidden `mod tests` re-exports break | low | `grep -n 'pub use' crates/cognicode-core/src/application/program_analysis/ast_lift.rs` first |
| `include!()` macro vs `#[path]` attribute | low | Use `#[path = "ast_lift_tests.rs"] #[cfg(test)] mod tests;` (canonical Rust pattern) |
| Feature gate breaks unrelated MCP tests | medium | Run `cargo test -p cognicode-core --lib --features program-analysis-server mcp_roundtrip_tests` AFTER the change; compare test counts |

## Acceptance boundary

After this cycle, the m5-1b verify report's 2 follow-ups are closed:

- `ast_lift.rs` ≤ 545 LOC, all ast_lift tests still pass (21/21 + 9/9 baseline).
- `mcp_roundtrip_tests.rs` compiles in BOTH `--features program-analysis-server`
  AND default-feature configurations.
- m5-1b's 8/8 `m5_program_analysis_roundtrip` regression check still passes.

## Decision

Proceed to **spec** with the 2-fix scope above. ~150 LOC, 1–2 stacked-to-main
commits. A-min path appropriate.
