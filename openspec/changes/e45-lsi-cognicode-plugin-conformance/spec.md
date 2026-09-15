# Spec — e45 — cognicode-plugin conformance

> Change: `e45-lsi-cognicode-plugin-conformance` | Phase: spec | Date: 2026-09-15

## Purpose

Lock down the user-visible contracts of `cogh plugin <subcommand>` and
`cogh init` for the two scenarios that are exercisable without network
or production filesystem mutation, so they reach `verified` status in
the conformance corpus.

## Requirements

### Requirement: `cogh init` installs bundled plugins

`cogh init --home <dir>` MUST install the bundled plugins shipped with
`cogh` into `<dir>/plugins/<name>/` and emit a success line per
plugin, plus a final `Installed N bundled plugin(s)` line. After
`init`, `cogh plugin list --home <dir>` MUST enumerate the bundled
plugins with their parsed `description` from `plugin.yaml`.

#### Scenario: `cogh init` installs bundled plugins

- GIVEN `--home` points to a fresh temp directory
- WHEN `cogh init --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains the substring `Installed ` followed by a digit
  AND stdout contains `bundled plugin(s)`.

#### Scenario: bundled plugins are listed after init

- GIVEN `cogh init --home <dir>` has populated `<dir>/plugins/`
- WHEN `cogh plugin list --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains the substring `Plugin`
- AND stdout contains the substring `mcp-server` (the canonical bundled plugin).

### Requirement: `plugin.yaml` is parsed end-to-end

`cogh plugin list --home <dir>` MUST read each plugin's
`plugin.yaml` and print its `description`. If `plugin.yaml` is missing
or malformed, `cogh plugin list` MUST fall back to an empty
description (graceful degradation) and continue.

#### Scenario: `plugin.yaml` parses with cogh

- GIVEN a plugin directory `<dir>/plugins/custom/plugin.yaml` with a
  valid manifest (apiVersion `cognicode/v1`, name, description, at
  least one version with sha256)
- WHEN `cogh plugin list --home <dir>` runs
- THEN exit status is `0`
- AND stdout contains the plugin's name
- AND stdout contains the plugin's `description`.

#### Scenario: Missing plugin manifest falls back gracefully

- GIVEN a plugin directory `<dir>/plugins/empty/` with no `plugin.yaml`
- WHEN `cogh plugin list --home <dir>` runs
- THEN exit status is `0`
- AND stdout does NOT abort, just prints the plugin's name with an
  empty description (the listing continues).

#### Scenario: `cogh plugin add` rejects unknown non-bundled names

- GIVEN the plugin name `not-a-real-plugin` is not in the bundled set
- WHEN `cogh plugin add not-a-real-plugin --home <dir>` runs
- THEN exit status is non-zero
- AND stderr mentions `not bundled` (or equivalent: the user is told
  to pass `--from-url`).

## Out of scope (network-dependent)

The remaining 2 of 4 specs in `cognicode-plugin/spec.md`:

- **REQ #2** "Versions are addressable by ref" — covered by the 4 unit
  tests in `manifest.rs::tests` plus the install path which requires
  `cogh install` (network).
- **REQ #3** "sha256 integrity check is mandatory" — covered by
  `manifest.rs::tests::reject_invalid_sha256` (unit); the
  end-to-end install + verify path is network-dependent.

These belong to a future cycle that mocks the registry.
