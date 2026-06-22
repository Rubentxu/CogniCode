# Plan 009: Audit Mutex::lock().expect() usage and replace with unwrap_or_else

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/infrastructure/graph/`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

`Mutex::lock().expect()` panics if the mutex is poisoned (another thread panicked while holding it). In production, this causes the process to abort even for recoverable errors. The codebase has 52+ occurrences across multiple files.

## Current state

`crates/cognicode-core/src/infrastructure/graph/graph_cache.rs:64,82,101,155,162,174` — all use `.lock().expect("graph cache poisoned")`.

## Scope

**In scope**:
- `crates/cognicode-core/src/infrastructure/graph/graph_cache.rs`
- Other high-traffic mutex lock sites in the codebase

**Out of scope**:
- All 52+ occurrences at once — focus on the highest-traffic sites first

## Git workflow

- Branch: `fix/mutex-expect-panic`
- Commit: `fix(core): replace Mutex::lock().expect() with unwrap_or_else in graph_cache`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Find all .lock().expect() patterns

```bash
rg '\.lock\(\)\.expect' crates/cognicode-core/src --stats
```

### Step 2: Replace with unwrap_or_else with panic on poison

Replace `.lock().expect("msg")` with:
```rust
.lock()
    .unwrap_or_else(|_| panic!("graph cache poisoned — a previous operation panicked"))
```

### Step 3: Verify

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — behavior should be unchanged (panic still occurs on poison, just explicitly)
- No new tests needed — the behavior change is cosmetic (same outcome, clearer intent)

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] graph_cache.rs no longer uses `.lock().expect()`
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The Mutex type used doesn't support unwrap_or_else (e.g., it's a StdMutex not ParkingLotMutex)

## Maintenance notes

For truly defensive code, consider whether poison recovery (unlocking and retrying) is appropriate for your use case. For graph_cache, panicking on poison is likely correct since the cache state is corrupted.
