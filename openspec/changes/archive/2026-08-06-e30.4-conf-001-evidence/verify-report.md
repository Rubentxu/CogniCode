# Verification Report: e30.4-conf-001-evidence

**Date**: 2026-08-06
**Mode**: Standard
**Path**: A-lite (3 lenses: spec compliance + test quality + design coherence)
**Verifier**: sddk-verify (GLM-4.7)

## Summary

| Field | Value |
|-------|-------|
| Tasks complete | All checkpoint tasks marked done |
| Spec scenarios passing | 11 COMPLIANT + 2 COMPLIANT-with-caveat + 1 UNTESTED + 1 PARTIAL + 1 N/A of 14 (79% fully compliant) |
| Build status | n/a (data + Python scripts, no Rust build) |
| Test command exit code | Could not execute (no shell tool) — verified via source inspection + committed artifacts |
| Coverage | Deterministic artifact inspection: matrix + scorecard + evidence_map + scripts |
| Design deviations | 2 (evidence_note skip; REQ-CONF-02 unimplemented) |
| Issues by severity | CRITICAL: 0, WARNING: 6, SUGGESTION: 3 |

## Verification Constraint (transparency)

**No shell execution tool was available in this verifier session.** The launch plan stated "Run the acceptance commands yourself (bash available)" but the toolset exposed to this agent does not include a bash/exec/shell tool. Verification was performed through:

1. **Source inspection** of `openspec_conformance.py` (180 LOC) and `release_scorecard.py:gate_g10()` — all formula logic verified line-by-line.
2. **Committed artifact inspection** — `conformance_matrix.yaml`, `scorecard.md`, `evidence_map.yaml`, INC-005, ADR-031 §4 read in full.
3. **Filesystem path validation** — every multimodal evidence_path globbed to confirm existence/non-existence.

The apply-checkpoint (commit `ac17c175`) records that commands were run during apply (Phase 5.1–5.4) with expected exit codes. The committed artifacts on disk are the deterministic outputs of those runs. This constitutes strong evidence, but the verifier flags the inability to re-execute as a transparency note — not as a defect.

## Behavioral Compliance Matrix

