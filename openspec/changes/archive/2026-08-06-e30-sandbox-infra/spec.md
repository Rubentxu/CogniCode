# Delta for Sandbox Validation System

Cambio `e30-sandbox-infra` — reparación Fase 0 de infraestructura del sandbox. Este delta modifica `openspec/specs/sandbox-validation-system/spec.md` (ADR-032).

## ADDED Requirements

### Requirement: Six-Container Setup Deployment

The `sandbox-setup` recipe MUST deploy all six language containers (rust, python, go, java, js, ts) to `~/.config/containers/systemd/` and start them via `systemctl --user`. The separate `sandbox-setup-js-ts` recipe SHALL be merged into the main recipe.

(Previously: `sandbox-setup` deployed only 3 containers; go/js/ts were in a separate unreferenced recipe.)

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

(Previously: manifest used `./gradlew`; ADR-032 and SETUP_REQUIREMENTS.md mandated Maven but marked it missing.)

#### Scenario: Manifest commands use Maven wrapper

- GIVEN `sandbox/manifests/java_repos.yaml`
- WHEN validation command lines are inspected
- THEN `compile` uses `./mvnw compile -q`
- AND `test` uses `./mvnw test -q`
- AND no line contains `./gradlew`
- AND `SETUP_REQUIREMENTS.md` shows Maven as `✅ DISPONIBLE (wrapper)`

## MODIFIED Requirements

### Requirement: Per-Language Hardened Quadlets

The validation system MUST run every scenario in a per-language container defined by a systemd quadlet unit, hardened with: pinned image digest, `Network=none`, `MemoryMax=2g`, `PidsLimit=128`, `ReadOnly=yes` root, writable mounts for `/workspace` and `/repos`, `NoNewPrivileges=yes`, `Tmpfs=/tmp`, `AutoUpdate=no`.
(Previously: "Six language services" counted postgres among the six; cognicode-go.container was not hardened.)

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
(Previously: spring-petclinic was pinned to branch `main`, not a concrete SHA.)

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

### Requirement: CI Automation

A GitHub Actions workflow (`sandbox-nightly.yml`) MUST run the full matrix nightly, including stability repeats and benchmark, archive results, and publish the scorecard. A fast smoke lane (`sandbox-ci-smoke`, < 5 min) MUST run on every PR.
(Previously: only `ci.yml` existed; no sandbox-specific nightly workflow.)

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
