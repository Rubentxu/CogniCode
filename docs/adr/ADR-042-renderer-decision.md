# ADR-042: Renderer decision outcome (placeholder)

**Status:** Proposed
**Date:** June 21, 2026
**Source:** E7 bench artifacts at `apps/explorer-ui/artifacts/e7-renderer-bench/`.

## Context

ADR-041 reserved the next renderer decision behind a reproducible
benchmark harness. The harness is now in place. E7 ends in one of
three explicit outcomes (ADR-041 §8):

1. Stay on Cytoscape canvas
2. Adopt Cytoscape WebGL selectively
3. Escalate to Sigma.js proof of concept

This ADR is the placeholder for the decision the team makes once a
real-browser run of the bench harness has been reviewed.

## Status

`Proposed`. The current `Proposed` state holds until a real-browser
bench run completes and the team reviews:

- `apps/explorer-ui/artifacts/e7-renderer-bench/results.json`
- `apps/explorer-ui/artifacts/e7-renderer-bench/report.md`

## Decision criteria

The decision will be informed by:

- Per-fixture timings (cold and warm) for `cytoscape-canvas`,
  `cytoscape-webgl`, and (if escalated) `sigma-poc`.
- Behavior flags in `MetricsRecord.behavior` for every run.
- Qualitative observations in `MetricsRecord.notes` and the
  Markdown regressions section.

## Out of scope

- Choosing a Rust-native renderer (see ADR-041 §5).
- Visual clustering, C4 backend inference, or new Explorer views.

## References

- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md`
- `apps/explorer-ui/src/bench/`
- `apps/explorer-ui/artifacts/e7-renderer-bench/`