# Archive Report: e14-narrative-runtime-cycle-2

**Date**: 2026-08-05
**Archived by**: sddk-archive
**Mode**: hybrid (engram artifact registry + filesystem)
**Branch**: `feat/e14-narrative-runtime-cycle-2` (HEAD = d133233f, pushed to origin)

---

## Cycle Summary

### What was delivered

NarrativeStore port + LadybugDB adapter + RuntimePorts wiring for snapshot-cache persistence of rendered `ContextualView` outputs.

| Component | Status | Evidence |
|-----------|--------|----------|
| `NarrativeStore` port trait | ✅ Complete | `crates/cognicode-core/src/domain/ports/narrative_store.rs` |
| `NarrativeSnapshot` DTO + `NarrativeError` | ✅ Complete | Same file |
| `init_narrative_view_schema` DDL | ✅ Complete | `crates/cognicode-ladybug/src/init_schema.rs` |
| `impl NarrativeStore for LadybugStore` | ✅ Complete | `crates/cognicode-ladybug/src/narrative_store.rs` (559 LOC) |
| `RuntimePorts::narrative_store` slot | ✅ Complete | `cognicode-runtime/src/lib.rs:104` |
| Bootstrap wiring | ✅ Complete | `cognicode-runtime/src/lib.rs:160, :191` |
| Unit tests | ✅ 8/8 STATIC-OK | `cargo test -p cognicode-ladybug --lib -- narrative_store` |

### Commits on branch

| SHA | Description |
|-----|-------------|
| `b8989fd5` | Original impl: NarrativeStore port + LadybugDB adapter + RuntimePorts wiring |
| `d6457355` | DDL lbug compat (STRING not TEXT NOT NULL), INSERT params, test isolation (TempDir) |
| `d133233f` | Remove redundant `init_narrative_view_schema` call in `temp_store()` |

### Verification outcomes

| Phase | Verdict | Notes |
|-------|---------|-------|
| `sddk-verify` | **FAIL** | S9/S10 spec/impl mismatch on missing-table contract (spec: "return error"; impl: `Ok(())`/`Ok(None)` like QualityStore) |
| `sddk-debt-verify` | **PASS_WITH_WARNINGS** | 1 WARNING (temp_store redundant schema init), 0 CRITICAL |
| `cargo test` | **50 passed, 0 failed** | Full test suite green |
| **User override** | `continua` | User explicitly authorized archive + release despite FAIL verdict |

---

## Delta Spec Sync

### Spec domain: `narrative-store`

**Action**: Created (delta spec IS the full spec — new domain)

The spec at `openspec/specs/narrative-store/spec.md` was written during this cycle as part of `sddk-spec`. It defines:

| Requirement | Scenarios | Status |
|-------------|-----------|--------|
| Snapshot Save with Upsert | S1 (new row), S2 (upsert) | STATIC-OK |
| Snapshot Load with Cache Hit | S3 (cache hit) | STATIC-OK |
| Snapshot Load with Cache Miss | S4 (cache miss returns None) | STATIC-OK |
| List Snapshots for Workspace | S5 (list all), S6 (filtered by view_kind) | STATIC-OK |
| Cache Invalidation by Source Revision | S7 (invalidate stale), S8 (no match) | STATIC-OK |
| Graceful Degradation on Missing Table | S9 (save fails), S10 (load fails) | **NON-COMPLIANT** — spec mandates "return error"; impl returns `Ok(())`/`Ok(None)` |

**No other specs affected.** No delta specs for `runtime-ladybug-wiring` or other domains — this cycle only added a new port slot (no changes to existing port contracts).

---

## Source of Truth Updated

- `openspec/specs/narrative-store/spec.md` — **created** during this cycle (v0.1.0, draft, tagged to `e14-narrative-runtime-cycle-2`)

---

## Archive Contents

| Artifact | Path | Status |
|---------|------|--------|
| proposal.md | `openspec/changes/archive/2026-08-05-e14-narrative-runtime-cycle-2/proposal.md` | ✅ (moved) |
| design.md | `sddk/e14-narrative-runtime-cycle-2/design.md` | ✅ |
| tasks.md | `sddk/e14-narrative-runtime-cycle-2/tasks.md` (5/5 tasks complete) | ✅ |
| verify-report.md | `sddk/e14-narrative-runtime-cycle-2/verify-report.md` (verdict: FAIL, user-overridden) | ✅ |
| debt-report.md | `sddk/e14-narrative-runtime-cycle-2/debt-report.md` (verdict: PASS_WITH_WARNINGS) | ✅ |
| explore-report.md | `sddk/e14-narrative-runtime-cycle-2/explore-report.md` | ✅ |
| apply-checkpoint.json | `sddk/e14-narrative-runtime-cycle-2/apply-checkpoint.json` | ✅ |
| archive-report.md | `sddk/e14-narrative-runtime-cycle-2/archive-report.md` (this file) | ✅ |

