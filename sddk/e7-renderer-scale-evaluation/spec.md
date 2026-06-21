# Kernel Spec: E7 — Explorer Renderer Scale Evaluation

## Purpose

Define the **behavior** of the E7 benchmark harness. Specs use Given/When/Then
scenarios. This document describes what the system does, not how it is
implemented. The design document covers implementation.

## Domain Model

### Fixture
A stable graph payload used by the benchmark.

| Field | Type | Description |
|-------|------|-------------|
| `fixture_id` | string | Stable identifier, e.g. `call-graph-medium` |
| `kind` | string | One of `call_graph`, `dependency_graph`, `architecture_c4`, `landing_overview` |
| `size_band` | string | One of `small`, `medium`, `large` |
| `node_count` | integer | Number of nodes |
| `edge_count` | integer | Number of edges |
| `nodes` | array | Graph nodes with `id`, `label`, `kind`, `style_class` |
| `edges` | array | Graph edges with `id`, `source`, `target`, `relation`, `style_class` |

### Renderer Adapter
A function that mounts a fixture into a specific renderer implementation and
yields a controller with timing hooks.

### Metrics Record
A JSON object produced per run.

| Field | Type | Description |
|-------|------|-------------|
| `schema_version` | string | `"e7.0"` |
| `runner` | object | `{ browser, browser_version, os, machine_profile }` |
| `fixture` | object | `{ fixture_id, kind, size_band, node_count, edge_count }` |
| `renderer` | object | `{ id, version, config }` |
| `run` | object | `{ mode: cold \| warm, index }` |
| `timings_ms` | object | `{ load, first_render, fit, pan, zoom, select, relayout }` |
| `behavior` | object | `{ selection_works, edge_highlight_works, layout_completed, regressions: [] }` |
| `notes` | string | Free-form observations |

### Scenario
A deterministic sequence of operations.

1. load fixture
2. mount renderer
3. measure first render
4. measure fit
5. pan
6. zoom
7. select a node
8. verify incident edge highlight
9. relayout when applicable
10. teardown

## Requirements

### REQ-1: The benchmark subtree exists
The system must expose a benchmark subtree under
`apps/explorer-ui/src/bench/` that compiles, type-checks, and lints.

**Acceptance:** `pnpm --filter explorer-ui lint` and `pnpm --filter
explorer-ui typecheck` both succeed with the new files present.

### REQ-2: Fixture schema is first-class
The system must define a `Fixture` schema that is exported from
`apps/explorer-ui/src/bench/fixture-schema.ts` and is the canonical shape used
by every renderer adapter.

**Acceptance:** importing `Fixture` from the schema module is the only
supported way to consume fixture data in adapters.

### REQ-3: A minimal fixture set exists
The system must ship fixtures for at least these shapes and bands:

| Fixture id | Kind | Size band | Approx nodes |
|------------|------|-----------|--------------|
| `call-graph-small` | call_graph | small | ~100 |
| `call-graph-medium` | call_graph | medium | ~1,000 |
| `call-graph-large` | call_graph | large | ~5,000 |
| `dependency-graph-small` | dependency_graph | small | ~100 |
| `dependency-graph-medium` | dependency_graph | medium | ~1,000 |
| `architecture-c4-medium` | architecture_c4 | medium | ~500 |
| `landing-overview-medium` | landing_overview | medium | ~500 |

**Acceptance:** every fixture validates against `Fixture` and lives under
`apps/explorer-ui/src/bench/fixtures/`.

### REQ-4: Metrics schema is first-class
The system must define a `MetricsRecord` schema exported from
`apps/explorer-ui/src/bench/metrics.ts`.

**Acceptance:** every adapter returns a `MetricsRecord` and the report writer
consumes only that shape.

### REQ-5: Scenario runner is deterministic
The benchmark runner must execute the fixed scenario sequence
(load → fit → pan → zoom → select → relayout) over a fixture and emit a
`MetricsRecord`.

**Acceptance:** running the same fixture twice with the same renderer in the
same browser produces records with timings within a documented tolerance.

### REQ-6: Cytoscape canvas adapter exists
The harness must include `renderers/cytoscape-canvas.ts` that mirrors the
production `InteractiveGraph` data path.

**Acceptance:** mounting the `cytoscape-canvas` adapter on the
`call-graph-medium` fixture produces the same node and edge count as the
fixture.

