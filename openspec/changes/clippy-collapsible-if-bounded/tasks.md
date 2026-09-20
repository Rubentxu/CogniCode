# Tasks: clippy-collapsible-if-bounded

## Task 1 — Apply collapsible_if collapses (10 files, 18 warnings)

For each of the 18 warnings, apply the clippy-suggested collapse.

### 1.1 — doctor.rs:141

File: `crates/cognicode-cli/src/cmd/doctor.rs`, line ~141
Original:
```rust
if let Some(r) = &c.remediation {
    if !matches!(c.status, CheckStatus::Pass) {
        ...
    }
}
```
Collapse to:
```rust
if let Some(r) = &c.remediation
    && !matches!(c.status, CheckStatus::Pass)
{
    ...
}
```

### 1.2 — ide.rs:217

File: `crates/cognicode-cli/src/cmd/ide.rs`, line ~217
Original:
```rust
if path.exists() {
    if let Ok(text) = std::fs::read_to_string(path) {
        ...
    }
}
```
Collapse.

### 1.3 — ide.rs:218 + 1.4 — ide.rs:219

These two warnings are the next level of the same nesting (line 217's
inner if-let into another if-let and another if-cond). Each is a
separate clippy suggestion; collapse each independently.

### 1.5 — ide.rs:511

Collapse the nested `cfg.get_mut("mcp").as_object_mut()` chain.

### 1.6 + 1.7 — ide.rs:779 + ide.rs:780

Collapse the triple-nested `if let Some(t)` / `if let Some(mcp_servers)`
/ `if let Some(mcp_table)` chain in `remove_codex_entry`. Note: this is
3 levels deep; clippy reports 2 collapsible_if warnings (the 779 and
780 pairs). The collapse produces a single `if let ... && let ... && let ...`
expression.

### 1.8 — ide.rs:1379

Same pattern as 1.5.

### 1.9 — installer_transaction.rs:91

Collapse `if !base.is_empty() && if let Some(rest) = ...`.

### 1.10 — installer_transaction.rs:103

Collapse `if let Ok(old) = std::env::var(...) && if !old.is_empty()`.

### 1.11 — layout.rs:897

Collapse `if let Ok(token) = std::env::var(...) && if !token.is_empty()`.

### 1.12–1.14 — lifecycle_resolver.rs:274, 279, 315

Three separate collapses in this file. Each is a different function.
Apply individually.

### 1.15 + 1.16 — platform_adapter.rs:147, 242

Two collapses in this file.

### 1.17 + 1.18 — rollback_journal.rs:328, 334

Two collapses in this file.

## Task 2 — Verification

### 2.1 — `cargo fmt --check`

Expected outcome: clean (no fmt diffs introduced).

### 2.2 — `cargo check --workspace --tests`

Expected outcome: clean compile.

### 2.3 — `cargo clippy -p cognicode-cli --bin cognicode-release --tests`

Expected outcome: collapsible_if count drops from 18 to 0; no new
warnings of any other category introduced.

### 2.4 — `cargo test -p cognicode-cli --bin cogh`

Expected outcome: same set of PASS results as parent commit. The
pre-existing failure `f3_t4_broken_same_version_install_is_repaired_
not_hidden` is acknowledged and is NOT introduced by this change.

## Task 3 — Commit

Single commit:
```
chore(lint): collapse collapsible_if (H5 phase 1)

Apply clippy's let_chains suggestion to 18 collapsible_if warnings
in 10 files of crates/cognicode-cli/src/cmd/. Behavior-preserving
refactor enabled by rustc 1.96 + edition 2024 stable let_chains.

Verification:
- cargo fmt --check: clean
- cargo check --workspace --tests: clean
- cargo clippy -p cognicode-cli --bin cognicode-release --tests:
  collapsible_if count 18 → 0
- cargo test -p cognicode-cli --bin cogh: PASS set unchanged
  (f3_t4_broken_same_version_install_is_repaired_not_hidden
  pre-existing, acknowledged)
- No #[allow(clippy::*)] introduced
- No AI attribution
- Conventional Commits format

Refs: REQ-COLLAPSIBLE-IF-01, REQ-COLLAPSIBLE-IF-02, REQ-COLLAPSIBLE-IF-03
```

## Task 4 — Verify report

Document each collapse (file, line, original, after) in
`openspec/changes/clippy-collapsible-if-bounded/verify-report.md`.

## Task 5 — Archive manifest

Create `openspec/changes/clippy-collapsible-if-bounded/archive-manifest.md`
linking the verify report to the commit hash, with closure date and
operational evidence.
