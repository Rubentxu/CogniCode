# Tasks: e30-sandbox-infra

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~380–480 |
| 400-line budget risk | Medium |
| Chained PRs recommended | Yes |
| Suggested split | PR 1 (T1–T3) → PR 2 (T4–T5) → PR 3 (T6–T7) |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Digests + go.container hardening | PR 1 | 6 `.container` + justfile `sandbox-pull` |
| 2 | Setup unificado + Maven migration | PR 2 | justfile + java_repos.yaml + clone_repos.sh + SETUP_REQUIREMENTS.md |
| 3 | Workflow nightly + smoke integration | PR 3 | `.github/workflows/sandbox-nightly.yml` + justfile `sandbox-ci-smoke` exit contract |

---

## Phase 1: Infrastructure — Digests + go Hardening (PR 1)

- [ ] 1.1 RED: `grep -cE 'sha256:[a-f0-9]{64}' sandbox/containers/*.container` → returns fewer than 6 (js/ts have no digest, go has none, rust/python/java have fake `e5e5...`)
- [ ] 1.1 GREEN: `podman pull` each of 5 images (rust:1.80-slim, python:3.12-slim, golang:1.23-alpine, eclipse-temurin:17-jammy, node:22-slim); `podman inspect --format '{{.ImageDigest}}'` for each; rewrite `Image=` in all 6 `.container` files with real `@sha256:<64-hex>`; update `sandbox/justfile: sandbox-pull` lines 45–53 with real digests — `sandbox/containers/rust.container`, `sandbox/containers/python.container`, `sandbox/containers/java.container`, `sandbox/containers/go.container`, `sandbox/containers/js.container`, `sandbox/containers/ts.container`, `sandbox/justfile`
  - Commit: `fix(sandbox): pin real SHA-256 digests in 6 .container files`

- [ ] 1.2 RED: `grep 'Network=host' sandbox/containers/go.container` → matches (not hardened)
- [ ] 1.2 GREEN: Rewrite `go.container` with `Network=none`, `AutoUpdate=no`, `MemoryMax=2g`, `MemorySwap=2g`, `PidsLimit=128`, `CPUWeight=50`, remove "NOT YET HARDENED" / "Placeholder" comment — `sandbox/containers/go.container`
  - Commit: `fix(sandbox): harden go.container — Network=none, AutoUpdate=no, MemoryMax=2g, PidsLimit=128`

---

## Phase 2: Setup Unification + Maven Migration (PR 2)

- [ ] 2.1 RED: `bash -c 'cp sandbox/containers/js.container ~/.config/containers/systemd/ 2>/dev/null || true; systemctl --user is-active cognicode-js'` → fails (js/ts not in `sandbox-setup`)
- [ ] 2.1 GREEN: Merge `sandbox-setup-js-ts` into `sandbox-setup`; copy all 6 `.container` files; start all 6 services; deprecate `sandbox-setup-js-ts` with inline comment alias — `sandbox/justfile: sandbox-setup` lines 55–70
  - Commit: `feat(sandbox): unify sandbox-setup to deploy all 6 containers`

- [ ] 2.2 RED: `grep -c './gradlew' sandbox/manifests/java_repos.yaml` → 4 matches (lines 21, 24, 109, 112)
- [ ] 2.2 GREEN: Replace `./gradlew compileJava -q` → `./mvnw compile -q`; `./gradlew test -q` → `./mvnw test -q`; add `Volume=%t/containers/cognicode-java-m2-cache:/root/.m2/repository:z` to `java.container`; add `sandbox-maven-warmup` recipe to justfile — `sandbox/manifests/java_repos.yaml` lines 21, 24, 109, 112; `sandbox/containers/java.container`; `sandbox/justfile`
  - Commit: `fix(sandbox): migrate java validation from Gradle to Maven wrapper (mvnw)`

- [ ] 2.3 RED: `grep 'edf4db28affcc4741c79850a3d95bc3f177b5ff9' sandbox/scripts/clone_repos.sh` → no match (pinned to `main`)
- [ ] 2.3 GREEN: Change `clone_repos.sh` line 192: `"main"` → `"edf4db28affcc4741c79850a3d95bc3f177b5ff9"` — `sandbox/scripts/clone_repos.sh`
  - Commit: `fix(sandbox): pin spring-petclinic to SHA edf4db28affcc4741c79850a3d95bc3f177b5ff9`

- [ ] 2.4 GREEN: Update `SETUP_REQUIREMENTS.md` line 42: Maven `❌ MISSING` → `✅ DISPONIBLE (mvnw wrapper)`; add maven to Current Container Status table — `sandbox/SETUP_REQUIREMENTS.md`
  - Commit: `docs(sandbox): mark Maven as available via mvnw wrapper`

---

## Phase 3: Nightly Workflow + Smoke Integration (PR 3)

- [ ] 3.1 RED: `test -f .github/workflows/sandbox-nightly.yml && echo exists` → no such file
- [ ] 3.1 GREEN: Create `.github/workflows/sandbox-nightly.yml` with `schedule: cron(0 3 * * *)`, `workflow_dispatch`, podman setup step, `just sandbox-pull && just sandbox-setup`, `just sandbox-ci-smoke` lane, `just sandbox-ci-probe` lane, `continue-on-error: true`, artifact uploads — `.github/workflows/sandbox-nightly.yml`
  - Commit: `feat(ci): add sandbox-nightly.yml workflow with smoke and probe lanes`

- [ ] 3.2 RED: `just sandbox-ci-smoke; echo "exit: $?"` → command not found or non-zero for wrong reason
- [ ] 3.2 GREEN: Verify `sandbox-ci-smoke` in justfile is correctly wired (already exists at lines 76–89; confirm exit contract documented: 0=pass, 1=product-fail, 2=infra-fail) — `sandbox/justfile` lines 73–89
  - Commit: `docs(sandbox): document sandbox-ci-smoke exit code contract`
