# Proposal: clippy-collapsible-if-bounded

## Intent

Resolve the 18 `collapsible_if` clippy warnings catalogued in the
post-H4.7 cosmetic-clippy inventory. These warnings are reported by
clippy as "this `if` statement can be collapsed" and suggest using
`let_chains` (stable in Rust 1.96 + edition 2024) to combine nested
if-let and if-conditionals into single boolean expressions.

## Scope

**In scope**:
- Apply clippy's auto-suggested fix for each of the 18 collapsible_if
  warnings in `crates/cognicode-cli/src/cmd/{doctor,ide,
  installer_transaction,layout,lifecycle_resolver,platform_adapter,
  rollback_journal}.rs` (10 files).
- Cargo verification: `cargo fmt --check`, `cargo check --workspace
  --tests`, `cargo clippy -p cognicode-cli --bin cognicode-release
  --tests` (collapsible_if count: 18 → 0).
- One bounded commit (`chore(lint): collapse collapsible_if (H5
  phase 1)`) + one verify report.

**Out of scope**:
- Other clippy categories (collapsible_if only per cycle).
- Code restructuring (no API surface changes, no `#[allow(clippy::*)]`
  attributes, no behavior changes).
- Carry-over test failures (`f3_t4_broken_same_version_install`,
  `test_clean_home_install`) — separate future cycles.

## Approach

For each warning, clippy suggests the exact diff. The bounded fix is:

```rust
// Before
if let Some(x) = foo {
    if let Some(y) = bar(x) {
        do_thing(y);
    }
}

// After
if let Some(x) = foo
    && let Some(y) = bar(x)
{
    do_thing(y);
}
```

This is a behavior-preserving refactor in edition 2024 (let_chains
stable).

## Approach comparison

| Approach | Pros | Cons | Effort |
|----------|------|------|--------|
| A. Manual surgery per warning (this proposal) | Lowest risk, exact mapping, evidence per file | Slower (18 individual edits) | Low |
| B. `cargo clippy --fix --allow-dirty --allow-staged` | Fast (one command) | Risk of mixing with H4.7 scope (lesson learned); requires post-fix audit | Low |
| C. Defer until Rustfmt/clippy auto-handle | Zero effort | May never happen; warnings persist | None (passive) |

**Recommendation**: Approach A. Approach B was used in H4.7 and
touched 26 unrelated files; the audit cost exceeded the time saved.
Approach C leaves debt visible indefinitely.

## Risks

- **let_chains stability across rustc versions**: this project uses
  rustc 1.96 + edition 2024 where let_chains is stable. The
  CI/Cargo.toml does not pin MSRV; if a downstream consumer is on
  rustc < 1.88 (let_chains-stabilization release), the refactor would
  fail to compile. **Mitigation**: add `#![feature(let_chains)]` only
  if MSRV is older; here edition 2024 makes that unnecessary.

## Verification

- `cargo fmt --check` must remain clean.
- `cargo check --workspace --tests` must compile.
- `cargo clippy -p cognicode-cli --bin cognicode-release --tests` must
  report 0 collapsible_if warnings (down from 18).
- `cargo test -p cognicode-cli --bin cogh` must continue to pass
  (the pre-existing `f3_t4_*` failure is acknowledged; not
  introduced by this cycle).

## Closure criteria

- 18 collapsible_if warnings eliminated.
- No new warnings introduced.
- No `#[allow(clippy::*)]` introduced.
- Single commit + single verify report.
- Archive manifest linked to release-receipt (deferred to v0.97.2
  release cycle if not yet cut).