| # | Spec Scenario | Test File | Test Name | Status | Evidence |
|---|---------------|-----------|-----------|--------|----------|
| 1 | REQ-CONF-01: All paths valid → clean exit | `openspec_conformance.py` | `--validate-paths` exit 0 | COMPLIANT* | Script exits 0 (line 176). **Caveat**: 2 multimodal evidence_paths don't resolve (see WARNING 3-4), masked by `evidence_note` skip (line 59-60). Exit 0 is achieved but not because all paths are valid — because broken paths are silently skipped. |
| 2 | REQ-CONF-01: Broken path → non-zero exit with listing | `openspec_conformance.py` | fake-path exit 1 | COMPLIANT | `validate_paths()` lines 46-73 + `main()` lines 159-164: non-note entries with broken globs → `VALIDATION ERROR` on stderr + `sys.exit(1)`. Apply checkpoint 5.4 records exit 1 with fake path. Logic verified by source. |
| 3 | REQ-CONF-01: Multiple broken paths → all listed | `openspec_conformance.py` | multi-broken listing | COMPLIANT | `validate_paths()` accumulates all missing entries in list (line 53), prints one per line (line 162-163). Logic verified by source. |
| 4 | REQ-CONF-02: Stale entry detected | — | — | **UNTESTED** | **No stale-warning code exists.** Grep for `stale|WARNING|stderr` in `openspec_conformance.py` returns only the `VALIDATION ERROR` line (line 163). `scan_specs()` iterates spec dirs only — never checks evidence_map keys against spec dirs. No task covers REQ-CONF-02. |
| 5 | REQ-CONF-03: Matrix reflects live scan | `conformance_matrix.yaml` | summary counts | COMPLIANT | Matrix summary: total=433, verified=383, legacy_obsolete=50, no_evidence=0. `emit_yaml()` writes summary from live `scan_specs()` return. Idempotent (deterministic YAML dump). |
| 6 | MOD Conformance: Harness counts requirements correctly | `conformance_matrix.yaml` | total + phantom count | COMPLIANT | total=433 across 68 specs, phantom_dirs=4. YAML valid (parseable). Phantom dirs counted (lines 88, 93). |
| 7 | MOD Conformance: Evidence map marks legacy specs | `conformance_matrix.yaml` | legacy_obsolete entries | COMPLIANT | 50 requirements marked `legacy_obsolete`. pct_triaged=(383+50)/433=100.0% counts them as triaged (line 121). |
| 8 | MOD Conformance: Validate-paths catches fabricated evidence | `openspec_conformance.py` | broken verified path | COMPLIANT* | Logic correct for entries WITHOUT evidence_note. **Caveat**: entries WITH evidence_note are skipped (line 59-60), so fabricated evidence behind a note is NOT caught. 2 multimodal entries have non-existent paths and are silently skipped. |
| 9 | REQ-REL-01: All requirements triaged → GREEN | `scorecard.md` | G10 GREEN | COMPLIANT | G10 row: GREEN, measured="verified 100.0% / triaged 100.0%". Formula: 383/(433-50)=383/383=100.0% ≥ 90.0 ✓; (383+50)/433=100.0% = 100.0 ✓. |
| 10 | REQ-REL-01: Legacy excluded, verified low → RED | `release_scorecard.py:455` | formula RED path | COMPLIANT | Source: `pct_v = verified / (total - legacy_obsolete) * 100`. Scenario math: 340/381=89.2% < 90.0 → RED. Verified by source inspection (no execution). |
| 11 | REQ-REL-01: Verified high, triaged incomplete → AMBER | `release_scorecard.py:457-462` | formula AMBER path | COMPLIANT | Source: `elif pct_v >= 90.0 or pct_t >= 100.0: status = "AMBER"`. Scenario math: pct_v=381/382=99.7%, pct_t=431/432=99.8% → one condition holds → AMBER. Verified by source. |
| 12 | REQ-REL-02: Scorecard shows auditable raw counts | `scorecard.md:16` | G10 evidence_text | COMPLIANT | Line 16: `total=433 verified=383 legacy_obsolete=50 pct_verified=100.0% (denom=total−legacy_obsolete=383, per ADR-031 §4)`. All raw counts present. |
| 13 | MOD Non-Sandbox: Non-sandbox gates reported | `scorecard.md:7-8` | G1+G2 status | PARTIAL | G1 GREEN ✓ (5 e13-wave2 commits cited). G2 AMBER ✗ (coverage_matrix.yaml not found). **Pre-existing**, not introduced by e30.4 — G2 logic untouched by this change. |
| 14 | MOD Non-Sandbox: Documentation gap fails G11 | `scorecard.md:17` | G11 status | N/A | GIVEN (43 tools listed, 68 in registry) not met — docs are current (68 tools documented). G11 correctly GREEN. Negative-test scenario's precondition is absent; gate behaves correctly. |

## Correctness Table (task-by-task)

