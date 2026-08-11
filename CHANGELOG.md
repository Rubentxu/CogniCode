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

### v1.0.0 — operational cut (pending pre-cut gates)

The actual v1.0.0 tag cut is gated by:

- **T7 stability cadence**: 5 consecutive nights CV < 10% (counter: 0/5)
- **E31-G scorecard streak**: 3 consecutive ALL-GREEN scorecards (counter: 0/3)
- **G8 (scalability)**: SCAL-001 / INC-004 documented; 1G→4G mitigation applied
- **G2 (MCP tool coverage)**: 68/68 = 100% (from e30-corpus-expansion)
- **G6 (consistency)**: warm-cache CV 0.0435 (per E31-E cold-cache filter)
- **G10 (conformance)**: 100% triaged (per E31-F)
- **G11 (docs)**: 14 ADRs reviewed (per E31-C)
- **G13 (test plan)**: T1-T6 GREEN (per E31-B + E31-E + E31-F)

## [v0.93.0] — 2026-08-11

Checkpoint release between E31 close (v0.92.0) and the operational v1.0.0
cut. MINOR bump justified by DEFECT-1's parameter-alias BC layer (public
surface change with backward-compat aliases) across 18 MCP tools.

### E31 program rollup closure

- **E31-E2** (`#254`): `retrieve_and_verify` CV 0.105 — ACCEPT (closes B1 deferred).
- **E31-B2-rollup** (`#255`): 178 Tier-3 scenarios quarantined via
  `known_quarantined.yaml` (closes B2); remote CI triggers
  (`push`/`pull_request`/`schedule`) disabled per E31-B5 user directive —
  `workflow_dispatch:` retained (closes B3).
- **E31-B4-rollup** (`#256`): Tier-1 closure round 3 — 8 more tools
  promoted to 5/5; T3 closure ~46.6%.
- **E31-B5-rollup** (commit `4d5f8bb6`): CHANGELOG.md v0.50–v0.86
  reconstruction (closes B5).
- **E31-B6-rollup** (`#258`): INC-001..004 closure as ACCEPT (closes B6).

### E32 distribution program (asdf-vm-inspired)

- **E32-A** (`#262`): `cogh` CLI binary core (install / list / current /
  latest / update / uninstall / plugin / reshim / doctor / where).
- **E32-B** (`#261`): plugin manifest + registry client + bundled plugins
  (mcp-server, skills-cognicode-core, sandbox-templates).
- **E32-C** (`#260`): portable skill bundles + `cogh skill validate`.
- **E32-D** (`#259`): opencode IDE adapter.
- **E32-E**: zcode IDE adapter.
- **E32-F**: claude IDE adapter.
- **E32-G** (`#264`): codex IDE adapter (TOML config).
- **E32-H** (`#265`): lifecycle integration tests.

### UAT defect closure (5 blockers flagged for v1.0.0)

- **DEFECT-1** (`#271`): `feat(mcp)` — parameter-alias BC layer
  (canonical → legacy naming) across 18 MCP tools. MINOR surface.
- **DEFECT-2** (`#268`): `test(mcp)` — build_graph `directory=.` and
  absolute path coverage.
- **DEFECT-3** (`#269`): `fix(mcp)` — `handle_smart_search` sub-handlers
  capped at 60s with graceful degradation.
- **DEFECT-4** (`#267`): `fix(uat)` — TC-1.3 path for requests fixture
  aligned to actual src-layout.
- **DEFECT-5** (`#270`): `fix(core)` — tree-sitter extractor honors
  `variable_types`; broader Rust variable shapes.

### Distribution deployment

- `chore(cogh)` (`#272`): bundled mcp-server bumped to v0.93.0 with real
  SHA256.
- `chore(cogh)` (`#273`): mcp-server manifest pointed at
  `Rubentxu/CogniCode/releases` (single-repo distribution).

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
