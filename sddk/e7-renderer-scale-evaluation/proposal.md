# Kernel Proposal: E7 — Explorer Renderer Scale Evaluation

## Intent

ADR-039 reserved the next renderer decision (item 5 of the evolution order) as a
WebGL / Sigma.js evaluation. ADR-041 reframed that decision: the next step is
**a reproducible benchmark of the current renderer family**, not a renderer
migration. This proposal activates that evaluation as the E7 milestone on the
Explorer roadmap.

The goal of E7 is to **reduce uncertainty** about the Explorer's interactive
graph surface so the team can choose the next renderer move with evidence. E7
must end in one of three explicit outcomes. It does not pick the outcome in
advance.

## Context Gate

| Knowledge Coverage | Quality | Taxonomy | Extra Effort |
|--------------------|---------|----------|--------------|
| sufficient | C3 | boundary-seam, api-contract, coupling, performance-budget | verify |

The context is well-covered because the proposal sits on top of recent
artifacts:

- ADR-039 (Explorer navigation model)
- ADR-040 (GraphViewRenderer routing)
- ADR-041 (E7 evaluation scope, exit criteria, decision guardrails)
- `docs/explorer-roadmap.md` (current state table, E2 marked implemented)
- `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`
- `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts`

## Knowledge Alignment

- **Roadmap / Backlog:** E7 entry already exists in `docs/explorer-roadmap.md`
  under "After E6: Future Evolution".
- **Work Items / Specs:** none yet — this proposal is the kickoff of the
  implementation.
- **ADR / Architecture Sources:** ADR-039 §5 (renderer base + WebGL future),
  ADR-041 (full E7 evaluation contract).
- **Ownership Source:** frontend-led, with backend support if the team decides
  to export fixtures from live `/api/graph/...` payloads.
- **Prior Learnings:** ELK is already wired into `InteractiveGraph.tsx` with
  progress, cancellation, and a size guard. Renderer adapters must not regress
  that flow.

## Knowledge Decisions

- **Stays memory-only:** none.
- **Promote to durable knowledge:** this proposal, the spec, the design, and
  the tasks all live under `sddk/e7-renderer-scale-evaluation/`.
- **Scope correction:** the previous notes conflated "WebGL" with "Sigma.js".
  ADR-041 separates them. This proposal honors that separation.

## Lens Routing

| Lens | Delegation | Status | Proposal Impact |
|------|------------|--------|-----------------|
| base-discipline | kernel | applied | Enforces the scope/exit-criteria discipline from ADR-041. |
| boundary-seam | custom heuristic | deepened | Benchmark adapters are isolated behind a renderer interface so the production `InteractiveGraph` does not absorb benchmark code. |
| api-contract | custom heuristic | applied | `Fixture` and `Metrics` schemas are first-class contracts shared by all adapters. |
| coupling-entropy | custom heuristic | verified | New code lives under `apps/explorer-ui/src/bench/`, which is a fresh subtree with no inbound production edges. |
| performance-budget | custom heuristic | applied | The harness measures render, layout, and interaction timings before any migration decision. |

## Scope

### In Scope

- A benchmark harness under `apps/explorer-ui/src/bench/` with:
  - `fixtures/` — stable graph payloads covering call, dependency,
    architecture/C4, and landing-overview shapes, in small/medium/large bands
  - `fixture-schema.ts` — first-class fixture contract
  - `runner.ts` — scenario executor
  - `metrics.ts` — common metrics record
  - `renderers/cytoscape-canvas.ts` — baseline adapter (production path)
  - `renderers/cytoscape-webgl.ts` — same data, different renderer config
  - `renderers/sigma-poc.ts` — only enabled after a documented gate
- A bench script (`bench:renderer`) that emits a JSON and Markdown report into
  `artifacts/e7-renderer-bench/`.
- A first-class fixture schema shared by all adapters.
- Vitest smoke tests that mount each renderer adapter against the fixture
  schema in jsdom.
- An update or follow-up ADR after E7 closes.

### Out Of Scope

- A Sigma.js migration. E7 only opens the door; it does not perform the move.
- A Rust/WASM renderer replacement (see ADR-041 §5).
- Visual clustering, C4 inference backend, or new Explorer views.
- Any change to the production `InteractiveGraph` API. The benchmark mirrors
  the production contract without mutating it.

## Invariants

- `InteractiveGraph` keeps its current public surface.
- `toCytoscapeElements` and `buildStylesheet` remain the source of truth for
  node and edge styling.
- ELK layout worker integration stays as-is. E7 does not change layout.
- Renderer adapters must not import production components outside the
  benchmark subtree.
