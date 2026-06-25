# CogniCode Roadmap

Last updated: 2026-06-25

## Active

| Change | Branch | Semver target | Notes |
|--------|--------|---------------|-------|
| _none_ | — | — | All active work in progress; see git branches |

## Completed

| Change | Tag | Closed | PR | Notes |
|--------|-----|--------|----|----|
| `e8-graphlanding-affordances` | v0.24.1 | 2026-06-25 | [#56](https://github.com/Rubentxu/CogniCode/pull/56) + [#57](https://github.com/Rubentxu/CogniCode/pull/57) + [#58](https://github.com/Rubentxu/CogniCode/pull/58) + [snapshot re-baseline `78b12eb`](https://github.com/Rubentxu/CogniCode/commit/78b12eb) | GraphLanding: truncation banner (dormant, awaiting `e8b`), canvas a11y (`role="application"` + `aria-label` + `tabIndex={0}`), node-list fallback of buttons, `selectObject` memoized via `useCallback`. Artifact endpoint: `/explorations/` → `/api/exploration-sessions/` aligned with ADR-040 Wave 3 (fixes pre-existing `generateArtifact` test failure). E2E: `page.route` → `addInitScript` for MSW compatibility; 24 visual-regression snapshots re-baselined. |
| `quality-stack-evolution` | v0.24.0 | 2026-06-25 | [#55](https://github.com/Rubentxu/CogniCode/pull/55) | C5 rename (`QualityIssue.file → file_path` with serde wire compat per D-1 B.1) + multi-workspace `quality_gate` scoping (`workspace_id: Option<&str>` per D-2) + quality agent ingest write-path (`QualityWritePort` trait + `PostgresQualityRepository` impl + `ingest_quality_issues` MCP tool with natural-key idempotency per D-3) |
| `quality-stack-pg-canonical` (+ v2) | v0.23.0 | 2026-06-25 | [#52](https://github.com/Rubentxu/CogniCode/pull/52) + follow-up `ad35e06` | Postgres-canonical quality stack: m0011_quality.sql migration + PostgresQualityRepository + issues_for_workspace + runtime wiring + 6 test mocks + 8 integration tests + parked-crates ADR |

## Future

_(none — all roadmap items closed)_

The 3 previously-listed items (`cognicode-axiom`, `cognicode-quality`, `cognicode-rule-test-harness` re-activation) were **archived** on 2026-06-25 per ADR-001 trigger (b) — moved to `docs/parked-crates/` rather than revived. See ADR-001 §Archive Action. The C5 rename, multi-workspace `quality_gate`, and quality agent ingest items shipped in v0.24.0.

## Conventions

- Roadmap entries are **date-sorted descending** within each section.
- Each entry links to: branch (Active), tag + PR (Completed), or ADR/scenario (Future).
- The `quality-stack-pg-canonical` entry includes a follow-up commit (`ad35e06`) that landed AFTER the original PR merged; both are part of the same change for the purposes of this roadmap.
- When an item shifts from Future to Completed (or to Archived), the entry is moved and the source ADR/spec is cited.
