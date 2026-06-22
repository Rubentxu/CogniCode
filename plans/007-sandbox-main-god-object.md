# Plan 007: Break up sandbox orchestrator main.rs

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-sandbox/src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P3
- **Effort**: L
- **Risk**: MED
- **Depends on**: plans/001-sandbox-pre-steps-field.md
- **Category**: tech-debt
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

A 6,791-line binary containing scenario loading, execution, scoring, reporting, and benchmarking is highly coupled. Testing requires full integration setup and makes incremental changes risky — developers fear modifying it.

## Current state

`crates/cognicode-sandbox/src/main.rs:1` — 6,791 lines, single binary.

## Scope

**In scope**:
- `crates/cognicode-sandbox/src/main.rs` — extract modules

**Out of scope**:
- Changes to orchestration logic itself
- Changes to cognicode-core or other crates

## Git workflow

- Branch: `refactor/sandbox-main-split`
- Commit: `refactor(sandbox): extract scoring and reporting modules from main.rs`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-sandbox` | exit 0 |
| Tests | `cargo test -p cognicode-sandbox --no-fail-fast` | pass |

## Steps

### Step 1: Extract scoring module

Create `crates/cognicode-sandbox/src/scoring.rs` containing the scoring logic currently inline in main.rs.

### Step 2: Extract reporting module

Create `crates/cognicode-sandbox/src/reporting.rs` containing the report generation logic.

### Step 3: Reduce main.rs to orchestration only

main.rs should become the entry point that delegates to the extracted modules.

**Verify**: `cargo check -p cognicode-sandbox` → exit 0

## Test plan

- Run `cargo test -p cognicode-sandbox --no-fail-fast` — existing tests should pass with refactored code
- Verify that `cargo test -p cognicode-sandbox --no-fail-fast` behavior is unchanged

## Done criteria

- [ ] `cargo check -p cognicode-sandbox` exits 0
- [ ] `cargo test -p cognicode-sandbox --no-fail-fast` passes
- [ ] main.rs reduced by at least 1,000 lines
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- Scoring/reporting logic has deep dependencies on main.rs local variables that make extraction complex
- Extracting modules requires changing public API signatures

## Maintenance notes

After this split, unit testing scoring logic in isolation becomes possible. This is a prerequisite for proper test coverage of the sandbox orchestrator.
