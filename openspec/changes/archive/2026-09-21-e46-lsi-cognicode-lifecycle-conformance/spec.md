# Spec — e46 — cognicode-lifecycle conformance

> Change: `e46-lsi-cognicode-lifecycle-conformance` | Phase: spec | Date: 2026-09-15

## Purpose

Lock down the user-visible lifecycle contracts of `cogh` for the
scenarios that are exercisable without network or registry, so they
reach `verified` status in the conformance corpus.

## Requirements

### Requirement: `cogh doctor` validates the install (healthy + uninitialised branches)

`cogh doctor --home <dir>` MUST emit the `==> cogh doctor (<dir>)`
header followed by a sequence of `✓`/`✗`/`⚠` markers reflecting the
home's filesystem state. On an uninitialised home, it MUST report
`home not initialized` and exit 0.

#### Scenario: `cogh doctor` PASS on healthy install

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh doctor --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains `cogh doctor`
- AND stdout contains `home exists`
- AND stdout contains `bin/ exists`
- AND stdout contains `shims/ exists`.

#### Scenario: `cogh doctor` FAIL on broken home

- GIVEN `--home` points to a fresh temp directory (no init)
- WHEN `cogh doctor --home <dir>` runs
- THEN exit status is `0` (the doctor never errors; it reports)
- AND stdout contains `home not initialized`.

#### Scenario: `cogh doctor` warns when tracker version missing

- GIVEN `--home` points to a temp directory after `cogh init`
- AND `tracker/version` does NOT exist on disk
- WHEN `cogh doctor --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains `tracker/version missing` (warn-level, not error).

### Requirement: `cogh reshim` reports the regeneration target

`cogh reshim --home <dir>` MUST emit a recognisable message that
identifies the path to regenerate. The current implementation is a
non-blocking no-op (the contract is the message shape).

#### Scenario: `cogh reshim` recreates shims

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh reshim --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains the substring `reshim`
- AND stdout references the `<dir>/shims` path.

### Requirement: `cogh uninstall` emits the recognised uninstall descriptor

`cogh uninstall <plugin> --version <v> --home <dir>` MUST emit a line
of the form `uninstall: plugin=<plugin> version=<v> ides=[...]`. The
current implementation is a stub; the contract is the message shape.

#### Scenario: `cogh uninstall` preserves other versions

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh uninstall mcp-server --version 0.92.0 --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains `uninstall: plugin=mcp-server version=0.92.0`.

### Requirement: `cogh list` and `cogh current` are lifecycle hooks

`cogh list --home <dir>` MUST render the installed-plugin table with
the `Plugin  Installed  Latest Available` header. `cogh current` MUST
report `(no version pinned)` when `tracker/version` is absent. Both
behaviours are asserted in their lifecycle-specific form (not
re-asserting the base CLI contract covered by e43).

#### Scenario: `cogh list` shows installed + available (lifecycle hook)

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh list --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains the `Plugin` header AND the `Installed` column
  AND the `Latest Available` column
- AND at least one row carries the `(installed)` marker.

#### Scenario: `cogh current` shows the pinned version (lifecycle hook)

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh current --home <dir>` runs
- THEN exit status is `0`
- AND stdout is `(no version pinned)` (the init step does not write
  a tracker version by default; this is the lifecycle-empty state).

## Out of scope (network-dependent)

The remaining 5 of 10 specs in `cognicode-lifecycle/spec.md`:

- REQ #1 `cogh install` is idempotent — requires download.
- REQ #2 `cogh install` is atomic — requires rollback-on-failure path.
- REQ #3 `cogh update` is reversible — requires download.
- REQ #5 `.cognicode.lock` pins project versions — requires install.
- REQ #6 `cogh update` respects the lock pin — requires update.

These belong to a future cycle that mocks the registry and the
versioned-install path.

The two unimplemented doctor checks (broken shim + plugin manifest
validity) are also deferred — they require production code change
in `cmd_doctor`, which is out of scope for this test-only cycle.
