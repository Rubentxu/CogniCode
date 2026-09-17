# Spec: rollback-journal-populated-dir + shim-reinstall + empty-profile

> e86.1 WU1. Operational authority is `state.yaml`.

## Purpose

The e86 followup shipped three regressions, all "pinned, not
fixed". All three reproduce on `HEAD`. This spec fixes them and
tightens the tests to assert the correct behaviour.

## Scope

In scope:

- `rollback_journal::SideEffect::CreatedDir` reversal uses
  `remove_dir_all` (not `remove_dir`), and is **idempotent** (a
  sibling reversal that already removed the path is a no-op).
- `LinuxAdapter::install_shim` and `MacOsAdapter::install_shim`
  detect an existing symlink before `symlink`-ing. If the link
  points at the same target, no-op. If it points elsewhere, or
  is not a link, remove and re-link. `WindowsAdapter` was
  already idempotent via `fs::copy`.
- `installer_transaction::run` returns
  `InstallerError::EmptyInstall { profile, version }` when the
  filtered component set is empty (BEFORE the commit path), so
  no manifest, tracker, or journal is written for a no-op
  install.
- `cmd_update` matches on the new variant and surfaces it as the
  install error.
- Tests: drop `#[ignore]` on the zero-component pinned test;
  replace `cmd_rollback_after_live_install`'s `expect_err` with
  `expect`; add unit tests for the new `CreatedDir` reversal
  semantics; add a strict-success tripwire test for sequential
  installs.

Out of scope:

- e85 (real artifact publishing).
- e87 (external channels).
- e88 (real Linux install UAT).
- Control Plane / Backstage.
- New MCP tools.
- Skill install (`cogh skill install`).
- Architecture execution (e77).

## Requirements

### REQ-LJ-04 — empty-profile install refuses

**When** `InstallerTransaction::run` filters the bundle
manifest's `components` by the user-supplied `--profile` AND the
resulting filtered set is empty,
**then** `run` MUST return
`InstallerError::EmptyInstall { profile, version }`
WITHOUT writing the install manifest, the lifecycle journal, or
the tracker.
**And** the `cmd_update` user-facing command MUST surface the
error with a clear message containing the profile name.

### REQ-LJ-05 — `CreatedDir` reversal removes populated dir

**When** `RollbackJournal::rollback` reverses a
`SideEffect::CreatedDir(path)` AND `path` is a non-empty
directory whose contents are not individually journaled,
**then** the reversal MUST succeed (using `remove_dir_all`,
not the empty-only `remove_dir`).

### REQ-LJ-06 — `CreatedDir` reversal is idempotent

**When** `RollbackJournal::rollback` reverses a
`SideEffect::CreatedDir(path)` AND `path` does not exist
(because a sibling reversal — typically a later
`CreatedDir(path)` for the same path — already removed it),
**then** the reversal MUST be a no-op (not an `ENOENT` error).

### REQ-LJ-07 — strict rollback-after-live-install test

The test `cmd_rollback_after_live_install` MUST assert that
`cmd_rollback` returns `Ok(())` (not `Err`) after a successful
`cmd_update`. The install manifest and lifecycle journal MUST
both be removed by the rollback.

### REQ-LJ-08 — strict zero-component-profile test

The test
`cmd_update_zero_component_profile_returns_empty_install_error`
MUST assert that `cmd_update --profile no-such-profile` returns
`Err(EmptyInstall)`, AND that the tracker, lifecycle journal,
and install manifest are NOT written.

### REQ-LJ-09 — sequential install succeeds (strict tripwire)

The test `cmd_update_sequential_installs_succeed_cleanly`
MUST assert that a second `cmd_update` against the same
`COGNICODE_HOME` (with the same profile and version) returns
`Ok(())`. The bug: a previous install leaves a symlink at
`home.shims/<bin>`, and `std::os::unix::fs::symlink` returns
`EEXIST` on the second install.

### REQ-LJ-10 — `install_shim` is idempotent on re-install

**When** `LinuxAdapter::install_shim` (or `MacOsAdapter`) is
called with a `shim_path` that already exists,
**then** the adapter MUST:

1. Read the existing link with `std::fs::read_link`.
2. If the existing link's target equals `bin_path`, return
   `Ok(ShimSideEffect::Symlinked { link, target })` without
   touching the filesystem further.
3. Otherwise (different target, OR `shim_path` is not a
   symlink), remove `shim_path` and re-symlink.

The `WindowsAdapter` is already idempotent via `fs::copy` (which
overwrites) and is exempt from this requirement.

## Scenarios

### S-LJ-04-A — typo'd profile fails loudly

**Given** a working `COGNICODE_HOME` and a bundle manifest
whose `core` profile has at least one component
**When** `cogh update --profile core-typo` is run
**Then** the command exits non-zero with a message containing
"matches no components" or "no-such-profile"
**And** `~/.cognicode/tracker/version` does not exist
**And** `~/.cognicode/journal/<version>.json` does not exist
**And** `~/.cognicode/install/<version>/manifest.yaml` does not
exist

### S-LJ-05-A — rollback reverses populated install dir

**Given** a `RollbackJournal` with one `CreatedDir(install/0.95.0/)`
side-effect
**And** `install/0.95.0/` contains un-journaled children
(manifest, extracted binaries, shims)
**When** `rollback()` is called
**Then** the call returns `Ok(())`
**And** `install/0.95.0/` is removed (recursively)

### S-LJ-06-A — rollback is idempotent for double-CreatedDir

**Given** a `RollbackJournal` with two `CreatedDir(install/0.95.0/)`
side-effects (the recording pipeline records the same path
twice: once for the install dir during extracting, once for the
manifest parent during writing-manifest)
**And** `install/0.95.0/` contains a file
**When** `rollback()` is called
**Then** the call returns `Ok(())`
**And** `install/0.95.0/` is removed

### S-LJ-07-A — live install then rollback succeeds

**Given** a working `COGNICODE_HOME` and a staging fixture for
v0.95.0
**When** `cmd_update --profile core` is run, then
`cmd_rollback`
**Then** `cmd_update` returns `Ok(())`
**And** `~/.cognicode/install/0.95.0/manifest.yaml` exists
**And** `~/.cognicode/journal/0.95.0.json` exists
**And** after `cmd_rollback`, both files are removed
**And** `cmd_rollback` returns `Ok(())`

### S-LJ-09-A — sequential install succeeds

**Given** a working `COGNICODE_HOME` and a staging fixture for
v0.95.0
**When** `cmd_update --profile core` is run twice in a row
**Then** the first call returns `Ok(())`
**And** the second call returns `Ok(())`
**And** the tracker still reads `0.95.0`
