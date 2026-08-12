# Release Readiness Gate

## Purpose

The mandatory verification gate for shipping CogniCode v1.0.0. Defines the 12 hard criteria (G1–G12) that MUST all be GREEN for 3 consecutive automated runs before the maintainer MAY tag `v1.0.0`. The gate is machine-generated, evidence-based, and repeatable — a scorecard, not an opinion checklist.

## Requirements

### Requirement: Scorecard Generation

A `release-scorecard` automation MUST generate a machine-readable scorecard (JSON) and a human-readable report (Markdown) covering all 12 criteria G1–G12. Each criterion MUST report a status (`GREEN`, `AMBER`, `RED`), a current measured value, the target value, and a path to its evidence artifact.

#### Scenario: Scorecard covers all 12 criteria

- GIVEN the release-scorecard automation
- WHEN it runs against a completed campaign
- THEN the scorecard contains exactly 12 criterion entries (G1–G12)
- AND each entry has `status`, `current`, `target`, `evidence` fields

#### Scenario: Scorecard run is deterministic given same results

- GIVEN identical campaign results in `sandbox/results-runs/<id>/`
- WHEN the scorecard is generated twice
- THEN both scorecards report identical statuses and values

### Requirement: Health Score Gate (G3)

The MCP Health Score (weighted average of correctness, latency, scalability, consistency, robustness dimensions, computed by `sandbox_core::scoring`) MUST be ≥ 85/100 for the candidate release.

#### Scenario: Health score above threshold is GREEN

- GIVEN a campaign with health score 87.3
- WHEN the scorecard evaluates G3
- THEN G3 status is GREEN
- AND current is 87.3
- AND target is 85.0

#### Scenario: Health score below threshold is RED

- GIVEN a campaign with health score 72.1
- WHEN the scorecard evaluates G3
- THEN G3 status is RED

### Requirement: Correctness on Tier-1 Repos (G4)

