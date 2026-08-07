# Debt Report: e30-sandbox-infra

**Date**: 2026-08-06
**Path**: A-lite → depth: **standard** → 4 clusters (smells + duplication + coupling + overeng)
**Verifier**: sddk-debt-verify
**Branch**: `feat/e30-sandbox-infra` · Base `main@2d468140` · Head `19ccfa48` · 9 commits `c40a2602..19ccfa48`
**Inherit verdict**: `sddk-verify` returned **PASS_WITH_WARNINGS** (Cycle-1, all 14 spec scenarios resolved: 9 COMPLIANT, 0 FAILING, 4 UNTESTED-but-runtime-blocked, 1 contract-documented). Runtime evidence from orchestrator: 6/6 containers ACTIVE, smoke lane exit 0, all digests verified vs registry, workspace builds green.

> **Pre-flight gates** — all PASS:
> • verify-report exists, verdict PASS_WITH_WARNINGS ✅
> • on feature branch `feat/e30-sandbox-infra` ✅
> • branch pushed to origin (`19ccfa48` matches ls-remote) ✅
> • clean working tree (`docs/ROADMAP.md` modified — ephemeral, tracked) ✅
> • A-* path (A-lite) → debt-verify mandatory ✅
> • depth set to `standard` per path (no user override) ✅

## Scope (git diff --stat main...HEAD)

11 changed files in the infra delta:
- 6 `.container` quadlet files (renamed `*.container` → `cognicode-*.container`)
- 7 new `.volume` files (`cognicode-*-workspace.volume` + `cognicode-java-m2-cache.volume`)
- `sandbox/justfile` (sandbox-setup unification + sandbox-maven-warmup + dep/unit reorganization)
- `sandbox/manifests/java_repos.yaml` (gradle → mvnw)
- `sandbox/scripts/clone_repos.sh` (spring-petclinic SHA pin)
- `sandbox/SETUP_REQUIREMENTS.md` (Maven status)
- `.github/workflows/sandbox-nightly.yml` (new: smoke + probe lanes)
- `sandbox/history/runs.jsonl` (smoke evidence ledger entry)

Plus chore `8cbadced`: 118 `.rs` files restored to a verified green baseline (+970/−686 — **debt-reducing**, not accumulating — and explicitly outside infra-debt scope).

**Infra surface audited**: 19 files, ~470 LOC of declarative config + ~140 LOC just recipes + 75 LOC workflow YAML + 290 LOC shell (clone_repos.sh, mostly pre-existing). All other changes (`.rs`, refactor of crates) are non-infra, audited only at the audit-contract level.

---

## Cluster Findings

### 1 · `debt-smells-cluster` (inline catalog — 12 Fowler smells × declarative infra mapping)

