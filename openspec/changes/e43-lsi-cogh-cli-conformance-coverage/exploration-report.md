# Exploration Report — e43 — cogh CLI conformance coverage

> Change: `e43-lsi-cogh-cli-conformance-coverage` | Phase: explore | Date: 2026-09-15

## Problem

The `openspec/specs/cognicode-cli/spec.md` capability carries **11 requirements** in the conformance corpus, all currently `no_evidence` (status not promoted to `verified`). The `cogh` binary exists at `crates/cognicode-cli/src/bin/cogh.rs` and dispatches 14 subcommands, but the crate has **no integration tests directory** (`crates/cognicode-cli/tests/` is absent). This means every contract documented in the spec is currently unenforceable at CI.

This is the only `cogh` capability listed in the conformance corpus that is wholly absent of tests; it represents a meaningful LSI/CP-N (Correctness Pressure) gap because:

- `cogh` is the single-binary CLI that distributes the entire CogniCode runtime (ADR-034, ADR-035).
- Without test coverage, any refactor to `cmd_*` handlers could silently break user-visible behaviour.
- 11 specs / 4.6 % of the corpus (`91.4%` → potentially `95.9%` if all verified) is a non-trivial compliance lift.

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/bin/cogh.rs` | `cogh` CLI dispatcher (clap-based; 14 subcommands) |
| `crates/cognicode-cli/src/cmd/layout.rs` | `cmd_list`, `cmd_current`, `cmd_doctor`, `cmd_where`, `cmd_init`, `cmd_reshim` |
| `crates/cognicode-cli/src/cmd/version.rs` | `cmd_version` |
| `crates/cognicode-cli/src/cmd/install.rs` | `cmd_install` (network-dependent; out of scope) |
| `crates/cognicode-cli/src/cmd/installer_transaction.rs` | install/uninstall transaction logic |
| `crates/cognicode-cli/src/cmd/registry.rs` | latest / update registry (network; not yet implemented for some queries) |
| `crates/cognicode-cli/src/cmd/lockfile.rs` | `.cognicode.lock` pin (network; not testable without fixtures) |

### Spec

`openspec/specs/cognicode-cli/spec.md` (11 requirements, 18 scenarios):

| # | Requirement | Testable today? |
|---|-------------|-----------------|
| 1 | `cogh` binary is a single static executable | **YES** (subprocess `cogh --version`) |
| 2 | `~/.cognicode/` layout mirrors `~/.asdf/` | partial (requires `cmd_init` over a temp home) |
| 3 | `~/.cognicode/shims/` regenerates on every install | NO (network: install downloads) |
| 4 | `cogh install` registers MCP server with IDEs | NO (network) |
| 5 | `cogh list` shows installed plugins and versions | **YES** (subprocess with temp home) |
| 6 | `cogh current` shows the active version pin | **YES** (subprocess with temp home) |
| 7 | `cogh latest <plugin>` queries the registry | partial (returns "not yet implemented" today) |
| 8 | `cogh update` resolves and installs latest | NO (network) |
| 9 | `cogh uninstall` removes a version cleanly | partial (local-file mutation) |
| 10 | `cogh doctor` validates installation | **YES** (subprocess with temp home) |
| 11 | `cogh` is curl-installable | NO (out of band; tests the installer script, not the binary) |

**Immediately testable (this cycle):** #1, #5, #6, #10.

`#2` is partially testable via `cogh init` (which creates the layout) but `init` requires `home.is_initialized() == false`; we'll exercise it via a temp home.

### Pre-cycle observation

The cycle proposes to lift these 4 specs to `verified` status (no production change, only test additions). Even just the 4 specs that are testable today are sufficient to close the most user-visible contract surfaces.

## Strategy

Write **integration tests** in `crates/cognicode-cli/tests/cogh_cli.rs` that:

1. Spawn the `cogh` binary using `std::process::Command` + the `CARGO_BIN_EXE_cogh` env var (Cargo-injected path to the test-time binary).
2. Override `--home <temp>` so each test runs against an isolated `~/.cognicode` fixture.
3. Assert stdout/stderr/exit-code per the spec scenarios.
4. Mirror the structure of `crates/cognicode-explorer/tests/api_landing_truncation.rs` (existing precedent for CLI-binary integration tests in this repo).

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Bumps conformance: 4 / 523 → `pct_verified` lifts from `91.4%` toward `92.2%`.
- Establishes the test harness that future cycles can extend (`#2`, `#9`).
- Closes the most user-visible contracts: `--version`, `list`, `current`, `doctor`.

## Scope non-goals

- Implementing `cogh latest` (currently returns "not yet implemented").
- Network-dependent specs (`#4`, `#7`, `#8`, `#11`) — they require either fixtures or live network.
- Spec `#2` layout assertion — useful but covered indirectly by `doctor`'s "✓ bin/ exists" / "✓ shims/ exists" lines.