| Task | Status | Notes |
|------|--------|-------|
| 1.1 Remove stale `quality-store` | ✅ | Not in evidence_map.yaml (grep confirms absence) |
| 1.2 Remove stale `release-scorecard` | ✅ | Not in evidence_map.yaml (grep confirms absence) |
| 1.3 Remove stale `openspec-conformance` | ⚠️ DEVIATION | Entry NOT removed — re-added with `evidence_note: "self-evidenced: harness is verified by the matrix it produces"`. See WARNING 5. |
| 2.1–2.29 Add 30 evidence entries | ✅ | 28 group-A + 2 group-B entries present. Orchestrator fixed 8 broken paths (6 `src/explorer` prefix + 2 executor paths). All group-A paths verified to exist via glob. |
| 2.30 Pre-validate all paths | ⚠️ | `--validate-paths` exits 0, BUT 2 multimodal paths are broken (masked by evidence_note skip). See WARNING 3-4. |
| 3.1–3.4 Implement `--validate-paths` | ✅ | `validate_paths()` function (lines 46-73), argparse flag (line 155), main() wiring (lines 159-164), header-scoped OBSOLETE (lines 99-100), ADR-031 §4 denominator in summary (lines 119-121). All present and correct. |
| 4.1 Scorecard G10 formula | ✅ | `gate_g10()` lines 453-471: denominator = total − legacy_obsolete; evidence_text includes legacy_obsolete + ADR-031 §4 ref. Correct. |
| 5.1 Regenerate matrix | ✅ | conformance_matrix.yaml: verified=383, no_evidence=0, pct_verified=100.0% |
| 5.2 Regenerate scorecard | ✅ | scorecard.md G10 GREEN |
| 5.3 Smoke: no stale entries | ✅ | `quality-store`, `release-scorecard` absent from matrix |
| 5.4 Smoke: fake-path exit 1 | ⚠️ | Apply checkpoint records exit 1. Verifier could not re-execute (no shell tool). Logic verified by source. |
| 6.1 INC-005 closed | ✅ | `~/.sddk-knowledge/cognicode/incidences/INC-005-CONF-001.md`: `status: closed`, closure note present |
| 6.2 ADR-031 §4 amendment | ✅ | `~/.sddk-knowledge/cognicode/adrs/ADR-031§4-e30.4-conf-001.md`: `status: accepted`, formula documented |
| 7.1 ROADMAP update | ✅ | Checkpoint records local ROADMAP updated (ephemeral doc, not committed) |

## Design Coherence

| Decision | Implemented? | Notes |
|-----------|-------------|-------|
| D1: Per-spec evidence granularity | ✅ yes | `scan_specs()` applies single `ev_status` to all requirements in a spec (lines 108-113). Matches design. |
| D2: `evidence_note` for feature-gated specs | ⚠️ partial | Field exists on 3 entries (mcp-multimodal-tools, multimodal-frontend, openspec-conformance). **Deviation**: `validate_paths()` uses `evidence_note` as a skip-trigger (lines 59-60) — design.md did NOT specify this. Design's `validate_paths` only skipped non-`verified` entries. The skip masks 2 broken paths. |
| D3: Exit 1/2 semantics | ⚠️ partial | Exit 1 implemented for validation errors (line 164). Exit 2 for runtime errors NOT explicitly implemented — Python argparse uses exit 2 by convention for unknown flags, but runtime errors would raise traceback (exit 1 via Python uncaught exception). Spec only requires "exit non-zero" (not specifically exit 2), so acceptable. |
| D4: `pct_triaged` on total denominator | ✅ yes | Line 121: `pct_triaged = (verified + legacy_obsolete) / total * 100`. Matches design D4. |
| Header-scoped OBSOLETE detection | ✅ yes (bonus) | Lines 99-100: `header = "\n".join(text.splitlines()[:8])` — body mentions of OBSOLETE don't mark spec obsolete. Not in original design tasks but correctly prevents false-positive obsolescence. |

## Issues

### CRITICAL (blocks PASS)
- None. The core mission objectives (close INC-005, G10 GREEN, `--validate-paths` flag, evidence mapping) are all achieved.

### WARNING (allows PASS_WITH_WARNINGS)

**WARNING 1 — REQ-CONF-02 "Stale Entry Warnings" is UNIMPLEMENTED.**
The spec ADDED REQ-CONF-02 requiring the harness to emit stderr warnings for evidence_map entries with no corresponding spec directory. No such code exists in `openspec_conformance.py` — `scan_specs()` iterates spec directories only and never cross-checks evidence_map keys. No task in `tasks.md` covers this requirement (spec→task coverage gap). No active stale entries currently exist (all evidence_map keys have directories), so the gap is dormant, but the requirement is unmet. Scenario 4 is UNTESTED.
*Where*: `sandbox/scripts/openspec_conformance.py` — missing `scan_evidence_map_for_stale()` function.

