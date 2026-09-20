# Archive of clippy-dead-code-zero-ref-removal-bd (H5 phase 7)

This folder was moved from `openspec/changes/clippy-dead-code-zero-ref-removal-bd/`
on 2026-09-20 (ISO).

Original location: `openspec/changes/clippy-dead-code-zero-ref-removal-bd/`
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: H5 phase 7 of the H5 clippy-hygiene program

## Implementation evidence

- Commit: `dedc66cb chore(lint): remove 14 zero-use pub items (dead_code)`
- Author: Ruben <rubentxu@cognicode.dev>
- Date: 2026-09-20 14:45

## Scope recap

14 strictly-zero-use pub items removed across 9 files (183 lines removed, 0 added).
No waivers, no `#[allow(dead_code)]` attributes, no visibility downgrades.

| Item | File |
|---|---|
| `DEFAULT_DOWNLOAD_BASE` | lifecycle_resolver.rs:66 |
| `detect_claude` | ide.rs:523 |
| `download_to` | registry.rs:63 |
| `download_with_rollback` | cache.rs:17 |
| `exe_suffix` | release_contract.rs:90 |
| `into_install_plan` | bundle_manifest.rs:382 |
| `journal_path_for_current` | lifecycle_journal.rs:52 |
| `list_committed` | lifecycle_journal.rs:135 |
| `read_claude_mcp_entry` | ide.rs:547 |
| `read_opencode_config` | ide.rs:195 |
| `sha256_bytes` | release_contract.rs:387 |
| `time_decl` | authorizer.rs:139 (mod tests) |
| `VersionMismatch` | error.rs:23 |
| `write_manifest` | registry.rs:127 |

`dead_code` clippy warnings: 93 → 82 (−11 net; −14 line-items).

## Delta specs

None — `chore(lint)`.

## Knowledge graph

No project-level findings registered. Tested with net-zero on CLI (286 cogh +
44 cognicode-release + 8 portable_skill_bundle passing).