- The benchmark report contains both quantitative metrics and qualitative
  notes about behavior regressions.

## Domain Language

- **Resolved terms:**
  - `Fixture` — a stable graph payload with `fixture_id`, `kind`, `nodes`,
    `edges`, `node_count`, `edge_count`.
  - `Scenario` — a deterministic sequence of operations over a fixture
    (load, fit, pan, zoom, select, relayout).
  - `Metrics record` — a JSON object describing browser, OS, machine, fixture,
    renderer, timings, and observations.
  - `Cold run` — first execution after a fresh module load.
  - `Warm run` — any subsequent execution after the first.
- **Unresolved ambiguities:** none blocking. Numeric thresholds (for example,
  FPS targets at 5K nodes) are deferred to the benchmark harness because they
  belong with the measurement methodology, not the proposal.

## Capabilities

### New Capabilities

- `renderer-bench-fixture`: load stable fixtures from disk.
- `renderer-bench-scenarios`: execute load/fit/pan/zoom/select/relayout
  scenarios over a fixture.
- `renderer-bench-metrics`: collect a metrics record per run.
- `renderer-bench-cytoscape-canvas`: baseline renderer adapter.
- `renderer-bench-cytoscape-webgl`: WebGL renderer adapter.
- `renderer-bench-report`: emit JSON and Markdown report artifacts.

### Modified Capabilities

- `explorer-build`: adds the `bench:renderer` script.

## Approach

**Frontend, isolated subtree.** All benchmark code lives under
`apps/explorer-ui/src/bench/`. The production `InteractiveGraph` and its
helpers are reused by import, not duplicated.

**Fixture-first.** The harness is designed around `Fixture` and `Metrics` as
contracts. Adapters consume them and produce metrics. New renderer families
plug in by adding one file under `renderers/`.

**Three-phase measurement.** Phase 1 measures the current Cytoscape canvas.
Phase 2 measures Cytoscape WebGL. Phase 3 activates only if a documented gate
is met, and measures a Sigma.js proof of concept.

**Decision-by-report.** The benchmark produces a report. The report is the
input to the next ADR. The next ADR selects exactly one of three branches
(ADR-041 §8).

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `apps/explorer-ui/src/bench/` | new | Benchmark subtree |
| `apps/explorer-ui/src/bench/fixtures/` | new | Stable fixture payloads |
| `apps/explorer-ui/src/bench/runner.ts` | new | Scenario runner |
| `apps/explorer-ui/src/bench/metrics.ts` | new | Metrics record schema |
| `apps/explorer-ui/src/bench/renderers/` | new | Renderer adapters |
| `apps/explorer-ui/package.json` | modify | Add `bench:renderer` script |
| `artifacts/e7-renderer-bench/` | new | Output of the harness |

## Entropy Budget

| Metric | Estimate | Status |
|--------|----------|--------|
| Existing change entropy | medium | OK — fixtures and adapters mirror production shapes |
| New connascence | low | OK — one fixture schema, one metrics schema, three adapters |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| jsdom hides real render timings | high | Use a jsdom smoke pass and a real-browser bench script; never claim jsdom timings are representative |
| Cytoscape WebGL preview API changes | medium | Pin a version range in `package.json`; record browser/cytoscape version in metrics |
| Sigma POC drifts in scope | medium | Gate Sigma adapter behind an explicit config flag, not a default path |
| Bench output becomes flaky in CI | medium | Record warm and cold runs separately; require a stable machine profile doc |
| Fixture set never reflects real product shapes | medium | Pull at least the call and dependency fixtures from real `/api/graph/...` payloads in E7.1 |

## Rollback Plan

Delete the `apps/explorer-ui/src/bench/` subtree, remove the `bench:renderer`
script, and remove the `artifacts/e7-renderer-bench/` output. No production
code is changed by E7.

## Success Criteria

- [ ] `apps/explorer-ui/src/bench/` subtree compiles and passes lint/typecheck
- [ ] Vitest smoke tests cover `cytoscape-canvas` and `cytoscape-webgl`
      adapters against the fixture schema
- [ ] `bench:renderer` script runs the full scenario set on the
      `cytoscape-canvas` adapter and produces a metrics record per fixture
- [ ] A Markdown report is generated at
      `artifacts/e7-renderer-bench/report.md` with per-fixture timings and
      qualitative observations
- [ ] The Sigma adapter is gated by a config flag and is not run by default
- [ ] This proposal, the spec, the design, and the tasks are committed on the
      branch `feat/e7-renderer-scale-evaluation`