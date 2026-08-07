# Debt Report: moldql-intent-syntax-v1

**Date**: 2026-07-02
**Mode**: Smoke (2 clusters: coupling + over-eng)
**Path**: A-min
**Auditor**: sddk-debt-verify
**Subject**: `feat/moldql-intent-syntax-v1` @ `18ead66` + uncommitted clippy cleanup + new integration test file
**Verify verdict (input)**: PASS_WITH_WARNINGS
**Debt verdict**: **PASS_WITH_WARNINGS**
**Re-iterate from**: `none`

---

## Executive Summary

The `lower_intent` lowering layer + facade wiring is structurally clean: zero hidden dependencies, zero mutable global state, zero SOLID violations, zero cycles. Both CRITICAL issues from verify-pass-1 (clippy + integration test file) are resolved.

The audit surfaced **0 CRITICAL**, **2 WARNING**, **4 SUGGESTION** findings — none of them blockers for archive. The two real issues are (a) a 10-line `match` block duplicated across `execute_query` and `execute_query_with_target`, and (b) `tests/intent_integration.rs` adds 166 lines / 9 tests where 8 are 1:1 duplicates of the unit tests in `intent.rs` under a false "integration" label (verify-report flagged this as a known caveat; the smoke auditor is sharper and calls it outright test duplication).

---

## Tech Debt Summary

| Cluster | Verdict | Critical | Warning | Suggestion | Notes |
|---------|---------|----------|---------|------------|-------|
| Architecture | SKIPPED (smoke) | — | — | — | depth=smoke |
| Smells | SKIPPED (smoke) | — | — | — | depth=smoke |
| Duplication | SKIPPED (smoke) | — | — | — | depth=smoke |
| **Coupling** | **PASS** | 0 | 0 | 3 | hidden deps clean; mutable-static audit clean |
| **Over-eng** | **PASS_WITH_WARNINGS** | 0 | 2 | 2 | accidental-bloat=0.55, threshold not breached |
| **TOTAL** | **PASS_WITH_WARNINGS** | **0** | **2** | **4** | |

---

## Findings — Coupling Cluster (`debt-coupling-cluster`)

**Verdict**: PASS

### COUP-001 → escalated to **WARNING** (corroborated by OVENG-002)

**Title**: Duplicated lowering+parse `match` block across two facade methods

**Location**: `crates/cognicode-explorer/src/facades/moldql.rs:57-66` and `:77-86`

**Evidence**:
```rust
// Identical 10-line block in both execute_query and execute_query_with_target:
let ast = match crate::moldql::lower_intent(query) {
    Some(Ok(ast)) => ast,
    Some(Err(e)) => {
        return Err(ExplorerError::ResolutionFailed(format!(
            "intent query `{query}` invalid: {e}"
        )));
    }
    None => crate::moldql::parser::parse(query)
        .map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))?,
};
```

**Impact**: Drift risk — any future change to lowering policy (telemetry, log, error format, fallback order) must touch both sites in lockstep. The methods diverge structurally after the match (one calls `.execute(ast)`, the other `.execute_with_target(ast, target)`), so the duplicated block is the only chance to silently desync.

**Recommendation**: Extract a private helper on `MoldQLServiceImpl`:
```rust
fn resolve_ast(&self, query: &str) -> ExplorerResult<MoldQLQuery> {
    match crate::moldql::lower_intent(query) {
        Some(Ok(ast)) => Ok(ast),
        Some(Err(e)) => Err(ExplorerError::ResolutionFailed(
            format!("intent query `{query}` invalid: {e}")
        )),
        None => crate::moldql::parser::parse(query)
            .map_err(|e| ExplorerError::ResolutionFailed(e.to_string())),
    }
}
```
Both methods then become `let ast = self.resolve_ast(query)?;` plus their distinct execute call.

### COUP-002 — SUGGESTION

**Title**: 3-state encoding uses `Option<Result<T, E>>` which obscures intent

**Location**: `crates/cognicode-explorer/src/moldql/intent.rs:47`

**Evidence**: `pub fn lower_intent(query: &str) -> Option<Result<MoldQLQuery, ParseError>>`

**Impact**: Three distinct outcomes (lowered-ok / lowered-bad / passthrough) are encoded via Option/Result nesting. The compiler can't enforce that callers handle all three states distinctly — `Some(Err(e))` is easy to silently flatten to `None`.

**Recommendation**: Consider a named enum for v2:
```rust
pub enum LoweringOutcome {
    Lowered(MoldQLQuery),
    InvalidIntent(ParseError),
    Passthrough,
}
```
Out of scope for v1; spec/tasks document the current shape as intentional.

### COUP-003 — SUGGESTION

**Title**: Error message leaks parser internals about the rewritten form

