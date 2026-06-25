# CogniCode Roadmap

Last updated: 2026-06-25

## Active

| Change | Branch | Semver target | Notes |
|--------|--------|---------------|-------|
| _none_ | — | — | All active work in progress; see git branches |

## Completed

| Change | Tag | Closed | PR | Notes |
|--------|-----|--------|----|----|
| `quality-stack-evolution` | v0.24.0 | 2026-06-25 | [#55](https://github.com/Rubentxu/CogniCode/pull/55) | C5 rename (`QualityIssue.file → file_path` with serde wire compat per D-1 B.1) + multi-workspace `quality_gate` scoping (`workspace_id: Option<&str>` per D-2) + quality agent ingest write-path (`QualityWritePort` trait + `PostgresQualityRepository` impl + `ingest_quality_issues` MCP tool with natural-key idempotency per D-3) |
| `quality-stack-pg-canonical` (+ v2) | v0.23.0 | 2026-06-25 | [#52](https://github.com/Rubentxu/CogniCode/pull/52) + follow-up `ad35e06` | Postgres-canonical quality stack: m0011_quality.sql migration + PostgresQualityRepository + issues_for_workspace + runtime wiring + 6 test mocks + 8 integration tests + parked-crates ADR |

## Future

| Change | Description | Source |
|--------|-------------|--------|
| `cognicode-axiom` re-activation | Trigger: fresh ADL for rule layer + storage adapter, OR explicit archive | `docs/adr/ADR-001-parked-crates.md` |
| `cognicode-quality` re-activation | Depends on axiom; archive if axiom archived | `docs/adr/ADR-001-parked-crates.md` |
| `cognicode-rule-test-harness` re-activation | Depends on axiom; deletable (no independent value) | `docs/adr/ADR-001-parked-crates.md` |

## Conventions

- Roadmap entries are **date-sorted descending** within each section.
- Each entry links to: branch (Active), tag + PR (Completed), or ADR/scenario (Future).
- The `quality-stack-pg-canonical` entry includes a follow-up commit (`ad35e06`) that landed AFTER the original PR merged; both are part of the same change for the purposes of this roadmap.
