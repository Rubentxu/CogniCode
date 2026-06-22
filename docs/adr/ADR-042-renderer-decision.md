# ADR-042: Renderer decision outcome

**Status:** Accepted
**Date:** June 21, 2026
**Source:** Real-browser bench run captured in
`apps/explorer-ui/artifacts/e7-renderer-bench/{results.json,report.md}`
on 2026-06-21.

## Context

ADR-041 reserved the next renderer decision behind a reproducible
benchmark harness. ADR-042 was the placeholder that turned into
this decision once the harness produced real Chromium timings.

## Decision

**Adopt Cytoscape WebGL selectively** (ADR-041 §8 outcome 2).

The Explorer continues to use Cytoscape.js as its primary renderer.
WebGL mode is enabled through the existing `InteractiveGraph` mount
path. The renderer config block follows the cytoscape 3.31+ preview
API: `renderer: { name: "canvas", webgl: true }`.

This decision does **not** require a Sigma.js migration and does not
require a production change to `InteractiveGraph`. The WebGL opt-in
is a renderer config flag; the data path, adapter, and ELK layout
worker all remain unchanged.

## Evidence

The bench produced 24 records across 6 fixtures x 2 renderers x
2 runs (cold + warm). Cold-load timings:

| Fixture (nodes) | Canvas (ms) | WebGL (ms) | Winner |
|------------------|-------------|------------|--------|
| call-graph-small (12) | 52.5 | 41.3 | WebGL 21% faster |
| call-graph-medium (1,000) | 522.6 | 417.1 | WebGL 20% faster |
| call-graph-large (5,000) | 2,197.4 | 2,080.9 | WebGL 5% faster |
| dependency-graph-small (14) | 16.2 | 19.4 | Canvas 17% faster |
| architecture-c4-medium (10) | 13.8 | 19.2 | Canvas 39% faster |
| landing-overview-medium (16) | 16.0 | 19.9 | Canvas 24% faster |

All 24 runs passed the behavior gates (selection_works,
edge_highlight_works, layout_completed). Zero regressions.

### Interpretation

WebGL mode shows measurable gains on the only workload that
actually exercises the Explorer's scale target — the call-graph
projections at 1,000+ nodes. On small fixtures the WebGL preview
has a fixed initialization cost that makes it slower than canvas
mode by 15-25%.

The clean rollout is therefore selective: WebGL is most valuable
where users actually feel the canvas-vs-WebGL difference, which is
the large-graph case.

## Rollout

1. The `InteractiveGraph` mount path accepts a renderer config
   block. The default today is `{ name: "canvas" }`. The new default
   becomes `{ name: "canvas", webgl: true }`.
2. A future opt-out exists for users on machines that report WebGL
   issues (none observed in the bench run, but the option is cheap
   to expose).
3. Sigma.js is **not adopted**. The Sigma POC adapter remains in
   `apps/explorer-ui/src/bench/renderers/sigma-poc.ts` behind
   `BENCH_ENABLE_SIGMA=1` for future exploration. ADR-041 §8
   outcome 3 is not selected.

## Consequences

### Positive

- Measurable 5-21% load improvement on call-graph projections at
  every scale band tested.
- No data-path migration. The cytoscape adapter, stylesheet, ELK
  layout worker, and InteractiveGraph component are untouched.
- Zero regressions across all 6 fixture shapes.
- The bench harness remains in place for future regressions.

### Negative

- WebGL initialization costs ~3-6 ms more than canvas for small
  graphs. Users on tiny graphs do not see a benefit.
- The cytoscape WebGL preview API is on a moving target. Future
  cytoscape releases may change or remove it. The harness detects
  the regression and reports it in `behavior.regressions`.

### Neutral

- The Sigma adapter stays as a future exploration path. It is not
  selected.

## References

- `docs/adr/ADR-039-explorer-navigation-model.md`
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md`
- `apps/explorer-ui/src/bench/`
- `apps/explorer-ui/artifacts/e7-renderer-bench/{results.json,report.md}`
- `apps/explorer-ui/e2e/bench-renderer.spec.ts`