# Plan 003: Handle JoinHandle from scan task in controller

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/application/ingest/controller.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: M
- **Risk**: MED
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

The background scan task's JoinHandle is dropped immediately after spawn. If the task panics (e.g., due to a bug in run_scan or a crash), the job stays in `JobState::Running` forever — no completion, no error, no cleanup. The only way to recover is a server restart.

## Current state

`crates/cognicode-core/src/application/ingest/controller.rs:234-248` — tokio::spawn with no handle capture:
```rust
tokio::spawn(async move {
    let result = run_scan(&repo, &cache, &ws_id_bg, &root, None).await;
    let mut map = jobs.write().await;
    if let Some(s) = map.get_mut(&job_id_bg) {
        s.result = Some(ScanResultPayload::from(&result));
        s.status = JobState::Completed;
        s.finished_at = Some(chrono::Utc::now().to_rfc3339());
    }
});
```

## Scope

**In scope**:
- `crates/cognicode-core/src/application/ingest/controller.rs` (lines 234-260)

**Out of scope**:
- Other JoinHandle patterns in other files
- run_scan itself

## Git workflow

- Branch: `fix/controller-scan-joinhandle`
- Commit: `fix(core): capture JoinHandle and set Failed on panic`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Capture the JoinHandle and await it in a spawned watcher task

Replace the direct spawn with handle capture and a spawned completion handler:

```rust
// Spawn background task
let repo = self.repo.clone();
let cache = self.cache.clone();
let jobs = self.jobs.clone();
let job_id_bg = job_id.clone();
let ws_id_bg = workspace_id.to_string();

let handle = tokio::spawn(async move {
    let result = run_scan(&repo, &cache, &ws_id_bg, &root, None).await;
    let mut map = jobs.write().await;
    if let Some(s) = map.get_mut(&job_id_bg) {
        s.result = Some(ScanResultPayload::from(&result));
        s.status = JobState::Completed;
        s.finished_at = Some(chrono::Utc::now().to_rfc3339());
    }
});

// Spawn a watcher that sets Failed if the task panics
let jobs_watch = self.jobs.clone();
let job_id_watch = job_id.clone();
tokio::spawn(async move {
    let result = handle.await;
    if result.is_err() {
        // Task panicked — mark job as failed
        let mut map = jobs_watch.write().await;
        if let Some(s) = map.get_mut(&job_id_watch) {
            s.status = JobState::Failed;
            s.finished_at = Some(chrono::Utc::now().to_rfc3339());
        }
    }
});
```

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — ensure existing controller tests still pass

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The job status enum doesn't have a `Failed` variant (check JobState enum first)
- The `handle.await` pattern causes compile errors due to lifetime issues

## Maintenance notes

This pattern (spawn + watch for panic) should be applied consistently to other background tasks in the codebase. After this fix, look for similar patterns in watcher.rs and server.rs.
