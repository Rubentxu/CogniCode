# Delta Spec — G10 Evidence Mapping (INC-005)

> Change: `e30.4-conf-001-evidence` | Phase: sddk-spec | Domains: `openspec-conformance`, `release-readiness-gate`

## ADDED Requirements — openspec-conformance

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

---

## MODIFIED Requirements — openspec-conformance

### Requirement: Conformance Harness

`sandbox/scripts/openspec_conformance.py` MUST parse `openspec/specs/*/spec.md` with regex `^#{2,3}\s+Requirement`, detect phantom directories (dir without spec.md or without requirements), and generate `sandbox/reports/conformance_matrix.yaml` + `.md`. Each requirement entry MUST have `{id, spec, status}` where status is `verified | legacy_obsolete | no_evidence`. The summary MUST include `{total, verified, legacy_obsolete, no_evidence, pct_verified, pct_triaged}`. It SHALL accept optional `--evidence-map <yaml>` (spec → status) and optional `--validate-paths` (verify `evidence_path` files exist). Exit code MUST be 0 by default; SHALL be non-zero when `--validate-paths` detects broken paths.
(Previously: no `--validate-paths` flag; exit code always 0 regardless of evidence_path validity.)

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

---

## ADDED Requirements — release-readiness-gate

### Requirement: G10 Conformance Gate Formula (REQ-REL-01)

G10 SHALL compute `pct_verified = verified / (total − legacy_obsolete) * 100`, rounded to 1 decimal. `pct_triaged` SHALL remain `(verified + legacy_obsolete) / total * 100`. G10 status is GREEN iff `pct_verified ≥ 90.0 AND pct_triaged = 100.0`. AMBER if only one condition holds. RED otherwise. The formula SHALL be documented in ADR-031 §4.

#### Scenario: All requirements triaged → GREEN

- GIVEN conformance matrix with 381 verified, 50 legacy_obsolete, 0 no_evidence (total=431)
- WHEN the scorecard evaluates G10
- THEN G10 status is GREEN
- AND `pct_verified` is 100.0
- AND `pct_triaged` is 100.0

#### Scenario: Legacy_obsolete excluded, verified below threshold → RED

- GIVEN 340 verified, 50 legacy_obsolete, 41 no_evidence (total=431)
- WHEN the scorecard evaluates G10
- THEN G10 status is RED
- AND `pct_verified` is 89.2 (< 90.0)

#### Scenario: Verified high but triaged incomplete → AMBER

- GIVEN 381 verified, 50 legacy_obsolete, 1 no_evidence (total=432, new spec added)
- WHEN the scorecard evaluates G10
- THEN G10 status is AMBER
- AND `pct_verified` is 99.7 but `pct_triaged` is 99.8 (< 100.0)

### Requirement: G10 Audit Trail (REQ-REL-02)

The scorecard output for G10 MUST include raw counts (`verified`, `legacy_obsolete`, `no_evidence`, `total`) alongside computed percentages so a human auditor can reproduce the math from `scorecard.md` alone. The evidence text SHALL cite the conformance matrix path.

#### Scenario: Scorecard shows auditable raw counts

- GIVEN a scorecard run with verified=381, legacy_obsolete=50, no_evidence=0, total=431
- WHEN `scorecard.md` is inspected for G10
- THEN the G10 section displays `total=431 verified=381 legacy_obsolete=50 no_evidence=0`
- AND `pct_verified=100.0%` and `pct_triaged=100.0%` are shown alongside the raw counts

---

## MODIFIED Requirements — release-readiness-gate

### Requirement: Non-Sandbox Gates (G1, G2, G10, G11, G12)

The scorecard MUST also evaluate gates sourced outside the sandbox: G1 knowledge layer completion (git evidence: 3 e13-wave2 PRs merged), G2 MCP tool coverage (coverage matrix: N/N tools with ≥1 scenario, where N is the runtime tools/list denominator — currently 68; probe via sandbox/scripts/list_mcp_tools.sh), G10 openspec conformance (≥90% verified of triaged active requirements + 100% triaged across all requirements, computed as `verified / (total − legacy_obsolete) * 100` per ADR-031 §4 amendment), G11 documentation currency (MCP-TOOLS verified, ADRs reviewed, ROADMAP reconciled), G12 release hygiene (changelog present, semver clean, no stale branches).
(Previously: G10 described as "401/401 requirements verified" with no denominator exclusion for legacy_obsolete.)

#### Scenario: Non-sandbox gates reported

- GIVEN a scorecard run after e13-wave2 merged
- WHEN the scorecard is inspected
- THEN G1 is GREEN with the merged PR refs as evidence
- AND G2 is GREEN with coverage matrix path as evidence

#### Scenario: Documentation gap fails G11

- GIVEN MCP-TOOLS.md lists 43 tools but handler registry exposes 68 via tools/list
- WHEN the scorecard evaluates G11
- THEN G11 status is RED
- AND evidence names the 2 undocumented tools

---

## INC-005 Closure Criterion

INC-005 status SHALL be updated to `closed` in the vault (`~/.sddk-knowledge/cognicode/incidences/INC-005-CONF-001.md`) when all three conditions hold:
- G10 reports GREEN in the scorecard (verified ≥ 381, pct_verified ≥ 90.0, pct_triaged = 100.0)
- `conformance_matrix.yaml` is regenerated with updated evidence map
- Incidence node status field changed from `tracked` to `closed`
