# Archive of clippy-style-lints-bounded (H5 phase 3)

This folder was moved from `openspec/changes/clippy-style-lints-bounded/` on
2026-09-20 (ISO).

Original location: `openspec/changes/clippy-style-lints-bounded/`
Archived by: orchestrator (roadmap-completion initiative)
Archive date: 2026-09-20
Cycle: H5 phase 3 of the H5 clippy-hygiene program

## Implementation evidence

- Commit: `be61724b chore(lint): apply clippy style and trivial lints (H5 phase 3)`
- Author: Ruben <rubentxu@cognicode.dev>
- Date: 2026-09-20

## Scope recap

Seven (7) trivial clippy warnings fixed across five (5) files:

| File | Warning |
|---|---|
| `crates/cognicode-cli/src/cmd/doctor.rs` | `doc_list_item_overindented` × 4 |
| `crates/cognicode-cli/src/cmd/ide.rs` | `useless_vec` |
| `crates/cognicode-cli/src/cmd/ide.rs` | `assert_eq with literal bool` |
| `crates/cognicode-cli/src/cmd/ide.rs` | `match for single pattern` |
| `crates/cognicode-cli/tests/cogh_cli.rs` | `map_or can be simplified` |
| `crates/cognicode-core/src/domain/behaviors/class.rs` | `for_loop over single element` |
| `crates/cognicode-core/src/domain/findings/ports.rs` | `redundant_closure` |

Five files, +16 / −18 lines total. Behavior-preserving.

## Deferred (recorded as future cycles)

- `if_statement_can_be_collapsed` at
  `crates/cognicode-core/src/application/self_hosting/acceptance.rs:51`
  was applied and reverted (clippy doesn't catch `path` reuse across branches).
- `variant_name_starts_with_enum_name` at
  `crates/cognicode-cli/src/cmd/release_contract.rs:106,108`
  deferred to `clippy-serde-shaped-rename-bd` (breaks deserialization).
- `derefed_type_is_same_as_origin` at
  `crates/cognicode-cli/src/cmd/installer_transaction.rs:128`
  deferred to `clippy-installer-signature-cleanup-bd`.

## Release status (as captured in original proposal)

Release stalled in `RELEASE_PENDING` at `phase=release / sequence=4`. Workspace
version stayed at `0.97.2` (no PATCH bump committed during this phase).

## Delta specs

None — `chore(lint)`.

## Knowledge graph

No project-level findings registered.