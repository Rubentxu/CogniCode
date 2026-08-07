# Archive Report — e30-metric-baseline

**Change**: e30-metric-baseline
**Branch**: `feat/e30-metric-baseline`
**Head**: `5de8c429` (5 commits, pushed)
**Archived**: 2026-08-06
**Status**: COMPLETED — Release Required

---

## Delta Spec Sync

| Domain | Status | Details |
|--------|--------|---------|
| `release-readiness-gate` | ✅ Already synced | Automated Scorecard Engine req (line 161: `release_scorecard.py`) present in main spec |
| `sandbox-validation-system` | ✅ Already synced | MemoryMax=4g raised from 2g (line 11: "...raised from 2g in e30-metric-baseline") present in main spec |

**Conclusion**: T10 openspec sync was flagged as pending in verify report, but main specs already reflect the e30-metric-baseline changes. Delta spec sync marked COMPLETED.

---

## Deliverables

| Deliverable | Status | Evidence |
|-------------|--------|----------|
| Scorecard engine (12 gates, G1–G12) | ✅ | `sandbox/scripts/release_scorecard.py` |
| Baseline frozen | ✅ | `sandbox/results/baseline/` |
| 3 measurement campaigns | ✅ | `sandbox/results/campaign-{1,2,3}/` |
| Stability analysis | ✅ | `sandbox/results/stability.json` (G6: 4.7% max CV) |
| G8 probe (typescript tier-3) | ⚠️ AMBER | Transient result.json; SCAL-001 defect logged |
| Container memory upgrade (4G) | ✅ | `cognicode-{js,ts}` 1G→4G, `cognicode-rust` 2G→4G |

---

## Scorecard Results

- **Verdict**: PASS_WITH_WARNINGS (17/19 scenarios compliant)
- **CRITICALs**: 0
- **G3 Health Score**: 66.04 / 66.1 / 66.1 (campaigns 1/2/3)
- **G4 Correctness**: 18.9 (AMBER — baseline measured, not defect)
- **G5 Latency**: search p95=31049ms (RED — baseline measured, not defect)
- **G6 Stability**: 4.7% max CV ✅
- **G8 Scalability**: AMBER (transient result.json, SCAL-001 tracked)

---

## Debt-Verify R1 Summary

- **Verdict**: PASS_WITH_WARNINGS
- **CRITICAL C-01**: CLOSED (spec scope pollution — 63 unrelated openspec docs removed)
- **DQS**: 0.74 (GOOD)
- **Archive**: UNBLOCKED

### Carry-Forward Warnings (W-01 → W-08)

| ID | Warning | Priority | Remediation |
|----|---------|----------|-------------|
| W-01 | 656-line scorecard mixes policy/I/O/rendering without tests | e30.2 medium | Add gate unit tests |
| W-02 | `analyze_stability.py` does NOT emit `families_runtorun` — G6 value ad-hoc | e30.2 high | W-02: Fix stability script reproducibility |
| W-03 | `full-run-N`/`run-N`/`campaign-N` naming drift | e30.2 low | Unify run discovery path |
| W-04 | SCAL-001/INF-001/LAT-001/LAT-002 not committed — scorecard emits generic "defect tracked" | e30.2 high | W-04: Commit defect tracker artifacts |
| W-05 | Optional evidence silently degrades to AMBER | e30.2 medium | Explicit degrade-to-amber logging |
| W-06 | Brittle G11 literal checks | e30.2 low | Parameterize literal thresholds |
| W-07 | Duplicated repeat orchestration | e30.2 medium | Deduplicate orchestration paths |
| W-08 | Zero committed test files | e30.2 critical | W-08: Write gate tests FIRST |

**e30.2 Priority Order**: W-08 (tests) → W-02 (stability script) → W-04 (defect IDs) → W-01/W-05/W-07 → W-03/W-06

---

## Incidences Found

| ID | Classification | Status | Description |
|----|----------------|--------|-------------|
| INC-001 | LAT-001 | Tracked | Launch latency defect — baseline measured, not unimplemented |
| INC-002 | LAT-002 | Tracked | Search latency defect — baseline measured, not unimplemented |
| INC-003 | INF-001 | Tracked | Infrastructure instability — transient G8 probe results |
| INC-004 | SCAL-001 | Tracked | Scalability boundary — typescript tier-3 timeout, result.json transient |

---

## Knowledge Graph Nodes

### Cycle
- **ID**: CYC-2026-08-06-e30-metric-baseline
- **Status**: completed
- **Path**: A-lite
- **Branch**: `feat/e30-metric-baseline`
- **Head**: `5de8c429`

### Milestone
- **ID**: M-E30-Fase-3
- **Status**: completed

### Incidences
- INC-001 (LAT-001) — Tracked
- INC-002 (LAT-002) — Tracked
- INC-003 (INF-001) — Tracked
- INC-004 (SCAL-001) — Tracked

---

## Artifacts (Engram)

| Artifact | Engram Topic |
|----------|--------------|
| proposal | `e30-metric-baseline-proposal` |
| spec (delta) | `spec/e30-metric-baseline` |
| verify-report | `sddk/e30-metric-baseline/verify-report` |
| debt-report | `sddk/e30-metric-baseline/debt-report` |

---

## Specs Made Stale

None. The main specs (`release-readiness-gate`, `sandbox-validation-system`) were updated by this cycle.

## ADRs Touched

- ADR-031 (release-readiness-gate framework) — maintenance recommended
- ADR-032 (corpus expansion) — maintenance recommended

---

## Jurisprudence Candidate

**No**. The verify verdict was PASS_WITH_WARNINGS (not PASS with first_pass_success=true). The decision to use ad-hoc G6 value with fallback chain is not reusable without modification.

---

## Archive Complete — Release Required

The e30-metric-baseline cycle has been planned, implemented, verified, and archived. The cycle remains open until mandatory `sddk-release` completes.

**Next**: `sddk-release e30-metric-baseline`
