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
