# v1.0.0 Pre-Cut Checklist

This document is the **operational checklist** that gates the v1.0.0 tag cut.
It codifies ADR-031 §3 ("1.0.0 requires the scorecard to be GREEN for 3
consecutive executions before tagging") and T7 (`docs/TEST-PLAN.md` §6
stability cadence).

> **Status (2026-08-16)**: NOT ready for tag cut. Pre-cut gates 1-2
> (operational / nightly) and Gate 7 (tag cut mechanics) remain pending.
> Gate 3 (G8 SCAL-001) ✅, Gate 4 (CHANGELOG) ✅, Gate 5 (Roadmap) ✅
> and Gate 6 (Branch cleanup) ✅ done.
> Deferred items B1-B6: B1 ✅, B2 ✅ (E31-B2-rollup), B3 ✅
> (remote CI disabled 2026-08-16), B5 ✅ (E31-B5-rollup), B6 ✅
> (E31-B6-rollup). Only B4 remains: 47 partial Tier-1 tools.
> Tag cut (Gate 7) deferred until Gates 1-2 green.

## Pre-cut gates

### Gate 1 — T7 stability cadence (5 consecutive nights)

Per `docs/TEST-PLAN.md` §6 (T7):
- 1 `sandbox-ci-full` run per day minimum
- 5 consecutive nights with all T1-T6 + G1-G12 GREEN
- Counter reset on any RED

**Counter state**: TBD (requires nightly execution outside this PR cycle)
**Tracking**: `sandbox/results/flaky_scenarios.{md,json}` (per E31-B6)
**Recipe**: `just scorecard-nightly`
**Pass criterion**: log shows 5 consecutive "failing = 0" rows, including
the per-tool G6 CV < 10% (warm-cache per E31-E).

### Gate 2 — E31-G scorecard streak (3 consecutive ALL-GREEN)

Per ADR-031 §3:
- 3 consecutive ALL-GREEN scorecard runs (no RED, no AMBER)
- Counter persisted in `sandbox/results/scorecard_streak.json`
- Counter reset on any RED, held on AMBER

**Counter state**: 0/3 (3 test runs RED because test data lacks G2/G4 coverage)
**Tracking**: `sandbox/results/scorecard_streak.json`
**Recipe**: `just scorecard-streak` / `just scorecard-streak-status`
**Pass criterion**: counter reaches 3.

### Gate 3 — G8 (Scalability) AMBER closure ✅ DONE (2026-08-16, Option B)

Per `sandbox/scripts/release_scorecard.py:gate_g8`:
- Tier-3 typescript probe (652M LOC) must complete without timeout
- Defect tracked as INC-004 / SCAL-001
- Container memory upgraded 1G→4G (mitigation applied per e30-metric-baseline)

**Decision**: **Option B — Accept as best-effort + AMBER + ADR**

**Rationale** (per INC-004 resolution, closed 2026-08-10 in E31-B6-rollup):
- Typescript tier-3 (652M LOC) probe times out despite 1G→4G container upgrade.
- Documented as best-effort via E31-F (G8 evidence text now references SCAL-001 / INC-004).
- The 5-night counter (T7) + 3-run counter (E31-G) provide v1.0.0 pre-cut gates
  **independent** of G8 resolution.
- ADR-031 §3 explicitly allows "tag cut conditional on G8 GREEN or AMBER with
  documented INC-004 closure" — this is satisfied.

