# CogniCode Lifecycle — install, update, uninstall, doctor

## Purpose

The complete lifecycle of a CogniCode installation: how `cogh`
manages versions, updates, and rollbacks across the user's
machine. Defines the semantics of every state-changing command
and the invariants that hold at each lifecycle stage.

## Requirements

### Requirement: `cogh install` is idempotent

Re-running `cogh install <plugin> --version <v>` for the same
plugin and version MUST be a no-op (no re-download, no re-extract,
no re-shim). The plugin marker in `~/.cognicode/plugins/<plugin>/`
indicates the installed version.

#### Scenario: `cogh install` is idempotent

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND the install succeeded
- WHEN `cogh install mcp-server --version 0.92.0` runs again
- THEN no download occurs
- AND no re-extract occurs
- AND shims are unchanged
- AND the install returns "0.92.0 already installed"

### Requirement: `cogh install` is atomic

If `cogh install` fails partway (download error, sha256 mismatch,
disk full), the install MUST roll back to the previous state. No
partial version directories are left behind.

#### Scenario: `cogh install` rolls back on download error

- GIVEN the network is unavailable
- WHEN `cogh install mcp-server --version 0.92.0` runs
- THEN the install fails with "network error: ..."
- AND `~/.cognicode/versions/0.92.0/` is NOT created
- AND the tracker is unchanged

#### Scenario: `cogh install` rolls back on sha256 mismatch

- GIVEN the manifest declares `sha256: abc123...`
- AND the artifact has a different hash
- WHEN `cogh install` runs
- THEN the partial version directory is removed
- AND the install aborts with "sha256 mismatch"
- AND the tracker is unchanged

### Requirement: `cogh update` is reversible

`cogh update <plugin>` MUST keep the previous version installed
until the new version is verified. The user can roll back with
`cogh install <plugin> --version <old-v>`.

#### Scenario: `cogh update` keeps previous version

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND the registry reports latest stable is `0.93.0`
- WHEN `cogh update mcp-server` runs
- THEN `~/.cognicode/versions/0.92.0/` is preserved
- AND `~/.cognicode/versions/0.93.0/` is created
- AND the user can roll back with `cogh install mcp-server --version 0.92.0`

### Requirement: `cogh uninstall` preserves other versions

`cogh uninstall <plugin> --version <v>` removes ONLY the specified
version. Other versions of the same plugin are preserved.

#### Scenario: `cogh uninstall` preserves other versions

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND `cogh install mcp-server --version 0.91.1` was run
- WHEN `cogh uninstall mcp-server --version 0.92.0` runs
- THEN `~/.cognicode/versions/0.92.0/` is removed
- AND `~/.cognicode/versions/0.91.1/` is preserved

### Requirement: `.cognicode.lock` pins project versions

`cogh install` MUST read `.cognicode.lock` in the project's root
and resolve `<plugin>` to the locked version unless `--version` is
explicitly given.

#### Scenario: `cogh install` reads `.cognicode.lock`

- GIVEN `.cognicode.lock` contains:
  ```yaml
  plugins:
    mcp-server: "0.92.0"
    skills-cognicode-core: "0.92.0"
  ```
- AND the user runs `cogh install` in the project root
- WHEN the install runs
- THEN `mcp-server` is installed at `0.92.0`
- AND `skills-cognicode-core` is installed at `0.92.0`

#### Scenario: `cogh install --version` overrides the lock

- GIVEN `.cognicode.lock` pins `mcp-server` at `0.92.0`
- WHEN `cogh install mcp-server --version 0.93.0` runs
- THEN `0.93.0` is installed (overriding the lock)
- AND `.cognicode.lock` is updated to `0.93.0`
- AND a warning is emitted: "lock updated to 0.93.0"

### Requirement: `cogh update` respects the lock pin

`cogh update <plugin>` MUST NOT update beyond the version pinned
in `.cognicode.lock`. If the user wants to update beyond the pin,
they must edit the lock file first.

#### Scenario: `cogh update` respects the lock

- GIVEN `.cognicode.lock` pins `mcp-server` at `0.92.0`
- AND the registry reports latest stable is `0.93.0`
- WHEN `cogh update mcp-server` runs
- THEN output is "version 0.92.0 is pinned by .cognicode.lock; refusing to update"
- AND no install happens

### Requirement: `cogh doctor` validates the install

`cogh doctor` runs a battery of checks and reports PASS / FAIL
for each. The user can use this to diagnose broken installs.

#### Scenario: `cogh doctor` PASS on healthy install

- GIVEN `cogh install mcp-server --version 0.92.0` was run
- AND all shims resolve to existing binaries
- AND all configured IDEs are present
- WHEN `cogh doctor` runs
- THEN output shows each check with PASS
- AND the exit code is 0

#### Scenario: `cogh doctor` FAIL on broken shim

- GIVEN `~/.cognicode/shims/cognicode-mcp` points to a missing binary
- WHEN `cogh doctor` runs
- THEN output shows "shim broken: cognicode-mcp"
- AND suggests "run `cogh install mcp-server` to repair"
- AND the exit code is 1

