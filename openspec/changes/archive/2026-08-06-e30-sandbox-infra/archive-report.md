# Archive Report: e30-sandbox-infra

**Change**: e30-sandbox-infra
**Archived**: 2026-08-06
**Branch**: `feat/e30-sandbox-infra` (HEAD a1b4b7c1, 12 commits c40a2602..a1b4b7c1)
**Mode**: openspec (hybrid)
**Verdict**: PASS_WITH_WARNINGS (correction cycle 1)
**Debt verdict**: PASS_WITH_WARNINGS (4 clusters, DQS 58, 0 CRITICAL)

---

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| sandbox-validation-system | Updated | 2 ADDED + 3 MODIFIED requirements merged into `openspec/specs/sandbox-validation-system/spec.md` |

### Delta → Main Spec Merge Summary

**ADDED requirements** (appended to main spec):
1. `Six-Container Setup Deployment` — unified 6-container deploy, postgres excluded
2. `Java Validation Manifest Uses Maven Wrapper` — `./mvnw` migration from `./gradlew`

**MODIFIED requirements** (replaced in main spec):
1. `Per-Language Hardened Quadlets` — scenarios updated: six containers excluding postgres, digest pinning enforcement, full hardening including Go, Go no longer provisional
2. `Pinned Real-Project Corpus` — added spring-petclinic concrete SHA scenario
3. `CI Automation` — added nightly workflow smoke+probe lanes and smoke exit-code contract scenarios

---

## Archive Contents

| Artifact | Path | Status |
|----------|------|--------|
| proposal.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/proposal.md` | ✅ |
| spec.md (delta) | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/spec.md` | ✅ |
| design.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/design.md` | ✅ |
| tasks.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/tasks.md` | ✅ |
| apply-progress.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/apply-progress.md` | ✅ |
| verify-report.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/verify-report.md` | ✅ |
| debt-report.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/debt-report.md` | ✅ |
| explore-report.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/explore-report.md` | ✅ |
| archive-report.md | `openspec/changes/archive/2026-08-06-e30-sandbox-infra/archive-report.md` | ✅ (this file) |

---

## Deliverables Summary — 6 Gaps Closed

| Gap | Fix | Evidence |
|-----|-----|----------|
| G1 Fake/missing digests | Real SHA-256 pins for 6 containers via `podman inspect` | Cycle-1: all 6 verified vs live Docker Hub API |
| G2 go.container not hardened | Network=none, AutoUpdate=no, MemoryMax=2g, PidsLimit=128 | Cycle-0 verified: 0 matches for stale directives |
| G3 Setup deploys 3/6 | Unified `sandbox-setup` deploys all 6; `sandbox-setup-js-ts` deprecated | Recipe structure correct; runtime confirmed 6/6 ACTIVE |
| G4 Maven missing | Migrated `./gradlew` → `./mvnw`; SETUP_REQUIREMENTS.md updated | 0 `gradlew` matches, 4 `mvnw` matches |
| G5 spring-petclinic branch pin | Re-pinned to SHA `edf4db28affcc4741c79850a3d95bc3f177b5ff9` | GitHub API confirmed real commit |
| G6 No nightly CI workflow | `sandbox-nightly.yml` created with smoke+probe lanes, cron, artifact uploads | Cycle-0 verified: 5 required elements present |

---

## Verification Evidence Summary

**Runtime evidence (orchestrator-executed, 2026-08-06)**:
- `systemctl --user is-active cognicode-{rust,python,go,java,js,ts}` → 6/6 ACTIVE
- `just sandbox-ci-smoke` exit 0 — 403 pass, health 77.95, 0 regressions
- `just sandbox-pull` succeeded for all 6 pinned digests
- `cargo check --workspace` clean after baseline restore
- Nightly YAML valid (python yaml.safe_load OK)

**Cycle 0 → Cycle 1 correction**: java digest (`eclipse-temurin:17-jammy`) corrected from fabricated `723151f3` and wrong `9824c276` to authoritative `29467857e8bde40ab1f7befecbda0ea764b95afec1cc7f89aa90f7a766577e19` (OCI index, Docker Hub verified, tag not moved since 2026-08-04).

---

## Debt Follow-ups Carried Forward

| Severity | Item | File | Effort | Pre-existing main? |
|----------|------|------|--------|---------------------|
| WARN | Insert `cognicode-go` into `sandbox-clean` lines 222-223 | `sandbox/justfile` | XS | Yes (widened by branch) |
| WARN | Delete `sandbox-setup-js-ts` (deprecated alias) | `sandbox/justfile:84-97` | S | No |
| WARN | Remove 15-line `TOOL PRE-INSTALLATION` heredoc in js/ts containers | `sandbox/containers/cognicode-{js,ts}.container:1-19` | S | No |
| WARN | Update stale comment `# Pinned at main` → concrete SHA | `sandbox/scripts/clone_repos.sh:187` | XS | Yes (carry-forward) |
| WARN | Centralize `cognicode-*-workspace.volume` listing | `sandbox/justfile:65-71, 88-94` | S | No |
| WARN | Hardcoded `%h/Proyectos/rust/CogniCode/sandbox/repos` host path | all 6 `.container` | M | Yes |
| SUGG | Add tier rationale comment (1G vs 2G / 64 vs 128) | all 6 `.container` | XS | No |
| SUGG | Remove commented-out npm-cache volume lines | `cognicode-{js,ts}.container` | XS | No |
| SUGG | Add `sandbox-maven-warmup` ordering note to `sandbox-ci-smoke` | `sandbox/justfile` | XS | No |

**Top cross-corroborated warnings** (DQS impact):
- C3.1: `sandbox-clean` missing `cognicode-go` (bug-class asymmetry)
- C3.2: hardcoded host path (CI bind fail on non-Spanish-layout runners)
- S1.3/O4.2: dead `TOOL PRE-INSTALLATION` heredoc
- D2.4: volume-list triplication

**Pre-existing main debt** (all traced to `6795951d` on main):
- C3.1, C3.2, S1.1 — recommend B-direct hotfix on `main` for C3.1 + S1.1

---

## Knowledge Graph Updates

### Cycle Node Created
- Path: `~/.sddk-knowledge/cognicode/cycles/CYC-2026-08-06-e30-sandbox-infra.md`
- Linked to: milestone `[[M-E30-Fase-0]]`, spec `[[sandbox-validation-system]]`, ADR `[[ADR-032]]`

### Milestone Updated
- `M-E30-Fase-0`: status → `completed`, closed date → `2026-08-06`

### ADRs Referenced
- ADR-032 (sandbox-validation-system architecture — unchanged, this cycle validated it)
- No ADR superseded by this change

---

## Jurisprudence Candidate

**No** — verdict is PASS_WITH_WARNINGS (not clean PASS), and first_pass_success was false (Cycle 0 FAIL → Cycle 1 PASS_WITH_WARNINGS). No reusable clean decision crystallized. The cycle is not a jurisprudence candidate for F3 save.

---

## Release Handoff

```yaml
ready_for_release: true
change: e30-sandbox-infra
branch: feat/e30-sandbox-infra
merge_policy: guided  # debt follow-ups attached to PR body
next_recommended: sddk-release
```

**Risk note**: C3.1 (`sandbox-clean` missing `cognicode-go`) is a bug-class pre-existing debt item. Recommend hotfix on `main` before next green baseline restore.

---

*Archive generated by sddk-archive (GLM-4.7) — 2026-08-06*
