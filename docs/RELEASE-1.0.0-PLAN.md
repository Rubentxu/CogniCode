# CogniCode Release 1.0.0 Plan — Production Ready with Strict Verification

**Status**: ACEPTADO (promoted 2026-08-10 in E31-A; pillar 7 "Test Plan comprehensivo" added per user directive at E31 pre-flight)
**Date**: 2026-08-05 (original PROPOSED) / 2026-08-10 (ACEPTADO + pillar 7)
**Owner**: Rubentxu (maintainer)
**Baseline (original)**: v0.85.0 (e14-C2 merged, PR #225)
**Baseline (current execution)**: v0.92.0 (PR #236, E30.5.1 release-gate carry-forwards closed)

---

## 1. Definition of 1.0.0

**1.0.0 means production ready, verified against real software projects, with automated, repeatable, evidence-based checks. Test Plan comprehensivo + E2E coverage must be in place before any cut.** No release without proof.

The release gate is a **Release Readiness Scorecard** — a machine-generated report (not a checklist of opinions) with 12 hard criteria, each with a measurable target, a current value, and an evidence artifact. Every criterion must be **GREEN** for 3 consecutive runs before tagging `v1.0.0`.

| # | Criterion | Target | Evidence |
|---|-----------|--------|----------|
| G1 | e13-wave2 knowledge layer complete | 100% tasks done | Branch merged, spotter 11 families |
| G2 | MCP tool coverage in sandbox | 100% of 43 tools covered by ≥1 scenario | coverage matrix (auto-generated) |
| G3 | MCP Health Score | ≥ 85/100 for 3 consecutive runs | `sandbox/results-runs/<id>/health.json` |
| G4 | Correctness (ground truth) | ≥ 90% on Tier-1 repos | scoring engine `correctitud` |
| G5 | Latency budget | search < 500ms p95; call-graph < 2s p95 (10k LOC); analytics < 5s p95 | benchmark + latency scores |
| G6 | Consistency | run-to-run variance < 10% | stability.json (repeat ≥ 3) |
| G7 | Robustness | 0 crashes (panic/SIGSEGV/OOM) across full campaign | failure class audit |
| G8 | Scalability | ingest 100k+ LOC repo without timeout/OOM | scale tier scenarios |
| G9 | No regressions vs baseline | 0 unexpected failures vs saved baseline | `orchestrator report --baseline` |
| G10 | Openspec conformance | 100% of 401 requirements verified | conformance audit report |
| G11 | Documentation current | MCP-TOOLS 43 tools verified; ADRs reviewed; ROADMAP reconciled | doc audit |
| G12 | Release hygiene | changelog v0.85.0 → v1.0.0; semver clean; no stale branches | git audit |
| **G13** | **Test Plan comprehensivo (Pillar 7)** | **`docs/TEST-PLAN.md` ACEPTADO + T1–T6 GREEN en scorecard run** | **`docs/TEST-PLAN.md` + flaky scenario log + coverage matrices** |

**Explicit non-goals for 1.0.0**: full ISO GQL/Cypher compatibility, WebGL renderer, production Neo4j backend, multi-user collaboration. These remain post-1.0 candidates.

---

## 1.1 Test Plan Comprehensivo (Pillar 7 — added 2026-08-10)

**v1.0.0 no se ejecuta sin un Test Plan firmado que cubra estrategia, niveles y ownership.**

Concretamente:

| # | Sub-criterion | Threshold |
|---|---------------|-----------|
| T1 | `docs/TEST-PLAN.md` existe y está firmado por el maintainer | required |
| T2 | Niveles definidos: unit / integration / sandbox-E2E / browser-E2E / perf-regression | 5 niveles obligatorios |
| T3 | Cada MCP tool tiene ≥1 scenario por Tier-1 language (rust, ts, py, go, java) | 100% Tier-1 coverage |
| T4 | Cada UI pane tiene ≥1 browser-E2E spec | 100% pane coverage |
| T5 | Sandbox-E2E corre en nightly CI y reporta flaky scenarios | 0 unknown-flaky, known-flaky documentados |
| T6 | Regression test nuevo por cada `fix(*)` desde v0.92.0 | 100% (el test del fix forma parte del PR) |
| T7 | Scorecard estable ≥N campañas consecutivas con T1–T6 GREEN | N=5 (~1 semana de cadencia diaria) |

La implementación del Test Plan es un sub-ciclo de E31 (E31-B) que produce el documento + matrices de cobertura. Sin E31-B cerrado, ningún otro criterio se valida como habilitado para el cut.

**Rationale (decisión del usuario, 2026-08-10)**: "no llegamos al v1.0.0 hasta que se pruebe todo bien con test plan y otras pruebas e2e". Convierte testing disciplinado de nice-to-have a gating pillar.

---

## 2. Current State (audited 2026-08-05)

### 2.1 Product
- **Version**: v0.85.0. Workspace 0.5.0; 14 crates; 2 binaries (explorer-api, explorer-mcp) + sandbox-orchestrator.
- **MCP tools**: 43 documented (`docs/MCP-TOOLS.md`), 8 functional groups.
- **Openspec**: 401 requirements, 956 scenarios across ~60 specs (none formally marked FULL — conformance audit pending).
- **Tests**: ~2,791 Rust tests (post-e29-3) + 38 vitest + 41 Playwright spec files.
- **Knowledge layer (e13-wave2)**: Phase 1 ports DONE on branch `feat/e13-wave2-knowledge-layer-ports` (commit `aa23af61`, build green, 949 explorer tests). Phases 2–4 (AdrInspectorExecutor, Ladybug stubs, wiring, UI tests) PENDING.

### 2.2 Sandbox — what exists (good bones, broken containers)
- **Orchestrator**: `sandbox-orchestrator` compiles. Supports run/plan/report/benchmark/autoresearch, JSONL history, trend detection, stability runs, HTML reports.
- **Scoring engine**: 5 dimensions (correctness, latency, scalability, consistency, robustness) with ground-truth matchers (symbols, edges, entry points, hot paths, leaf functions, usages, search results, outline, code, complexity, index completeness, behavioral preservation, merge accuracy).
- **Manifests**: 40+ YAML, schema.json, tiers A/B/C, per-language + per-repo manifests.
- **Repos cloned** (22): ripgrep, serde, anyhow, chalk, express, commander, zod, cobra, bubbletea, lo, spring-petclinic, click, urllib3, requests, hiredis, json, spectre-console, elixir, slim, sinatra, argument-parser + fixtures.
- **History**: `sandbox/results-runs/` with stability.json pattern (last runs 2026-06-19 — stale).
- **BROKEN**: `sandbox/containers/*.container` are templates with fake digest pins. Real quadlets exist in `~/.config/containers/systemd/` (cognicode-postgres running, podman 5.8.4). Maven missing. No sandbox CI workflow in GitHub Actions.

### 2.3 CI
- GitHub Actions: `ci.yml` — unit tests, fmt, clippy, ownership feature test. **No sandbox lane, no nightly matrix, no PG job.**

---

## 3. Architecture of the Verification System

```
Real GitHub projects (pinned commits)
        │  clone_repos.sh
        ▼
sandbox/repos/ (22+ repos, Tier 1/2/3)
        │  manifests/*.yaml (scenarios × tools × languages)
        ▼
sandbox-orchestrator (Rust)
        │  podman quadlets (isolated per-language runtimes)
        ▼
result.json per scenario  ──►  scoring engine (5 dims, ground truth)
        │                         │
        ▼                         ▼
report.html              runs.jsonl (health history + trends)
        │                         │
        ▼                         ▼
Release Readiness Scorecard  stability.json (variance)
```

### 3.1 Container strategy (podman + quadlets — real)
Per-language hardened quadlets at `~/.config/containers/systemd/`:

| Service | Image (pinned digest) | Purpose |
|---------|----------------------|---------|
| `cognicode-postgres` | postgres:16 (already running) | canonical graph store |
| `cognicode-rust` | rust:1.80-slim | Rust repos (ripgrep, serde, anyhow, tokio…) |
| `cognicode-python` | python:3.12-slim | Python repos (click, urllib3…) |
| `cognicode-go` | golang:1.23-alpine | Go repos (cobra, bubbletea, lo) |
| `cognicode-java` | eclipse-temurin:17-jammy + maven | spring-petclinic |
| `cognicode-node` | node:22-slim | JS/TS repos (chalk, express, commander, zod) |

Hardening per quadlet: `Network=none` (or a dedicated `cognicode.network`), `MemoryMax`, `PidsLimit`, `ReadOnly=yes` with rw mounts for `/workspace` and `/repos`, `NoNewPrivileges=yes`, `Tmpfs=/tmp`, pinned digest + `AutoUpdate=no`.

Digest pinning procedure: `podman pull <image>` → `podman inspect --format '{{.ImageDigest}}' <image>` → write digest into quadlet → `systemctl --user daemon-reload && systemctl --user start cognicode-*`.

### 3.2 Real-project corpus (candidates)

**Tier 1 — Rust validation core (deep ground truth)**
| Repo | Why | Metrics focus |
|------|-----|---------------|
| ripgrep | Large real CLI (14k LOC), multi-crate | symbol accuracy, call graph, subgraph |
| serde | Procedural macros, generics (8k LOC) | macro handling, generics, derive |
| anyhow | Small, idiomatic error handling | edge cases, entry points |
| tokio | Large async runtime (**NEW — recommended**) | async, traits, scaling |
| clap | CLI framework (**NEW — recommended**) | generics, builder API |

**Tier 2 — Multi-language (capability breadth)**
| Repo | Lang | Focus |
|------|------|-------|
| cobra | Go | CLI, packages |
| bubbletea | Go | TUI, generics |
| lo | Go | utility library |
| zod | TS | types, generics |
| commander | TS | CLI |
| express | JS | middleware, routing |
| spring-petclinic | Java | OOP, annotations, Maven |
| click / urllib3 / requests | Python | decorators, modules |

**Tier 3 — Stress/extreme (scalability proof)**
| Repo | Lang | Why |
|------|------|-----|
| rust-analyzer (**NEW**) | Rust | 200k+ LOC, real-world scale |
| typescript (**NEW**) | TS | huge codebase, language server |
| react (**NEW**) | TS/JS | large app framework |

Cloning is fully automated via `clone_repos.sh` with **pinned commits** (reproducibility): every repo pinned to a SHA; script verifies current HEAD and re-pins on drift.

### 3.3 Metrics (already implemented in scoring engine — wire into scorecard)

| Dimension | Metric | Ground-truth matchers |
|-----------|--------|----------------------|
| Correctness | precision/recall, F1, exact match | symbols, edges, entry points, hot paths, usages, search results, outline, code, complexity, index completeness, merge accuracy |
| Latency | target_ms, max_ms, p95/p99 | timing capture per scenario |
| Scalability | linear/sub-linear/quadratic class, breakpoint_kb | scale fixtures |
| Consistency | variance threshold across repeats | stability.json (repeat ≥ 3) |
| Robustness | edge-case pass rate, failure classes | failure classification |

**Health Score** = weighted average (defaults in `scoring.rs`). **Trends** = improving/stable/regressing vs history. Both already code — the plan only wires them into the release gate.

### 3.4 Automations

| Automation | Trigger | Output |
|-----------|---------|--------|
| `just sandbox-pull` | manual | pinned images |
| `just sandbox-setup` | manual | quadlets deployed + repos pinned |
| `just sandbox-ci-smoke` | CI (PR) | < 5 min Tier-A validation |
| `just sandbox-ci-probe` | CI (PR) | capability probes (expected-fail) |
| `just sandbox-ci-full` | nightly cron | full matrix, JSONL, regressions vs baseline |
| `just sandbox-stability <manifest> 5` | nightly | stability.json + variance report |
| `just sandbox-benchmark <tool>` | nightly | latency percentiles |
| `just sandbox-report-html` | nightly | HTML dashboard |
| **NEW `just release-scorecard`** | nightly | Release Readiness Scorecard (12 gates) |

New GitHub Actions workflow `sandbox-nightly.yml` runs the full matrix every night on a self-hosted runner (or ubuntu + podman), archives `results-runs/` as artifacts, and publishes the scorecard to `gh-pages` or an issue.

---

## 4. Execution Plan (5 phases, SDDK cycles)

### Phase 0 — Sandbox infrastructure repair (1 cycle, manual + SDDK A-lite)
**Goal**: real quadlets, real images, `just sandbox-ci-smoke` green.
1. Pull images, extract real digests, rewrite `sandbox/containers/*.container` (source of truth) and deploy to `~/.config/containers/systemd/`.
2. Add Maven to Java image (Dockerfile + build script).
3. Re-pin all repos via `clone_repos.sh` (verify HEAD matches pinned SHA).
4. Run smoke lane; fix infra failures (exit 2) until green.
5. Add `sandbox-nightly.yml` GH workflow.
**Exit**: `just sandbox-ci-smoke` = exit 0; containers list shows 5 services active.

### Phase 1 — Complete knowledge layer (e13-wave2 Phases 2–4) (2 cycles)
- Cycle A (PR 2): `AdrInspectorExecutor` + `EvidenceStore` wiring in `SearchServiceImpl` + registry entries + unit tests (tasks 2.1–2.5).
- Cycle B (PR 3): Ladybug adapter stubs + `kindDefaultView.ts` + E2E unskip + ADR snapshot test (tasks 3.1–3.5, 4.1–4.3).
**Exit**: e13-wave2 100% tasks done; spotter 11 families; G1 green.

### Phase 2 — Corpus expansion + ground truth (1 cycle)
1. Add tokio, clap (Tier 1), rust-analyzer, typescript (Tier 3) to `clone_repos.sh` + manifests.
2. Generate ground-truth fixtures for new repos (symbol lists, call edges, entry points) — automated extraction script + human review.
3. Extend manifests to cover all 43 MCP tools (coverage matrix generator: tool × repo × scenario).
**Exit**: coverage matrix shows 100% tool coverage; G2 green.

### Phase 3 — Metric baseline (2 cycles, nightly automations)
1. Run full campaign 3× (repeat), save `baseline/` (JSONL + stability.json).
2. Run benchmark suite; record p95 per tool family.
3. Generate first Release Readiness Scorecard — expect several AMBER/RED gates; each becomes a tracked defect.
4. Fix defects found (SDDK cycles per cluster).
**Exit**: baseline frozen; scorecard artifacts; G5/G6/G8 measured.

### Phase 4 — Conformance audit + gap closing (2–3 cycles)
1. Openspec conformance audit: map 401 requirements → tests/scenarios; mark specs FULL; list gaps.
2. Close functional gaps (SDDK cycles): missing MCP tool behaviors, view executors, error-path conformance.
3. Doc audit: MCP-TOOLS vs actual handlers, ADR review (23 ADRs), ROADMAP reconcile.
4. Git hygiene: prune stale branches, changelog v0.85.0→v1.0.0.
**Exit**: G10, G11, G12 green.

### Phase 5 — Release gate (1 cycle, manual)
1. Run full campaign 3× consecutive nights — all 12 gates GREEN.
2. Tag `v1.0.0` (MINOR bump from v0.85.0, no breaking changes).
3. Publish scorecard + release notes; archive scorecard in repo `docs/analysis/release-1.0.0-scorecard.md`.
**Exit**: `git tag v1.0.0` pushed; release-report confirms `HEAD == origin/main` + tag.

---

## 5. Risk Register

| Risk | Impact | Mitigation |
|------|--------|------------|
| Sandbox runs too slow (full matrix > 30 min) | Nightly infeasible | Parallel `-j N` manifests; tiered lanes; smoke < 5 min |
| Container images drift | Non-reproducible results | Pinned digests + AutoUpdate=no + re-pin script |
| Ground truth wrong for new repos | False FAIL verdicts | Human review of extracted fixtures; confidence markers |
| GH runner lacks podman privileges | Nightly fails | Self-hosted runner or ubuntu-latest with podman; document setup |
| 401 requirements audit is large | Phase 4 slips | Batch by spec; automate mapping; accept partial FULL with tracked debt |
| Scorecard gate too strict | Release blocked forever | 12 gates negotiated with owner before Phase 3 baseline |

---

## 6. Deliverables per phase

| Phase | Deliverable | Location |
|-------|-------------|----------|
| 0 | Working quadlets + smoke green + nightly workflow | `sandbox/containers/`, `.github/workflows/` |
| 1 | e13-wave2 merged (3 PRs) | `main` |
| 2 | Expanded corpus + 100% tool coverage matrix | `sandbox/repos/`, `sandbox/manifests/`, `docs/inventory/tool-coverage-matrix.md` |
| 3 | Baseline + scorecard + defect backlog | `sandbox/results-runs/`, `sandbox/results/baseline/` |
| 4 | Conformance audit + closed gaps + docs current | `openspec/specs/`, `docs/` |
| 5 | Scorecard all GREEN + `v1.0.0` tag | `docs/analysis/release-1.0.0-scorecard.md` |

---

## 7. Immediate next actions (Phase 0 kickoff)

1. `podman pull` the 5 language images; record real digests.
2. Rewrite `sandbox/containers/*.container` with real digests; deploy via `sandbox-setup`.
3. Install Maven (or build Java image with Maven).
4. Run `just sandbox-ci-smoke`; fix until exit 0.
5. Open SDDK cycle for Phase 1 (e13-wave2 PR 2).
