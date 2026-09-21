# Spec — e44 — portable-skill-bundle conformance

> Change: `e44-lsi-portable-skill-bundle-conformance` | Phase: spec | Date: 2026-09-15

## Purpose

Lock down the user-visible contracts of `cogh skill validate <path>`
for the four scenarios that are exercisable without network or
production filesystem mutation, so they reach `verified` status in
the conformance corpus.

## Requirements

### Requirement: `cogh skill validate <path>` accepts a portable directory

`cogh skill validate <path>` MUST accept a directory whose contents are a portable skill bundle (manifest.yaml + SKILL.md, with optional references/ and assets/), and report validity on stdout. If the path is not a directory, it MUST exit non-zero with an error mentioning "not a directory".

#### Scenario: Skill bundle loads

- GIVEN a directory containing `manifest.yaml` and `SKILL.md`
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is `0`
- AND stdout contains `✓ skill bundle valid`
- AND stdout contains the bundle name
- AND stdout contains the bundle version.

#### Scenario: Non-directory path is rejected

- GIVEN `<path>` points to a non-existent or non-directory file
- WHEN `cogh skill validate <path>` runs
- THEN exit status is non-zero
- AND stderr contains `not a directory`.

### Requirement: `manifest.yaml` is required and parses

`cogh skill validate <dir>` MUST require `manifest.yaml` at the bundle root. The manifest MUST be parseable as YAML with the `SkillManifest` shape (`apiVersion`, `name`, `description`, `version`, `maturity`).

#### Scenario: `manifest.yaml` declares bundle metadata

- GIVEN a bundle with a valid `manifest.yaml`
- WHEN `cogh skill validate <dir>` runs
- THEN stdout contains the `description`
- AND stdout contains the `maturity` value (one of `experimental`/`beta`/`stable`/`deprecated`).

#### Scenario: Missing `manifest.yaml` is rejected

- GIVEN a bundle directory without `manifest.yaml`
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is non-zero
- AND stderr contains `missing manifest.yaml`.

### Requirement: `SKILL.md` is required and the frontmatter is valid

`cogh skill validate <dir>` MUST require `SKILL.md` at the bundle root. The bundle's mandatory metadata fields (`name`, `maturity`) MUST be valid (non-empty `name`; `maturity` one of `experimental`/`beta`/`stable`/`deprecated`).

#### Scenario: Missing `SKILL.md` is rejected

- GIVEN a bundle directory without `SKILL.md`
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is non-zero
- AND stderr contains `missing SKILL.md`.

#### Scenario: Invalid `maturity` is rejected

- GIVEN a manifest with `maturity: bogus`
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is non-zero
- AND stderr contains `maturity must be one of`.

### Requirement: Portable bundle has no IDE-specific fields

`cogh skill validate <dir>` MUST reject any bundle whose `SKILL.md` frontmatter contains `compatibility: opencode` (or any other IDE-locked field).

#### Scenario: Portable bundle is IDE-agnostic

- GIVEN a bundle whose `SKILL.md` frontmatter contains `compatibility: opencode`
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is non-zero
- AND stderr contains `IDE-specific 'compatibility: opencode'`.

### Requirement: Referenced scripts and assets must exist

`cogh skill validate <dir>` MUST verify that every entry in `manifest.yaml::scripts` and `manifest.yaml::assets` corresponds to an existing file under the bundle root.

#### Scenario: Reference files are copied recursively

- GIVEN a manifest whose `scripts` list contains `references/ghost.sh`
- AND the file `references/ghost.sh` does NOT exist on disk
- WHEN `cogh skill validate <dir>` runs
- THEN exit status is non-zero
- AND stderr contains `referenced script missing`.