---

## Knowledge Impact

### Specs made stale
- None — this is a net-new spec (`narrative-store`) that did not exist before this cycle.

### ADRs superseded
- None directly. However, `ADR-002 Phase 3` mentions "persisted in Postgres" — explored and clarified during this cycle that ADR-002 already correctly says "LadybugDB" (the brief's premise was stale, not the ADR).

### Jurisprudence candidate
**Yes — topic_key: `sddk/e14-narrative-runtime-cycle-2/jurisprudence`**

Rationale: `verify_verdict=FAIL` + `first_pass_success=false` + reusable decision on the missing-table graceful-degradation contract (QualityStore pattern vs. spec-mandated error). The S9/S10 decision (silent Ok vs. descriptive error) is a reusable architectural choice that should be codified.

The finding is:
- **Decision needed**: Should `NarrativeStore` missing-table operations return `NarrativeError::Database("…table is missing…")` (matching the spec's "MUST return a descriptive error") OR `Ok(())`/`Ok(None)` (matching the QualityStore pattern where reads degrade silently)?
- **Current impl**: chose `Ok(())`/`Ok(None)` — QualityStore-style silent degradation
- **Spec says**: "All operations MUST return a descriptive error" (S9, S10)
- **Resolution**: Either amend the spec to match the QualityStore pattern, OR update the impl to return descriptive errors

### Entropy Trend
Not computed — `entropy-sdd` was not available during this run. Qualitative assessment: the implementation introduced a new port following the established QualityStore pattern with minimal architectural entropy. The main concern is the spec/impl mismatch on S9/S10, not structural entropy.

---

## Semver Bump Decision

**New feature port + DDL + runtime wiring → minor bump**

| Artifact | Version | Change type | New version |
|----------|---------|-------------|-------------|
| `cognicode-core` (NarrativeStore port trait) | 0.84.x | MINOR (new API surface) | 0.85.0 |
| `cognicode-ladybug` (NarrativeStore impl + DDL) | 0.84.x | MINOR (new node table + impl) | 0.85.0 |
| `cognicode-runtime` (RuntimePorts slot) | 0.84.x | MINOR (new port slot) | 0.85.0 |

**Decision**: `v0.85.0` — this is a new feature port (not a bug fix or patch).

---

## Release Handoff

```yaml
ready_for_release: true
change: e14-narrative-runtime-cycle-2
branch: feat/e14-narrative-runtime-cycle-2
merge_policy: guided
semver: v0.85.0
override_authorized: true  # user "continua" overriding verify FAIL on S9/S10
```

### Pre-release conditions
- [x] Branch pushed to origin (`d133233f`)
- [x] All 5 implementation tasks complete
- [x] 50 cargo tests passing
- [x] Debt audit PASS_WITH_WARNINGS
- [x] Spec written to `openspec/specs/narrative-store/spec.md`
- [x] Archive report written
- [x] User override on verify FAIL recorded

### Post-merge requirements
1. Tag: `git tag v0.85.0 && git push origin v0.85.0`
2. ROADMAP update: move E14 milestone from "Active" → "Completed"
3. ADR jurisprudence save: S9/S10 decision (silent Ok vs. descriptive error) → F3 jurisprudence

---

## Standard Envelope

```yaml
status: success
executive_summary: >
  e14-narrative-runtime-cycle-2 delivered NarrativeStore port + LadybugDB adapter + RuntimePorts
  wiring (5/5 tasks complete, 50 tests green, debt audit PASS_WITH_WARNINGS). The main spec
  was created at openspec/specs/narrative-store/spec.md. Verification found FAIL on S9/S10
  (spec mandates error on missing table; impl returns Ok/None) but user authorized continuation
  via "continua" override. Archive is complete. Release is mandatory.
artifacts:
  - "sddk/e14-narrative-runtime-cycle-2/archive-report.md"
  - "openspec/specs/narrative-store/spec.md"
specs_synced:
  - domain: narrative-store
    action: created
    details: 6 requirements, 10 scenarios (S1–S10); S9/S10 NON-COMPLIANT per verify
archive_path: openspec/changes/archive/2026-08-05-e14-narrative-runtime-cycle-2
knowledge_impact:
  specs_stale: []
  adrs_superseded: []
  jurisprudence_candidate: "sddk/e14-narrative-runtime-cycle-2/jurisprudence"
ready_for_release: true
next_recommended: sddk-release e14-narrative-runtime-cycle-2
risks:
  - "S9/S10 spec/impl mismatch (user-overridden FAIL) — resolution needed post-merge"
  - "Concurrent upserts on same synthetic id without transaction (W-2, single-writer safe)"
semver: v0.85.0
context_quality: C2
```
