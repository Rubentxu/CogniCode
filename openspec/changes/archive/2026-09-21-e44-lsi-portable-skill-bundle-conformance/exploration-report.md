# Exploration Report — e44 — portable-skill-bundle conformance

> Change: `e44-lsi-portable-skill-bundle-conformance` | Phase: explore | Date: 2026-09-15

## Problem

The 7 specs in `openspec/specs/portable-skill-bundle/spec.md` are all
in `no_evidence` status (verified=0 / 7). The validator exists at
`crates/cognicode-cli/src/cmd/skill.rs::validate_bundle` and is
exposed via `cogh skill validate <path>`, but it has no test coverage.

This continues the e43 pattern: every `cognicode-cli` capability that
ships in the conformance corpus must be locked down with tests.

## Landscape

### Source

| Path | Purpose |
|------|---------|
| `crates/cognicode-cli/src/cmd/skill.rs` | `validate_bundle` (pure), `cmd_skill_validate` (CLI handler), `SkillManifest` struct |
| `crates/cognicode-cli/src/bin/cogh.rs:251` | `Command::Skill { action }` dispatcher |

### Spec

`openspec/specs/portable-skill-bundle/spec.md` (7 requirements):

| # | Requirement | Testable today? |
|---|-------------|-----------------|
| 1 | Skill bundle is a portable directory tree | YES (cogh skill validate on a directory) |
| 2 | `SKILL.md` has portable YAML frontmatter | YES (validate checks `name`, `maturity`, etc. via `SkillManifest::validate`) |
| 3 | Portable bundle has NO IDE-specific fields | YES (validate rejects `compatibility: opencode`) |
| 4 | `manifest.yaml` declares cogh metadata | YES (validate parses and prints fields) |
| 5 | Skill bundles are versioned with the CogniCode version | NO (requires `cogh install`, network) |
| 6 | Skills with `requires` plugins are installed together | NO (network) |
| 7 | `references/` and `assets/` are copied recursively | partial (validate confirms referenced files exist; actual copy is in ide-adapter plugin) |

**Immediately testable (this cycle):** #1, #2, #3, #4, #7 = **5 of 7**.

### Empirical probe (commit `02e92f87`)

```
$ cogh skill validate /tmp/sb-test  # valid
✓ skill bundle valid: test-skill v1.0.0
  description: A test skill bundle for conformance coverage
  maturity: stable
  requires: []
  ide_compatibility: []
  scripts: 1 referenced
  assets: 0 referenced

$ cogh skill validate /tmp/sb-no-manifest
Error: skill bundle missing manifest.yaml

$ cogh skill validate /tmp/sb-bad-maturity
Error: skill x: maturity must be one of experimental|beta|stable|deprecated (got bogus)

$ cogh skill validate /tmp/sb-ide-specific
Error: skill x SKILL.md has IDE-specific 'compatibility: opencode' field

$ cogh skill validate /tmp/sb-missing-script
Error: skill x: referenced script missing: /tmp/sb-missing-script/references/ghost.sh

$ cogh skill validate /tmp/nonexistent
Error: skill bundle path is not a directory: /tmp/nonexistent
```

All observable failure modes are reachable through the public CLI.

## Strategy

Write **integration tests** in `crates/cognicode-cli/tests/portable_skill_bundle.rs`
that:

1. Spawn `cogh` via `std::process::Command::new(env!("CARGO_BIN_EXE_cogh"))`.
2. Build isolated bundle fixtures with `tempfile::tempdir()`.
3. Assert stdout/stderr/exit-code per the spec scenarios.

Five tests will land (one per testable REQ):

- `portable_skill_bundle_valid_layout_passes` (REQ #1, scenario "Skill bundle loads").
- `portable_skill_bundle_frontmatter_mandatory_fields_validated` (REQ #2, scenario "frontmatter parses" + "missing mandatory fields fail" — both branches).
- `portable_skill_bundle_ide_specific_compatibility_field_rejected` (REQ #3, scenario "Portable bundle is IDE-agnostic").
- `portable_skill_bundle_manifest_yaml_required_and_parsed` (REQ #4, scenario "manifest declares bundle metadata").
- `portable_skill_bundle_references_scripts_must_exist` (REQ #7, scenario "Reference files are copied recursively" — partial coverage).

A sixth test covers the structural preconditions of "Skill bundle loads" by
asserting `validate_bundle` rejects a non-directory path (REQ #1 negative branch).

## Why a bounded test-only slice

- No production code change → minimal regression surface.
- Bumps conformance: 5 / 523 → `pct_verified` lifts from `93.7%` toward `94.7%`.
- Establishes the contract for the offline-testable surface of `cogh skill validate`.
- Closes the remaining gaps in the `cognicode-cli` crate (the next un-tackled group is `cognicode-ide-adapter`, `cognicode-lifecycle`, `cognicode-plugin`).

## Scope non-goals

- Specs #5 (versioning with MCP server) and #6 (requires cascading): require
  `cogh install` which is network-dependent. Out of scope for this cycle.
- Spec #7's "copied recursively" semantics: only the existence pre-condition
  is validated; the actual `references/`/`assets/` copy logic lives in the
  ide-adapter plugin (separate crate), so partial coverage is the most we can
  pin from the CLI side.
