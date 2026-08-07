# Release Report — e30-metric-baseline

**Change**: `e30-metric-baseline`
**Branch**: `feat/e30-metric-baseline`
**Cycle**: `CYC-2026-08-06-e30-metric-baseline`
**Milestone**: `M-E30-Fase-3`
**Released**: 2026-08-06
**Status**: ✅ SUCCESS — Released as v0.89.0

---

## Result

| Field | Value |
|-------|-------|
| **PR** | [#230](https://github.com/Rubentxu/CogniCode/pull/230) |
| **Branch head SHA** | `5de8c429fb202adbbfd1e2b51bebba84d66fe99a` |
| **Merge SHA** | `09b701c52bf06f7e372f6e3757067742612fd676` |
| **Merge mode** | `--auto --merge` |
| **Merged at** | 2026-08-06T15:15:31Z |
| **Merged by** | Rubentxu |
| **Tag** | `v0.89.0` |
| **Tag target** | `09b701c52bf06f7e372f6e3757067742612fd676` (merge commit) |
| **Semver bump** | **MINOR** (engine de scorecard + baseline — primera medición real) |
| **Base** | `main` @ `a8140d19c62137b8e78391f0959c335df5e7fd2b` |
| **Trunk sync** | ✅ `HEAD == origin/main` |

## Ancestry verification

```
git merge-base --is-ancestor 5de8c429 09b701c5  → OK (branch_head → merge_sha)
git merge-base --is-ancestor 09b701c5 origin/main → OK (merge_sha → origin/main)
git merge-base --is-ancestor 5de8c429 origin/main → OK (branch_head → origin/main)
```

---

## Release Checklist — 12/12 steps

| # | Step | Status | Evidence |
|---|------|--------|----------|
| 1 | Verify preconditions | ✅ | archive-report: PASS_WITH_WARNINGS (17/19) |
| 2 | Detect merge policy | ✅ | auto (locked, repo sin branch protection) |
| 3 | push-branch | ✅ | origin/feat/e30-metric-baseline == 5de8c429 |
| 4 | create-or-reuse-pr | ✅ | PR #230 creado (estado inicial OPEN, MERGEABLE) |
| 5 | merge-pr (5a/5b/5c) | ✅ | `--auto --merge` aplicado; state=MERGED inmediatamente (sin required checks bloqueantes) |
| 6 | verify-merge | ✅ | 3/3 ancestry checks OK |
| 7 | semver-tag | ✅ | v0.89.0 anotado, pusheado, target = MERGE_SHA |
| 8 | html-closing-report | ✅ | `openspec/changes/archive/2026-08-06-e30-metric-baseline/reports/cierre.html` |
| 9 | close-tracking-issue | ⏭️ no-op | No hay tracking issue abierta |
| 10 | update-knowledge-graph | ✅ | Vault actualizado: cycle, milestone, incidences, ADRs |
| 11 | release-lock | ✅ | Lock permanece AVAILABLE (no se adquirió en archive) |
| 12 | trunk-sync-end | ✅ | `git checkout main && git pull` → HEAD = `09b701c5` = origin/main |

---

## Deliverables shipped

| # | Componente | Path |
|---|------------|------|
| 1 | Motor scorecard (12 gates G1-G12) | `sandbox/scripts/release_scorecard.py` |
| 2 | Baseline congelado | `sandbox/results/baseline/` |
| 3 | Campañas de medición (1/2/3) | `sandbox/results/campaign-{1,2,3}/` |
| 4 | Análisis de estabilidad | `sandbox/results/stability.json` (G6 max CV 4.7%) |
| 5 | Containers 4G (cognicode-js/ts/rust) | `sandbox/containers/cognicode-{js,ts,rust}.container` |
| 6 | Justfile wiring | `sandbox/justfile` (+112/-X líneas) |
| 7 | Run campaign script fix | `sandbox/scripts/run_campaign.sh` |
| 8 | Spec release-readiness-gate sync | `openspec/specs/release-readiness-gate/spec.md` (+10) |
| 9 | Spec sandbox-validation-system sync | `openspec/specs/sandbox-validation-system/spec.md` (+2) |

**Diff stats** (vs main base `a8140d19`):
- 8 files changed, 747 insertions(+), 47 deletions(-)

---

## Scorecard snapshot

**Verdict**: PASS_WITH_WARNINGS (17/19 escenarios) — **6G / 3A / 3R**

| Gate | Estado | Métrica |
|------|--------|---------|
| G3 Health | ⚠️ AMBER | 66.04 / 66.1 / 66.1 (campaigns) |
| G4 Correctness | ⚠️ AMBER | 18.9 (baseline measured) |
| G5 Latency | 🔴 RED | search p95 = 31049 ms (LAT-001, LAT-002) |
| G6 Stability | ✅ | max CV 4.7% |
| G8 Scalability | ⚠️ AMBER | typescript tier-3 timeout (SCAL-001) |

## Defects tracked (carry-forward)

| ID | INC | Tipo | Prioridad e30.2 |
|----|-----|------|-----------------|
| LAT-001 | INC-001 | Launch latency | W-04 (high) |
| LAT-002 | INC-002 | Search latency | W-04 (high) |
| INF-001 | INC-003 | Infra instability | W-04 (high) |
| SCAL-001 | INC-004 | Scalability | W-04 (high) |

## Carry-forward warnings (W-01..W-08)

| ID | Warning | Prioridad |
|----|---------|-----------|
| W-08 | Zero committed test files | **critical** |
| W-02 | `analyze_stability.py` no emite `families_runtorun` | **high** |
| W-04 | Defect IDs no committed | **high** |
| W-01 | scorecard sin tests | medium |
| W-05 | Optional evidence degrada a AMBER silencioso | medium |
| W-07 | Orquestación duplicada | medium |
| W-03 | Naming drift | low |
| W-06 | G11 literal checks brittle | low |

**e30.2 priorities**: W-08 → W-02 → W-04 → W-01/W-05/W-07 → W-03/W-06

---

## Semver reasoning

- Outer scope: **`feat(sandbox)`** → MINOR per atomic-semver rules
- No `BREAKING CHANGE:` footer en ningún commit
- Engine de scorecard + baseline congelado = primera medición real del sandbox (capacidades nuevas)
- MINOR → **v0.89.0** (desde v0.88.1)

---

## Next milestone

**Fase 4 — e30-conformance-audit** (siguiente ciclo):
- Resolver W-08 (tests) → W-02 (stability) → W-04 (defect IDs)
- Auditar conformidad de los 3 defectos trackeados
- Cerrar carry-forward antes de e30.2

---

## Risks

None blocking. Carry-forward W-01..W-08 tracked for e30.2.

## Artifacts persisted

- `openspec/changes/archive/2026-08-06-e30-metric-baseline/release-report.md` (this file)
- `openspec/changes/archive/2026-08-06-e30-metric-baseline/reports/cierre.html` (HTML closing report)
- `~/.sddk-knowledge/CogniCode/cycles/CYC-2026-08-06-e30-metric-baseline.md`
- `~/.sddk-knowledge/CogniCode/milestones/M-E30-Fase-3.md`
- `~/.sddk-knowledge/CogniCode/incidences/INC-{001..004}-*.md`