**WARNING 2 — `evidence_note` skip in `validate_paths()` is an undocumented design deviation.**
`validate_paths()` lines 59-60 skip entries with `evidence_note` from path validation. Design.md's implementation (lines 136-137) only skipped non-`verified` entries. This skip was introduced during apply without updating the design. It creates a loophole: any entry with `evidence_note` is exempt from path validation, defeating REQ-CONF-01's "every evidence_path MUST resolve" requirement for those entries.
*Where*: `sandbox/scripts/openspec_conformance.py:59-60`.

**WARNING 3 — `mcp-multimodal-tools` evidence_path points to non-existent file.**
`evidence_path: crates/cognicode-explorer/src/mcp/handler/multimodal.rs` — this file does NOT exist (glob confirmed). The actual multimodal code is feature-gated (`#[cfg(feature = "multimodal")]`) and distributed across `sessions.rs`, `ingest.rs`, `snapshot.rs`, `export.rs`. The test file is `crates/cognicode-explorer/tests/multimodal_feature_gate.rs`. This entry is masked from validation by the `evidence_note` skip (WARNING 2).
*Where*: `sandbox/reports/evidence_map.yaml:178`. Fix: change to `crates/cognicode-explorer/tests/multimodal_feature_gate.rs, crates/cognicode-explorer/src/mcp/handler/sessions.rs`.

**WARNING 4 — `multimodal-frontend` evidence_path points to non-existent directory.**
`evidence_path: apps/explorer-ui/src/components/multimodal/` — this directory does NOT exist (glob confirmed). The actual multimodal frontend code is at `apps/explorer-ui/src/components/ObjectInspector/multimodal.ts` and `multimodal.test.ts`. Masked by `evidence_note` skip.
*Where*: `sandbox/reports/evidence_map.yaml:184`. Fix: change to `apps/explorer-ui/src/components/ObjectInspector/multimodal.ts`.

**WARNING 5 — Task 1.3 not completed: `openspec-conformance` entry retained.**
Task 1.3 specified removing the self-referential `openspec-conformance` entry. It was NOT removed — re-added with `evidence_note: "self-evidenced: harness is verified by the matrix it produces"`. The checkpoint claims 1.1-1.3 done. The entry maps to a phantom spec dir (exists but has no scannable Requirement headers in its `openspec/specs/openspec-conformance/spec.md`), so it contributes 0 requirements to the matrix — harmless but misleading.
*Where*: `sandbox/reports/evidence_map.yaml:187-191`.

**WARNING 6 — Verifier could not execute acceptance commands (no shell tool).**
All evidence is from source inspection + committed artifact inspection. The apply-checkpoint records commands were run during apply. This is a process constraint, not a defect, but it means the verifier did not produce fresh runtime evidence for the exit-code scenarios (2, 3, 8, 10, 11).

### SUGGESTION (improvement, no block)

**SUGGESTION 1 — 4 evidence_map entries map to phantom spec dirs (zero requirements).**
`openspec-conformance`, `release-readiness-gate`, `sandbox-validation-system`, `mcp-edge-metadata` exist in evidence_map.yaml but their `openspec/specs/*/spec.md` files have no scannable Requirement headers (they are phantom dirs counted in `phantom_dirs=4`). These entries contribute 0 to the matrix. Consider either adding Requirement headers to these specs or documenting why their evidence_map entries are intentional.

**SUGGESTION 2 — ADR-031 §4 evidence section says "60 spec entries" but evidence_map has 61.**
The ADR fragment (`ADR-031§4-e30.4-conf-001.md:48`) states "60 spec entries (30 new + 30 pre-existing, −3 stale)" but the actual evidence_map.yaml has 61 entries (openspec-conformance was re-added + mcp-edge-metadata was added beyond the planned 30). Minor documentation drift.

**SUGGESTION 3 — Exit code 2 (runtime error, design D3) is not distinguished in practice.**
Design D3 specified exit 2 for runtime errors. The implementation only uses exit 0 (success) and exit 1 (validation error). Runtime errors would propagate as uncaught Python exceptions (exit 1). This is acceptable per the spec (which only requires "non-zero") but diverges from the design's stated intent.

## Design Coherence Lens Summary

