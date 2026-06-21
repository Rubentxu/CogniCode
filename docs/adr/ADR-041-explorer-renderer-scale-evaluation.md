# ADR-041: Explorer renderer scale evaluation (E7)

**Status:** Proposed
**Date:** June 21, 2026
**Source:** ADR-039 follow-up after code audit, documentation sync, and web
research on browser graph renderers

## Context

The Explorer already runs on a clear frontend graph stack:

- `Cytoscape.js` is the main interactive graph renderer.
- `elkjs` computes hierarchical and architecture layouts.
- `InteractiveGraph.tsx` already integrates the ELK worker with async layout,
  cancellation, progress feedback, and a size guard.

That means the next question is no longer, "Should we replace a static preset
graph with a real layout engine?" That work is done.

The real open question is different:

> When the Explorer needs to show larger visible graphs, what is the lowest-risk
> path to preserve interaction quality?

Recent external research changes the decision space:

1. `Sigma.js` remains a strong WebGL renderer for large graph exploration, but
   it requires a different graph model (`Graphology`) and would force a broader
   migration of adapters, styling, interaction glue, and tests.
2. `Cytoscape.js` now offers a preview WebGL mode, which may improve rendering
   throughput without replacing the current renderer contract.
3. Other options such as Cosmograph, Reagraph, `egui_graphs`, `three-d`, or
   Rust-native visualization crates do not fit the current Explorer as well as a
   React + Cytoscape-based product with C4 and architecture views.

We therefore need an evaluation milestone before any migration decision.

This ADR stays **Proposed** until the benchmark harness exists and the team has
measured the current renderer family against product-real workloads. The ADR is
deliberately approving the evaluation method first, not pre-approving a
renderer switch.

## Decision

### 1. E7 is an evaluation milestone, not a migration milestone

Sprint E7 evaluates renderer scale paths. It does **not** commit the product to
Sigma.js, a Rust/WASM renderer rewrite, or any immediate renderer replacement.

### 2. The evaluation order is fixed

The team evaluates options in this order:

1. **Cytoscape canvas baseline** — measure current behavior with the existing
   renderer and current styles.
2. **Cytoscape WebGL preview** — measure whether the existing renderer family
   can reach the required scale with acceptable feature tradeoffs.
3. **Sigma.js proof of concept** — only if Cytoscape WebGL fails the agreed
   thresholds or breaks required interaction patterns.

This order minimizes migration cost and protects the current architecture.

### 3. The evaluation uses product-real workloads

Benchmarks must use CogniCode-shaped graphs, not synthetic force-graph demos
only. The minimum workload set is:

- call graph slice
- dependency graph slice
- architecture / C4-style layered graph
- landing-page overview graph

Each workload should be tested at multiple visible scales, with at least these
bands:

- ~500 visible nodes
- ~2,000 visible nodes
- ~5,000 visible nodes

If the product expects larger visible graphs later, the benchmark set may grow,
but those three bands are the minimum gate.

### 4. Success metrics are explicit

Each candidate is judged on the same dimensions:

- first render time
- re-layout time
- pan / zoom interaction quality
- steady-state frame rate during interaction
- memory footprint
- feature parity with required node, edge, label, and selection behaviors
- integration cost with the existing `toCytoscapeElements`-style adapter layer
- test impact and migration cost

The evaluation is not only about FPS. A faster renderer that breaks C4-style
layout readability, pane-stack navigation, or style-class semantics is not a
win.

### 5. Rust-native renderer replacement is out of scope for E7

Rust crates may still help with preprocessing, clustering, or layout
experiments. They are not the primary renderer path for E7.

Specifically, E7 does **not** attempt to replace the Explorer graph surface
with:

- `egui_graphs`
- `three-d`
- `visgraph`
- a custom WASM renderer

These options can be revisited only if the JavaScript-based renderer families
fail the product constraints.

### 6. E7 deliverables are concrete

E7 must produce:

1. a benchmark fixture set with stable graph payloads
2. a reproducible benchmark harness
3. a short comparative report for Cytoscape canvas, Cytoscape WebGL, and, if
   needed, Sigma.js
