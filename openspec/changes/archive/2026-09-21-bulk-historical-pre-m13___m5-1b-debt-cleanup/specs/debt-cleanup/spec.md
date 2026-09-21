# Spec: m5-1b-debt-cleanup — Close 2 verify-report follow-ups

> Change: `m5-1b-debt-cleanup` | Cycle: `p-c1fac1fea05615c6/m5-1b-debt-cleanup`
> Phase: specify
> Path: **A-min**
> Goal: close the 2 follow-up items flagged by the upstream m5-1b verify
> report so the m5-1b cycle can be revisited as PASS (no warning).

## Scope

2 small fixes; ~150 LOC; no architectural change. Out-of-scope: any new
domain type, port, dependency, or algorithm change.

## Requirements

### REQ-DC-01 — `ast_lift.rs` production code ≤ 545 LOC

**Given** `crates/cognicode-core/src/application/program_analysis/ast_lift.rs`
is 845 LOC after m5-1b (with ~330 LOC in the `#[cfg(test)] mod tests`
block: 5 conformance bodies + 2 helpers + 11 module unit tests)

**When** this cycle is applied

**Then** the production code (everything outside the `#[cfg(test)] mod tests`
block) is **≤ 545 LOC** and the tests are split into a sibling
`ast_lift_tests.rs` included via `#[cfg(test)] #[path = "ast_lift_tests.rs"]
mod tests;`.

All existing `ast_lift` tests still pass:

- `cargo test -p cognicode-core --features program-analysis-server --lib
  program_analysis::ast_lift` → **21 / 21 passed**
- `cargo test -p cognicode-core --features program-analysis-server --lib
  ingest::extractor::tests` → **9 / 9 passed**

### REQ-DC-02 — `mcp_roundtrip_tests` compiles in both feature configurations

**Given** `crates/cognicode-core/src/interface/mcp/mcp_roundtrip_tests.rs`
imports `handle_interproc_summary` (line 997) which is
`#[cfg(feature = "program-analysis-server")]`-gated at
`interface/mcp/handlers/program_analysis_handlers.rs:166`

**When** this cycle is applied

**Then** the import (or the test submodule containing it) is cfg-gated so
the lib test binary compiles cleanly WITHOUT the feature flag:

- `cargo test --lib -p cognicode-core` → **exit 0** (no errors;
  feature-gated tests are simply not built)
- `cargo test --lib -p cognicode-core --features program-analysis-server`
  → **8 / 8 `m5_program_analysis_roundtrip` passed** (regression check)

### REQ-DC-03 — No new lint warnings or forbidden imports

**Given** the changed files

**When** this cycle is applied

**Then** `cargo clippy -p cognicode-core --features program-analysis-server
--lib --tests -- -D warnings` → **exit 0**, `cargo fmt --check` → exit 0,
and `grep -rn 'tokio\|sqlx\|reqwest' crates/cognicode-core/src/application/
program_analysis/ast_lift.rs crates/cognicode-core/src/application/
program_analysis/ast_lift_tests.rs crates/cognicode-core/src/interface/
mcp/mcp_roundtrip_tests.rs` → **0 matches**.

## Acceptance boundary

After the cycle:
- m5-1b follow-up #1 (ast-lift-loc-cap) is closed.
- m5-1b follow-up #2 (pre-existing-feature-gate) is closed.
- All pre-existing tests still pass.
- No clippy warnings, no fmt drift, no forbidden imports introduced.

## Scenarios

| ID | Given | When | Then |
|---|---|---|---|
| SCN-DC-01 | ast_lift.rs is 845 LOC | split into ast_lift.rs (prod) + ast_lift_tests.rs | production LOC ≤ 545, total test LOC preserved (~330) |
| SCN-DC-02 | tests live in separate file | `cargo test ... --lib program_analysis::ast_lift` | 21/21 pass |
| SCN-DC-03 | mcp_roundtrip_tests imports feature-gated symbol | feature flag OFF | `cargo test --lib -p cognicode-core` exits 0 |
| SCN-DC-04 | same | feature flag ON | 8/8 `m5_program_analysis_roundtrip` pass |
| SCN-DC-05 | split + gate applied | clippy + fmt + grep | clean / 0 warnings / 0 forbidden imports |

## Out of scope

- m5-1b verify report's other findings (2 false_positives already documented).
- Any algorithm change, new port, new domain type, new dependency.
- Splits of OTHER modules beyond `ast_lift.rs` (e.g. `extractor.rs` is
  1034 LOC but no cap declared in its cycle scope; tracked separately).
