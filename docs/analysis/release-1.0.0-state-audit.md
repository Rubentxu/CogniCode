# Release 1.0.0 Readiness — State Audit (2026-08-05)

**Status**: EPHEMERAL (working analysis — kept local until stable)
**Sources**: ROADMAP.md (1324 lines), docs/RELEASE-1.0.0-PLAN.md, ADR-031, ADR-032, sandbox/ audit

---

## 1. Product state (v0.85.0)

### Shipped programs (all DONE)
- **E28** Graph Query & Analytics Platform — 6 subprograms, MoldQL patterns, differential executors, analytics registry (11 algorithms)
- **E29** LadybugDB migration — spike 6/6, Phase 0-1.5-2-3-4, full PostgreSQL removal, 9+ ports on LadybugStore
- **E18-E21** Moldable UX — Spotter, C4, diagrams, investigations
- **E12-E14** ViewKind realization, narrative runtime, decision trace
- **e13-wave1** Universal Spotter (8 families)

### In progress
- **e13-wave2-knowledge-layer-ports** — Phase 1 DONE (`aa23af61` on branch, build green, 949 explorer tests). Phases 2-4 pending: AdrInspectorExecutor, EvidenceStore wiring, Ladybug stubs, UI tests.

### Scale
- 14 crates, 2 main binaries + sandbox-orchestrator
- 43 MCP tools documented (`docs/MCP-TOOLS.md`)
- 401 openspec requirements, 956 scenarios, ~60 specs
- ~2,791 Rust tests + 38 vitest + 41 Playwright spec files
- 23 ADRs (now 25 with ADR-031/032)

## 2. Sandbox state

### Working
| Component | State |
|-----------|-------|
| sandbox-orchestrator (Rust) | ✅ compiles; run/plan/report/benchmark/autoresearch |
| Scoring engine (5D) | ✅ correctness/latency/scalability/consistency/robustness + 13 ground-truth matchers |
| History + trends | ✅ runs.jsonl, TrendDirection, health score |
| Manifests | ✅ 40+ YAML, schema.json, tiers A/B/C |
| Repos | ✅ 22 cloned (ripgrep, serde, anyhow, cobra, bubbletea, lo, chalk, express, commander, zod, spring-petclinic, click, urllib3, requests, + tier-B/C fixtures) |
| Stability + HTML reports | ✅ scripts present |

### Broken / missing
| Component | Issue |
|-----------|-------|
| Quadlets | ❌ `sandbox/containers/*.container` have fake SHA digests |
| Maven | ❌ missing (spring-petclinic blocked) |
| Sandbox CI | ❌ no GitHub Actions workflow |
| Results | ❌ last results-runs 2026-06-19 (stale ~7 weeks) |
| Digest pins | ❌ `just sandbox-pull` references fake digests |

## 3. Gap analysis vs 1.0.0 gates

| Gate | Current | Gap |
|------|---------|-----|
| G1 e13-wave2 | Phase 1 only | Phases 2-4 (~63 tasks file, 3 remaining work units) |
| G2 tool coverage | unknown | no coverage matrix exists |
| G3 health score | stale (June) | need fresh runs + ≥85 threshold |
| G4 correctness | unknown | need Tier-1 ground truth runs |
| G5 latency | unknown | need benchmark baselines |
| G6 consistency | unknown | need --repeat runs |
| G7 robustness | unknown | need failure class audit |
| G8 scalability | no tier-3 repos | need rust-analyzer/typescript/react |
| G9 regressions | no frozen baseline | need Phase 3 baseline |
| G10 conformance | 0 specs marked FULL | need 401-req audit |
| G11 docs | MCP-TOOLS 43 tools (unverified) | need handler registry cross-check |
| G12 hygiene | 20+ stale branches | need prune + changelog |

## 4. Recommended sequence (5 phases)

```
Phase 0: sandbox infra repair (quadlets reales, Maven, smoke green, nightly workflow)
Phase 1: e13-wave2 PR 2 + PR 3 (knowledge layer complete)
Phase 2: corpus expansion (tokio/clap Tier-1, tier-3 stress) + coverage matrix
Phase 3: metric baseline (3× campaign, benchmark, first scorecard)
Phase 4: conformance audit (401 reqs) + gap closing + docs
Phase 5: release gate (3 nights 12/12 GREEN → tag v1.0.0)
```

See `docs/RELEASE-1.0.0-PLAN.md` for the full plan with risk register and deliverables.
