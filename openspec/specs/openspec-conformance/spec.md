# OpenSpec Conformance Harness

## Purpose

Automated conformance audit for the openspec spec corpus. Parses all `spec.md` files, counts requirements, detects legacy/obsolete specs, and produces a machine-readable conformance matrix used by the Release Readiness Gate G10.

## Requirements

### Requirement: Evidence Path Validation (REQ-CONF-01)

The harness SHALL support a `--validate-paths` flag. When active, every `evidence_path` in `evidence_map.yaml` MUST resolve to at least one existing file. The harness MUST exit non-zero and list all broken entries on stdout when any path fails validation. Without `--validate-paths`, the harness MUST exit 0 regardless of path validity (backward-compatible).

#### Scenario: All paths valid → clean exit

- GIVEN `evidence_map.yaml` with 30 `verified` entries, each `evidence_path` pointing to an existing file
- WHEN `openspec_conformance.py --evidence-map evidence_map.yaml --validate-paths` runs
- THEN exit code is 0
- AND stdout contains no validation errors

#### Scenario: Broken path → non-zero exit with listing

- GIVEN `evidence_map.yaml` where entry `spotter-search` has `evidence_path: nonexitent/test.rs`
- WHEN the harness runs with `--validate-paths`
- THEN exit code is non-zero
- AND stdout lists `spotter-search: evidence_path "nonexitent/test.rs" not found`

#### Scenario: Multiple broken paths → all listed

- GIVEN `evidence_map.yaml` with 3 entries pointing to missing files
- WHEN the harness runs with `--validate-paths`
- THEN exit code is non-zero
- AND stdout lists ALL 3 broken spec:path pairs before exiting

### Requirement: Stale Entry Warnings (REQ-CONF-02)

The harness SHALL emit a warning for each `evidence_map.yaml` entry whose spec name has no corresponding directory under `openspec/specs/`. Stale entries MUST NOT cause non-zero exit. They SHALL appear on stderr as warnings.

#### Scenario: Stale entry detected

- GIVEN `evidence_map.yaml` contains entry `quality-store` but no `openspec/specs/quality-store/` directory exists
- WHEN the harness runs
- THEN stderr includes `WARNING: stale entry "quality-store" — spec directory not found`
- AND exit code is 0

### Requirement: Output Artifacts Regeneration (REQ-CONF-03)

Every harness run MUST regenerate `conformance_matrix.yaml` and `conformance_matrix.md` atomically. The summary section SHALL reflect the live counts — no stale cached values. A run with the same inputs SHALL produce identical outputs.

#### Scenario: Matrix reflects live scan

- GIVEN 3 new `verified` entries added to `evidence_map.yaml`
- WHEN the harness runs
- THEN `conformance_matrix.yaml` summary shows `verified` increased by the count of requirements in those 3 specs
- AND `conformance_matrix.md` shows matching numbers

### Requirement: Conformance Harness

`sandbox/scripts/openspec_conformance.py` MUST parse `openspec/specs/*/spec.md` with regex `^#{2,3}\s+Requirement`, detect phantom directories (dir without spec.md or without requirements), and generate `sandbox/reports/conformance_matrix.yaml` + `.md`. Each requirement entry MUST have `{id, spec, status}` where status is `verified | legacy_obsolete | no_evidence`. The summary MUST include `{total, verified, legacy_obsolete, no_evidence, pct_verified, pct_triaged}`. It SHALL accept optional `--evidence-map <yaml>` (spec → status) and optional `--validate-paths` (verify `evidence_path` files exist). Exit code MUST be 0 by default; SHALL be non-zero when `--validate-paths` detects broken paths.

#### Scenario: Harness counts requirements correctly

- GIVEN `openspec/specs/` with specs containing requirements plus phantom directories
- WHEN `openspec_conformance.py --evidence-map evidence_map.yaml` runs
- THEN total matches the documented count, YAML is valid, and phantom dirs appear as warnings

#### Scenario: Evidence map marks legacy specs

- GIVEN `evidence_map.yaml` maps `postgres-call-edges` to `legacy_obsolete`
- WHEN the harness runs with that map
- THEN all entries for that spec show `status: legacy_obsolete`
- AND `pct_triaged` counts them as triaged

#### Scenario: Validate-paths catches fabricated evidence

- GIVEN `evidence_map.yaml` with a `verified` entry pointing to a non-existent file
- WHEN the harness runs with `--validate-paths`
- THEN exit code is non-zero
- AND stdout lists the broken spec and path

### Requirement: Tier-C Obsolete Marking

Six PostgreSQL/SQLite specs (`postgres-call-edges`, `postgres-callgraph-persistence`, `postgres-symbol-repository`, `explorer-postgres-bridge`, `ci-postgres-pipeline`, `sqlite-feature-gate`) MUST contain the banner `OBSOLETE — 2026-08-04`. `postgres-symbol-repository` SHALL be the last to receive it.

#### Scenario: All six OBSOLETE specs carry the banner

- GIVEN the 6 postgres/sqlite spec files
- WHEN `grep "OBSOLETE.*2026-08-04" openspec/specs/{postgres,explorer,ci,sqlite}*/spec.md` runs
- THEN all 6 files return a match
