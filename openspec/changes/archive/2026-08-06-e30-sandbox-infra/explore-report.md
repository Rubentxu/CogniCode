# Kernel Exploration: e30-sandbox-infra

## Context Quality
- **Level**: C2 (mostly known — infra exists, gaps identified, but exact digest values and CI-runner capabilities are unknown until runtime)
- **Evidence Present**: ADR-031, ADR-032, RELEASE-1.0.0-PLAN.md, openspec/specs/sandbox-validation-system/spec.md, sandbox/containers/*.container (6 files), sandbox/justfile (186 lines), sandbox/scripts/clone_repos.sh (290 lines), SETUP_REQUIREMENTS.md, .github/workflows/ci.yml, 60 manifests, 20 cloned repos
- **Missing Context**: real digest SHA-256 values (require `podman pull` + `podman inspect`); whether GitHub-hosted runners have podman/rootless privileges; whether "6 servicios activos" means 6 .container files or 5 language + postgres
- **Recommended Effort**: deepen (gaps are concrete and actionable, but Maven/Gradle contradiction and Go hardening regression need resolution)

## Current State

The sandbox infrastructure has solid foundations — the orchestrator compiles, scoring 5D exists, 60 manifests cover a broad language matrix, and 20 repos are cloned with pinning. However, **every container image reference is either fake or unpinned**, the Go container is not hardened, the setup recipe only deploys 3 of 6 containers, Maven is documented as missing, and there is no sandbox CI workflow.

### Evidence summary

| Criterion (Phase 0 exit) | Current state | Gap |
|---|---|---|
| Real digest pins | rust/python/java: fake `sha256:...e5e5e5...` patterns; js/ts/go: floating tags (no digest at all) | All 6 need real digests |
| Go container hardened | `Network=host`, `AutoUpdate=registry`, no MemorySwap | Violates spec (Network=none, AutoUpdate=no) |
| Maven available | SETUP_REQUIREMENTS: ❌ MISSING; java_repos.yaml validates via `./gradlew` (Gradle wrapper exists in repo) | Contradiction: ADR-032 says "+ Maven" but manifests use Gradle |
| Repos re-pinned | clone_repos.sh has pinning logic + drift detection; 20 dirs present | Works; spring-petclinic pinned to `main` (not a SHA — weak pin) |
| `just sandbox-ci-smoke` green | Recipe exists (line 76); runs rust_fixture + python_fixture + js + js_smoke | Cannot pass: orchestrator needs `--server-binary` (MCP binary) + containers with real images |
| 6 services active | sandbox-setup deploys only 3 containers (rust, python, java); go/js/ts NOT deployed | 3 of 6 language containers never reach systemd |
| Nightly workflow | Only ci.yml exists (unit tests, fmt, clippy) | No sandbox-nightly.yml; no podman setup in CI |

### Critical code evidence

**Fake digests** (`sandbox/justfile:44-49`):
```
podman pull docker.io/library/rust@sha256:16b31a5e2b37d0e1c9c0e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5e5
```
The `e5e5e5...` repeating pattern is a placeholder — invalid hex, wrong length (SHA-256 = 64 hex chars; these have ~48).

**Go container not hardened** (`sandbox/containers/go.container:15-16,28`):
```
Network=host          # spec requires Network=none
AutoUpdate=registry   # spec requires AutoUpdate=no
```

**Setup deploys only 3 of 6** (`sandbox/justfile:57-62`):
```bash
cp .../rust.container .../python.container .../java.container \
   ~/.config/containers/systemd/
```
go/js/ts containers have a separate recipe (`sandbox-setup-js-ts`) that is NOT called by `sandbox-setup`.

**Maven vs Gradle contradiction** (`sandbox/manifests/java_repos.yaml:21-25`):
```yaml
- name: build
  commands: ["./gradlew compileJava -q"]
- name: test
  commands: ["./gradlew test -q"]
```
But `sandbox/repos/java/spring-petclinic/` has BOTH `pom.xml` (Maven) and `build.gradle` + `gradlew` (Gradle). ADR-032 §1 says "eclipse-temurin:17-jammy + Maven". SETUP_REQUIREMENTS says Maven is missing. The manifest uses Gradle wrapper (which works without Maven). **Decision needed: pin Maven in the image AND switch manifest validation to `mvn`, OR keep Gradle and update ADR-032.**

## Affected Areas

- `sandbox/containers/*.container` (6 files) — all need real digest pins; go.container needs hardening fixes
- `sandbox/justfile` — sandbox-pull needs real digests; sandbox-setup must deploy all 6 containers (merge go + js/ts into main setup)
- `sandbox/manifests/java_repos.yaml` — validation commands (Gradle vs Maven) must align with ADR decision
- `sandbox/SETUP_REQUIREMENTS.md` — Maven status must reflect final decision
- `.github/workflows/` — new sandbox-nightly.yml needed; ci.yml may need a smoke job added
- `sandbox/scripts/clone_repos.sh` — spring-petclinic pinned to `main` (branch, not SHA); weak for reproducibility

## Domain Language

### Resolved terms
- **Quadlet**: systemd unit file (`.container`) consumed by podman to create a systemd service. Deployed to `~/.config/containers/systemd/`. Source of truth: `sandbox/containers/`.
- **Digest pin**: `image@sha256:<64-hex-chars>` — immutable content hash. Procedure: `podman pull` → `podman inspect --format '{{.ImageDigest}}'` → write into quadlet.
- **Smoke lane**: `just sandbox-ci-smoke` — fast (<5 min) Tier-A fixture validation. Runs rust_fixture + python_fixture + js + js_smoke manifests.
- **Tier A/B/C**: Manifest classification. Tier A = fixture-based (fast, no network); Tier B = real repos (read-only or mutation); Tier C = exotic languages (stress probes).
- **AutoUpdate=no**: quadlet directive preventing automatic image refresh — required for reproducibility (gate G9).

### Unresolved ambiguities
1. **"6 servicios activos"** — Does this mean (a) the 6 .container files (rust, python, go, java, js, ts) or (b) 5 language containers + postgres (already running as `cognicode-postgres` in `~/.config/containers/systemd/`)? There is NO postgres.container in `sandbox/containers/` — it lives only on the host. The spec scenario "Six language services exist" lists rust, python, go, java, node, postgres — suggesting 5 language + 1 postgres = 6. But that conflicts with having separate js and ts containers (6 language + postgres = 7).
2. **Maven vs Gradle for spring-petclinic** — ADR-032 mandates Maven, but the repo has a Gradle wrapper and the manifest uses it. Must resolve before implementation.
3. **CI runner podman capability** — ubuntu-latest runners have Docker but rootless podman + systemd user units may not work. Self-hosted runner may be required. Unknown until tested.

## Approaches

### 1. A-min (Minimal: pin digests + fix setup + smoke green)
- Pull 6 images locally, extract real digests, rewrite all .container files + justfile
- Fix go.container hardening (Network=none, AutoUpdate=no)
- Merge sandbox-setup-js-ts + go into main sandbox-setup
- Re-pin spring-petclinic to a concrete SHA
- Run smoke until green
- Add sandbox-nightly.yml skeleton (schedule only, may fail on hosted runner)
- **Pros**: Smallest scope, directly satisfies exit criteria, low risk
- **Cons**: Nightly workflow may not actually run on ubuntu-latest (podman privilege issue); Maven ambiguity left unresolved if we keep Gradle
- **Effort**: Medium (image pulls + file rewrites + debugging smoke failures)

### 2. A-lite (A-min + resolve Maven + working nightly)
- Everything in A-min
- Resolve Maven/Gradle: install Maven in java image (derived Dockerfile) + switch java_repos.yaml to `mvn` commands, OR explicitly keep Gradle and update ADR-032
- Test nightly workflow on ubuntu-latest with podman action; if fails, document self-hosted runner requirement
- **Pros**: Complete infra, no deferred decisions, nightly actually runnable
- **Cons**: More moving parts; Maven image build adds a derived-image layer (js/ts already document this pattern)
- **Effort**: Medium-High

### 3. A-full (A-lite + Tier-3 repos + coverage matrix)
- Everything in A-lite
- Add tokio, clap, rust-analyzer, typescript, react repos (Phase 2 scope)
- Generate tool coverage matrix
- **Pros**: Advances multiple gates at once
- **Cons**: Scope creep — Phase 0 should be infra repair only; corpus expansion is explicitly Phase 2
- **Effort**: High

## Recommendation

**A-lite** — The Phase 0 exit criteria explicitly requires "6 servicios activos" and "smoke lane verde", which means all 6 containers must be deployable and working. A-min risks leaving the Maven/Gradle contradiction unresolved (which would cause `sandbox-ci-probe` to fail later). A-lite resolves the contradiction, fixes all hardening, and produces a nightly workflow that at least attempts to run.

Specifically:
1. Pull all 6 images, extract real digests, rewrite containers + justfile
2. Fix go.container hardening
3. Merge all 6 containers into single sandbox-setup recipe
4. Resolve Maven/Gradle (recommend: keep Gradle wrapper since it's already wired and works; update ADR-032 to reflect dual-build reality; OR install Maven if owner prefers Maven for the "Maven missing" gate)
5. Re-pin spring-petclinic to concrete SHA
6. Add sandbox-nightly.yml with podman setup step
7. Iterate smoke until exit 0

## Risks

- **CI podman privileges**: ubuntu-latest may not support rootless podman + systemd user quadlets. Mitigation: document self-hosted runner requirement; nightly can be manual-run until runner is configured.
- **Digest pin drift**: Images update on Docker Hub; pins will go stale. Mitigation: AutoUpdate=no + document re-pin procedure (already in ADR-032).
- **Maven/Gradle decision blocks Java lane**: If we keep Gradle but ADR-032 says Maven, the spec scenario "Every quadlet pins a real digest" passes but the Java validation pipeline diverges from documented intent.
- **Smoke needs MCP server binary**: `sandbox-ci-smoke` passes `--server-binary` (the cognicode-mcp binary). If the binary has runtime issues against real repos, smoke fails for product reasons, not infra reasons. Need to distinguish infra-fail (exit 2) from product-fail (exit 1).
- **spring-petclinic pinned to `main`**: Branch pin means HEAD drifts on every upstream commit. Must pin to concrete SHA for reproducibility (gate G9).

## Ready for Proposal

**Yes** — with one clarification needed from the orchestrator/user:

> "6 servicios activos" — does this mean the 6 .container files in sandbox/containers/ (rust, python, go, java, js, ts), or 5 language containers + the already-running postgres? The spec lists "rust, python, go, java, node, postgres" (6) but the repo has separate js and ts containers (7 with postgres). Recommend: interpret as "6 language .container files deployed + active" and treat postgres as pre-existing infrastructure (not part of this change's deliverables).

The proposal should also surface the Maven/Gradle contradiction for an explicit decision.

---

## Reference artifacts
- ADR-031: `docs/adr/ADR-031-release-1.0.0-definition.md` (12-gate scorecard definition)
- ADR-032: `docs/adr/ADR-032-sandbox-validation-system.md` (sandbox architecture — source of truth for this change)
- Spec: `openspec/specs/sandbox-validation-system/spec.md` (executable requirements + scenarios)
- Plan: `docs/RELEASE-1.0.0-PLAN.md` §4 Phase 0 (exit criteria + 5-step plan)
- CI pattern: `.github/workflows/ci.yml` (existing workflow to follow for nightly structure)