4. a recommendation ADR update or follow-up ADR that either:
   - stays on Cytoscape,
   - enables Cytoscape WebGL for large views, or
   - approves a Sigma.js migration path

### 7. The benchmark protocol is part of the decision

The benchmark harness must be reproducible enough that future renderer changes
can be compared against the same baseline.

At minimum, each candidate run must record:

- browser and version
- operating system
- machine profile used for the run
- graph fixture id
- node and edge counts
- whether the run is cold or warm
- measured render and interaction timings
- feature gaps or visual regressions observed during the run

Each workload must be exercised in the same sequence:

1. load graph
2. first fit / initial render
3. pan interaction
4. zoom interaction
5. selection of a node with incident-edge highlighting
6. re-layout when applicable

The benchmark report must include both the measured timings and the qualitative
notes about behavior regressions. Pure throughput without behavior fidelity is
not sufficient.

### 8. Exit criteria are explicit

E7 ends with one of three outcomes only:

1. **Stay on Cytoscape canvas**
   - Chosen when the current renderer is already good enough for the target
     workload bands and no migration is justified.

2. **Adopt Cytoscape WebGL selectively**
   - Chosen when Cytoscape WebGL materially improves interaction quality for
     large visible graphs while preserving required interaction semantics and
     enough styling fidelity.
   - This path should prefer an opt-in threshold such as large-view activation,
     not a blanket switch, unless evidence supports a full replacement.

3. **Escalate to Sigma.js proof of concept**
   - Chosen only when the Cytoscape family fails the scale target or breaks
     required user behavior for large graphs.
   - A Sigma path must be justified not only by FPS, but also by a migration
     plan for data model, styling, interaction glue, and test coverage.

No fourth outcome exists. E7 must reduce uncertainty enough to choose one of
these branches.

## Consequences

### Positive

- We stop treating Sigma.js as the default future answer without evidence.
- We preserve the current architecture unless measured data proves it is not
  enough.
- We compare renderer options using real CogniCode views, not generic network
  demos.
- We keep Rust effort focused on places where Rust is strongest: backend graph
  analysis, layout support, and preprocessing.

### Negative

- E7 adds benchmark work before feature work.
- Cytoscape WebGL preview may expose feature gaps or instability.
- If Sigma.js is needed, the delayed migration decision still becomes a real
  cost later.

### Decision guardrails

- The benchmark must not optimize only for force-directed "hairball" graphs if
  the product also depends on hierarchical and architecture views.
- The benchmark must not treat a renderer as successful if node rendering is
  fast but edge, label, or selection behavior regresses below product needs.
- The benchmark must not let a synthetic demo outweigh a worse result on real
  Explorer workloads.

### Neutral

- This ADR does not change the current renderer. It only changes how the next
  renderer decision is made.

## Alternatives considered

### A. Migrate directly to Sigma.js now

Rejected for now.

Why:

- It assumes the answer before benchmarking.
- It forces a graph-model and adapter migration too early.
- It spends product energy on renderer replacement before proving Cytoscape's
  current family is insufficient.

### B. Keep the old roadmap wording: "WebGL / Sigma.js" as a single path

Rejected.

Why:

- It collapses two very different decisions into one label.
- It hides the new Cytoscape WebGL option.
- It makes the roadmap sound like Sigma.js is the only serious future branch.

### C. Rewrite the graph renderer in Rust/WASM

Rejected for E7.

Why:

- It creates the highest engineering cost.
- It provides the weakest short-term leverage for a React Explorer.
- It shifts effort away from product behavior into infrastructure invention.

## References

- `docs/adr/ADR-039-explorer-navigation-model.md`
- `docs/explorer-roadmap.md`
- `docs/explorer-graph/visualization-stack.md`
- `apps/explorer-ui/src/components/InteractiveGraph/InteractiveGraph.tsx`
- `apps/explorer-ui/src/components/InteractiveGraph/layout.worker.ts`
