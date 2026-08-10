# Changelog

Todos los cambios notables de CogniCode se documentan en este archivo.
Formato basado en [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

> **Nota de reconstrucción**: v0.50.0–v0.86.0 no tenían entradas individuales
> en este archivo. Se reconstruyeron en E31-B5-rollup (B5) con un resumen
> de alto nivel; el historial commit-por-commit está disponible vía
> `git log v0.50.0..v0.86.0`. Las versiones anteriores a v0.50.0 no
> tienen entradas aquí; el historial completo puede reconstruirse
> desde `docs/ROADMAP.md` (working doc local, no versionado).

## [v0.50.0 — v0.86.0] — 2026-07-22 → 2026-08-05 (reconstructed summary)

Period summary: 307 commits across 36 version tags (v0.50.0 → v0.86.0)
over ~2 weeks. Major program cycles: E12 (ViewKind Realization),
E13-wave2 (knowledge-layer ports), E14 (narrative runtime),
E22 (knowledge-layer), E28.x (conformance audit), E29 (PostgreSQL →
LadybugDB migration), and E30 (sandbox infra + release-gate framework).

Cycle-by-cycle highlights:

- **E12 ViewKind Realization** (v0.76.0): vertical_slice, seam_map,
  summary, source_view executors (#170)
- **E13-wave2 Knowledge-Layer Ports** (v0.86.0): AdrInspector + Ladybug
  stubs + e2e green (#226); Phase 1 of the knowledge-layer
- **E14 Narrative Runtime** (v0.83.0): EmbedResolver + wiring in 4
  narrative shapers (#217)
- **E14-C2** (v0.85.0): NarrativeStore port + LadybugDB adapter +
  runtime wiring (#225)
- **E22 Knowledge Layer** (v0.74.x): `feat(e22-knowledge-layer)` Phase 1
- **E28.x Conformance Audit** (v0.75.0): E28 program complete
- **E29 PostgreSQL → LadybugDB Migration** (v0.76.5 — v0.80.1):
  - 6/6 spike stages (S1 build, S2 schema-load, S3 concurrency,
    S4 crash-recovery, S5 latency, S6 cypher-compat)
  - Phase 0 (clean-ports + define-new-ports + refactor-call-sites)
  - Phase 1 (9/9 lbug ports in `cognicode-ladybug` crate)
  - Phase 1.5 (e29-7 full PostgreSQL removal: RuntimePorts DTO +
    bootstrap_with_backend canonical entry; zero `pg_repo` +
    zero `PostgresBackend`)
  - Phase 3 (e29-3 port abstraction audit + debt-e29-3-1)
  - ADR-029 (CallGraphProjectionPort seam) + ADR-030 (QualityStore
    lbug schema)
- **E30 Sandbox Infra** (v0.87.0 era, started v0.86): 6/6 containers
  with real digest pins + hardened quadlets + Maven wrapper

**Commit distribution** (v0.50.0..v0.86.0):
- 101 feat
- 52 docs
- 44 fix
- 41 refactor
- 27 Merge
- 14 test
- 9 chore
- 7 style

**ADRs authored** (17): 001, 002, 003, 004, 005, 006, 007, 008, 009, 010,
011, 012, 013, 014, 015 (×2 — see E31-C renumber), 016, 017, 018, 026,
027, 028, 029, 030, 031, 032, 033.

For granular commit history, see `git log v0.50.0..v0.86.0`.

## [Unreleased]

### E31 — v1.0.0 readiness program (rounds 1-14)

This Unreleased section accumulates the E31 sub-cycles into the upcoming
v1.0.0 release. The actual tag cut is gated by:

- **T7 stability cadence**: 5 consecutive nights CV < 10% (counter: 0/5)
- **E31-G scorecard streak**: 3 consecutive ALL-GREEN scorecards (counter: 0/3)
- **G8 (scalability)**: SCAL-001 / INC-004 documented; 1G→4G mitigation applied
- **G2 (MCP tool coverage)**: 68/68 = 100% (from e30-corpus-expansion)
- **G6 (consistency)**: warm-cache CV 0.0435 (per E31-E cold-cache filter)
- **G10 (conformance)**: 100% triaged (per E31-F)
- **G11 (docs)**: 14 ADRs reviewed (per E31-C)
- **G13 (test plan)**: T1-T6 GREEN (per E31-B + E31-E + E31-F)

Program context:

- **E31-A** (PR #237): Release plan ACEPTADO + Pillar 7 (Test Plan comprehensivo, G13 scorecard gate)
- **E31-B** (PR #239): `docs/TEST-PLAN.md` (T1+T2 closed)
- **E31-D** (PR #238): W-2b multimodal skip-loopholes → OBSOLETE banners
- **E31-B2** (PR #240): 8 Tier-1 python scenarios (build_graph → 5/5)
- **E31-B3** (PR #241): 8 Tier-1 typescript scenarios (4 tools → 5/5)
- **E31-B4** (PR #242): 16 Tier-1 go+java scenarios (8 tools → 3/5)
- **E31-B5** (PR #243): T6 CI gate (LOCAL-ONLY via `act` + `podman` per user directive)
- **E31-B6** (PR #244): T7 stability cadence (per-scenario flaky log + nightly)
- **E31-B7** (PR #245): 8 Tier-1 ts+py closure (8 B4 tools → 5/5; closure 24.7%)
- **E31-B8** (PR #246): 8 Tier-1 ts+py closure round 2 (8 more → 5/5; closure 35.6%, ≥30% target met)
- **E31-C** (PR #247): 14 ADRs PROPOSED → ACCEPTED/SUPERSEDED + ADR-015 renumber
- **E31-E** (PR #248): read_file CV 0.528 outlier → cold-cache filter + `cv_warm` field
- **E31-F** (PR #249): conformance matrix reconcile + SCAL-001 evidence ref in G8
- **E31-G** (PR #250): 3-consecutive-scorecard-runs counter (per ADR-031 §3)

### Tier-1 closure progression

| Cycle | Tools @ 5/5 | python col | ts col | go col | java col | Closure % |
|-------|-------------|------------|--------|--------|----------|-----------|
| E30.5 (PR #236, v0.92.0) | 5 | 5 | 7 | 16 | 16 | 6.8% |
| E31-B2 (PR #240) | 6 | 11 | 7 | 16 | 16 | 8.2% |
| E31-B3 (PR #241) | 10 | 11 | 15 | 16 | 16 | 13.7% |
| E31-B4 (PR #242) | 10 | 11 | 15 | 24 | 24 | 13.7% |
| E31-B7 (PR #245) | 18 | 19 | 23 | 24 | 24 | 24.7% |
| E31-B8 (PR #246) | 26 | 27 | 31 | 24 | 24 | 35.6% |

## [v0.92.0] — 2026-08-10

- E30.5 release-gate carry-forwards: `score_smoke_matchers()` helper extracted in `sandbox-core/scoring.rs` (W-1 closed); `assert_family_consistency()` startup guard in `release_scorecard.py` (W-2a closed). Resolves the E30 program's 2 carry-forwards. Direct merge PR #236, merge commit `42fcdb14`.

## [v0.91.1] — 2026-08-07

- E30 release-gate: 12/12 nightly scorecards executed; G1-G12 baseline frozen; G5/G6/G8 measured; carry-forwards W-1 (smoke matchers in scoring) + W-2 (assert_family_consistency guard) trackeados. Note: tag downshifted from user-spec v0.91.0 to v0.91.1 due to existing v0.91.0 on origin. Direct merge PR #233.

## [v0.91.0] — 2026-08-06

- E30.4 conformance evidence: `openspec_conformance.py` harness with `--validate-paths` and `--evidence-map` (`sandbox/reports/evidence_map.yaml` 61 entries); G10 wired (verified 100.0% / triaged 100.0% on 433 reqs after legacy_obsolete exclusion per ADR-031 §4); 6/6 Tier-C specs marked OBSOLETE; SPEC delta sync (`openspec-conformance` + `release-readiness-gate`). PR #232 direct merge, v0.91.0 MINOR taggeado out-of-band.

## [v0.90.0] — 2026-08-06

- E30 conformance-audit: openspec conformance harness (`openspec_conformance.py` 133 LOC, 431 reqs / 67 specs / 4 phantom dirs), `evidence_map.yaml` (33 entries), `conformance_matrix.{yaml,md}` derived; scorecard gates G10/G11/G12 wired; CHANGELOG.md canonical (Keep a Changelog v0.87.0→v0.89.0 + Unreleased); branch pruning `prune_stale_branches.sh`; ADR-031 + ADR-032 → ACEPTADO. PR #231 direct merge, v0.90.0 MINOR taggeado.

## [v0.89.0] — 2026-08-06

- E30 Fase 3: `e30-metric-baseline` — primer Release Readiness Scorecard de 12 gates (6 GREEN / 3 AMBER / 3 RED), baseline de rendimiento congelado, 3 campañas full, stability.json (G6 CV < 5%), G8 probe (typescript tier-3 timeout → SCAL-001), límites de contenedor a 4G.

## [v0.88.1] — 2026-08-06

- Hotfix: `js_repos.yaml` / `ts_repos.yaml` con `pinned_sha` a SHAs exactos (deuda C3.3).

## [v0.88.0] — 2026-08-06

- E30 Fase 2: `e30-corpus-expansion` — G2 tool coverage 68/68 (denominador runtime real vía probe paginado), corpus +5 repos (tokio, clap Tier-1; rust-analyzer, TypeScript, react Tier-3), SHA-pinning 27 repos, coverage generator + scorecard, MCP-TOOLS.md regenerado, matchers count-only.

## [v0.87.1] — 2026-08-06

- `e30.1-clippy-baseline-reset` — 490 clippy errors → 0 (baseline reset por archivo), match arms duplicados eliminados (-1557 LOC), deuda sandbox cerrada, CI Format & Lint GREEN por primera vez.

## [v0.87.0] — 2026-08-06

- E30 Fase 0: `e30-sandbox-infra` — 6/6 quadlets activos con digests reales, hardening go.container, migración Maven (mvnw), workflow nightly, smoke lane exit 0.