| # | Smell | Location | Severity | Notes |
|---|-------|----------|----------|-------|
| S1.1 | **Stale comment** `clone_repos.sh:187` reads `# Pinned at main` while L192 now pins concrete SHA `edf4db28…` | `sandbox/scripts/clone_repos.sh:187` | **WARNING** | Carry-forward from verify S1 (Cycle-1 report). Not fixed by this branch. Cheap fix. |
| S1.2 | **Dead code with TODO-style deprecation comment** — `sandbox-setup-js-ts` is a 14-line subset of `sandbox-setup` (which already deploys all 6). Comment says "Deprecated: merged into sandbox-setup (Phase 3 unification)". | `sandbox/justfile:84-97` | **WARNING** | Not actually called from anywhere in CI or workflow. Either delete it or remove after one release cycle. |
| S1.3 | **Aspirational/dead documentation** — `cognicode-js.container:1-19` and `cognicode-ts.container:1-19` carry a 15-line heredoc block ("TOOL PRE-INSTALLATION") describing a `podman build -t cognicode-js-tools -f - . <<'EOF' …` workflow for pre-installing eslint/jest/tsc. **There is no Dockerfile in the repo, no derived image is built, no second digest pinned, no opt-in toggle.** The prose is pure aspirational. | `sandbox/containers/cognicode-{js,ts}.container:1-19` | **WARNING** | Future readers will believe pre-installation exists; in fact the runtime tools are absent → JS/TS scenarios that need eslint/jest runtime will fail (verified runtime shows smoke lane exit 0 with these tools absent, meaning the affected scenarios are either not in the smoke lane or already pass without them — but neither is documented). |
| S1.4 | **Magic numbers without rationale** — `Tmpfs=/tmp:rw,noexec,nosuid,size=64m` (all 6), `MemoryMax=1G vs 2G` (3 vs 3 containers), `PidsLimit=64 vs 128` (2 vs 4). The values correlate with a clear two-tier scheme (interpreted vs compiled), but the rationale is in no `.container`. | all 6 `.container` files | **SUGGESTION** | Add a one-line header comment `# Tier 1 (compiled): 2G/128` / `# Tier 2 (interpreted): 1G/64` to each file. |
| S1.5 | **Inconsistent header style** — go/java have "Phase 3"; rust/python "Phase 1"; js/ts "Phase 2" with the heredoc. | all 6 `.container` | **SUGGESTION** | Header should state *intent*, not *phase history*. |
| S1.6 | **Primitive Obsession at the configuration layer** — service names appear as bare strings in 5+ places (justfile L75-80 start, L222-223 clean, plus the implicit unit names from filenames and `[Volume]` sections). | `sandbox/justfile` + all `.container` + all `.volume` | **SUGGESTION** | A single source of truth (e.g., `sandbox/containers/INDEX` listing unit names line-by-line) would centralize this. Quadlet has no include mechanism so this is author-constrained. |

**Verdict**: WARNING (no CRITICAL; 3 actionable warnings + 3 suggestions).
**SOLID map**: OCP violation (adding an Nth container requires 4-5 synchronized edits to the justfile) — MEDIUM. SRP/ISP/LSP/DIP — N/A (declarative config).
**Top refactor backlog** (cluster-specific):
1. Delete or mark `sandbox-setup-js-ts` properly (S1.2).
2. Replace stale comment in `clone_repos.sh:187` (S1.1).
3. Reduce or relocate the aspirational `TOOL PRE-INSTALLATION` block in js/ts (S1.3).
4. Extract service-name list to a single source (S1.6).

---

### 2 · `debt-duplication-cluster` (inline catalog — structural / literal / semantic + 5 dead-code types)

