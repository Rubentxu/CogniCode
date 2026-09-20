# Proposal: clippy-needless-borrow-bounded

## Intent

Resolve the 9 borrow-related warnings catalogued after the
collapsible_if cycle (H5 phase 1):
- 8 `needless_borrow` warnings (this expression creates a reference
  which is immediately dereferenced by the compiler)
- 1 `borrowed_expression` warning (the borrowed expression
  implements the required traits)

Both lints suggest removing unnecessary `&` operators. They are
behavior-preserving refactors.

## Scope

**In scope**:
- 9 borrow-related warnings in 3 files:
  - crates/cognicode-cli/src/cmd/lifecycle.rs:158
  - crates/cognicode-cli/src/cmd/lifecycle_resolver.rs:537, 557,
    580, 603, 647, 675, 718
  - crates/cognicode-cli/src/cmd/release_test_support.rs:144
- Single bounded commit: `chore(lint): remove needless_borrow (H5
  phase 2)`.
- Cargo verification: `cargo fmt --check`, `cargo check --workspace
  --tests`, `cargo clippy -p cognicode-cli --bin cognicode-release
  --tests` (borrowed-expression count: 9 → 0).

**Out of scope**:
- Other clippy categories.
- Carry-over test failures (`f3_t4_*`, `test_clean_home_install`).
- API surface changes.
- Release engineering.

## Approach

For each warning, apply clippy's auto-suggested fix: remove the
unnecessary `&`.

## Risks

- **Behavior change risk**: removing `&` from function arguments that
  accept `&Path` may inadvertently change types if the inner
  expression returns a `&Path` itself (e.g., `tmp.path()` returns
  `&Path`, so `&tmp.path()` is `&&Path`). The fix collapses
  `&&Path` → `&Path`, which is what the function expects.
- **Lifetime concerns**: removing `&` in cases where the inner
  expression is a temporary that would be dropped may extend or
  shorten lifetimes. Clippy's analysis accounts for this; the fix is
  safe.

## Verification

- `cargo fmt --check` must remain clean.
- `cargo check --workspace --tests` must compile.
- `cargo clippy -p cognicode-cli --bin cognicode-release --tests` must
  report 0 borrowed-expression warnings (down from 9).
- `cargo test -p cognicode-cli --bin cogh` must continue to pass.

## Closure criteria

- 9 borrow warnings eliminated.
- No new warnings introduced.
- No `#[allow(clippy::*)]` introduced.
- Single commit + verify report.

## Note on prior cycle

This is the second bounded cycle of H5 (after collapsible_if phase 1).
The release of H5 phase 1 (v0.97.2 tag) is deferred pending operator
authorization for the version bump. This cycle operates on the same
release scope and will be included in the same v0.97.2 release if
authorized.
