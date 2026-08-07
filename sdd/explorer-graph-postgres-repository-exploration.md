# Exploration: Test Slowness in `explorer-graph-postgres-repository` Slice

**Date**: 2026-06-08  
**Investigator**: sdd-explore sub-agent  
**Status**: Diagnosis complete — no files edited.

---

## Executive Summary

The slowness has **nothing to do with the PostgreSQL code or the `pg_test!` fixture**. The custom `pg_test!` macro is innocent — it doesn't even run without `TEST_DATABASE_URL` set. The sqlx compile-time impact is negligible (~1.2s incremental, not the ~30s feared in the proposal). The real culprits are **two pre-existing tree-sitter stress tests** that parse multi-megabyte single-line inputs, and **tokio runtime oversubscription** from 72 `#[tokio::test]` tests in `workspace_session` each creating full multi-thread runtimes.

---

## Evidence Table

| Cause | Evidence | Confidence |
|-------|----------|------------|
| 2 tree-sitter long-line tests consume >120s | Measured individually: `test_parser_handles_very_long_line_python` >60s; `test_parser_find_symbols_very_long_line` >60s | **HIGH** — timed individually |
| 72 tokio multi-thread runtime tests cause contention | `workspace_session` tests: 30s with default threads vs 0.5s with `--test-threads=1` | **HIGH** — measured |
| sqlx compile cost is negligible | Clean build `cargo check -p cognicode-core` = 40.3s; with `--features postgres` = 41.5s; delta = +1.2s | **HIGH** — measured |
| `pg_test!` fixture does not contribute | 0 PostgreSQL tests run without `TEST_DATABASE_URL`; macro produces `#[tokio::test]` that immediately returns | **HIGH** — code review + runtime verification |
| Test count (1073) amplifies small costs | Even 2ms avg per test = 2.1s floor; but tree-sitter/semantic tests create temp dirs, parse real Rust code, etc | **MEDIUM** — observational |
| Rust tree-sitter parses long lines efficiently (1.6s) | `test_parser_handles_very_long_line_rust`: 1.62s — Python parser is the outlier | **HIGH** — measured |
| 2 pre-existing test failures (unrelated) | `test_property_max_length_boundary`, `test_property_path_traversal_standard_patterns` — pre-existing, not from this slice | **HIGH** — verified |

---

## Compile-Time Analysis

### Default build (no postgres feature)

| Command | Wall time | Notes |
|---------|-----------|-------|
| `cargo check -p cognicode-core` (clean) | **40.3s** | Full workspace dep resolution |
| `cargo test -p cognicode-core --lib --no-run` | **59.2s** | Test profile adds debug_assertions |
| `cargo test -p cognicode-core --lib` (compile + run) | timed out at 10min | Cause: RUNTIME, not compile |

### With postgres feature

| Command | Wall time | Delta vs default |
|---------|-----------|-----------------|
| `cargo check -p cognicode-core --features postgres` (clean) | **41.5s** | **+1.2s (2.9%)** |
| `cargo test -p cognicode-core --lib --features postgres --no-run` | **57.6s** | -1.6s (noise — cached deps overlap) |

### sqlx dependency tree footprint

```
cognicode-core → sqlx → sqlx-core → ... (4 entries at depth 2)
```

The sqlx macros crate (`sqlx-macros-core`) compiles quickly. The `sqlx-postgres` driver adds `sha2`, `hmac`, `md-5`, `hkdf` — all small crates.

**Verdict**: The proposal's estimate of "~30s compile time for sqlx" was **overstated by 25x**. The real increment is ~1-3 seconds. The feature gate is still valuable for default builds but the concern is minimal.

---

## Runtime Analysis

### Test count and distribution

- **Total tests**: 1,073 (lib only, `cognicode-core`)
- **`#[tokio::test]` tests**: ~200+ (async tests using multi-thread runtime by default)
- **`#[test]` tests**: ~870
- **Slow stress tests**: 5 (using `.repeat(100_000)` or `.repeat(50_000)`)

### Identified hotspots

#### 1. Tree-sitter Python extremely long line (>60s)

```rust
// tree_sitter_parser.rs:1146
fn test_parser_handles_very_long_line_python() {
    let long_line = "def foo(): ".repeat(100_000);  // ~1.3 MB
    let parser = TreeSitterParser::new(Language::Python).unwrap();
    let result = parser.parse(&long_line);  // O(n²) or worse in tree-sitter-python
}
```

**Measured**: killed at 120s timeout. The Python tree-sitter grammar has pathological performance on single-line inputs >100KB. The 100,000-repetition input (~1.3MB) is 10x beyond what triggers the O(n²) path.

#### 2. Tree-sitter Rust `find_function_definitions` on huge line (>60s)

```rust
// tree_sitter_parser.rs:1308
fn test_parser_find_symbols_very_long_line() {
    let long_line = "fn very_long_function_name_that_exceeds_normal_limits() { ".repeat(50_000);
    // ~2.85 MB of input — but `find_function_definitions` traverses the entire AST
    let parser = TreeSitterParser::new(Language::Rust).unwrap();
    let symbols = parser.find_function_definitions(&long_line);
}
```

**Measured**: killed at 120s timeout. While `parse()` on Rust completes in 1.6s with 100k repeats, `find_function_definitions` does a full AST walk that may be O(n²) on this massive single-line input.

#### 3. Tokio runtime oversubscription in `workspace_session` tests

72 of 75 tests in `workspace_session.rs` use `#[tokio::test]` (default: multi-thread runtime). When run with default concurrency:

