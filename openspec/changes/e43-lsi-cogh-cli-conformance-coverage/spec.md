# Spec — e43 — cogh CLI conformance coverage

> Change: `e43-lsi-cogh-cli-conformance-coverage` | Phase: spec | Date: 2026-09-15

## Purpose

Lock down the user-visible contracts of the `cogh` CLI for the four
scenarios that are exercisable without network or production
filesystem mutation, so they reach `verified` status in the
conformance corpus.

## Requirements

### Requirement: `cogh --version` reports the binary version

`cogh --version` MUST print a single line containing the literal token `cogh` followed by the binary version reported by `CARGO_PKG_VERSION`. The exact version string is environment-dependent; the test only asserts the `cogh <semver>` shape.

#### Scenario: `cogh --version` reports the binary version

- GIVEN `cogh` is installed at the path injected by `CARGO_BIN_EXE_cogh`
- WHEN the user runs `cogh --version`
- THEN stdout contains `cogh ` followed by a semver-looking token (e.g. `cogh 0.94.15`)
- AND exit status is `0`.

### Requirement: `cogh list` renders the plugin table

`cogh list --home <temp>` MUST render a three-column table with header `Plugin  Installed  Latest Available` even when the home is uninitialised (printing `(home not initialized)`). When initialised it MUST list the installed plugins.

#### Scenario: `cogh list` on an uninitialised home

- GIVEN `--home` points to a fresh temp directory
- WHEN `cogh list --home <temp>` runs
- THEN stdout contains the substring `(home not initialized)`
- AND exit status is `0`.

#### Scenario: `cogh list` on an initialised home renders the table

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh list --home <temp>` runs
- THEN stdout contains the substring `Plugin`
- AND stdout contains the substring `Installed`
- AND exit status is `0`.

### Requirement: `cogh current` reads the tracker

`cogh current --home <temp>` MUST read `~/.cognicode/tracker/version` and print its contents (trimmed). When the tracker file is absent, it MUST print `(no version pinned)`.

#### Scenario: `cogh current` with no pinned version

- GIVEN `--home` points to a fresh temp directory
- WHEN `cogh current --home <temp>` runs
- THEN stdout is exactly `(no version pinned)` (possibly with trailing whitespace)
- AND exit status is `0`.

#### Scenario: `cogh current` reads the tracker

- GIVEN `--home` points to a temp directory whose `tracker/version` contains `0.92.0`
- WHEN `cogh current --home <temp>` runs
- THEN stdout (trimmed) is `0.92.0`
- AND exit status is `0`.

### Requirement: `cogh doctor` validates installation

`cogh doctor --home <temp>` MUST print the home path, then a sequence of `✓`/`✗` markers reflecting directory state, then a summary line with the issue count.

#### Scenario: `cogh doctor` on an uninitialised home

- GIVEN `--home` points to a fresh temp directory
- WHEN `cogh doctor --home <temp>` runs
- THEN stdout contains the substring `cogh doctor`
- AND stdout contains `home not initialized`
- AND exit status is `0`.

#### Scenario: `cogh doctor` on an initialised home

- GIVEN `--home` points to a temp directory after `cogh init`
- WHEN `cogh doctor --home <temp>` runs
- THEN stdout contains `home exists`
- AND stdout contains `bin/ exists`
- AND stdout contains `shims/ exists`
- AND exit status is `0`.
