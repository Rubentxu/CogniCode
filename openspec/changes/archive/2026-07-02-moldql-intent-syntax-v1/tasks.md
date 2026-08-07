# Tasks: MoldQL Intent Syntax v1 (Lowering Layer)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 160-200 (1 new file + 2 small edits) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: Yes
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Foundation — `intent.rs` skeleton + contract tests

- [ ] 1.1 Create `crates/cognicode-explorer/src/moldql/intent.rs` with module-level doc and `pub fn lower_intent(query: &str) -> Option<Result<MoldQLQuery, ParseError>>` signature (no body yet — `unimplemented!()` placeholder)
- [ ] 1.2 RED: add table-driven `#[cfg(test)] mod tests` block in `intent.rs` with the 7 spec scenarios (lowercase `symbols where …` happy path, `calls from 'id' depth N` happy path, depth default, quoted id, unrecognized lowercase → `None`, uppercase `FIND …` → `None`, malformed lowercase → `Some(Err)`); each case asserts exact `lower_intent` return value before any parsing happens
- [ ] 1.3 RED: add 2 roundtrip tests in the same `tests` mod — `lower_intent("symbols where fan_out > 5")` must equal `Some(Ok(parser::parse("FIND symbols WHERE fan_out > 5")))`; `lower_intent("calls from 'symbol:a.rs:a:1' depth 2")` must equal `Some(Ok(parser::parse("EXPLORE symbol:a.rs:a:1 THROUGH callees DEPTH 2")))`
- [ ] 1.4 RED: add edge-case tests — empty string → `None`, whitespace-only → `None`, `SYMBOLS WHERE …` (uppercase) → `None`, `find symbols where …` (mixed-case `find`) → must NOT match v1 (case-sensitive lowercase prefix per design)

## Phase 2: Core Implementation — `lower_intent` body

- [ ] 2.1 GREEN: implement pattern 1 in `lower_intent` — `lazy_static!` (or `std::sync::OnceLock`) regex `^symbols\s+where\s+(.+)$` matching the full line; on match return `Some(parser::parse(&format!("FIND symbols WHERE {captures[1]}")))`; non-match falls through
- [ ] 2.2 GREEN: implement pattern 2 in `lower_intent` — regex `^calls\s+from\s+['\"]?([^'"\s]+)['\"]?(?:\s+depth\s+(\d+))?$`; on match produce `"EXPLORE {id} THROUGH callees DEPTH {depth_or_1}"` then delegate to `parser::parse`; preserve original `ParseError` if the rewrite fails to parse (so diagnostics reference the original intent query, not the rewritten form)
- [ ] 2.3 GREEN: implement return — `None` when neither pattern matches; rely on `regex::Regex::is_match` short-circuit; pure function, no `SymbolRepository`, no I/O
- [ ] 2.4 REFACTOR: extract the two rewrite helpers as `fn rewrite_symbols_where(rest: &str) -> String` and `fn rewrite_calls_from(id: &str, depth: Option<&str>) -> String` private fns; keep regexes module-level statics to avoid recompilation per call

## Phase 3: Integration — wire into facade + module exports

- [ ] 3.1 Edit `crates/cognicode-explorer/src/moldql/mod.rs`: add `pub mod intent;` to the module list and `pub use intent::lower_intent;` to the re-exports block
- [ ] 3.2 Edit `crates/cognicode-explorer/src/facades/moldql.rs`: in `execute_query`, replace the direct `crate::moldql::parser::parse(query)` call with a `match lower_intent(query) { Some(Ok(ast)) => ast, Some(Err(e)) => return Err(ExplorerError::ResolutionFailed(format!("intent query `{query}` invalid: {e}"))), None => crate::moldql::parser::parse(query).map_err(|e| ExplorerError::ResolutionFailed(e.to_string()))? }` block
- [ ] 3.3 Apply the same lowering wrapper to `execute_query_with_target` in the same file (same 3-arm match, identical error wording)
- [ ] 3.4 RED integration test in `facades/moldql.rs` (or a new `tests/intent_integration.rs`): with a `MockRepo`, `execute_query("symbols where fan_in > 0")` returns `Ok(_)` (no `ParseError`); `execute_query("FIND symbols WHERE fan_in > 0")` still returns the same `Ok(_)` (uppercase regression); `execute_query("symbols where ") ` returns `Err` with message containing `"intent query"`

## Phase 4: Verification

- [ ] 4.1 Run `cargo test -p cognicode-explorer moldql::` — all existing parser tests + new intent tests pass; confirm zero new failures in `parser.rs` / `parser_explorerql.rs` tests
- [ ] 4.2 Run `cargo clippy -p cognicode-explorer -- -D warnings` — no new lints; verify regex statics don't trip `clippy::regex_creation_in_loop` or similar
- [ ] 4.3 Run `cargo bench -p cognicode-explorer --no-run` (or compile-only check) — confirm no benchmark regressions from the added match arm in the facade hot path
- [ ] 4.4 Manual smoke: feed the 2 supported scaffold examples from `assets/moldql-scaffolds.yaml` through `execute_query`; confirm both execute successfully

## Notes / Constraints (carry into sddk-apply)

- **Pure function contract** — `lower_intent` must not depend on `SymbolRepository`, ports, or async runtime. If a future v2 needs bare-name resolution, the function will move behind the service, not gain a parameter.
- **`parse()` contract untouched** — the lowerer is the only new code path; the parser itself gains no new keywords and no new branches.
- **Error fidelity** — `Some(Err(parse_err))` from `lower_intent` must surface the ORIGINAL intent query in the user-facing error, not the rewritten uppercase string. This is the `format!("intent query `{query}` invalid: {e}")` wrapper in 3.2/3.3.
- **Case sensitivity** — v1 patterns are case-sensitive lowercase prefixes (`symbols`, `calls`); uppercase or mixed-case `Find`/`CALLS` fall through to `parse()` unchanged.

## Discrepancies vs request (flagged for orchestrator)

The request summary diverges from the stored design `#3935` in two places — tasks follow the **design**:

1. **Signature**: design says `lower_intent(&str) -> Option<Result<MoldQLQuery, ParseError>>`; request summary said `Option<String>`. AST-direct path avoids a re-parse and gives the facade a single decision point (`Some(Ok)` = use, `Some(Err)` = surface original-intent error, `None` = fallback to `parse(query)`).
2. **EXPLORE rewrite**: design says `EXPLORE {id} THROUGH callees DEPTH {n|1}` (matches the parser's `EXPLORE <object_ref> THROUGH <direction> DEPTH <n>` shape — direction is `callees` because "calls from X" = forward edges = outgoing); request summary said `EXPLORE FROM {id} [DEPTH {n}]`, which would fail `parse()` (no `FROM` keyword in the EXPLORE grammar). Tasks use the design-correct form.

If you want the request-summary form anyway, say so and I'll re-task — but expect `execute_query("calls from 'symbol:src/a.rs:a:1' depth 2")` to return `ParseError("expected THROUGH …")` instead of executing.
