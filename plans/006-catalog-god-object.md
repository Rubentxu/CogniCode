# Plan 006: Break up catalog.rs into per-rule files

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
- **Effort**: L
- **Risk**: MED
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

A 27,801-line file is unmaintainable. Any rule change risks side effects in the 331 other rules. Code review is ineffective at this scale. Navigation (grep, IDE) is nearly useless.

## Current state

`crates/cognicode-axiom/src/rules/catalog.rs:1` — 27,801 lines, single file with 332 rules and `#![allow(unused_variables)]` at line 32.

## Scope

**In scope**:
- `crates/cognicode-axiom/src/rules/catalog.rs` — split into per-rule files
- `crates/cognicode-axiom/src/rules/` — new per-rule module files

**Out of scope**:
- Changes to rule logic itself (only file reorganization)
- Changes to cognicode-core or other crates

## Git workflow

- Branch: `refactor/axiom-catalog-split`
- Commit: `refactor(axiom): split 27k-line catalog.rs into per-rule files`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-axiom` | exit 0 |
| Tests | `cargo test -p cognicode-axiom --no-fail-fast` | pass |

## Steps

### Step 1: Create rules directory structure

Create `crates/cognicode-axiom/src/rules/rules/` and organize rules by language category using the existing `s1135_rule.rs` as the file naming pattern.

### Step 2: Extract rules from catalog.rs to individual files

Each rule function goes into its own file named `<rule_id>_rule.rs`. Keep the module structure and re-exports intact.

### Step 3: Reduce catalog.rs to re-exports only

After extraction, catalog.rs should contain only re-export statements:
```rust
pub mod rule_s1135;
pub mod rule_s1136;
// etc.
pub use rule_s1135::*;
pub use rule_s1136::*;
```

### Step 4: Remove the #![allow(unused_variables)]

With rules in separate files, each rule's unused variables are isolated and won't require a blanket allow.

**Verify**: `cargo check -p cognicode-axiom` → exit 0

## Test plan

- Run `cargo test -p cognicode-axiom --no-fail-fast` — rules should behave identically
- Verify that `axiom::rules::catalog::s1135_rule` etc. are still accessible at their original paths

## Done criteria

- [ ] `cargo check -p cognicode-axiom` exits 0
- [ ] `cargo test -p cognicode-axiom --no-fail-fast` passes
- [ ] catalog.rs reduced to < 500 lines (re-exports only)
- [ ] `grep -c '^pub mod rule_' crates/cognicode-axiom/src/rules/catalog.rs` shows all rules as re-exports
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- Rules have interdependencies that make per-file separation complex (some rules may call others)
- The module re-export path breaks existing imports in cognicode-core or cognicode-quality

## Maintenance notes

After this split, adding a new rule means creating one small file, not editing a 27k-line file. This is a prerequisite for any meaningful rule development or review.
