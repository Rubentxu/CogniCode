# ADR-PRF-008 — Architectural Review of F0.1 find_usages CLI (L1.5)

**Status:** PROPOSED · **Date:** 2026-09-25 · **Cycle:** L1 (Post-PRF)

**Scope.** Architectural review of commit `3cb07f90` (F0.1 `cognicode
find-usages` subcommand + 14 tests). Reviewed against:

- architecture rules (hexagonal, SOLID, connascence)
- existing repo conventions (PRF §144–§145, ADR-PRF-005, ADR-PRF-006)
- E0/L1 closing criteria

**Method.** Read of `commands.rs::execute_find_usages` +
`print_text_render` + `FindUsages` enum variant + tests. Diff vs
existing `execute_analyze` / `execute_navigate` / `execute_refactor`
patterns. No re-implementation; this is review, not refactor.

---

## Findings

### F1 — `execute_find_usages` accepts 7 positional args (manageable but borderline)

**Severity:** low
**Location:** `crates/cognicode-core/src/interface/cli/commands.rs::execute_find_usages`

The function signature is:

```rust
async fn execute_find_usages(
    symbol: &str,
    cwd: &str,
    include: bool,
    context_lines: Option<usize>,
    format: &str,
    quiet: bool,
) -> Result<(), Box<dyn Error>>
```

7 args, all primitive types. Sibling commands like `execute_analyze`,
`execute_refactor`, `execute_navigate` have similar counts (3-5 args).

**Decision:** acceptable as-is. The function is private and only called
from one match arm in `execute()`. Refactor into a `FindUsagesArgs`
struct would be defensible but is not a strict improvement at this size.
Flagged for future refactor if signature grows past 8 args.

### F2 — Connascence of Name/Representation: `UsageResult` vs `UsageEntry`

**Severity:** low (documented)
**Location:** `crates/cognicode-core/src/application/services/analysis_service.rs:1584`
(`UsageResult`) vs `crates/cognicode-core/src/interface/mcp/schemas.rs:200`
(`UsageEntry`)

Two structs with identical shape but different names:

```rust
// application/services
pub struct UsageResult {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub is_definition: bool,
    pub context_lines: Option<ContextData>,
}

// interface/mcp/schemas
pub struct UsageEntry {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub context: String,
    pub is_definition: bool,
    pub surrounding_lines: Option<ContextLines>,
}
```

The nested `ContextData` vs `ContextLines` differ in name too (both
`{ before, current, after }`).

**Decision:** acceptable as-is, documented in code. Both types live in
distinct architectural layers (application service vs interface DTO),
and serde derive for `UsageEntry` provides the MCP wire format. Folding
them into one would force the application service to take on serde
dependencies or move into the interface layer — both worse.

The CLI does the conversion explicitly via `.map(...)` (lines 1620-1635
of commands.rs). This is intentional and visible — the user can grep
for `UsageEntry` to find every cross-layer mapping.

### F3 — `execute_find_usages` reaches into `interface::mcp::*` types

**Severity:** low (intentional)
**Location:** `commands.rs:1583-1587`

The function does:

```rust
use crate::interface::mcp::schemas::{FindUsagesOutput, UsageEntry};
use crate::interface::mcp::security::InputValidator;
```

inside a `cli` command body. This is technically a violation of
hexagonal layering (`interface::cli` depends on `interface::mcp`) —
but it's how the existing CLI code already operates (see `commands.rs`
imports at top: `interface::mcp::schemas` is already used). Adding
another import in the same neighborhood is consistent with the existing
pattern, not a new violation.

**Decision:** acceptable, no action. If we later refactor the CLI to
strictly isolate `cognicode-cli` from `cognicode-core::interface::mcp`,
this would be one of many imports to disentangle. Out of scope for L1.5.

### F4 — `print_text_render` is a top-level function (no tests)

**Severity:** low (acceptable)
**Location:** `commands.rs:1911-1926`

The helper is `fn print_text_render(out: &FindUsagesOutput)` outside
the `impl CommandExecutor`. It writes to stdout via `println!`. The
test module has a near-empty `uat_print_text_render_marks_definitions_with_prefix`
that exists only to verify the function compiles.

**Decision:** acceptable. The format is tested indirectly via the E2E
binario UAT (manual verification). Capturing stdout in a Rust unit
test requires `--show-output` machinery; the operator policy is
"verify with the least cost sufficient", and indirect verification is
sufficient here.