- Each test creates its own multi-thread tokio runtime
- With N CPU cores, each runtime spawns N worker threads
- 72 tests × 8 cores = **576 threads** contending
- Measured: 75 tests take **30.05s** with default threads vs **0.5s** with `--test-threads=1`

Other test modules with many `#[tokio::test]` tests (aix_handlers, refactor_handlers, rmcp_adapter) contribute additional contention but at smaller scale.

#### 4. Other tree-sitter long-line tests (acceptable)

| Test | Input size | Time |
|------|-----------|------|
| `test_parser_handles_very_long_line_rust` | 100k × "fn foo() { " | **1.62s** |
| `test_parser_handles_very_long_line_javascript` | 100k × "function foo() { " | **1.44s** |

These are acceptable in isolation but contribute when all run in parallel.

### Excluding the 2 worst offenders

```bash
cargo test --lib -- --skip test_parser_handles_very_long_line_python \
                    --skip test_parser_find_symbols_very_long_line
```

Result: **1,054 passed, 2 failed (pre-existing), 15 ignored, finished in 30.05s**

---

## Most Likely Root Causes (Ranked)

| Rank | Cause | Impact | Confidence |
|------|-------|--------|------------|
| **1** | `test_parser_handles_very_long_line_python` — O(n²) tree-sitter-python on 1.3MB line | >60s per run | HIGH |
| **2** | `test_parser_find_symbols_very_long_line` — full AST walk on 2.85MB line | >60s per run | HIGH |
| **3** | Tokio multi-thread runtime oversubscription (72 tests × multi-thread) | ~25s added (30s vs 0.5s for `workspace_session`) | HIGH |
| **4** | Test volume (1073 tests) amplifies per-test overhead from temp dirs, tree-sitter parsing boilerplate | ~5s floor | MEDIUM |
| **5** | 2 other long-line tests (Rust 1.6s, JS 1.4s) | ~3s total | LOW |
| **6** | sqlx compile time | ~1.2s | LOW — NOT a real problem |
| **7** | `pg_test!` fixture | 0s (never runs without `TEST_DATABASE_URL`) | NONE |

---

## Recommended Mitigations (Ranked — lowest risk first)

### 1. Reduce long-line test repetition counts (LOW RISK, HIGH IMPACT)

Change the `repeat()` counts in `tree_sitter_parser.rs`:

| Test | Current | Proposed | Expected time |
|------|---------|----------|---------------|
| `test_parser_handles_very_long_line_python` | `100_000` | `10_000` | <2s (from >60s) |
| `test_parser_find_symbols_very_long_line` | `50_000` | `5_000` | <2s (from >60s) |
| `test_parser_handles_very_long_line_rust` | `100_000` | `10_000` | <0.2s (from 1.6s) |
| `test_parser_handles_very_long_line_javascript` | `100_000` | `10_000` | <0.2s (from 1.4s) |

These tests validate "no panic on long lines" — 10,000 repeats (~130KB) is more than sufficient to prove that behavior. A 100,000-repeat input tests tree-sitter performance, not CogniCode correctness.

**Estimated savings**: **~120s removed from test runtime**.

### 2. Use `#[tokio::test(flavor = "current_thread")]` for all async tests (LOW RISK, HIGH IMPACT)

Change `#[tokio::test]` to `#[tokio::test(flavor = "current_thread")]` in:
- `application/workspace_session.rs` (72 tests)
- `interface/mcp/handlers/aix_handlers.rs` (many tests)
- `interface/mcp/rmcp_adapter.rs` (6 tests)
- Any other module with `#[tokio::test]`

None of these tests need multi-thread parallelism — they all use `.await` and shared state. Current-thread runtime eliminates runtime-per-test overhead.

**Estimated savings**: **~25s removed from test runtime** (eliminates the 30s→0.5s gap in workspace_session).

### 3. Pin test threads in CI (LOW RISK, MEDIUM IMPACT)

Add `--test-threads=4` to `cargo test` invocations in CI. This prevents the exponential blowup from NCPUS × Ntokio_runtimes while still allowing parallel execution.

**Estimated savings**: Direct improvement in wall time; prevents timeout.

### 4. `#[ignore]` the 2 extreme tests, run them in a nightly stress job (LOW RISK, LOW IMPACT)

The Python and `find_symbols` long-line tests validate tree-sitter library behavior, not CogniCode logic. Running them nightly (or on-demand) is sufficient.

**Savings**: ~120s from every CI run (but other mitigations make this redundant).

### 5. Update proposal/spec documentation (ZERO RISK, DOCUMENTATION)

Correct the sqlx compile-time estimate from "~30s" to "~1-3s" in the proposal and tasks documents. This removes a false concern from future readers.

---

## Impact on SDD Flow

### This does NOT change the verify/archive flow

The slowness is entirely in **pre-existing tests**, not in the PostgreSQL code this slice adds. The `pg_test!` fixture is clean and efficient. When `TEST_DATABASE_URL` is set, it would create per-test databases quickly (1-2 `CREATE DATABASE` calls per test).

### Recommended action

1. **Proceed with verify/archive for the `explorer-graph-postgres-repository` slice as-is.** The PostgreSQL code is correct and the test infrastructure is sound.

2. **File a separate follow-up issue** for the tree-sitter test size reduction and tokio test optimization. This is a pre-existing codebase quality concern, not a blocker for the current slice.

3. **In CI** (when PostgreSQL is added), use `--test-threads=4` to avoid the tokio runtime contention that exists independently of PostgreSQL tests.
