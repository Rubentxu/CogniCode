# Archive of clippy-needless-borrow-bounded (H5 phase 2)

This folder was moved from `openspec/changes/clippy-needless-borrow-bounded/` on
2026-09-20 (ISO).

Original location: `openspec/changes/clippy-needless-borrow-bounded/`
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: H5 phase 2 of the H5 clippy-hygiene program

## Implementation evidence

- Commit: `344ff66b chore(lint): remove needless_borrow (H5 phase 2)`
- Author: Ruben <rubentxu@cognicode.dev>
- Date: 2026-09-20

## Scope recap

9 borrow-related warnings eliminated (8 `needless_borrow` + 1 `borrowed_expression`):
- `crates/cognicode-cli/src/cmd/lifecycle.rs:158`
- `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs:537,557,580,603,647,675,718`
- `crates/cognicode-cli/src/cmd/release_test_support.rs:144`

Behavior-preserving; no `#[allow(clippy::*)]` introduced; one bounded commit.

## Delta specs

None — this is a `chore(lint)` cycle. No durable spec additions or modifications.

## Knowledge graph

No project-level findings registered. The H5 program is local bookkeeping for
clippy hygiene; durable knowledge is captured in `.agent/TESTING-STATE.md`
under the H4.7 / H5 section.