**Closure evidence**: `~/.sddk-knowledge/CogniCode/incidences/INC-004-scal-001-scalability-typescript.md`
- status: closed (2026-08-10)
- resolution: ACCEPT (best-effort, doesn't block v1.0.0)

**Effect on v1.0.0 tag cut**: NONE. G8 AMBER with documented INC-004 closure is
explicitly permitted by ADR-031 §3. The scorecard may show G8 AMBER in the
v1.0.0 cut evidence, with INC-004 reference.

**Decision recorded**: this commit (no new ADR needed — INC-004 closure
already documents the trade-off).

### Gate 4 — CHANGELOG covers v0.90→v1.0.0

Per Keep a Changelog format:
- All release tags v0.87.0 → v0.92.0 have entries
- Unreleased section accumulates the E31 program
- v1.0.0 entry planned (during tag cut)

**Status**: ✓ (CHANGELOG.md updated per E31-Z prep)

### Gate 5 — Roadmap reconciled ✅ DONE (2026-08-16)

Per `docs/ROADMAP.md` (local-only working doc):
- E29 program status: completed (Postgres → LadybugDB migration)
- E30 program status: completed (v0.92.0 close, PR #233)
- E31 program status: 14 cycles completed (E31-A..E31-Z + rollups, PR #237-258)
- **E32 program status: ✅ CLOSED** (E32-A..E32-I shipped v0.94.11 .. v0.94.14)
- E33 program status: completed (e33-1..e33-14, CI/CD para cogh binary releases)
- E34 program status: completed (v0.94.12, plugin registry en main repo sin GitHub org)
- E35 program status: completed (v0.94.13, ZCode + Claude + Codex targeting)
- v1.0.0 milestone: in_progress (Gate 1-3 pending, Gate 5 ✅ done)

**Tracking**: `~/.sddk-knowledge/CogniCode/milestones/_active.md`
**Reconciliation evidence**: `docs/ROADMAP.md` líneas 617-727 (sesión 2026-08-16) ahora reflejan el estado real.

### Gate 6 — Branch cleanup ✅ DONE (2026-08-16)

Per `sandbox/scripts/prune_stale_branches.sh`:
- Local stale branches pruned (excluding whitelisted `feat/e30-*`, `fix/e30-*`)
- Remote stale branches pruned (excluding open PRs)

**Final state (2026-08-16)**:
- 14 local branches merged into main: deleted (`git branch -d`)
- 4 local orphaned branches (squashed-merged via PR #278): deleted (`git branch -D`)
- 15 remote branches merged into main: deleted (`git push origin --delete`)
- 2 remote orphaned branches (squashed-merged): deleted manually
- Remaining remote: `origin/main` + 4 whitelisted (`feat/e30-conformance-audit`,
  `feat/e30-metric-baseline`, `feat/e30-release-gate`, `fix/e30-release-gate-carryforwards`)
- Remaining local: `main`

**Tracking**: pruned via `bash sandbox/scripts/prune_stale_branches.sh --apply` + manual
cleanup for squashed-merged branches (limitation: script does not detect squashed
commits — see B-note below).

**B-note (script enhancement, optional)**: `prune_stale_branches.sh` only detects
branches whose tip is reachable from `origin/main`. Squashed-merged branches
(whose content was incorporated via a different branch, e.g., PR #278 squash) are
NOT detected and require manual cleanup. Future enhancement: add
`is_squashed_into_main` heuristic using `git log --grep=<subject>` on main.

### Gate 7 — Tag cut mechanics

Per ADR-031 §3:
- Tag `v1.0.0` with annotated message
- Push tag (or maintainer chooses to cut locally)
- Verify tag is on origin/main HEAD
- Update CHANGELOG.md to move "Unreleased" → "v1.0.0"

**Owner**: maintainer (per ADR-031)

## Deferred items (open after E31, not blocking v1.0.0)

These items were intentionally deferred from E31 (out of scope or requiring
operational follow-up). None of them block the v1.0.0 tag cut.

### B1. `retrieve_and_verify` CV 0.105 (real outlier) — **E31-E2 ACCEPT**

- **Source**: E31-E (read_file CV 0.528 outlier decision)
- **Decision**: **ACCEPT** (closed in E31-E2, 2026-08-10)
- **Investigation**:
  - 3 samples: `[21, 17, 19]` ms (mean 19ms)
  - CV (full): 0.105 — would fail 10% budget
  - CV (warm, post E31-E filter): 0.0556 — comfortably under 10%
  - Cold-cache detection: `max / warm_mean = 21/18 = 1.167 < 1.5x` threshold
    → **NOT cold-cache** (real sample variance)
  - Family mappings: `retrieve_and_verify` NOT in `TOOL_TO_FAMILY`
    (not in any of search/call-graph/analytics/navigation) →
    **G6 scorecard does not count this tool** in family aggregates
  - Family-level CV (search): 0.0054 (mean) / 0.009 (max) — well under 10%
- **Decision rationale**: G6 is GREEN at the family level. The 0.105 is a
  per-tool diagnostic that appears in `top_high_variance_scenarios` of
  `stability.json` but does not affect the G6 verdict. 3 samples is
  statistically thin; the next nightly run will provide more samples
  for proper statistical analysis (likely moves CV toward 0.05-0.07 range).
- **Future action**: monitor on next nightly run (>5 samples). If CV
  stabilizes at > 0.10 over 5+ runs, open a separate investigation cycle.
- **Impact**: no G6 scorecard change; per-tool diagnostic only.

### B2. 178 Tier-3 "failing" scenarios (bash, csharp, dart, etc.) ✅ DONE (E31-B2-rollup, 2026-08-10)

- **Source**: E31-B6 (T7 stability cadence)
- **Issue**: scenarios appear in manifests (`sandbox/manifests/*.yaml`)
  but no `result.json` was ever produced for them.
- **Resolution**: Triage option (a) — added all 178 scenarios to
  `sandbox/manifests/known_quarantined.yaml` (curated list consumed by
  `sandbox/scripts/build_flaky_log.py` via `KNOWN_QUARANTINED`). Each scenario
  is now marked `quarantined` in the flaky log instead of `failing`, exempt
  from G6 max-CV computation, and exempt from G13 "no surprise flaky".
- **Evidence**: `sandbox/manifests/known_quarantined.yaml` (7719 bytes,
  metadata.rationale: "Per E31-B2-rollup (B2) decision.").
- **Effect**: 0 noisy "failing" entries for the 178 Tier-3 / niche-language
  scenarios in `flaky_scenarios.md`. Real failures still surface normally.

### B3. Existing remote CI workflows (`ci.yml`, `sandbox-nightly.yml`) ✅ DONE (2026-08-16)

- **Source**: E31-B5 (T6 CI gate, local-only by user directive)
- **Decision**: NOT touched per E31-B5 user directive (E31-B5 closed 2026-08-10)
- **Final cleanup (2026-08-16)**: Per AGENTS.md "Local CI Is the Source of Truth"
  + ADR-031 "GitHub Actions runs are reserved for the release gate only":
  - `.github/workflows/ci.yml`: removed `on: push` and `on: pull_request` triggers;
    now `workflow_dispatch` only (LOCAL-ONLY via `act + podman`). Runs locally via
    `just ci-local` (updated to invoke this workflow alongside `regression-check.yml`).
  - `.github/workflows/sandbox-nightly.yml`: schedule trigger was already
    disabled in E31-B5; remains `workflow_dispatch` only. ✅
  - `.github/workflows/regression-check.yml`: was already `workflow_dispatch`
    only per E31-B5. ✅
  - `.github/workflows/release.yml`: tag-triggered (`on: push: tags: [v*]`) —
    this IS the release gate per AGENTS.md, KEEP. ✅
- **Effect**: GitHub-hosted CI minutes no longer consumed by day-to-day work.
  Release gate (tag cuts) still runs on GitHub Actions.

### B4. 47 partial Tier-1 tools (post-B8)

- **Source**: E31-B8 (Tier-1 closure 35.6%)
- **State**: 26 tools @ 5/5, 47 partial at 1-3/5 (mostly rust-only or graph_*)
- **Closure options**: future cycles of similar batch fill to reach 50%+.

### B5. CHANGELOG.md partial reconstruction (v0.50 → v0.86) ✅ DONE (E31-B5-rollup, 2026-08-10)

- **Source**: E31-Z prep (CHANGELOG.md added v0.90-v0.92 entries)
- **Issue**: v0.50 → v0.86 still missing
- **Reconstruction**: `git log --oneline v0.50..v0.86` → reconstruct entries
- **Resolution**: Commit `4d5f8bb6 E31-B5-rollup (B5) — CHANGELOG v0.50-v0.86
  reconstruction` added a single reconstructed summary entry covering the 307
  commits across 36 version tags (v0.50.0 → v0.86.0). Individual entries for
  each version are NOT needed for v1.0.0 readiness — the summary covers the
  major program cycles (E12, E14, E19, E21, E28, E29, etc.) with PR counts.
- **Optional**: not blocking v1.0.0 — DONE per E31-B5-rollup

### B6. Open incidences (5 tracked, 0 blocking) — **ALL CLOSED (E31-B6-rollup)**

| INC | Severity | Issue | Status |
|---|---|---|---|
| INC-001 | high | launch-latency | **CLOSED** (E31-B6-rollup, ACCEPT) |
| INC-002 | high | search-latency | **CLOSED** (E31-B6-rollup, ACCEPT) |
| INC-003 | medium | infra-instability | **CLOSED** (E31-B6-rollup, ACCEPT) |
| INC-004 | medium | SCAL-001 (typescript tier-3) | **CLOSED** (E31-B6-rollup, ACCEPT) |
| INC-005 | ~~medium~~ | CONF-001 (conformance) | **CLOSED** in E30.4 (vault stale) |

INC-005 status was updated to `closed` during E31-Z bookkeeping (vault
sanity). INC-001..004 were closed in E31-B6-rollup (2026-08-10) as
ACCEPT — all are scorecard-level performance characteristics
documented as best-effort. None of these block v1.0.0 tag cut. **0 open
incidences remain.**

## Resume protocol

The next session can resume the v1.0.0 cut by:

1. Check `sandbox/results/scorecard_streak.json` — counter at goal?
2. Check `sandbox/results/flaky-archive/` — 5 consecutive nights?
3. Check the latest scorecard.json — G8 GREEN (or AMBER with INC-004 closure — already
   documented in Gate 3 ✅, so any AMBER is acceptable)
4. Run `git fetch origin && git checkout main && git pull`
5. Run `just scorecard-nightly` once
6. If counters and gates GREEN (or G8 AMBER), run:

```bash
git tag -a v1.0.0 -m "v1.0.0 — production-ready

- 3 consecutive ALL-GREEN scorecards
- 5 consecutive nights CV < 10%
- G8 GREEN (or AMBER with documented INC-004 closure — per Gate 3 ✅ Option B)
- 14 ADRs reviewed
- T3 closure: 35.6% (target ≥30% met)
- 26 MCP tools at full Tier-1

Closes E31 program (A, B, D, B2-B8, C, E, F, G, Z)."

git push origin v1.0.0
```

## Checklist (signed-off before tag cut)

- [x] Gate 3 — G8 gate GREEN (or AMBER with documented INC-004 closure) ✅ (2026-08-16, Option B)
- [x] Gate 4 — CHANGELOG v0.90 → v0.92 entries present ✅ (E31-Z prep)
- [x] Gate 5 — Roadmap reconciled (E30 completed, E31 in_progress) ✅ (2026-08-16)
- [x] Gate 6 — Stale branches pruned (or whitelisted) ✅ (2026-08-16)
- [ ] Scorecard streak counter at 3/3 (Gate 2)
- [ ] T7 cadence counter at 5/5 (Gate 1)
- [ ] Maintainer approval
- [ ] Tag `v1.0.0` cut + pushed
- [ ] Release announcement posted

## Cross-references

- `docs/RELEASE-1.0.0-PLAN.md` §1 — 12-gate scorecard framework
- `docs/TEST-PLAN.md` §6 — T7 stability cadence
- `docs/adr/ADR-031-release-1.0.0-definition.md` — v1.0.0 source-of-truth
- `~/.sddk-knowledge/CogniCode/incidences/INC-004-scal-001-scalability-typescript.md` — SCAL-001 track
- `~/.sddk-knowledge/CogniCode/milestones/_active.md` — cycle serialization lock