#### Scenario: `cogh doctor` checks plugin manifest validity

- GIVEN a plugin manifest with an invalid `sha256`
- WHEN `cogh doctor` runs
- THEN output shows "plugin mcp-server: sha256 invalid"
- AND exit code is 1

### Requirement: `cogh reshim` regenerates the shims directory

`cogh reshim [<plugin>]` regenerates `~/.cognicode/shims/`
based on the current tracker. Used when shims are manually deleted
or corrupted.

#### Scenario: `cogh reshim` recreates shims

- GIVEN `~/.cognicode/shims/cognicode-mcp` is missing
- AND `~/.cognicode/versions/0.92.0/mcp-server/bin/cognicode-mcp` exists
- WHEN `cogh reshim` runs
- THEN `~/.cognicode/shims/cognicode-mcp` is recreated
- AND points to the versioned binary

### Requirement: `cogh current` reads the tracker

`cogh current` reads `~/.cognicode/tracker/version` and prints it.
The tracker file is plain text.

#### Scenario: `cogh current` shows the pinned version

- GIVEN `~/.cognicode/tracker/version` contains `0.92.0`
- WHEN `cogh current` runs
- THEN output is `0.92.0`

### Requirement: `cogh list` shows installed plugins

`cogh list` outputs a table of installed plugins with their
installed version and the available versions.

#### Scenario: `cogh list` shows installed + available

- GIVEN `mcp-server 0.92.0` is installed
- AND the registry reports latest is `0.93.0`
- WHEN `cogh list` runs
- THEN output shows:
  ```
  Plugin          Installed        Latest Available
  mcp-server      0.92.0           0.93.0
  ```

## Cross-references

- `docs/specs/cognicode-cli/spec.md`
- `docs/specs/cognicode-plugin/spec.md`

## Implementation Log

- **2026-08-10 (E32-C plan)**: Spec drafted. Lifecycle semantics
  documented (idempotent install, atomic rollback, lock pin).

### Requirement: Rollback journal is a one-shot capability (DEBT-4)

The rollback journal (`~/.cognicode/journal/<version>.json`) is a
one-shot operational capability to undo ONE committed lifecycle
transition. It is NOT an audit log, NOT an install registry, and NOT
history. History/audit belongs to a future separate concept.

#### Scenario: applicability is explicit

- GIVEN version B is pinned in the tracker
- AND a journal for B exists
- WHEN `cogh rollback` runs
- THEN the journal for B is executed
- AND on success the journal is consumed (removed)
- AND a second `cogh rollback` reports "nothing applicable" without
  mutating any state

#### Scenario: no tracker means no implicit rollback

- GIVEN no version is pinned in the tracker
- AND journals exist on disk for several versions
- WHEN `cogh rollback` runs
- THEN it reports "nothing to roll back"
- AND no journal is selected by semver, lexicographic, or any other
  heuristic order
- AND no journal is executed or consumed

#### Scenario: stale journal fails closed

- GIVEN the tracker pins version B
- AND the journal file for B describes version C
- WHEN `cogh rollback` runs
- THEN the command fails with an explicit "stale journal" error
- AND no journal side-effect is executed

#### Scenario: uninstall invalidates the journal

- GIVEN version B is installed with a journal for B
- WHEN `cogh uninstall ... 0.95.0` (B) completes
- THEN the journal for B is removed
- AND journals of other versions are untouched
- AND if B was the active pin, the tracker is cleared (post-state is
  explicitly "no current version"; no automatic restore of a previous
  version)
- AND uninstalling a non-active version leaves the tracker untouched

#### Scenario: uninstall is idempotent

- GIVEN version B was already uninstalled
- WHEN `cogh uninstall ... 0.95.0` (B) runs again
- THEN it reports "not installed; nothing to do" and succeeds
- AND tracker/journal state is unchanged

#### Scenario: journal removal only after success

- GIVEN a rollback whose reversal fails partway
- WHEN the rollback errors
- THEN the journal file still exists (crash-safety ordering: consume
  the capability only AFTER the invalidating operation succeeded)

#### Scenario: deserialized journals are Drop-neutralized

- GIVEN a journal is loaded from disk (`lifecycle_journal::load` or
  `RollbackJournal::from_json`)
- WHEN the loaded journal is dropped without an explicit `rollback()`
  call
- THEN no side-effect is reversed
- (Architectural rule: a deserialized RollbackJournal MUST NOT retain
  armed Drop rollback behaviour; pinning is enforced by the tripwire
  `t_debt4_loaded_journal_is_drop_neutralized`.)

### Requirement: multiple journals may exist but at most one applies

Stale or historical journal files may accumulate (one per past
transition target), but exactly zero or one journal is operationally
applicable at any moment: the one whose version equals the tracker
pin.

#### Scenario: journal disagreement is never resolved by guessing

- GIVEN the tracker pin and any journal on disk disagree
- WHEN a rollback is attempted
- THEN the answer is deterministic: refuse (fail closed) if the
  journal file for the pinned version describes a different version,
  report "nothing applicable" if it is absent
- AND the disagreement is never resolved by picking a highest-version
  or most-recent journal
