# Plan 008: Remove duplicate ensure_graph_built in consolidated_handlers

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/interface/mcp/handlers/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: M
- **Risk**: LOW
- **Depends on**: none
- **Category**: tech-debt
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

Two near-identical `ensure_graph_built` functions exist in different handler files with different return types. The simpler version in consolidated_handlers.rs lacks the richer version's logging and timing info. Callers are scattered across 3 files.

## Current state

- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs:1639` — richer version returning `EnsureResult`
- `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs:263` — simpler version returning `()`

## Scope

**In scope**:
- `crates/cognicode-core/src/interface/mcp/handlers/consolidated_handlers.rs`
- `crates/cognicode-core/src/interface/mcp/handlers/mod.rs`

**Out of scope**:
- Other handler files that might call ensure_graph_built

## Git workflow

- Branch: `refactor/ensure-graph-built-dedup`
- Commit: `refactor(core): dedupe ensure_graph_built in consolidated_handlers`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Read both versions

Compare the two functions to understand their differences.

### Step 2: Remove the simpler version and update callers

Replace calls in consolidated_handlers.rs to use the richer version from mod.rs. If the simpler return type is needed, have consolidated_handlers call the canonical version and discard the extra fields.

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — ensure behavior is unchanged

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] Only one `ensure_graph_built` implementation remains accessible from handlers
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The two versions have subtle behavioral differences that can't be safely unified
- Changing the return type breaks callers in unexpected ways

## Maintenance notes

After dedup, logging/timing is consistent across all ensure_graph_built call sites.
