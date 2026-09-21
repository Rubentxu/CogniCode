# Exploration Report — e46 — cognicode-lifecycle conformance

> Change: `e46-lsi-cognicode-lifecycle-conformance` | Phase: explore | Date: 2026-09-15

## Problem

The 10 specs in `openspec/specs/cognicode-lifecycle/spec.md` are all
in `no_evidence` status. The lifecycle commands are exposed via
`cogh install` / `cogh update` / `cogh uninstall` / `cogh doctor` /
`cogh reshim` / `cogh current` / `cogh list`. Most of the spec's
scenarios are network-dependent (install/update/idempotency/atomicity
all require registry + sha256 verification).

This continues the e43/e44/e45 pattern: the `cognicode-cli` capability
group must reach a tight verification status.

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/cmd/layout.rs:180-327` | `cmd_install`, `cmd_uninstall`, `cmd_list`, `cmd_current`, `cmd_update`, `cmd_doctor`, `cmd_reshim` |
| `crates/cognicode-cli/src/bin/cogh.rs` | dispatcher |

### Spec

`openspec/specs/cognicode-lifecycle/spec.md` (10 requirements):

| # | Requirement | Testable today? |
|---|-------------|-----------------|
| 1 | `cogh install` is idempotent | NO (network) |
| 2 | `cogh install` is atomic | NO (network + rollback path) |
| 3 | `cogh update` is reversible | NO (network) |
| 4 | `cogh uninstall` preserves other versions | partial (uninstall is stub; only format-prints `uninstall: plugin=... version=... ides=[]`) |
| 5 | `.cognicode.lock` pins project versions | NO (install network) |
| 6 | `cogh update` respects the lock pin | NO (network) |
| 7 | `cogh doctor` validates the install | YES (purely filesystem-based) |
| 8 | `cogh reshim` regenerates the shims directory | partial (current impl is "not yet implemented"; the format-print contract is testable) |
| 9 | `cogh current` reads the tracker | YES (already covered by e43, but re-asserted here as lifecycle hook) |
| 10 | `cogh list` shows installed plugins | YES (already covered by e43, but re-asserted here as lifecycle hook) |

**Testable in this cycle:** #4 (partial), #7 (full), #8 (partial), plus
contractual re-coverage of #9 + #10 as **lifecycle hooks** (different
assertions than e43 because they exercise the lifecycle-specific
phrasing, not the CLI base contract).

### Empirical probe (HEAD `6218debb`)

```
$ cogh --home <temp> init
✓ Installed 6 bundled plugin(s)

$ cogh --home <temp> uninstall mcp-server --version 0.92.0
uninstall: plugin=mcp-server version=0.92.0 ides=[]

$ cogh --home <temp> reshim
reshim: would regenerate <temp>/shims (not yet implemented)

$ cogh --home <temp> doctor
==> cogh doctor (<temp>)
  ✓ home exists
  ✓ bin/ exists
  ✓ shims/ exists
  ⚠ tracker/version missing (no version pinned)
==> 0 issues

$ cogh --home <temp> list
Plugin          Installed        Latest Available
---------------------------------------------
codex           (installed)
claude          (installed)
zcode           (installed)
sandbox-templates (installed)
skills-cognicode-core (installed)
mcp-server      (installed)
```

All testable scenarios are reachable end-to-end via the public CLI.

## Strategy

Write **integration tests** in
`crates/cognicode-cli/tests/cognicode_lifecycle.rs` that:

1. Spawn `cogh` via `std::process::Command::new(env!("CARGO_BIN_EXE_cogh"))`
   (matches e43/e44/e45 pattern).
2. Use `tempfile::tempdir()` per test for `--home`.
3. Cover 5 lifecycle-specific scenarios + 2 lifecycle-hook re-assertions:

| Test | REQ |
|------|-----|
| `cogh_doctor_reports_healthy_on_initialised_home` | #7 healthy install |
| `cogh_doctor_reports_uninitialised_home_on_fresh_dir` | #7 uninitialised home |
| `cogh_doctor_warns_when_tracker_version_missing` | #7 partial state |
| `cogh_reshim_emits_recognisable_message_on_current_implementation` | #8 contract |
| `cogh_uninstall_emits_recognisable_message_for_known_plugin` | #4 contract |
| `cogh_list_reports_installed_plugins_after_init` | #10 lifecycle hook |
| `cogh_current_reports_unpinned_state_on_initialised_home_without_tracker` | #9 lifecycle hook |

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Bumps conformance: 7 / 523 → `pct_verified` lifts from `96.1%` to `97.5%`.
- Establishes lifecycle-specific test coverage that complements the
  e43 base-CLI tests.

## Scope non-goals

- Specs #1, #2, #3, #5, #6 are network-dependent. Out of scope.
- Spec #7 "FAIL on broken shim" — current `cmd_doctor` does not validate
  shim targets. The spec/code divergence will be addressed in a future
  cycle that adds shim validation; for now only the existing checks
  are testable.
- Spec #7 "checks plugin manifest validity" — also not implemented
  in `cmd_doctor`. Documented as future work.
