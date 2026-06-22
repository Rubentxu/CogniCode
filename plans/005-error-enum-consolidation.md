# Plan 005: Introduce CommonError base enum to reduce error enum overlap

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/domain/error.rs crates/cognicode-core/src/application/error.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

Five error enums with overlapping variants (NotFound, Internal, InvalidInput) scattered across layers require 30+ From implementations. Adding a new variant means touching 4+ files. This is maintenance friction that slows down error handling changes.

## Current state

- `crates/cognicode-core/src/domain/error.rs` — DomainError (8 variants, 33 lines)
- `crates/cognicode-core/src/application/error.rs` — AppError (261 lines)
- `crates/cognicode-core/src/interface/mcp/error.rs` — HandlerError (38 lines)
- `crates/cognicode-explorer/src/error.rs` — ExplorerError (84 lines)
- plus InterfaceError (174 lines) in interface layer

DomainError at `crates/cognicode-core/src/domain/error.rs`:
```rust
pub enum DomainError {
    SymbolNotFound(String),
    InvalidSymbolKind(String),
    CycleDetected,
    LocationOutOfBounds(String),
    ParseError(String),
    AnalysisError(String),
    RefactoringError(String),
    InvalidRange(String),
}
```

## Scope

**In scope**:
- `crates/cognicode-core/src/domain/error.rs` — add CommonError here
- `crates/cognicode-core/src/application/error.rs` — add #[from] for CommonError to AppError

**Out of scope**:
- ExplorerError in cognicode-explorer (different crate)
- Changing existing error propagation chains beyond adding From impls

## Git workflow

- Branch: `refactor/error-enum-consolidation`
- Commit: `refactor(core): add CommonError base enum`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Define CommonError in domain/error.rs

In `crates/cognicode-core/src/domain/error.rs`, add after DomainError:

```rust
/// Common error variants shared across all layers.
/// Use thiserror's #[from] to allow ? operator to convert automatically.
#[derive(Error, Debug)]
pub enum CommonError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("internal error: {0}")]
    Internal(String),
}
```

### Step 2: Add #[from] to AppError variants that overlap

In `crates/cognicode-core/src/application/error.rs`, add `#[from]` to variants that map to CommonError, then add:
```rust
#[from]
NotFound(String)  // maps to CommonError::NotFound
```

### Step 3: Verify compilation

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — existing tests should pass
- Verify that `?` operator works for CommonError conversions

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] CommonError is defined with at least NotFound, InvalidInput, Internal variants
- [ ] AppError can be constructed from CommonError via `?`
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The existing error enum variants don't map cleanly to CommonError (they may have different semantics)
- Adding #[from] causes ambiguity in existing From impls

## Maintenance notes

This is the first step of a larger consolidation. After this, callers can use `?` to convert between error types automatically. The full migration of all 30+ From impls is deferred to a follow-up effort.
