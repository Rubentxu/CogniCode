# Plan 011: Remove #![allow(unused_variables)] from catalog.rs

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-axiom/src/rules/catalog.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: S
- **Risk**: LOW
- **Depends on**: plans/006-catalog-god-object.md (should be done first)
- **Category**: tech-debt
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

The blanket `#![allow(unused_variables)]` at catalog.rs:32 hides dozens of unused variables across 332 rules. This is a blunt instrument that prevents the compiler from catching real issues. After breaking catalog.rs into per-rule files (Plan 006), each rule can have its own allow list scoped to its actual unused variables.

## Current state

`crates/cognicode-axiom/src/rules/catalog.rs:32`:
```rust
#![allow(unused_variables)]
```

## Scope

**In scope**:
- `crates/cognicode-axiom/src/rules/catalog.rs` (line 32)

**Out of scope**:
- Individual rule files (that work is done in Plan 006)
- Other `#![allow]` directives

## Git workflow

- Branch: `fix/axiom-remove-blanket-allow`
- Commit: `fix(axiom): remove blanket #![allow(unused_variables)] from catalog.rs`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-axiom` | exit 0 (may show warnings) |
| Tests | `cargo test -p cognicode-axiom --no-fail-fast` | pass |

## Steps

### Step 1: Remove the blanket allow

Delete line 32: `#![allow(unused_variables)]`

### Step 2: Add targeted allows per file after splitting

After Plan 006 creates per-rule files, add `#![allow(unused_variables)]` only to rule files that need it, not to the aggregator.

**Verify**: `cargo check -p cognicode-axiom` → exit 0 (with or without warnings)

## Test plan

- Run `cargo test -p cognicode-axiom --no-fail-fast` — behavior should be unchanged

## Done criteria

- [ ] `#![allow(unused_variables)]` removed from catalog.rs
- [ ] `cargo test -p cognicode-axiom --no-fail-fast` passes
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- Removing the allow causes compilation errors that can't be fixed with targeted allows

## Maintenance notes

This depends on Plan 006 being completed first. Without splitting catalog.rs, removing this allow would hide unused variables across all 332 rules at once, making cleanup impossible to prioritize.
