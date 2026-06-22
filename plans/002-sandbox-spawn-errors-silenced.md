# Plan 002: Fail explicitly when McpServer spawn fails

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

- **Priority**: P1
- **Effort**: M
- **Risk**: HIGH
- **Depends on**: plans/001-sandbox-pre-steps-field.md
- **Category**: bug
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

When McpServer::spawn fails (binary not found, permission denied, etc.), the code silently falls through with `server = None`. The scenario then runs without the MCP server it expects, producing invalid results with no error visible to the user. This is a silent data corruption risk.

## Current state

`crates/cognicode-sandbox/src/main.rs:671-680` — all three spawn paths use `.ok()` converting errors to None:
```rust
server = spawned.or_else(|| {
    if !extra.is_empty() {
        let extra_refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();
        McpServer::spawn_with_env(&server_binary, &workspace_path, &server_env, &extra_refs).ok()
    } else if !server_env.is_empty() {
        McpServer::spawn_with_env(&server_binary, &workspace_path, &server_env, &[]).ok()
    } else {
        McpServer::spawn(&server_binary, &workspace_path).ok()
    }
});
```

At line 682, if server is None only a WARN prints and execution continues with `server = None`.

## Scope

**In scope**:
- `crates/cognicode-sandbox/src/main.rs` (specifically lines 665-700)

**Out of scope**:
- McpServer::spawn implementations themselves
- Other error handling paths in sandbox

## Git workflow

- Branch: `fix/sandbox-spawn-fail-fast`
- Commit: `fix(sandbox): fail explicitly when MCP server spawn fails`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-sandbox` | exit 0 |
| Tests | `cargo test -p cognicode-sandbox --no-fail-fast` | pass |

## Steps

### Step 1: Change the fallback chain to propagate errors

Replace the `.ok()` calls with proper error handling using `.transpose()`:

```rust
server = spawned.or_else(|| {
    if !extra.is_empty() {
        let extra_refs: Vec<&str> = extra.iter().map(|s| s.as_str()).collect();
        Some(McpServer::spawn_with_env(&server_binary, &workspace_path, &server_env, &extra_refs)
            .map_err(|e| anyhow::anyhow!("spawn_with_env failed: {}", e))?)
    } else if !server_env.is_empty() {
        Some(McpServer::spawn_with_env(&server_binary, &workspace_path, &server_env, &[])
            .map_err(|e| anyhow::anyhow!("spawn_with_env (no extra) failed: {}", e))?)
    } else {
        Some(McpServer::spawn(&server_binary, &workspace_path)
            .map_err(|e| anyhow::anyhow!("spawn failed: {}", e))?)
    }
}).transpose();
```

### Step 2: Change the else branch to fail the scenario

After the `if let Some(ref mut s) = server { ... } else { warn }` block, replace the warn with an explicit error return:

```rust
} else {
    return Err(anyhow::anyhow!(
        "MCP server unavailable — cannot run scenario {} without MCP server",
        scenario.id
    ));
}
```

**Verify**: `cargo check -p cognicode-sandbox` → exit 0

## Test plan

- Existing sandbox tests that cover spawn failure paths may need updating if they expected None
- Run `cargo test -p cognicode-sandbox --no-fail-fast` after changes
- Check for tests that assert on server being None — those should be updated to expect an error

## Done criteria

- [ ] `cargo check -p cognicode-sandbox` exits 0
- [ ] `cargo test -p cognicode-sandbox --no-fail-fast` passes (or failures are unrelated to this change)
- [ ] `grep -n '\.ok()' crates/cognicode-sandbox/src/main.rs | grep -i spawn` returns no matches in the spawn chain
- [ ] No files outside scope modified (`git status --porcelain`)
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The McpServer::spawn API signature changed (it returns a different type than expected)
- Changing to `.transpose()` causes type inference errors that can't be resolved with the existing code shape

## Maintenance notes

Any test that was previously passing because scenarios ran "successfully" with `server = None` will now fail. This is correct behavior — those tests were measuring invalid runs.
