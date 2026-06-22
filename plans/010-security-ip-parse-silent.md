# Plan 010: Log IP parse failures instead of silently suppressing them

> **Executor instructions**: Follow this plan step by step. Run every
> verification command and confirm the expected result before moving to the
> next step. If anything in the "STOP conditions" section occurs, stop and
> report — do not improvise. When done, update the status row for this plan
> in `plans/README.md` — unless a reviewer dispatched you and told you they
> maintain the index.
>
> **Drift check (run first)**: `git diff --stat e55c781..HEAD -- crates/cognicode-core/src/interface/mcp/security.rs`
> If any in-scope file changed since this plan was written, compare the
> "Current state" excerpts against the live code before proceeding; on a
> mismatch, treat it as a STOP condition.

## Status

- **Priority**: P2
- **Effort**: S
- **Risk**: MED
- **Depends on**: none
- **Category**: security
- **Planned at**: commit e55c781, 2026-06-22
- **Issue**: omit

## Why this matters

At `security.rs:576`, `client_ip_str.parse::<IpAddr>().ok()?` silently returns `None` when IP parsing fails. This means malformed X-Forwarded-For headers cause the rate limiting function to return `None`, potentially bypassing IP-based protections without any visibility.

## Current state

`crates/cognicode-core/src/interface/mcp/security.rs:576`:
```rust
client_ip_str.parse::<IpAddr>().ok()?;
```

## Scope

**In scope**:
- `crates/cognicode-core/src/interface/mcp/security.rs` (line 576)

**Out of scope**:
- Other IP parsing locations
- Rate limiting logic itself

## Git workflow

- Branch: `fix/security-ip-parse-logging`
- Commit: `fix(core): log IP parse failures at debug level in security.rs`

## Commands

| Purpose | Command | Expected |
|---------|---------|----------|
| Check | `cargo check -p cognicode-core` | exit 0 |
| Tests | `cargo test -p cognicode-core --no-fail-fast` | pass |

## Steps

### Step 1: Replace .ok()? with logging

Replace line 576 with:
```rust
match client_ip_str.parse::<IpAddr>() {
    Ok(ip) => Some(ip),
    Err(e) => {
        tracing::debug!(%client_ip_str, error = %e, "failed to parse client IP from X-Forwarded-For");
        None
    }
}
```

**Verify**: `cargo check -p cognicode-core` → exit 0

## Test plan

- Run `cargo test -p cognicode-core --no-fail-fast` — behavior is unchanged (still returns None), just with logging
- No new tests needed — the panic path is not normally reachable

## Done criteria

- [ ] `cargo check -p cognicode-core` exits 0
- [ ] `cargo test -p cognicode-core --no-fail-fast` passes
- [ ] `grep -n 'parse::<IpAddr>().ok()' crates/cognicode-core/src/interface/mcp/security.rs` returns no matches at line 576
- [ ] No files outside scope modified
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:
- The `client_ip_str` variable is not available in scope at that location
- `tracing::debug` is not available in that module (check imports)

## Maintenance notes

This is a security hardening fix — the behavior (return None on parse failure) is preserved, but now there's diagnostic visibility into when it happens.