**Location**: `crates/cognicode-explorer/src/facades/moldql.rs:60-62` and `:79-81`

**Evidence**:
```rust
return Err(ExplorerError::ResolutionFailed(format!(
    "intent query `{query}` invalid: {e}"
)));
```
`{query}` is the original intent (`symbols where ...`), but `{e}` references the parser's view of the *rewritten* form (`FIND symbols WHERE ...`).

**Impact**: User sees "intent query \`symbols where foo > 5\` invalid: unexpected token at position 18" — the parser error references a string the user never typed. Confusion compounds as more intent patterns are added.

**Recommendation**: Either (a) have `lower_intent` carry the original query in its error variant, or (b) suppress the inner parser error and report only `"intent query \`{query}\` is malformed"`. SUGGESTION — not blocking.

### Coupling Cluster Audit Detail

| Audit dimension | Result |
|---|---|
| `lower_intent` deps | `std::sync::OnceLock`, `regex::Regex`, `crate::moldql::parser`, `MoldQLQuery`, `ParseError` — explicit, no surprises |
| Facade wiring deps | explicit; no panic-on-unwrap; no thread-locals; no Arc cycles |
| Mutable static | none |
| Static `OnceLock` race risk | none — `Regex::captures` is `&self`, init is idempotent |
| Hidden deps | none |
| Brittle contracts | 1 (`Option<Result<T,E>>` — COUP-002) |
| Error format coupling | yes (COUP-003) |
| Re-parse dependency | yes (acknowledged in verify-report SUGGESTION #4) |

---

## Findings — Over-Engineering Cluster (`debt-overeng-cluster`)

**Verdict**: PASS_WITH_WARNINGS
**Accidental-bloat score**: 0.55 (threshold not breached)
**Ponytail debt ledger**: empty — zero `ponytail:`/`TODO`/`FIXME`/`HACK`/`WORKAROUND` markers in scope
**YAGNI violations**: none
**Reinventing the wheel**: none
**Speculative generality**: none

### OVENG-002 — **WARNING** (corroborates COUP-001)

See COUP-001 above. Same finding, same recommendation.

### OVENG-003 — **WARNING**

**Title**: `intent_integration.rs` duplicates `intent.rs` unit tests 1:1 — false integration

**Location**: `crates/cognicode-explorer/tests/intent_integration.rs` (166 lines, 9 tests)

**Evidence**: Module doc claims *"verify that ... intent queries are correctly lowered through the facade's execute path"*. But every test imports and calls `lower_intent` directly; none invoke the facade.

| Integration test | Duplicates unit test |
|---|---|
| `lower_intent_symbols_where_parses` | `test_symbols_where_lowercase_happy_path` |
| `lower_intent_calls_from_with_depth_parses` | `test_calls_from_with_explicit_depth` |
| `non_intent_queries_return_none` | `test_uppercase_find_falls_through` |
| `empty_query_returns_none` | `test_empty_string_returns_none` |
| `uppercase_intent_returns_none` | `test_uppercase_symbols_where_returns_none` |
| `unrecognized_lowercase_returns_none` | `test_unrecognized_lowercase_returns_none` |
| `roundtrip_symbols_where` | `test_roundtrip_symbols_where` |
| `roundtrip_calls_from_depth` | `test_roundtrip_calls_from_depth` |

Only `lower_intent_preserves_query_in_error` is not a strict duplicate (but is itself misleading — see OVENG-004).

**Impact**: 166 lines / 9 tests add zero coverage signal beyond the 15 unit tests. Future semantics changes require updating 21 tests instead of 12. The `#[tokio::test]` attribute is also pointless for synchronous `lower_intent` calls. The verify-report documented this as "WARNING #1 — integration tests do NOT drive the facade end-to-end"; the smoke auditor sharpens this: it's not just incomplete, it's outright duplication under a false label.

**Recommendation**: Either
- (a) rewrite the file to actually exercise the facade (mock repo, drive `execute_query`, assert on the lowered AST reaching the executor), OR
- (b) delete it — unit tests in `intent.rs` already cover the same surface with stricter assertions on AST fields (`object_ref`, `depth`, `conditions.len`).

Verify-report SUGGESTION #3 also points at option (a). The orchestrator may attach this as follow-up work to the PR.

### OVENG-001 — SUGGESTION

**Title**: `OnceLock` + getter-helper is more verbose than `LazyLock` for static regex cache

**Location**: `crates/cognicode-explorer/src/moldql/intent.rs:24-40`

**Evidence**:
```rust
static RE_SYMBOLS_WHERE: OnceLock<Regex> = OnceLock::new();
fn re_symbols_where() -> &'static Regex {
    RE_SYMBOLS_WHERE.get_or_init(|| {
        Regex::new(r"^symbols\s+where\s+(.+)$").expect("regex is valid")
    })
}
// (same for RE_CALLS_FROM)
```
17 lines of plumbing for 2 immutable, never-reset regexes.

**Recommendation**: `LazyLock<Regex>` (stable since Rust 1.80, Feb 2024) collapses each pair to a single line:
```rust
static RE_SYMBOLS_WHERE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^symbols\s+where\s+(.+)$").expect("regex is valid")
});
```
Helper functions disappear. Pure ergonomics — no correctness difference.

**Note**: verify-report explicitly states *"case-sensitive lowercase prefixes only"* and *"Module-level regex statics (avoid recompilation per call)"*. `OnceLock` and `LazyLock` both satisfy that. Project's MSRV may need verification before this swap.

### OVENG-004 — SUGGESTION

**Title**: Test name promises error preservation; test asserts `None`

**Location**: `crates/cognicode-explorer/tests/intent_integration.rs:49-65`

**Evidence**:
```rust
#[tokio::test]
async fn lower_intent_preserves_query_in_error() {
    // doc: "error messages reference the ORIGINAL intent query"
    let query = "symbols where";  // missing condition
    let result = lower_intent(query);
    assert!(result.is_none(), "malformed intent should return None");
}
```

**Impact**: Misleading test name will confuse future maintainers. They will look for the "preserves query in error" assertion and find a `None` assertion. Either the doc claim is aspirational (unverified) or the test is wrong.

**Recommendation**: Rename to `lower_intent_malformed_returns_none` to match the actual assertion. If error-preservation is a real requirement, write a separate test that drives `MoldQLService::execute_query` with a malformed input and asserts on the `ResolutionFailed` message contains the original query string.

---

## Findings Summary

### By severity

| Severity | Count | Findings |
|---|---|---|
| **CRITICAL** | **0** | — |
| **WARNING** | **2** | F1 (duplicated facade `match`), F2 (integration test duplication) |
| **SUGGESTION** | **4** | F3 (`Option<Result<T,E>>` shape), F4 (error leaks parser internals), F5 (`OnceLock` vs `LazyLock`), F6 (misleading test name) |

### By SOLID principle

| Principle | CRITICAL | WARNING | SUGGESTION |
|---|---|---|---|
| SRP | 0 | 0 | 0 |
| OCP | 0 | 0 | 0 |
| LSP | 0 | 0 | 0 |
| ISP | 0 | 0 | 0 |
| DIP | 0 | 0 | 0 |

### By file

| File | CRITICAL | WARNING | SUGGESTION |
|---|---|---|---|
| `crates/cognicode-explorer/src/moldql/intent.rs` | 0 | 0 | 2 |
| `crates/cognicode-explorer/src/facades/moldql.rs` | 0 | 1 (corroborated) | 1 |
| `crates/cognicode-explorer/tests/intent_integration.rs` | 0 | 1 | 1 |
| `crates/cognicode-explorer/src/moldql/mod.rs` | 0 | 0 | 0 |

### Corroborated findings (raised by one notch)

| Finding | Coupling | Over-eng | Final severity |
|---|---|---|---|
| Duplicated facade `match` block | COUP-001 (SUGG) | OVENG-002 (WARN) | **WARNING** (max reported) |

---

## Decision Gate Application

| Gate | Triggered? | Value |
|---|---|---|
| Any CRITICAL | ❌ | 0 |
| ≥3 HIGH across clusters | ❌ | 2 |
| ≥3 SOLID CRITICAL | ❌ | 0 |
| DQS < 0.3 | ❌ | n/a (architecture cluster skipped at smoke) |
| Connascence pair > 5 bits | ❌ | n/a (architecture cluster skipped at smoke) |
| Any cycle | ❌ | 0 |
| God-class / shotgun-surgery CRITICAL | ❌ | 0 |
| Accidental-bloat trajectory / ≥10 ponytail | ❌ | score=0.55, ledger=0 |

**Decision Gate rule applied**: *"1–2 HIGH findings, mostly LOW/MEDIUM"* → **PASS_WITH_WARNINGS**

---

## Re-iterate Decision

**`re_iterate_from: none`**

Rationale: All findings are WARNINGS or SUGGESTIONS — none block archive. The two WARNINGS (facade duplication + integration test duplication) are best handled as debt-report-attached PR follow-ups, not as a `refactor/debt-moldql-intent-syntax-v1-1` fix cycle. Path A-min was smoke by user choice; the depth selection was correct for the value delivered (~225 lines of pure lowering + 24 tests + facade wiring).

---

## Pre-existing main debt

`false`

Every finding is in code introduced by this branch (commit `18ead66` + working-tree). `git blame` of `crates/cognicode-explorer/src/facades/moldql.rs:57-66` and `:77-86` shows the duplicated `match` block was added by commit `18ead66` (not on main). `crates/cognicode-explorer/tests/intent_integration.rs` is a new untracked file. The `OnceLock<Regex>` statics at `moldql/intent.rs:24-27` are also introduced by `18ead66`.

---

## Recommended next step (orchestrator)

**Path: `sddk-archive`** — the change is ready to be archived with this debt report attached to the PR body.

**Optional PR follow-ups** (non-blocking):
1. Extract `MoldQLServiceImpl::resolve_ast(&self, query)` helper to remove the duplicated facade `match` (5 min refactor).
2. Rewrite or delete `tests/intent_integration.rs` per OVENG-003 — drive the facade end-to-end with a mock repo, or delete in favor of the stricter unit tests.
3. Migrate `OnceLock<Regex>` to `LazyLock<Regex>` after verifying project MSRV ≥ 1.80.
4. Rename `lower_intent_preserves_query_in_error` to `lower_intent_malformed_returns_none`.

---

## Standard Envelope

```yaml
status: success
executive_summary: >
  Smoke debt-verify on moldql-intent-syntax-v1 finds zero CRITICAL, 2 WARNING
  (corroborated facade duplication + integration test duplication), 4 SUGGESTION.
  No cycles, no mutable global state, no SOLID violations, no pre-existing main debt.
  Verdict PASS_WITH_WARNINGS — proceed to sddk-archive with this report attached
  to the PR body as follow-up backlog.
artifacts:
  - "sddk/moldql-intent-syntax-v1/debt-report.md"
verdict: PASS_WITH_WARNINGS
re_iterate_from: none
clusters_run:
  - debt-coupling-cluster
  - debt-overeng-cluster
clusters_skipped:
  - debt-architecture-cluster (smoke depth)
  - debt-smells-cluster (smoke depth)
  - debt-duplication-cluster (smoke depth)
findings_by_severity:
  critical: 0
  warning: 2
  suggestion: 4
findings_by_solid:
  SRP: 0
  OCP: 0
  LSP: 0
  ISP: 0
  DIP: 0
findings_by_file:
  crates/cognicode-explorer/src/moldql/intent.rs: 2
  crates/cognicode-explorer/src/facades/moldql.rs: 2
  crates/cognicode-explorer/tests/intent_integration.rs: 2
  crates/cognicode-explorer/src/moldql/mod.rs: 0
corroborated_findings: 1
pre_existing_main_debt: false
accidental_bloat_score: 0.55
ponytail_debt_count: 0
next_recommended: sddk-archive
risks:
  - "Integration tests do not exercise MoldQLServiceImpl::execute_query end-to-end (WARNING F2); if facade wiring regressed, only static review catches it (documented in verify-report as known caveat)"
  - "Duplicated 10-line match in facades/moldql.rs (WARNING F1) creates drift risk for future lowering policy changes"
context_quality: C1
mode: smoke
path: A-min
```

---

## PR Attachment (markdown for PR body)

```markdown
## Debt Audit (smoke: coupling + over-eng)

**Verdict**: PASS_WITH_WARNINGS

### Findings

🟡 **WARNING** (×2)
- **Duplicated facade `match`** — `execute_query` and `execute_query_with_target` carry the same 10-line block at `facades/moldql.rs:57-66` and `:77-86`. Extract `MoldQLServiceImpl::resolve_ast(&self, query: &str) -> ExplorerResult<MoldQLQuery>`. ~5 min refactor.
- **`tests/intent_integration.rs` duplicates unit tests** — 8/9 tests are 1:1 duplicates of unit tests in `moldql/intent.rs`. Either rewrite to drive `MoldQLService::execute_query` end-to-end with a `MockRepo`, or delete in favor of the stricter unit tests.

💡 **SUGGESTION** (×4, non-blocking)
- `Option<Result<T, E>>` return shape from `lower_intent` could become a named enum for clarity.
- Error message at `facades/moldql.rs:60-62, 79-81` leaks parser internals about the rewritten form.
- `OnceLock<Regex>` + getter-helper at `moldql/intent.rs:24-40` could collapse to `LazyLock<Regex>` if MSRV ≥ 1.80.
- `lower_intent_preserves_query_in_error` test name is misleading (asserts `None`, not error preservation).

### Pre-existing main debt
None. All findings trace to this branch (`18ead66` + working-tree).

### Decision gate
0 CRITICAL · 2 WARNING · 4 SUGGESTION → `PASS_WITH_WARNINGS` (per *"1–2 HIGH findings, mostly LOW/MEDIUM"* rule).

Re-iterate from: `none`. Proceed to archive; attach this report as PR follow-up backlog.
```
