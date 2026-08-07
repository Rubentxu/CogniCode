# Proposal: G10 Evidence Mapping — Close INC-005 (CONF-001)

## Intent
G10 (openspec conformance) is **RED** — 193 requirements across 30 specs have no mapped evidence. This blocks G10 GREEN → blocks E30 Phase 5 → blocks v1.0.0 per ADR-031's 12-GREEN rule. Fix: map evidence for all 30 specs (193 reqs), renegotiate the G10 `pct_verified` denominator to exclude 50 permanently-removed legacy_obsolete requirements (dead PG/SQLite code from E29/ADR-026), and add harness guardrails against future rubber-stamping. User approved denominator renegotiation (ADR-031 §4 clause).

## Scope

### In Scope
- Add 30 `evidence_map.yaml` entries: 28 implemented specs (existing tests, zero code changes) + 2 feature-gated multimodal specs (debt-noted)
- Renegotiate G10 formula in ADR-031 §4 + `release_scorecard.py:gate_g10`: `pct_verified = verified / (total − legacy_obsolete)`, `pct_triaged = (verified+legacy_obsolete)/total = 100%`
- Add `--validate-paths` guardrail to `openspec_conformance.py` (fail on non-existent evidence_path files/globs)
- Fix 3 stale evidence_map entries (quality-store, release-scorecard, openspec-conformance)
- Close INC-005, regenerate conformance_matrix + scorecard, update ROADMAP

### Out of Scope
- No new tests for ~10 feature-gated multimodal reqs (debt tracked)
- No implementation of missing features (none exist — all 30 specs are implemented)
- No per-requirement evidence granularity (per-spec is the harness contract)
- No changes to G1-G9, G11-G12

## Capabilities

> CONTRACT with sddk-spec. Research: `openspec/specs/openspec-conformance/spec.md`, `openspec/specs/release-readiness-gate/spec.md`.

### New Capabilities
None

### Modified Capabilities
- `openspec-conformance`: add `--validate-paths` flag requiring `evidence_path` globs resolve to ≥1 file; fix `OBSOLETE_RE` false-positive edge case; document per-spec evidence mapping contract
- `release-readiness-gate`: G10 metric formula changes from `pct_verified = verified/total` to `verified/(total − legacy_obsolete)` with new Given/When/Then scenarios; pct_triaged unchanged

## Approach
**Approach A (recommended by exploration, adopted by user).** Three tracks in parallel:

1. **Evidence mapping**: ~30 YAML entries (`status: verified`, real test-file paths). 28 specs map to existing Rust/TS tests. 2 multimodal specs (`mcp-multimodal-tools`, `multimodal-frontend`) mapped with compile-debt noted — implementation and tests exist behind `#[cfg(feature = "multimodal")]`.
2. **Metric renegotiation**: single-line change in `release_scorecard.py:455` — denominator becomes `total − legacy_obsolete`. ADR-031 §G10 updated with formula + justification (50 dead reqs inflated denominator, making ≥90% unreachable).
3. **Harness hardening**: `--validate-paths` arg checks each `evidence_path` exists (file or glob). AMBER on missing, RED if all paths invalid.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `sandbox/reports/evidence_map.yaml` | Modified | +30 entries, −3 stale, net 60 entries |
| `sandbox/scripts/openspec_conformance.py` | Modified | +`--validate-paths` arg (~15 LOC) |
| `sandbox/scripts/release_scorecard.py` | Modified | `gate_g10` line 455 denominator (~1 LOC) |
| `docs/adr/ADR-031-release-1.0.0-definition.md` | Modified | G10 formula + justification |
| `openspec/specs/openspec-conformance/spec.md` | Delta | Path validation req + scenarios |
| `openspec/specs/release-readiness-gate/spec.md` | Delta | G10 formula scenarios |
| `docs/ROADMAP.md` | Modified | E30.4 completed, G10 GREEN |
| `~/.sddk-knowledge/cognicode/incidences/INC-005-CONF-001.md` | Modified | status → closed |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Rubber-stamping (fake evidence_paths) | Low | `--validate-paths` gate + manual spot-check 5 specs |
| Metric gaming perception | Low | Honest ADR-031 documentation: 50 dead reqs excluded, triage still 100% |
| Feature-gated multimodal specs | Low | Map as `verified` + note compile debt; tracked outside this change |
| OBSOLETE_RE false-positive fragility | Low | Explicit fix scoped in spec delta |

## Rollback Plan
Revert `evidence_map.yaml` to 33 entries (git checkout). Revert `release_scorecard.py:455` to original `pct_v >= 90.0 and pct_t >= 100.0` with `pct_v = verified/total`. Remove `--validate-paths` from harness. Regenerate matrix. No DB migrations, no irreversible operations.

## Dependencies
- User approved G10 denominator renegotiation (CONFIRMED in launch plan)
- `openspec_conformance.py` (exists, 133 lines) — `--validate-paths` extends, does not break
- `release_scorecard.py` (exists, G10 at lines 434-468)

## Success Criteria
- [ ] `openspec_conformance.py --evidence-map evidence_map.yaml --validate-paths` exits 0 with verified ≥ 381, no_evidence = 0
- [ ] `release_scorecard.py` reports G10 GREEN: pct_verified ≥ 90.0 AND pct_triaged = 100.0
- [ ] INC-005 status is `closed`
- [ ] All 60 evidence_map entries survive `--validate-paths` (files exist)