### REQ-7: Cytoscape WebGL adapter exists
The harness must include `renderers/cytoscape-webgl.ts` that mounts the same
data through Cytoscape's preview WebGL renderer mode.

**Acceptance:** the WebGL adapter consumes the same `Fixture` shape and
returns a `MetricsRecord`. The renderer config is the only difference from the
canvas adapter.

### REQ-8: Sigma adapter is gated
The harness must include `renderers/sigma-poc.ts`, but it must not run by
default. Activation requires the `BENCH_ENABLE_SIGMA=1` env var or an
explicit flag passed to the runner.

**Acceptance:** running `bench:renderer` without the env var never invokes the
Sigma adapter.

### REQ-9: Cold and warm runs are recorded separately
For each fixture and renderer combination, the harness must record at least
one cold run and one warm run.

**Acceptance:** every metrics record carries a `run.mode` of either `cold` or
`warm`.

### REQ-10: Behavior checks are required, not optional
A run is invalid unless selection works, edge highlight works, and the layout
completes. These checks live in `MetricsRecord.behavior`.

**Acceptance:** the report writer flags any run with `behavior.selection_works
= false`, `behavior.edge_highlight_works = false`, or
`behavior.layout_completed = false`.

### REQ-11: JSON and Markdown report artifacts
The harness must produce two artifacts per run:

1. `artifacts/e7-renderer-bench/results.json` — one record per run
2. `artifacts/e7-renderer-bench/report.md` — a comparative summary

**Acceptance:** running the harness on `cytoscape-canvas` and
`cytoscape-webgl` produces both files with consistent fixture and renderer
ids.

### REQ-12: The bench script is wired
`apps/explorer-ui/package.json` must expose a `bench:renderer` script that
invokes the harness headlessly.

**Acceptance:** `pnpm --filter explorer-ui bench:renderer` exits cleanly and
writes both artifacts.

## Scenarios (Given/When/Then)

### SCN-1: Adapter mounts the same fixture shape

**Given** a `Fixture` with `kind = call_graph` and `node_count = 1000`
**When** the `cytoscape-canvas` adapter mounts it
**Then** the resulting cytoscape instance contains 1000 nodes
**And** the resulting cytoscape instance contains the same number of edges as
the fixture's `edge_count`.

### SCN-2: WebGL adapter differs from canvas only in renderer config

**Given** the same `Fixture`
**When** the runner mounts it with `cytoscape-canvas` and `cytoscape-webgl`
**Then** both adapters report the same `node_count` and `edge_count`
**And** the renderer config reported in each metrics record differs only in
the renderer block.

### SCN-3: Sigma adapter stays inert by default

**Given** the `BENCH_ENABLE_SIGMA` env var is not set
**When** the runner executes the harness
**Then** no run is recorded for the `sigma-poc` renderer
**And** the report does not include a Sigma section.

### SCN-4: Sigma adapter runs only with explicit activation

**Given** the `BENCH_ENABLE_SIGMA` env var is set
**When** the runner executes the harness
**Then** at least one cold run and one warm run are recorded for the
`sigma-poc` renderer.

### SCN-5: Cold and warm runs are distinguishable

**Given** a single `Fixture` and a single renderer
**When** the runner records two runs
**Then** the first record has `run.mode = cold`
**And** the second record has `run.mode = warm`.

### SCN-6: Behavior checks fail the run

**Given** a fixture is mounted with a renderer that fails selection
**When** the runner records the metrics
**Then** `behavior.selection_works` is `false`
**And** the report flags this run as invalid.

### SCN-7: Report contains both quantitative and qualitative notes

**Given** a full harness run on `cytoscape-canvas`
**When** the Markdown report is generated
**Then** it includes a per-fixture timings table
**And** it includes a regression/observation section per fixture.

### SCN-8: Bench script is wired

**Given** the harness is implemented
**When** `pnpm --filter explorer-ui bench:renderer` is invoked
**Then** the script exits with status 0
**And** the two artifacts exist on disk.

## Out Of Scope (Spec)

- Numeric thresholds for "good enough" performance
- A Sigma.js migration plan
- Production changes to `InteractiveGraph`
- New Explorer views or lenses
- CI integration beyond local invocation

## References

- `docs/adr/ADR-039-explorer-navigation-model.md`
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md`
- `docs/explorer-roadmap.md`
- `sddk/e7-renderer-scale-evaluation/proposal.md`