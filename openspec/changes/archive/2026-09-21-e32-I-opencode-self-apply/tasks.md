# Tasks: e32-I-opencode-self-apply

Fix `cogh install --ide opencode` to dispatch to the OpenCode adapter
(writes absolute shim path) even when `--profile` is set. Document
recipe in `CONTEXT.md`; verify on temp HOME before touching the real
`~/.config/opencode/opencode.json`.

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~150–200 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | single-pr |
| Chain strategy | pending |

Decision needed before apply: Yes
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

Files: `cogh.rs`, `install.rs`, `ide.rs`, `lifecycle.rs`, `CONTEXT.md`. Under 400 lines → `single-pr`.

## Phase 1: Foundation — shared shim path + dispatch order

- [ ] 1.1 Add `shim_path(binary: &str) -> PathBuf` to `CognicodeHome` in `crates/cognicode-cli/src/cmd/layout.rs` returning `home.shims().join(binary)`.
- [ ] 1.2 In `install.rs`, replace inline `dirs::home_dir().unwrap_or(...).join(".cognicode/shims/cognicode-mcp")` (lines 50-56) with `home.shim_path("cognicode-mcp")`; thread `home` via `CognicodeHome::resolve`.
- [ ] 1.3 In `cogh.rs` `Command::Install` arm (lines 204-235), always call `install::run_install(&profile)` first, then iterate `ide` to dispatch `ide::cmd_ide_install` — never drop `--profile` when `--ide` is set.

## Phase 2: Core — test coverage + pre-existing failure

- [ ] 2.1 RED: in `lifecycle.rs`, strengthen `test_self_apply_opencode_adapter` (line 800) to assert `command[0]` equals `tmp.join(".cognicode/shims/cognicode-mcp")`.
- [ ] 2.2 GREEN: if 2.1 fails, patch `cmd_ide_install` in `ide.rs`.
- [ ] 2.3 Add `test_install_with_ide_and_profile_dispatches_both` in `lifecycle.rs` — run `cogh install --ide opencode --profile core` via `run_cogh` on temp HOME; assert tracker updated AND `opencode.json` has `cognicode-mcp`.
- [ ] 2.4 Investigate `test_clean_home_install` failure (line 488): confirm `install::run_install` honors `COGNICODE_HOME`. If 1-2 line fix, apply; if deeper, punt and document in PR description.

## Phase 3: Integration — apply on temp HOME

- [ ] 3.1 Hand-verify on temp HOME: run `cogh install --ide opencode --profile core` against throwaway `/tmp` home, confirm `opencode.json` contains `command[0] = <tmp>/.cognicode/shims/cognicode-mcp` and tracker updated.
- [ ] 3.2 Capture temp HOME check output in PR description (exact shim path written).
- [ ] 3.3 Flag real `~/.config/opencode/opencode.json` apply as user-driven step (do NOT auto-apply) — call out in PR description so user runs it after review.

## Phase 4: Documentation

- [ ] 4.1 In `CONTEXT.md` (Spanish, ephemeral, do NOT push), add E32-I recipe: (a) `cogh install --ide opencode --profile core` flow, (b) absolute shim path rationale, (c) binary-resolution gotcha.
- [ ] 4.2 Cross-link from new `CONTEXT.md` section to `openspec/changes/e32-I-opencode-self-apply/spec.md`.

## Phase 5: Cleanup

- [ ] 5.1 `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` — fix new lints.
- [ ] 5.2 Confirm `cogh.rs` keeps `--bin cogh` autobins=false. Verify no `docs/ROADMAP.md`, `docs/adr/**`, or `CONTEXT.md` staged for remote (ephemeral docs per `AGENTS.md`).

## Spec traceability

| Spec scenario | Tasks |
|---|---|
| `cogh install --ide opencode` dispatches + patches | 1.3, 2.1, 2.2, 3.1 |
| `cogh install --ide zcode\|claude\|codex\|all` | 1.3 (regression) |
| opencode integrate writes absolute shim path | 1.2, 2.1, 2.2 |
| opencode integrate overwrites stale entry | 2.1 (assertion) |

## Out of scope

- ZCode / Claude / Codex adapters — regression check only.
- `cogh uninstall --ide opencode` — spec scopes only install.
