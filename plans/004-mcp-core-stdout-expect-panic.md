# Plan 004: Replace expect with proper error handling in mcp_core.rs

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/sandbox_core/mcp_core.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

The `.expect()` calls will panic the process if the OS fails to capture stdin/stdout pipes. While this is an OS-level failure (not expected in normal operation), the panic is catastrophic — aborting the entire server. A proper error return is proportionate.

## Current state

`crates/cognicode-core/src/sandbox_core/mcp_core.rs:113-114`:
```rust
let stdin = child.stdin.take().expect("no stdin");
let stdout = child.stdout.take().expect("no stdout");
```

## Scope

**In scope**:
- `crates/cognicode-core/src/sandbox_core/mcp_core.rs` (lines 113-115)

**Out of scope**:
- Other .expect() calls in the codebase
- McpError enum itself

## Git workflow

- Branch: `fix/mcp-core-stdout-expect`
- Commit: `fix(core): return error instead of panicking on missing stdin/stdout`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Replace .expect() with .ok() and map to McpError

Replace lines 113-114 with:

```rust
let stdin = child.stdin.take()
    .ok_or_else(|| McpError::SpawnError("failed to capture stdin from child process".into()))?;
let stdout = child.stdout.take()
    .ok_or_else(|| McpError::SpawnError("failed to capture stdout from child process".into()))?;
```

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — the change is error-level, tests should be unaffected unless they specifically tested the panic path (unlikely)
- No new tests needed — the panic path is not normally reachable

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] `grep -n '\.expect.*stdin\|.expect.*stdout' crates/cognicode-core/src/sandbox_core/mcp_core.rs` returns no matches
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- McpError doesn't have a SpawnError variant (check the enum first)
- Taking stdin/stdout as owned values (not refs) causes borrowing errors elsewhere in the function

## Maintenance notes

Same pattern should be applied to the identical `.expect("no stdin")` / `.expect("no stdout")` in sandbox main.rs at lines 2424-2425.
