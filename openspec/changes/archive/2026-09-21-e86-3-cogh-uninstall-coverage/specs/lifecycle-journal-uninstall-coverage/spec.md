# Spec: cogh uninstall regression coverage (e86.3)

> Cycle: `p-c1fac1fea05615c6/e86-3-cogh-uninstall-coverage`
> Operational authority: `state.yaml`

## Purpose

e86.1 remediated three pinned bugs in `cogh install` + `cogh rollback`:

1. rollback populated `CreatedDir` (uses `remove_dir_all`, idempotent)
2. stale shim on sequential install (LinuxAdapter/MacOsAdapter detect
   existing symlink and no-op on match)
3. zero-component profile silent install
   (`InstallerError::EmptyInstall`)

The `cogh uninstall` command had only ONE regression test
(`uninstall_opencode_ide_removes_entry_and_skills`). This cycle
delivers a parity test suite so the three bug patterns above (and
two uninstall-specific failure modes) cannot fire without a
deterministic pin.

## Requirements

### REQ-LJ-11 — uninstall fails loudly on uninitialized home

**When** `cmd_uninstall` runs against a `CognicodeHome` whose
`is_initialized()` returns false (no `.cognicode/bin/` layout
planted by `cogh init`),
**then** it MUST return an error whose message contains
`"not initialized"` (or `"init"`) and the command MUST exit
non-zero.
**And** no filesystem mutation MUST occur (no partial install
tree removed, no journal written, no tracker cleared).

#### Scenario: T1 — uninstall on uninitialized home

- GIVEN no `~/.cognicode/` exists (no `cogh init` was run)
- WHEN `cogh uninstall mcp-server --ide opencode --version 0.95.0` runs
- THEN the command exits non-zero
- AND stderr contains "not initialized" or "init"
- AND `~/.cognicode/versions/0.95.0/` is NOT created
- AND no journal file is written
- AND the tracker is unchanged

### REQ-LJ-12 — uninstall is idempotent across repeated calls

**When** `cmd_uninstall` runs successfully for version `v` and
the same call is issued again immediately,
**then** the second call MUST succeed (exit 0) without erroring
on already-removed files, already-removed skills, or
already-removed config entries.
**And** the second call MUST be a true no-op — no extra
filesystem operations beyond those strictly necessary to determine
"already gone".

#### Scenario: T2 — double uninstall is a clean no-op

- GIVEN `cogh install mcp-server --ide opencode --version 0.95.0` ran
- AND `~/.config/opencode/opencode.json` contains a `cognicode-mcp`
  entry under `mcp`
- AND `~/.config/opencode/skills/cognicode-0.95.0/SKILL.md` exists
- WHEN `cogh uninstall mcp-server --ide opencode --version 0.95.0`
  runs twice
- THEN both calls exit 0
- AND after the first call: config entry gone, skills dir gone,
  install tree gone
- AND after the second call: state is identical to post-first-call
  (no errors, no re-creations)

### REQ-LJ-13 — uninstall handles a missing IDE config file cleanly

**When** `cmd_uninstall` runs against a home that has an
installed version but no IDE config file (e.g. opencode.json is
absent because opencode was never used),
**then** the command MUST succeed (exit 0).
**And** the install tree MUST be removed.
**And** the journal MUST be removed.
**And** the tracker MUST be cleared if the uninstalled version was
active.

#### Scenario: T3 — uninstall with no IDE config present

- GIVEN `cogh init` ran
- AND `~/.cognicode/versions/0.95.0/` exists (planted by test)
- AND `~/.config/opencode/opencode.json` does NOT exist
- WHEN `cogh uninstall mcp-server --ide opencode --version 0.95.0` runs
- THEN the command exits 0
- AND `~/.cognicode/versions/0.95.0/` is removed
- AND no error is logged about the missing config

### REQ-LJ-14 — uninstall without `--ide` prints a diagnostic

**When** `cmd_uninstall` is invoked without any `--ide` flag,
**then** the command MUST produce a diagnostic (stdout or stderr)
that mentions `--ide` or `no IDE` (so the user knows what to add).
**And** the command MUST exit non-zero (this is a user-input
error, not a no-op).

#### Scenario: T4 — uninstall without `--ide`

- GIVEN `cogh init` ran
- WHEN `cogh uninstall mcp-server --version 0.95.0` runs (no `--ide`)
- THEN the command exits non-zero
- AND stdout or stderr contains `--ide` or `no IDE`
- AND no install tree is removed

### REQ-LJ-15 — uninstall with unknown `--ide` fails cleanly

**When** `cmd_uninstall` is invoked with `--ide <name>` where
`<name>` is not in the supported set (`opencode`, `zcode`,
`claude`, `codex`),
**then** the command MUST exit non-zero without panicking.
**And** the error message MUST name a supported IDE
(`opencode`, `zcode`, `claude`, `codex`) OR contain the literal
phrase `"not supported"`.

#### Scenario: T5 — uninstall with unknown IDE name

- GIVEN `cogh init` ran
- AND `~/.cognicode/versions/0.95.0/` exists
- WHEN `cogh uninstall mcp-server --ide vscode --version 0.95.0` runs
- THEN the command exits non-zero
- AND the error message names `opencode` (or one of the supported
  IDEs) or contains `"not supported"`
- AND `~/.cognicode/versions/0.95.0/` is unchanged (no partial
  removal)

## Out of scope

- Removing `uninstall_opencode_ide_removes_entry_and_skills` (stays
  additive).
- Changes to `cmd_uninstall`'s contract (REMOVE semantics unchanged).
- Real-PC UAT (e86.2 owns that).
- Rollback-to-version support (e86.4 owns that).
- Generalising `cmd_uninstall` to be a full reverse of any install
  (e86.4's scope).

## Cross-references

- Pinning test harness: `crates/cognicode-cli/src/cmd/lifecycle.rs`
  `tests::t_e86_3_*` (5 tests, all green at HEAD)
- Sequencer: `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md`,
  `openspec/changes/e86-4-cogh-rollback-select-version/proposal.md`
- Predecessor: `openspec/changes/archive/2026-09-17-e86-1-cogh-pinned-bug-remediation/specs/lifecycle-journal-populated-dir-empty-profile/spec.md`
  (REQ-LJ-04..10 — same shape, install/rollback side)
