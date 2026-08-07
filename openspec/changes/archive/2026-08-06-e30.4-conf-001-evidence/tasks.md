# Tasks: e30.4-conf-001-evidence — G10 Evidence Mapping (Close INC-005)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 110–140 (evidence_map.yaml ~95, openspec_conformance.py ~18, release_scorecard.py ~20) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR — all changes are data edits + two small script tweaks |
| Delivery strategy | exception-ok (small data+script change, no risk) |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

---

## Phase 1: Fix 3 Stale Entries in evidence_map.yaml

- [x] **1.1** Remove stale `quality-store` entry (lines 35-37) from `sandbox/reports/evidence_map.yaml`
  - File: `sandbox/reports/evidence_map.yaml`
  - LOC delta: −3
  - Commit: `fix(evidence): remove stale quality-store entry`
  - Verification: `grep -c "quality-store" sandbox/reports/evidence_map.yaml && echo "FAIL" || echo "PASS"`

- [x] **1.2** Remove stale `release-scorecard` entry (lines 97-99) from `sandbox/reports/evidence_map.yaml`
  - File: `sandbox/reports/evidence_map.yaml`
  - LOC delta: −3
  - Commit: `fix(evidence): remove stale release-scorecard entry`
  - Verification: `grep -c "release-scorecard" sandbox/reports/evidence_map.yaml && echo "FAIL" || echo "PASS"`

- [x] **1.3** Remove stale `openspec-conformance` self-referential entry (lines 100-102) from `sandbox/reports/evidence_map.yaml`
  - File: `sandbox/reports/evidence_map.yaml`
  - LOC delta: −3
  - Commit: `fix(evidence): remove stale openspec-conformance self-referential entry`
  - Verification: `grep -c "^openspec-conformance:" sandbox/reports/evidence_map.yaml && echo "FAIL" || echo "PASS"`

---

## Phase 2: Add 30 New Evidence Map Entries

> **Evidence honesty constraint**: For every entry below, the apply agent MUST verify the `evidence_path` resolves to ≥1 existing file on disk using `ls`, `test -f`, or glob before writing the entry. Entries whose paths fail pre-validation MUST be flagged to the user before proceeding. The 2 feature-gated specs (mcp-multimodal-tools, multimodal-frontend) MUST include the `evidence_note` field documenting compile debt — do NOT rubber-stamp them as plain verified.

### Group A — 28 entries (verified, real test files confirmed to exist)

- [x] **2.1** Add `impact-analysis-service` entry — DONE
- [x] **2.2** Add `spotter-search` entry — DONE
- [x] **2.3** Add `explorer-impact-tools` entry — DONE
- [x] **2.4** Add `named-view-persistence` entry — DONE
- [x] **2.5** Add `generic-graph-model` entry — DONE
- [x] **2.6** Add `graphlanding-affordances` entry — DONE
- [x] **2.7** Add `ask-router` entry — DONE
- [x] **2.8** Add `edge-provenance` entry — DONE
- [x] **2.9** Add `docs-source-adapter` entry — DONE
- [x] **2.10** Add `pane-navigation` entry — DONE
- [x] **2.11** Add `snapshot-graph-executor` entry — DONE
- [x] **2.12** Add `unsupported-operation-errors` entry — DONE
- [x] **2.13** Add `repository-trait-bridge` entry — DONE
- [x] **2.14** Add `lsp-testing` entry — DONE
- [x] **2.15** Add `moldplan-graphplan` entry — DONE
- [x] **2.16** Add `renderer-registry-frontend` entry — DONE
- [x] **2.17** Add `view-registry-backend` entry — DONE
- [x] **2.18** Add `view-spec-domain` entry — DONE
- [x] **2.19** Add `viewspec-authoring-flow` entry — DONE
- [x] **2.20** Add `lsp-proxy` entry — DONE
- [x] **2.21** Add `entry-point-resolver` entry — DONE
- [x] **2.22** Add `explorer-forward-reach` entry — DONE
- [x] **2.23** Add `ownership-map` entry — DONE
- [x] **2.24** Add `relation-candidates` entry — DONE
- [x] **2.25** Add `example-object-view` entry — DONE
- [x] **2.26** Add `project-diary-view` entry — DONE
- [x] **2.27** Add `runtime-ladybug-wiring` entry — DONE
- [x] **2.28** Add `mcp-multimodal-tools` entry (feature-gated) — DONE
- [x] **2.29** Add `multimodal-frontend` entry (feature-gated) — DONE
- [x] **2.30** Pre-validate ALL 30 evidence paths — DONE

