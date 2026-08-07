# Release Readiness Scorecard

**Generated**: 2026-08-07T05:14:26Z

| Gate | Status | Measured | Budget | Evidence |
|------|--------|----------|--------|----------|
| G1 Git Hygiene / e13-wave2 PR Evidence | ✅ GREEN | — | — | Found 3 e13-wave2 commits in last 30: ce94a598 Merge pull request #226 from Rubentxu/feat/e13-wave2-knowledge-layer-ports |
| G2 MCP Tool Coverage | ⚠️ AMBER | — | — | coverage_matrix.yaml not found |
| G3 Sandbox Health Score | ⚠️ AMBER | — | — | no health_score data in any run |
| G4 Corpus Quality / Correctitud | ⚠️ AMBER | — | — | dimension_scores.correctitud not found |
| G5 Latency Budget by Tool Family | ⚠️ AMBER | no data for families: search, call-graph, analytics | — | sandbox/results |
| G6 Run-to-Run Stability | ⚠️ AMBER | — | — | stability.json not found |
| G7 Robustness — Zero Crashes | ✅ GREEN | 0 | 0 | no crash-class failures detected |
| G8 Scalability Proof (Tier-3) | ⚠️ AMBER | — | — | no g8-probe results found |
| G9 No Regressions vs Baseline | ✅ GREEN | 0 regressions | 0 | regressions_vs_baseline is empty in all runs |
| G10 Openspec Conformance Audit | ✅ GREEN | verified 100.0% / triaged 100.0% | >=90% verified, 100% triaged | total=433 verified=383 legacy_obsolete=50 pct_verified=100.0% (denom=total−legacy_obsolete=383, per ADR-031 §4) |
| G11 Documentation Currency | ✅ GREEN | — | — | MCP-TOOLS.md found (68 tools); ADR-031 found (ACEPTADO); ADR-032 found (ACEPTADO); ROADMAP.md found |
| G12 Git Hygiene (tags/changelog/branches) | ✅ GREEN | tag=v0.91.0 stale_branches=3 | — | latest semver tag: v0.91.0; CHANGELOG.md found; stale merged remote branches: 3 |