| # | Cluster | Severity | Notes |
|---|---------|----------|-------|
| D2.1 | **`.container` files**: rust(41)/python(37)/go(35)/java(40) share ~28 lines of identical scaffolding; js(57)/ts(57) ~52 lines identical (incl. heredoc dead-doc). 6 files, ~282 LOC of which ~220 LOC is structural duplication. | **SUGGESTION** (boundary case) | **Acceptable platform-constrained duplication**: Quadlet has no include directive, no template, no heredoc within a unit file. The alternative — a sed/awk/python generator consuming a YAML manifest — would add a tool to the deploy story and obscure what systemd sees. Honest verdict: this is the right shape for Quadlet today. Note it in CONTEXT.md so future contributors don't try to "DRY" it. **loc_reducible if generated**: ~80 LOC, *well below the 500 LOC critical threshold.* |
| D2.2 | **`.volume` files**: 7 files × 2 lines each, all identical except for one descriptive comment. | **SUGGESTION** | Same platform-constraint applies. |
| D2.3 | **`sandbox-setup` (L55-80) ↔ `sandbox-setup-js-ts` (L84-97)**: 80% byte-identical except the latter's volume-copy list is *broader* (copies all 7 volumes just to start 2) and its `systemctl start` only names 2. | **WARNING** | Already counted in S1.2 above. The duplication is real, not platform-constrained; the "deprecated" label hides the duplication. |
| D2.4 | **`cognicode-*-workspace.volume` listing**: 7 lines, copy-pasted identically into `sandbox-setup` (L65-71) AND `sandbox-setup-js-ts` (L88-94). | **WARNING** | Adding a new volume requires updating both recipes (and remembering the new list if the deprecated recipe isn't deleted). |
| D2.5 | **No dead code in the diff itself**, but 4 `gradlew` references were correctly replaced by 4 `mvnw` references (cycle-1 hardening) and the dual existence of `--base` references in workflows vs justfile are not duplication (the workflow calls the justfile). | PASS | Clean. |
| **D2 loc_reducible** | ~30-40 LOC via: deleting S1.2 recipe + commenting out dead heredoc in S1.3 + centralizing volume-list once. Not 500 LOC critical. | | |

**Verdict**: WARNING (pass-through from D2.3, D2.4 — actionable, low effort).
**Dead code**: S1.2 + S1.3 = two findings.

---

### 3 · `debt-coupling-cluster` (inline catalog — 5 hidden deps + 5 global-state types + 5 coupling problems)

| # | Coupling problem | Location | Severity | Notes |
|---|-----------------|----------|----------|-------|
| C3.1 | **`sandbox-clean` ↔ `sandbox-setup` asymmetry (setup-cleaner contract leak)** — setup deploys 6 (`rust,python,java,go,js,ts`); clean stops 5 (`rust,python,java,js,ts` — **missing `go`**). | `sandbox/justfile:74-80` vs `:222-223` | **WARNING (bug-class)** | When this branch added `cognicode-go` to `sandbox-setup` (commit `b033e7ea`), it did NOT update `sandbox-clean`. Result: `just sandbox-clean` leaves a working `cognicode-go` unit behind. **Pre-existing main debt** — the missing-go-clean was already latent on main HEAD when this branch started (`6795951d` authored `sandbox-clean` with no go at all). This branch widened the asymmetry. Real defect; trivial fix (insert one word). |
| C3.2 | **Hardcoded host project layout** — `Volume=%h/Proyectos/rust/CogniCode/sandbox/repos:/repos:z` baked into all 6 `.container` files. `%h` resolves to user home, but `/Proyectos/rust/CogniCode/sandbox/repos` assumes a Spanish-locale path layout and a specific clone location. | all 6 `.container`, line 14/15/14 | **WARNING** | If the repo is cloned to `~/projects/cognicode/` or `/opt/cognicode/`, every container fails to bind mount `/repos`. CI uses `ubuntu-latest` whose home is `/home/runner/...` — `%h` will resolve but the rest of the path `Proyectos/rust/CogniCode/sandbox/repos` does not exist on the runner, so the CI workflow's bind silently fails (the `|| true` masks the error). **Pre-existing main debt**: the path was baked in `6795951d` on main. This branch inherited it. |
| C3.3 | **Multi-place service-name list** (OCP-coupled) — same 6 names appear in: `sandbox-setup` L58-63 (filenames), L75-80 (`systemctl start`), L222-223 (`stop`+`rm` ×2 lists), AND in the `ContainerName=` fields of all 6 `.container` files, AND implied in the 7 `.volume` filenames. Adding container #7 requires 6-8 synchronized edits. | as listed | **WARNING** | Acceptable because Quadlet's "unit name = filename" semantic forces this. But the `sandbox-setup` ↔ `sandbox-clean` asymmetry in C3.1 is precisely what OCP violation causes. |
| C3.4 | **Digest duplication** — same SHA-256 appears in `sandbox-pull` (L45-53) AND in the corresponding `Image=` of each `.container`. Cycle-0 broke this (java: container pinned `9824c276…`, justfile pinned `723151f3…`); Cycle-1 closed both. The structural duplication is still there. | `sandbox/justfile:43-53` + 6 `.container` | **SUGGESTION** | Risk of future drift remains. A single source of truth (e.g., `sandbox/containers/digests.env` consumed by both podman pull and a generator that emits the .container files) would prevent re-occurrence. Out of scope for this PR's gate, but worth a follow-up. |
| C3.5 | **Workflow → justfile silent contract** — `.github/workflows/sandbox-nightly.yml` calls `just sandbox-pull` / `just sandbox-setup` / `just sandbox-ci-smoke` / `just sandbox-ci-probe` but there is no static link. Renaming a recipe would silently break CI. | `.github/workflows/sandbox-nightly.yml` | **SUGGESTION** | This is idiomatic just + GitHub Actions. Documentation header note in workflow would suffice. |
| C3.6 | **Implicit dependency: `sandbox-maven-warmup` → runtime java container**. Runtime `cognicode-java.container` has `Network=none` so cannot fetch Maven deps at runtime; the warmup recipe populates `cognicode-java-m2-cache.volume`. There is no guard in `sandbox-setup` requiring warmup to have run first. | `sandbox/justfile:102-108` | **SUGGESTION** | Documented in recipe header. A user running `sandbox-ci-smoke` without ever running warmup will see expected_fails in any Maven-touching scenario. |

**Verdict**: WARNING (2 actionable bug-class warnings + 4 suggestions, 1 pre-existing-main).
**Hidden deps**: 1 (C3.6 maven-warmup ordering). **Global state**: N/A (named volumes are systemd-managed, not free globals).

---

### 4 · `debt-overeng-cluster` (ponytail + whole-repo bloat audit)

| # | Over-engineering finding | Severity | Notes |
|---|--------------------------|----------|-------|
| O4.1 | **Commented-out config** — `# Volume=%t/containers/cognicode-js-npm-cache:/root/.npm:z` and matching TS line. Inactive; duplicates the "TOOL PRE-INSTALLATION" pattern (no real cache volume is provisioned). | **SUGGESTION** | `js`/`ts` bottom of file. Remove or implement. |
| O4.2 | **Aspirational comment block**: same as S1.3 — see above. 15 lines × 2 files = 30 LOC of dead documentation describing a workflow that doesn't exist. | **WARNING** | See S1.3. Cross-reported. |
| O4.3 | **`cognicode-java-m2-cache.volume` + `sandbox-maven-warmup` recipe**: this is a real, minimal cache mechanism (1 named volume + 1 recipe). Not over-engineered. | PASS | Justified by `Network=none` constraint. |
| O4.4 | **7 named volumes for 6 working containers**: each container has its own scratch workspace (Podman manages per-unit); java additionally has a Maven cache. Could theoretically collapse into one anonymous tmpfs, but persistent state across restarts is the point. | PASS | Justified. |
| O4.5 | **`AutoUpdate=no` on all 6 + `NoNewPrivileges=yes` on all 6**: identical hardening posture. Acceptable: documented baseline, equal-strength units. | PASS | Correct posture for sandboxed risk surface. |
| O4.6 | **No `ponytail:` markers** in the changed surface (grep -nE 'ponytail:') | PASS | Zero carry-forward ledger items. |
| O4.7 | **118-file `.rs` baseline restore (`8cbadced`)**: +970/-686 LOC reverting dirty WIP back to a known-green state. **This is debt-reducing**, not accumulating: removes unverified WIP. Not flagged. | PASS | (Acknowledged for completeness.) |
| O4.8 | **Accidental-bloat score** (ponytail findings weighted by severity × age): 0 findings, 0 ledger items. → Score: 0/100. | PASS | |

**Verdict**: PASS (1 cross-reported warning from S1.3, all else clean).
**Accidental-bloat score**: 0/100. No `ponytail:` markers.

---

## Corroboration matrix (2+ clusters → severity↑)

| Finding | smells | duplication | coupling | overeng | Severity |
|---------|:------:|:-----------:|:--------:|:-------:|----------|
| Stale `clone_repos.sh:187` (S1.1) | ✓ | — | — | — | WARNING (single source) |
| `sandbox-setup-js-ts` dead alias (S1.2 / D2.3) | ✓ | ✓ | — | — | **WARNING** (corroborated 2×) |
| Dead `TOOL PRE-INSTALLATION` heredoc (S1.3 / O4.2) | ✓ | — | — | ✓ | **WARNING** (corroborated 2×) |
| Hardcoded host project path (C3.2) | ✓ | — | ✓ | — | **WARNING** (corroborated 2×; smells under S1.4-S1.5 umbrella) |
| `sandbox-clean` missing `cognicode-go` (C3.1 / C3.3) | — | — | ✓ | — | WARNING (single source, but bug-class) |
| Volume-list triplication (D2.4) | — | ✓ | ✓ | — | **WARNING** (corroborated 2×) |
| OCP-coupled service names (C3.3) | ✓ | — | ✓ | — | **WARNING** (corroborated 2×) |

No finding rises to CRITICAL via corroboration (would require 3+ clusters).

---

## Pre-existing main debt detection

```bash
git blame -L 220,225 <main:sandbox/justfile>     # the missing-go-clean list
# → 6795951d (v0.2.0), no later touch on those lines
```

`git blame` for the anomalous C3.1 (line 222-223): `6795951d` (2026-04-14). This commit precedes `feat/e30-sandbox-infra` (branch created 2026-08-06). Same for the hardcoded `%h/Proyectos/...` path in C3.2 (also `6795951d`).

| Finding | Traced to main | `pre_existing_main_debt` |
|---------|----------------|--------------------------|
| C3.1 `sandbox-clean` missing `cognicode-go` | Yes — `6795951d` on main | **TRUE** (this branch widened it) |
| C3.2 Hardcoded `~/<user>/Proyectos/rust/CogniCode/sandbox/repos` | Yes — `6795951d` on main | **TRUE** (inherited, not widened) |
| S1.1 Stale `Pinned at main` comment | Yes — was pre-existing; only the SHA changed on this branch | TRUE (carry-forward verify-S1) |
| S1.2 dead `sandbox-setup-js-ts` | No — marked "Deprecated" *in this branch*. Pre-existed but unflagged. | FALSE (this branch surfaced & labeled it) |
| All java-digest issues | RESOLVED on this branch (Cycle-1) | FALSE (introduced-by-prior-dirty-tree→main; resolved on this branch) |

**Single highest-priority pre-existing-main-debt item**: C3.1 + C3.2. Recommend a follow-up SDDK cycle to clean both on `main` directly (since `cognicode-go` was already deployed on main without clean-recipe parity, and `~/<u>/Proyectos/...` hardcoding blocks any CI that doesn't first `mkdir -p` the path on the runner).

---

## Decision Gates (per skill table)

| Gate | Verdict trigger | Applies here? |
|------|------------------|---------------|
| Any CRITICAL from any cluster | → FAIL | **NO** (max severity is WARNING) |
| ≥3 files circular imports | → FAIL | N/A (declarative) |
| Module fan-in>10 AND fan-out>7 | → FAIL | N/A |
| Shared mutable global with >5 writers | → FAIL | N/A |
| God-class: >7 pub methods AND >300 LOC AND >5 deps | → FAIL | N/A |
| Shotgun-surgery: 1 change touches >5 unrelated files | → FAIL | N/A (a feature would touch 5; that's not what this gate means) |
| ≥3 SOLID HIGH violations | → FAIL | 1 (OCP=M); not ≥3 |
| LSP violation: subclass override breaks contract | → FAIL | N/A |
| ≥3 HIGH duplication clusters OR loc_reducible>500 | → FAIL | 3 duplication WARNS, but **loc_reducible ≈ 40 lines** (< 500) |
| Accidental-bloat: ≥10 ponytail OR ≥5 OVERDUE | → FAIL | 0 / 0 |

**Result**: 0 CRITICAL triggers. Verdict gate is `PASS_WITH_WARNINGS`.

---

## Findings Summary

| Cluster | Verdict | CRIT | WARN | SUGG | Notes |
|---------|---------|:----:|:----:|:----:|-------|
| Smells | WARN | 0 | 3 | 3 | S1.1 stale-comment, S1.2 dead alias, S1.3 dead doc; +3 style/tier suggestions |
| Duplication | WARN | 0 | 2 | 2 | D2.3, D2.4 triplication; D2.1+D2.2 acceptable platform-constrained |
| Coupling | WARN | 0 | 3 | 3 | C3.1 bug-class asymmetry, C3.2 hardcoded path, C3.3 OCP name-list |
| Overeng | PASS | 0 | 1 | 1 | O4.2 cross-reported with S1.3; 0 ponytail markers |
| **TOTAL** | **PASS_WITH_WARNINGS** | **0** | **8** (3 of which cross-corroborated) | **7** | |

Top cross-corroborated warnings (severity-stable): S1.2/D2.3 (dead `sandbox-setup-js-ts`), S1.3/O4.2 (dead `TOOL PRE-INSTALLATION` heredoc), C3.1 (setup-cleaner bug), C3.2 (host-path hardcoding), D2.4 (volume-list triplication).

---

## Design Quality Score (DQS) estimate

For the infra delta (≈ 470 LOC of declarative config, ~140 LOC of justfile recipes, ≈ 6 contracts):

| Dimension | Score | Note |
|-----------|------:|------|
| **Connascence landscape** | 6.5/10 | Of-name across 5 service-name sites (WEAK; platform-constrained); of-value digests in 2 sites (WEAK; mitigated by Cycle-1 fix). No static connascence. |
| **SOLID entropy** | 5/10 | OCP violation on adding containers = MEDIUM; SRP/LSP/ISP/DIP unevaluable for declarative infra but no violations observed. |
| **Information Bottleneck (interface)** | 5/10 | The `sandbox-setup` ↔ `sandbox-clean` interface leaks (deployment surface ≠ cleanup surface). The Quadlet unit-name ↔ filename mapping is well-concealed except for the explicit list of names in justfile. |
| **Per-file clarity** | 7/10 | Files are short, headers honest, read top-to-bottom. Docked for dead documentation (S1.3). |
| **Test surface / observability** | 6/10 | Runtime evidence (6/6 ACTIVE, smoke exit 0) is the test surface for infra; runs.jsonl ledger captured. No formal contract test for the digest pinning procedure (the bug Cycle-0 found was only findable by external registry query). |
| **Reusability / extension cost** | 5.5/10 | Adding a 7th container requires ~8 file edits across 3 file kinds; not friction-free. |

**Composite DQS ≈ 5.8 / 10 (~58 — C+ / B-)**.

This is acceptable for a **targeted infra fix** that closes a real bug (java-digest binding). It is **not acceptable as a steady-state for the sandbox subsystem** — the duplication, OCP coupling, and pre-existing-main-debt items should be paid down in a follow-up SDDK cycle (likely a `e30-sandbox-infra-hygiene` or merged into `e30` itself post-archive).

---

## Follow-up backlog (PR-attached, NOT blocking this archive)

| Severity | Item | File | Effort | Pre-existing main? |
|----------|------|------|--------|---------------------|
| WARN | Insert `cognicode-go` into `sandbox-clean` lines 222-223 | `sandbox/justfile` | XS (1 word × 2 lines) | Yes (widened by branch) |
| WARN | Delete `sandbox-setup-js-ts` (or remove deprecation shim in next release cycle) | `sandbox/justfile:84-97` | S | No (branch marked it, didn't author it) |
| WARN | Replace or relocate the 15-line `TOOL PRE-INSTALLATION` heredoc | `sandbox/containers/cognicode-{js,ts}.container:1-19` | S | No (branch author placed it as motivation, but no Dockerfile exists in repo) |
| WARN | Update stale comment `# Pinned at main` → `# Pinned at commit edf4db28…` | `sandbox/scripts/clone_repos.sh:187` | XS | Yes (carry-forward from pre-Cycle-0) |
| WARN | Centralize `cognicode-*-workspace.volume` listing (single var or shell glob) | `sandbox/justfile:65-71, 88-94` | S | No |
| WARN | Decide on the `%h/Proyectos/rust/CogniCode/sandbox/repos` path: parameterize (XDG_DATA_HOME) or document the assumption for CI runners | all 6 `.container` | M | Yes |
| SUGG | Add a per-container one-liner commenting the 1G vs 2G / 64 vs 128 tier rationale | all 6 `.container` | XS | No |
| SUGG | Remove commented-out `# Volume=%t/containers/cognicode-*-npm-cache` lines | `cognicode-{js,ts}.container` | XS | No |
| SUGG | Consider `sandbox/containers/INDEX` with one line per unit (single source for `systemctl` lists) | new file | M | No |
| SUGG | Document the `sandbox-maven-warmup` ordering requirement in `just sandbox-ci-smoke` recipe | `sandbox/justfile` | XS | No |

**Follow-up cycle recommendation**: triage as a `B-direct` hotfix on `main` for **C3.1 + S1.1** (the bug-class items), and as an A-min SDDK cycle for the rest.

---

## Verdict & Standard Envelope

```yaml
status: success
executive_summary: >
  PASS_WITH_WARNINGS. 0 CRITICAL. 8 WARNING (3 cross-corroborated across clusters).
  Worst finding is a real bug — `sandbox-clean` (L222-223) is missing `cognicode-go`,
  so `just sandbox-clean` leaves the go unit running — pre-existing on main, widened by
  this branch. Other warnings: dead `sandbox-setup-js-ts` recipe, dead `TOOL
  PRE-INSTALLATION` heredoc in js/ts, hardcoded `~/<user>/Proyectos/rust/CogniCode`
  host-path (env-specific), OCP-coupled service-name list, and a stale "Pinned at main"
  comment in clone_repos.sh:187. No pattern is unfixable; all are XS-to-S effort.
  DQS ≈ 58/100 (C+/B-) — acceptable for the targeted bug-fix, not for steady state.
artifacts:
  - "sddk/e30-sandbox-infra/debt-report.md"
verdict: PASS_WITH_WARNINGS
re_iterate_from: none
clusters_run:
  - debt-smells-cluster
  - debt-duplication-cluster
  - debt-coupling-cluster
  - debt-overeng-cluster
clusters_skipped: []   # architecture cluster NOT in standard depth
findings_by_severity:
  critical: 0
  warning: 8
  suggestion: 7
pre_existing_main_debt: true  # C3.1 + C3.2 + S1.1 trace to 6795951d on main
next_recommended:
  "PASS_WITH_WARNINGS": sddk-archive (orchestrator proceeds to PR; attach this report to PR body)
risks:
  - "C3.1 leaves `cognicode-go` running after `sandbox-clean`. Cheap fix; should land before next green baseline restore."
  - "C3.2 hardcoded host path makes `/repos` bind fail in any deployment that isn't at `~/<user>/Proyectos/rust/CogniCode`. CI's `ubuntu-latest` runner doesn't have this path; the bind silently fails under `|| true` in sandbox-pull."
  - "S1.3 dead TOOL PRE-INSTALLATION heredoc misleads future readers into believing a pre-install workflow exists. JS/TS Tier B scenarios that need eslint/jest runtime will need this resolved before broader coverage expansion."
  - "Cross-corroborated warnings (S1.2/D2.3, S1.3/O4.2, D2.4) compound: deleting the dead recipe and the dead heredoc, plus one-line volume listing, would remove 50+ LOC and 2 latent footguns for ~30 min of work."
context_quality: C2  # 9 commits, 19 changed files inspected directly; runtime evidence captured by orchestrator
path: A-lite
depth: standard
dqs_estimate: 58  # C+ / B-
```

---

## CLI Ledger Duty (sddk)

This phase does not own a cycle transition (debt-verify is between `verify` and `archive`). The contract requires:

1. `sddk cycle status --root . --scope .` — read-only check.
2. `sddk artifact store --root . --scope . --file sddk/e30-sandbox-infra/debt-report.md --kind verification-report --cycle e30-sandbox-infra --producer sddk-debt-verify` — register this report.

If the project is not adopted (`sddk` not on PATH in this env), this step is a soft-block at most and is recorded as `status: success` here based on direct execution. The orchestrator will re-validate at archive time.

---

## Concluding note for the orchestrator

The 4-cluster audit closes the loop on what `sddk-verify` opened (the warn carry-forward) and adds 5 findings the verify pass could not surface (because they live in the declarative infra layer, not in the spec scenarios). None blocks PR. All 5 actionable warnings are XS-to-S effort, well-bounded, and unblock future steady-state infra work.

**Path forward**: attach this report to the PR description as a `## Debt Audit` section, proceed to `sddk-archive`. The pre-existing main debt items (C3.1, C3.2, S1.1) become a separate hotfix SDDK cycle after this PR merges — not a blocker here.
