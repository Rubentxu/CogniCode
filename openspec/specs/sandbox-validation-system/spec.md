# Sandbox Validation System

## Purpose

The automated validation infrastructure that exercises CogniCode MCP tools against real GitHub repositories in isolated per-language containers (podman quadlets), scores results across 5 dimensions, tracks history and trends, and feeds the Release Readiness Gate (ADR-031 / `release-readiness-gate` spec).

## Requirements

### Requirement: Per-Language Hardened Quadlets

The validation system MUST run every scenario in a per-language container defined by a systemd quadlet unit, hardened with: pinned image digest, `Network=none`, `MemoryMax=4g` (tier-3 scale lane; raised from 2g in e30-metric-baseline), `PidsLimit=128`, `ReadOnly=yes` root, writable mounts for `/workspace` and `/repos`, `NoNewPrivileges=yes`, `Tmpfs=/tmp`, `AutoUpdate=no`.

#### Scenario: Six language containers exist in source

- GIVEN the quadlet sources in `sandbox/containers/`
- WHEN they are enumerated
- THEN exactly six `.container` files exist: rust, python, go, java, js, ts
- AND `postgres` is NOT counted among them (it is pre-existing infrastructure)

#### Scenario: Every quadlet pins a real digest

- GIVEN each of the six language quadlets
- WHEN its `Image=` line is parsed
- THEN the digest is a valid SHA-256 in `@sha256:<64-hex-chars>` format
- AND `podman image exists <image@sha256:...>` succeeds
- AND no container uses a floating tag without a digest pin

#### Scenario: All containers are hardened including Go

- GIVEN a running `cognicode-{rust,python,go,java,js,ts}` container
- WHEN its quadlet is inspected
- THEN `Network=none` is present (not `host`)
- AND `AutoUpdate=no` is present (not `registry`)
- AND `MemoryMax` ≤ 2g (go SHALL be upgraded from 1g to 2g)
- AND `PidsLimit` ≤ 128 (go SHALL be upgraded from 64 to 128)
- AND `ReadOnly=yes` is present
- AND `NoNewPrivileges=yes` is present

#### Scenario: Go container is no longer provisional

- GIVEN `sandbox/containers/cognicode-go.container`
- WHEN its hardening directives are verified
- THEN `Network=none` replaces `Network=host`
- AND `AutoUpdate=no` replaces `AutoUpdate=registry`
- AND `MemoryMax=2g` replaces `MemoryMax=1g`
- AND `PidsLimit=128` replaces `PidsLimit=64`
- AND the header comment no longer says "NOT YET HARDENED" or "Placeholder"

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

#### Scenario: spring-petclinic pinned to concrete SHA

- GIVEN `sandbox/scripts/clone_repos.sh`
- WHEN the spring-petclinic `pin_repo` call is inspected
- THEN the ref argument is a concrete 40-char SHA, not `"main"`
- AND the pinned SHA matches `edf4db28affcc4741c79850a3d95bc3f177b5ff9` as recorded in `java_repos.yaml`

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

### Requirement: Tools-List Pagination Probe

The sandbox MUST expose a paginated `tools/list` probe (`sandbox/scripts/list_mcp_tools.sh`) that collects the complete runtime tool surface from the MCP server (base64-encoded offset cursor, PAGE_SIZE=20) and emits canonical JSON with every tool name and description.

- GIVEN the MCP server binary is built AND the probe script is invoked
- WHEN the probe paginates `tools/list` until `nextCursor` is absent
- THEN the output MUST contain every runtime tool exactly once AND the `total` field MUST equal the number of distinct tools
- AND the probe MUST NOT truncate at the first page

### Requirement: MCP-TOOLS Documentation Regeneration

`docs/MCP-TOOLS.md` MUST be regenerated from the runtime `tools/list` surface (not hand-maintained), and MUST declare the probe as the source of truth.

- GIVEN the canonical probe output (N tools)
- WHEN `docs/MCP-TOOLS.md` is regenerated from that output
- THEN the document MUST list exactly the N runtime tools AND state that the runtime `tools/list` is the source of truth
- AND the document MUST NOT claim a fixed tool count that differs from the runtime surface

### Requirement: CI Automation

A GitHub Actions workflow (`sandbox-nightly.yml`) MUST run the full matrix nightly, including stability repeats and benchmark, archive results, and publish the scorecard. A fast smoke lane (`sandbox-ci-smoke`, < 5 min) MUST run on every PR.

#### Scenario: Nightly workflow exists with smoke and probe lanes

- GIVEN `.github/workflows/`
- WHEN workflows are enumerated
- THEN `sandbox-nightly.yml` exists with `schedule: cron(0 3 * * *)` and `workflow_dispatch`
- AND the workflow includes: podman setup → `just sandbox-pull && just sandbox-setup` → `just sandbox-ci-smoke` lane → `just sandbox-ci-probe` lane
- AND results are uploaded as artifacts (scorecard, trends, failure logs)
- AND the job uses `continue-on-error: true` if running on `ubuntu-latest` (hosted runners may lack rootless podman + systemd)

#### Scenario: Smoke lane reports infra-failure vs product-failure

- GIVEN `just sandbox-ci-smoke` executes
- WHEN the orchestrator exits
- THEN exit 0 means all scenarios passed or failed as expected (infra green)
- AND exit 1 means unexpected product failure (infra still green)
- AND exit 2 means infrastructure failure (containers missing, images not pulled, binary not found)
- AND CI interprets exit 0 and exit 1 as smoke lane passing for Phase 0 verification

### Requirement: Six-Container Setup Deployment

The `sandbox-setup` recipe MUST deploy all six language containers (rust, python, go, java, js, ts) to `~/.config/containers/systemd/` and start them via `systemctl --user`. The separate `sandbox-setup-js-ts` recipe SHALL be merged into the main recipe.

#### Scenario: Setup deploys all six containers

- GIVEN `sandbox/containers/` contains six `.container` files (rust, python, go, java, js, ts)
- WHEN `just sandbox-setup` executes
- THEN `systemctl --user is-active cognicode-{rust,python,go,java,js,ts}` returns `active` for all six
- AND no container is left un-deployed

#### Scenario: Postgres is excluded from setup count

- GIVEN `cognicode-postgres` is already running as pre-existing infrastructure
- WHEN `systemctl --user is-active cognicode-postgres` succeeds
- THEN `sandbox-setup` does NOT manage or restart `cognicode-postgres`
- AND the six active language containers are independent of postgres availability

### Requirement: Java Validation Manifest Uses Maven Wrapper

The `java_repos.yaml` manifest MUST use `./mvnw` (Maven wrapper) for validation commands, not `./gradlew`. The wrapper MUST be executable in the java container, and `SETUP_REQUIREMENTS.md` SHALL reflect Maven as available via wrapper.

#### Scenario: Manifest commands use Maven wrapper

- GIVEN `sandbox/manifests/java_repos.yaml`
- WHEN validation command lines are inspected
- THEN `compile` uses `./mvnw compile -q`
- AND `test` uses `./mvnw test -q`
- AND no line contains `./gradlew`
- AND `SETUP_REQUIREMENTS.md` shows Maven as `✅ DISPONIBLE (wrapper)`