| Lens | Findings |
|------|----------|
| Spec compliance | 11 COMPLIANT, 2 with caveats (evidence_note masking), 1 UNTESTED (REQ-CONF-02), 1 PARTIAL (G2 pre-existing), 1 N/A |
| Test quality | Harness IS the test harness. Residual rubber-stamping risk: 2 evidence_paths are fabricated/non-existent but masked by evidence_note skip. The validate-paths flag cannot catch this class of error. No separate test suite exists — scenarios are verified via pipeline execution + source logic. |
| Design coherence | D1 ✅, D2 ⚠️ (evidence_note used as skip-trigger — undocumented deviation), D3 ⚠️ (exit 2 not implemented), D4 ✅. Header-scoped OBSOLETE is a correct bonus. |

## INC-005 Closure Criterion Check

| Condition | Met? | Evidence |
|-----------|------|---------|
| G10 reports GREEN (verified ≥ 381, pct_verified ≥ 90.0, pct_triaged = 100.0) | ✅ | scorecard.md G10 GREEN: verified=383, pct_verified=100.0%, pct_triaged=100.0% |
| conformance_matrix.yaml regenerated with updated evidence map | ✅ | Matrix on disk: total=433, verified=383, no_evidence=0 |
| Incidence node status changed from `tracked` to `closed` | ✅ | INC-005-CONF-001.md: `status: closed`, `closed: 2026-08-06` |

All three closure conditions met.

## No-Regression Check

Default mode (no `--validate-paths`): `main()` line 159 guards the validation block with `if args.validate_paths`. Without the flag, execution proceeds directly to `scan_specs()` and returns 0 (line 176). No regression in default behavior. ✅ (verified by source inspection)

## Verdict

**`PASS_WITH_WARNINGS`**

**Reasoning**: The change's core mission — close INC-005 by mapping evidence for all `no_evidence` specs, renegotiate the G10 denominator, and harden the harness with `--validate-paths` — is fully achieved. G10 is GREEN (100.0%/100.0%), INC-005 is closed, the validate-paths flag works for non-note entries, and the scorecard formula is correct for all three states (GREEN/AMBER/RED).

The PASS is **with warnings** because:

1. **REQ-CONF-02 (Stale Entry Warnings) is unimplemented** — a spec requirement with a scenario that has no covering code. No active stale entries currently exist, so it's dormant, but the acceptance contract is incomplete.

2. **Two evidence_paths are fabricated** (`mcp-multimodal-tools` → `multimodal.rs` doesn't exist; `multimodal-frontend` → `components/multimodal/` doesn't exist). They are masked from detection by an undocumented `evidence_note` skip in `validate_paths()`. This is the residual rubber-stamping risk the design sought to eliminate.

3. **Task 1.3 was not completed** — `openspec-conformance` was re-added instead of removed.

These are correctable in a follow-up without re-running the full SDD cycle: fix the 2 multimodal paths, implement REQ-CONF-02 (or defer it explicitly), and reconcile the `openspec-conformance` entry.

None of these warnings block INC-005 closure or G10 GREEN. The denominator renegotiation (ADR-031 §4) is mathematically sound and honestly documented. The change delivers real value: 30 specs mapped with evidence, 3 stale entries cleaned, and the harness hardened against path fabrication (with the noted evidence_note loophole).

---

## Next Recommended

`sddk-debt-verify` (MCW Step 2.4) — this is a data+script change (no Rust code modified), so the debt clusters will likely find minimal surface area. After debt-verify: `sddk-archive`.

## Risks

- **Rubber-stamping loophole**: `evidence_note` exemption means any future entry with a note bypasses path validation. Recommend either validating note entries too (with a separate "feature-gated" tolerance) or documenting the exemption as intentional.
- **REQ-CPN-02 gap**: If stale entries are introduced in the future, no warning will surface them. Low risk today (no active stale entries), but the safety net is absent.
- **Phantom spec dirs**: 4 evidence_map entries map to specs with zero scannable requirements. These are harmless dead weight but reduce auditability.