---

## Phase 3: Implement `--validate-paths` in openspec_conformance.py

- [x] **3.1 RED** Write failing test — DONE
- [x] **3.2 GREEN** Add `validate_paths()` function — DONE
- [x] **3.3 GREEN** Wire `--validate-paths` into `main()` — DONE
- [x] **3.4 REFACTOR** Refine error format — DONE

---

## Phase 4: Apply Scorecard Formula Change

- [x] **4.1** Modify `gate_g10()` in `release_scorecard.py` — DONE

---

## Phase 5: Regenerate Artifacts and Verify G10 GREEN

- [x] **5.1** Run harness with `--validate-paths` — DONE
- [x] **5.2** Run scorecard to regenerate `scorecard.md` — DONE
- [x] **5.3** Smoke-test: verify no stale entries — DONE
- [x] **5.4** Smoke-test: validate-paths fails on fake path — DONE

---

## Phase 6: Vault Updates

- [x] **6.1** Update INC-005 status to `closed` — DONE
- [x] **6.2** Write ADR-031 §4 amendment — DONE

---

## Phase 7: ROADMAP Update

- [x] **7.1** Update `docs/ROADMAP.md` — DONE

---

## Dependency Chain

```
1.1 → 1.2 → 1.3          (Phase 1: stale removals — sequential, but independent)
         ↓
2.1 → 2.2 → ... → 2.30  (Phase 2: add entries — sequential within phase; 2.30 is the pre-validate gate)
         ↓
3.1 → 3.2 → 3.3 → 3.4   (Phase 3: validate-paths — RED must precede GREEN)
         ↓
4.1                         (Phase 4: scorecard formula)
         ↓
5.1 → 5.2 → 5.3 → 5.4   (Phase 5: regeneration + verification — sequential)
         ↓
6.1 → 6.2                 (Phase 6: vault updates — after G10 GREEN confirmed)
         ↓
7.1                         (Phase 7: ROADMAP — last step)
```

---

## LOC Estimate Summary

| Phase | Files | Est. LOC |
|-------|-------|----------|
| Phase 1 | `evidence_map.yaml` | −9 (3 stale entries removed) |
| Phase 2 | `evidence_map.yaml` | +95 (30 entries × ~3 lines each) |
| Phase 3 | `openspec_conformance.py` | +18 (`validate_paths()` + argparse wiring) |
| Phase 4 | `release_scorecard.py` | +12 (gate_g10 denominator + evidence_text) |
| Phase 5 | (artifact regeneration, no code) | 0 |
| Phase 6 | vault files | +30 (INC-005 + ADR-031 §4) |
| Phase 7 | `ROADMAP.md` | +5 |
| **Total** | | **~151 net** |

---

## TDD Note

Strict TDD applies only to Phase 3 (`--validate-paths` flag + `validate_paths()` function) and Phase 4 (scorecard formula). The harness itself is the test harness:

- **RED**: fake-path fixture → expect exit 1 + `VALIDATION ERROR` on stderr
- **GREEN**: implement `validate_paths()` + wire flag
- **REFACTOR**: ensure error format matches spec (`VALIDATION ERROR: <spec>: <reason>`)

The scorecard formula change (Phase 4) is verified by the full pipeline integration test (Phase 5.2): `scorecard.md` grep for `G10.*GREEN` is the test.

---

## Evidence Path Pre-validation Instruction (MANDATORY)

Before writing ANY of the 30 entries in Phase 2, the apply agent MUST run the pre-validation check for each evidence_path. If the path does not exist:

1. Report which spec:path pair is broken
2. Do NOT silently skip — the user must decide whether to accept a missing-path entry or find an alternative path
3. The 2 feature-gated specs (mcp-multimodal-tools, multimodal-frontend) are KNOWN MISSING on some build configurations — record the honest `evidence_note` but still verify the files exist at the paths cited

**Pre-validation command template:**
```bash
# Single path
test -f "<path>" && echo "EXISTS" || echo "MISSING: <spec> <path>"

# Multi-path (comma/semicolon separated)
for p in <paths>; do test -f "$p" || echo "MISSING: $p"; done
```