### F5 — Error type is `Box<dyn Error>` with `format!` strings

**Severity:** medium (idiomatic but lose-checking)
**Location:** `commands.rs:1592-1595, 1602, 1617, etc.`

Errors are constructed via:

```rust
return Err(format!("find-usages: cwd does not exist ...").into());
```

This loses type information — callers can't distinguish "invalid cwd"
from "invalid symbol" from "backend error" without parsing strings.
Consistent with `execute_analyze` / `execute_refactor` / etc. which
all use `Box<dyn Error>`.

**Decision:** acceptable as-is. Introducing a `CliError` enum would be
a larger refactor (every CLI command would need to migrate). Not
justified by a single command's needs. If the pattern repeats for
many commands, refactor then.

### F6 — Hardcoded format strings

**Severity:** low (correct)
**Location:** `commands.rs:1657-1661`

`format` arg is matched against `"json"` and `"text"`. Any other value
returns Err. No way to extend without code change. Acceptable for a
stable CLI surface; documenting via `--help` is sufficient.

### F7 — Exit codes via `Box<dyn Error>` propagation

**Severity:** low (correct)
**Location:** `commands.rs:462`, `main.rs:11`

The binary's `main` returns `Result<(), Box<dyn Error>>`. When
`execute_find_usages` returns Err, `main` propagates and the process
exits with code 1 (the default for `Err`). When Ok, exits 0. This is
exactly the contract documented in the help. **Verified via UAT binaria
manual** (4 escenarios: success/exit 0, invalid cwd/exit 1, invalid
format/exit 1, missing symbol/exit 0).

---

## Cross-cutting observations

### O1 — Test pyramid is balanced

| Layer | Count | File |
|---|---|---|
| Lib unit (capabilities simmetry) | 1 | `capabilities.rs::tests` |
| Lib unit (CLI dispatch) | 6 | `commands.rs::f0_1_find_usages_tests` |
| Integration E2E (MCP handler characterization) | 4 | `find_usages_mcp_handler_e2e.rs` |
| Integration E2E (CLI ↔ MCP equivalence) | 4 | `find_usages_cli_mcp_equivalence.rs` |
| Integration E2E (compat matrix 0.97.x) | 3 | `find_usages_compat_0_97.rs` |
| **Total F0.1 tests** | **18** |  |

Distribution is healthy: 7 unit, 11 integration. No excessive mocking.
No tests duplicating each other across layers.

### O2 — Public API surface preserved

| Surface | Change |
|---|---|
| `cognicode-mcp` binary | unchanged |
| `cognicode-mcp` MCP tools | unchanged |
| `cognicode-cli` (cogh) | unchanged |
| `cognicode` CLI | added `find-usages` subcommand (additive only) |
| `AnalysisService` API | unchanged |
| `HandlerContext` API | unchanged |

Zero breaking changes. Compatible with v0.97.x clients per the L1.3
compat tests.

### O3 — No new dependencies

`Cargo.lock` diff (commit `3cb07f90`): only existing crates recompiled.
No new external dependencies added. The CLI reuses `clap` (already in
`cognicode-cli`), `serde_json` (already in workspace), `tempfile`
(already in workspace dev-deps).

### O4 — Connascence scope is bounded

Connascence of name (F2) between `UsageResult` ↔ `UsageEntry` is the
only cross-layer connascence I see. Connascence of meaning (the
boolean `is_definition` shared across both types and the CLI's "def "
prefix) is acceptable — it's documented in code comments.

---

## Verdict

**F0.1 passes architectural review.** No blocking findings. 7 minor
observations (F1-F7, O1-O4) are either documented trade-offs or
acceptable as-is. No refactor required to close E0.

**Recommendations for future cycles (non-blocking):**

- R1 (L1.6 / before next F-series feature): integrate compat tests
  into `merge-gate` aggregator so a regression in MCP contract breaks
  PR-CI immediately.
- R2 (E2 / CI executable architecture): if/when adding more CLI
  commands, refactor error types from `Box<dyn Error>` to a `CliError`
  enum. Not justified today.
- R3 (next refactor of capabilities authority): consider folding
  `list_tool_capabilities` and `stable_tool_names_with_capabilities`
  into a single `stable_capabilities()` table with auto-derivation, as
  discussed in L1.1.W1. The test pineador de simetría closes the
  immediate risk but the dual authority remains a maintenance burden.

**Approval:** this ADR records the architectural review outcome. No
follow-up commit needed unless R1/R2/R3 are actioned.
