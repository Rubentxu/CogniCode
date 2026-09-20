## Why

Workspace clippy currently emits 93 `dead_code` warnings (after H5 phases 1–6
resolved 29 trivial lints across `collapsible_if`, `needless_borrow`,
`style-lints`, `trivial-std`, `unused-variable-prefix`, and `print_literal`).
Of these, 14 items have **exactly 1 workspace reference** (the declaration
itself) — definitive zero-use signal.

This cycle removes only those 14 strictly-zero-use items. No waivers, no
`#[allow(dead_code)]` attributes, no visibility downgrades.

## What changes

| Item | File | Kind |
|------|------|------|
| `DEFAULT_DOWNLOAD_BASE` | lifecycle_resolver.rs:66 | const |
| `detect_claude` | ide.rs:523 | fn |
| `download_to` | registry.rs:63 | fn |
| `download_with_rollback` | cache.rs:17 | fn |
| `exe_suffix` | release_contract.rs:90 | fn |
| `into_install_plan` | bundle_manifest.rs:382 | method |
| `journal_path_for_current` | lifecycle_journal.rs:52 | fn |
| `list_committed` | lifecycle_journal.rs:135 | fn |
| `read_claude_mcp_entry` | ide.rs:547 | fn |
| `read_opencode_config` | ide.rs:195 | fn |
| `sha256_bytes` | release_contract.rs:387 | fn |
| `time_decl` | authorizer.rs:139 | fn (mod tests) |
| `VersionMismatch` | error.rs:23 | enum variant |
| `write_manifest` | registry.rs:127 | fn |

## Evidence

Workspace-wide grep for each item yielded exactly 1 match (the declaration
itself). No internal consumer. Items appear in archived OpenSpec
`proposal.md` and `release-report.md` files as historical references — these
references describe the items as they were when the parent cycle closed,
not as current consumers.

## Impact

- Clippy `dead_code` warnings: 93 → 82 (−11 net; −14 line-items because
  some share an `associated items … are never used` report line).
- Test results: net-zero on CLI (286 cogh + 44 cognicode-release + 8
  portable_skill_bundle passing). The 50 pre-existing failures in
  `cognicode-core` lib tests and 4 in `cognicode-ladybug` lib tests are
  unrelated to these deletions (verified via `git stash` baseline).
- Files changed: 9 source files, 183 lines removed, 0 added.
- 0 `#[allow(clippy::*)]` introduced. 0 waivers. 0 force-push.
