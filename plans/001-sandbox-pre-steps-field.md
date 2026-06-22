# Plan 001: Add pre_steps field to ExpandedScenario

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/sandbox_core/manifest.rs crates/cognicode-sandbox/src/main.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P1
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: bug
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

The sandbox crate does not compile because ExpandedScenario lacks a `pre_steps` field that the executor code at main.rs:757 accesses. All 332 sandbox fixtures have `pre_steps: None` as a workaround. Until this is fixed, no sandbox tests can run and `cargo test --workspace` fails.

## Current state

`crates/cognicode-core/src/sandbox_core/manifest.rs:222-250` — ExpandedScenario struct definition has NO `pre_steps` field.

`crates/cognicode-sandbox/src/main.rs:757` — code that uses the missing field:
```rust
if let Some(ref pre_steps) = scenario.pre_steps {
```

## Scope

**In scope**:
- `crates/cognicode-core/src/sandbox_core/manifest.rs` — add the field
- `crates/cognicode-sandbox/src/main.rs` — remove or guard the pre_steps access

**Out of scope**:
- Any other ExpandedScenario fields or validation changes
- Sandbox execution logic beyond the pre_steps field access

## Git workflow

- Branch: `fix/sandbox-pre-steps-field`
- Commit per step: `fix(core): add pre_steps field to ExpandedScenario`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-sandbox` | exit 0, no errors |
| Tests | `cargo test -p cognicode-sandbox --no-fail-fast` | pass |

## Steps

### Step 1: Add pre_steps field to ExpandedScenario struct

In `crates/cognicode-core/src/sandbox_core/manifest.rs`, add the `pre_steps` field to ExpandedScenario after `container_image`:

```rust
/// Explicit pre-requisite tools listed in the scenario manifest.
#[serde(skip_serializing_if = "Option::is_none")]
pub pre_steps: Option<Vec<String>>,
```

**Verify**: `cargo check -p cognicode-core` → exit 0

### Step 2: Remove the pre_steps dead code path in sandbox

In `crates/cognicode-sandbox/src/main.rs` at line 757, remove the entire `if let Some(ref pre_steps) = scenario.pre_steps { ... }` block since the field will now be `None` for all existing scenarios.

**Verify**: `cargo check -p cognicode-sandbox` → exit 0

## Test plan

- Run `cargo test -p cognicode-sandbox --no-fail-fast` to confirm the crate compiles and its existing tests pass
- No new tests needed — the field addition is a type fix; the dead-code removal doesn't change behavior

## Done criteria

- [ ] `cargo check -p cognicode-sandbox` exits 0
- [ ] `cargo test -p cognicode-sandbox --no-fail-fast` passes (or is blocked by other unrelated issues)
- [ ] `cargo check --workspace` shows no new errors related to ExpandedScenario
- [ ] No files outside scope are modified (`git status --porcelain`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report back if:
- The field definition in manifest.rs already exists (codebase drifted)
- Adding the field causes compilation errors elsewhere in cognicode-core
- The sandbox/main.rs pre_steps access pattern is fundamentally different than described

## Maintenance notes

The YAML manifests in `sandbox/manifests/` already have `pre_steps:` keys. When the field is added, those YAML values will be deserializable. The dead code removal is safe because pre_steps behavior was never functional — it was added as a placeholder.