Correctness (ground-truth comparison via the scoring engine's matchers) MUST be ≥ 90% on every Tier-1 repository (ripgrep, serde, anyhow, tokio, clap).

#### Scenario: All Tier-1 repos above threshold

- GIVEN Tier-1 repos with correctness scores 93.1, 95.0, 91.4, 90.2, 92.7
- WHEN the scorecard evaluates G4
- THEN G4 status is GREEN

#### Scenario: One Tier-1 repo below threshold fails the gate

- GIVEN Tier-1 repos with correctness scores 93.1, 88.0, 91.4, 90.2, 92.7
- WHEN the scorecard evaluates G4
- THEN G4 status is RED
- AND the failing repo is named in the scorecard evidence

### Requirement: Latency Budget (G5)

Latency percentiles MUST respect the per-family budget: spotter search p95 < 500ms, call-graph p95 < 2s on a 10k LOC repo, analytics p95 < 5s.

#### Scenario: Latency within budget

- GIVEN p95 latencies search=312ms, call-graph=1450ms, analytics=3.2s
- WHEN the scorecard evaluates G5
- THEN G5 status is GREEN

#### Scenario: Latency over budget

- GIVEN p95 latency call-graph=2350ms
- WHEN the scorecard evaluates G5
- THEN G5 status is RED
- AND the violating family is named

### Requirement: Consistency Across Runs (G6)

Run-to-run variance (via `stability.json` from `--repeat ≥ 3`) MUST be < 10% per dimension.

#### Scenario: Stable across repeats

- GIVEN stability.json with max variance 4.2%
- WHEN the scorecard evaluates G6
- THEN G6 status is GREEN

#### Scenario: Flaky across repeats

- GIVEN stability.json with variance 14.8% in latency
- WHEN the scorecard evaluates G6
- THEN G6 status is RED

### Requirement: Robustness — Zero Crashes (G7)

The full campaign MUST complete with zero process crashes (panic, SIGSEGV, OOM) classified in failure classes.

#### Scenario: No crashes in campaign

- GIVEN failure class audit with 0 crash-classified failures
- WHEN the scorecard evaluates G7
- THEN G7 status is GREEN

#### Scenario: Crash present

- GIVEN failure class audit with 1 panic-classified failure
- WHEN the scorecard evaluates G7
- THEN G7 status is RED

### Requirement: Scalability Proof (G8)

The campaign MUST ingest a repository of 100k+ LOC (Tier-3 corpus) without timeout or OOM.

#### Scenario: Large repo ingested

- GIVEN rust-analyzer scenario completes within timeout with no OOM
- WHEN the scorecard evaluates G8
- THEN G8 status is GREEN

#### Scenario: Large repo times out

- GIVEN rust-analyzer scenario fails with timeout
- WHEN the scorecard evaluates G8
- THEN G8 status is RED

### Requirement: No Regressions vs Baseline (G9)

The campaign diff vs the frozen baseline (saved at the end of Phase 3) MUST show zero unexpected failures.

#### Scenario: Clean diff vs baseline

- GIVEN `orchestrator report --baseline` shows 0 unexpected failures
- WHEN the scorecard evaluates G9
- THEN G9 status is GREEN

#### Scenario: Regression detected

- GIVEN `orchestrator report --baseline` shows 2 unexpected failures
- WHEN the scorecard evaluates G9
- THEN G9 status is RED

### Requirement: Three Consecutive Green Runs

The release candidate MUST show all 12 gates GREEN in 3 consecutive nightly scorecard runs before the maintainer MAY tag `v1.0.0`. Any RED in the sequence resets the counter.

#### Scenario: Three consecutive green

- GIVEN scorecards for nights N, N+1, N+2 all 12/12 GREEN
- WHEN the release checklist is evaluated
- THEN the maintainer MAY tag v1.0.0

#### Scenario: A RED resets the streak

- GIVEN scorecards for nights N (12/12), N+1 (11/12), N+2 (12/12)
- WHEN the release checklist is evaluated
- THEN the streak is 1 (only N+2 counts)
- AND the maintainer MUST NOT tag v1.0.0

### Requirement: Automated Scorecard Engine

The release scorecard MUST be computed by an automated engine (`sandbox/scripts/release_scorecard.py`) consuming campaign summaries, baseline, stability.json, and the G2 coverage matrix, emitting scorecard.json + scorecard.md with all 12 gates (G1-G12), each with status GREEN/AMBER/RED, measured value, budget, and evidence path. Gates with missing data MUST degrade to AMBER, never crash.

- GIVEN campaign summaries, baseline, stability, and coverage data exist
- WHEN the scorecard engine runs
- THEN it MUST emit 12 gate verdicts with measured/budget/evidence
- AND missing inputs MUST produce AMBER with "no data" evidence
- AND gate REDs MUST NOT block the engine (exit 0) — they are tracked defects

### Requirement: Non-Sandbox Gates (G1, G2, G10, G11, G12)

The scorecard MUST evaluate: G1 (3 e13-wave2 PRs merged), G2 (N/N tools with ≥1 scenario, N = 68 via tools/list), G10 openspec conformance (≥90% verified AND 100% triaged of 430 requirements from conformance_matrix.yaml), G11 documentation currency (MCP-TOOLS.md at 68 tools, ADR-031 and ADR-032 found at correct paths with `**Estado**: ACEPTADO`, ROADMAP reconciled), G12 release hygiene (CHANGELOG.md present, latest semver tag via `git tag --sort=-v:refname | head -1`, stale branches via `git branch -r --merged main | wc -l`: < 20 GREEN, < 50 AMBER, else RED).

#### Scenario: Non-sandbox gates reported

- GIVEN a scorecard run after e13-wave2 merged
- WHEN the scorecard is inspected
- THEN G1 is GREEN with PR refs as evidence
- AND G2 is GREEN with coverage matrix path as evidence

#### Scenario: G10 GREEN with high coverage

- GIVEN conformance_matrix.yaml shows pct_verified ≥ 90 and pct_triaged == 100
- WHEN the scorecard evaluates G10
- THEN G10 status is GREEN

#### Scenario: G10 RED below threshold

- GIVEN conformance_matrix.yaml shows pct_verified < 90
- WHEN the scorecard evaluates G10
- THEN G10 status is RED with the gap in evidence

#### Scenario: G11 detects correct ADR paths and ACEPTADO status

- GIVEN ADR-031 at `docs/adr/ADR-031-release-1.0.0-definition.md` with status ACEPTADO
- AND ADR-032 at `docs/adr/ADR-032-sandbox-validation-system.md` with status ACEPTADO
- WHEN the scorecard evaluates G11
- THEN G11 status is GREEN

#### Scenario: G12 checks CHANGELOG, tag, and branches

- GIVEN CHANGELOG.md exists, latest tag is v0.89.0, stale branch count is 15
- WHEN the scorecard evaluates G12
- THEN G12 status is GREEN with measured values for all three checks

#### Scenario: Documentation gap fails G11

- GIVEN MCP-TOOLS.md tool count does not match tools/list
- WHEN the scorecard evaluates G11
- THEN G11 status is RED with the mismatch in evidence

### Requirement: CHANGELOG

`CHANGELOG.md` MUST exist at repository root with a version table from v0.87.0 to current, plus an empty `Unreleased` section. Pre-v0.87.0 history SHALL note it is reconstructible from ROADMAP.

#### Scenario: CHANGELOG present with versions

- GIVEN the repository root
- WHEN G12 evaluates changelog
- THEN `grep "v0.87.0\|v0.88.0\|v0.89.0" CHANGELOG.md` returns matches
- AND an `Unreleased` section exists

### Requirement: Branch Pruning

`sandbox/scripts/prune_stale_branches.sh` MUST support `--dry-run` (default) and `--apply`. It MUST delete remote branches merged into main, excluding `feat/e30-*`, `fix/e30*`, `master`, `main`, and branches with open PRs (`gh pr list --json headRefName`). Toxic local branches (e12g, e12h, e14, e28-5, e29-6, relation-candidates, e19-4, e21-1, e28-1-pr2, composed-narrative-fix, impl/e13) SHALL be deleted only if no open PR exists.

#### Scenario: Dry-run lists candidates without side effects

- GIVEN stale remote and toxic local branches
- WHEN `prune_stale_branches.sh --dry-run` runs
- THEN candidate branch names are printed and no branches are actually deleted

#### Scenario: Apply preserves branches with open PRs

- GIVEN `feat/e28-1` merged into main with no open PR
- AND `feat/e30-conformance-audit` with an open PR
- WHEN `prune_stale_branches.sh --apply` runs
- THEN `feat/e28-1` is deleted and `feat/e30-conformance-audit` is NOT

### Requirement: ADR Acceptance

ADR-031 and ADR-032 MUST have `**Estado**: ACEPTADO`. ADR-031 G10 row MUST reflect 430 requirements (not 401).

#### Scenario: Both ADRs accepted

- GIVEN `docs/adr/ADR-031-release-1.0.0-definition.md` and `ADR-032-sandbox-validation-system.md`
- WHEN `grep "Estado.*ACEPTADO"` runs on both
- THEN both return a match
