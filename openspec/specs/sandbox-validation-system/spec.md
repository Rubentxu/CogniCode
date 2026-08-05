# Sandbox Validation System

## Purpose

The automated validation infrastructure that exercises CogniCode MCP tools against real GitHub repositories in isolated per-language containers (podman quadlets), scores results across 5 dimensions, tracks history and trends, and feeds the Release Readiness Gate (ADR-031 / `release-readiness-gate` spec).

## Requirements

### Requirement: Per-Language Hardened Quadlets

The validation system MUST run every scenario in a per-language container defined by a systemd quadlet unit, hardened with: pinned image digest, `Network=none`, `MemoryMax=2g`, `PidsLimit=128`, `ReadOnly=yes` root, writable mounts for `/workspace` and `/repos`, `NoNewPrivileges=yes`, `Tmpfs=/tmp`, `AutoUpdate=no`.

#### Scenario: Six language services exist

- GIVEN the quadlet sources in `sandbox/containers/`
- WHEN they are enumerated
- THEN there are services for rust, python, go, java, node, and postgres

#### Scenario: Every quadlet pins a real digest

- GIVEN each language quadlet
- WHEN its `Image=` line is parsed
- THEN the digest is a valid SHA-256 of the pulled image
- AND `podman inspect --format '{{.ImageDigest}}' <image>` matches it

#### Scenario: Containers are isolated

- GIVEN a running `cognicode-rust` container
- WHEN its network and resource limits are inspected
- THEN `Network=none` is active
- AND `MemoryMax` ≤ 2g
- AND `PidsLimit` ≤ 128
- AND root filesystem is read-only

### Requirement: Pinned Real-Project Corpus

The system MUST validate against real GitHub repositories pinned to exact commit SHAs. Drift from the pinned SHA MUST be detected and re-pinned automatically by `clone_repos.sh`.

#### Scenario: Tier-1 Rust repos present and pinned

- GIVEN `sandbox/repos/`
- WHEN repos are enumerated and HEAD checked
- THEN ripgrep, serde, anyhow, tokio, clap exist
- AND each HEAD equals its pinned SHA recorded in the manifest

#### Scenario: Tier-2 multi-language repos present

- GIVEN `sandbox/repos/`
- WHEN repos are enumerated
- THEN cobra, bubbletea, lo, zod, commander, express, spring-petclinic, click, urllib3, requests exist

#### Scenario: Tier-3 stress repos present

- GIVEN `sandbox/repos/`
- WHEN repos are enumerated
- THEN rust-analyzer, typescript, react exist
- AND each is at least 100k LOC

#### Scenario: Drift is detected and re-pinned

- GIVEN a repo whose HEAD has drifted from the pinned SHA
- WHEN `clone_repos.sh` runs
- THEN it emits a WARNING with old and expected SHAs
- AND re-pins the repo to the expected SHA

### Requirement: Scenario Execution via Orchestrator

Every scenario in the expanded matrix (language × tool × variant × repo) MUST execute through `sandbox-orchestrator` with structured JSON results. Exit codes: 0 = all pass/fail as expected; 1 = unexpected failure; 2 = infrastructure failure.

#### Scenario: Orchestrator executes manifests

- GIVEN manifest YAML files conforming to `schema.json`
- WHEN `sandbox-orchestrator run <manifests>` executes
- THEN one `result.json` per scenario is written under `--results-dir`
- AND each result carries `outcome`, `failure_class`, `timing_ms`, `tool`, `language`

#### Scenario: Exit code reflects outcome class

- GIVEN a campaign where all scenarios pass or fail as expected
- WHEN the orchestrator exits
- THEN exit code is 0
- AND a campaign with an unexpected failure exits 1
- AND an infrastructure failure exits 2

### Requirement: Five-Dimension Scoring

Each scenario result MUST be scored across 5 dimensions — correctness (ground-truth matchers), latency, scalability, consistency, robustness — producing a per-dimension score 0–100 and a weighted Health Score.

#### Scenario: Ground-truth matchers exist for core tools

- GIVEN the scoring engine
- WHEN matchers are enumerated
- THEN symbol, edge, entry-point, hot-path, leaf-function, usage, search-result, outline, code, complexity, index-completeness, merge-accuracy matchers exist

#### Scenario: Health score aggregates dimensions

- GIVEN per-scenario dimension scores
- WHEN the engine computes the run Health Score
- THEN it equals the weighted average of the 5 dimension averages

### Requirement: History, Trends, and Stability

The system MUST append every run to a JSONL history (`runs.jsonl`) with health score and dimension averages, MUST compute trend direction (improving/stable/regressing/insufficient-data) vs prior runs, and MUST compute per-scenario stability across repeated runs (`--repeat ≥ 3`).

#### Scenario: History records each run

- GIVEN a completed campaign
- WHEN `runs.jsonl` is read
- THEN one line per run exists with timestamp, health_score, dimensions, pass_rate

#### Scenario: Trends compare latest vs previous

- GIVEN at least 2 runs in history
- WHEN trend analysis runs
- THEN each dimension reports a trend direction
- AND health score reports an overall direction with change percentage

#### Scenario: Stability quantifies flakiness

- GIVEN a campaign run with `--repeat 5`
- WHEN `stability.json` is generated
- THEN per-scenario outcome distribution and timing variance are present

### Requirement: Release Scorecard Integration

The system MUST expose a `just release-scorecard` command that aggregates campaign results, scoring, history trends, baseline diff, and non-sandbox sources (git, openspec, docs) into the Release Readiness Scorecard (12 gates, G1–G12).

#### Scenario: Scorecard command produces artifacts

- GIVEN a completed campaign and baseline
- WHEN `just release-scorecard` runs
- THEN `scorecard.json` and `scorecard.md` are produced
- AND both cover all 12 gates of the release-readiness-gate spec

### Requirement: CI Automation

A GitHub Actions workflow (`sandbox-nightly.yml`) MUST run the full matrix nightly, including stability repeats and benchmark, archive results, and publish the scorecard. A fast smoke lane (`sandbox-ci-smoke`, < 5 min) MUST run on every PR.

#### Scenario: Nightly workflow exists

- GIVEN `.github/workflows/`
- WHEN workflows are enumerated
- THEN `sandbox-nightly.yml` exists with a nightly schedule

#### Scenario: Smoke lane runs on PRs

- GIVEN a pull request opened
- WHEN CI runs
- THEN the smoke lane (Tier-A fixtures + read-only real-repo scenarios) executes
- AND completes in under 5 minutes
