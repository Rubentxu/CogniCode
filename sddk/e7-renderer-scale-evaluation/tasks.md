# Kernel Tasks: E7 — Explorer Renderer Scale Evaluation

## Scope

These tasks implement the E7 benchmark harness proposed in
`proposal.md`, scoped by `spec.md`, and structured per `design.md`.

Each task is sized to land as a single PR. Tasks land in numeric order.
Each task ends with tests passing and lint clean.

## Atomic commits

Per the git contract, every commit is a conventional commit and includes
the relevant unit tests and docs.

## Work Units

### T1 — Benchmark subtree skeleton

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `chore(explorer-ui): scaffold bench subtree with fixture and metrics schemas`

**Files:**
- `apps/explorer-ui/src/bench/fixture-schema.ts`
- `apps/explorer-ui/src/bench/metrics.ts`
- `apps/explorer-ui/src/bench/fixtures/.gitkeep`

**Steps:**
1. Create `apps/explorer-ui/src/bench/` directory.
2. Implement `Fixture` schema and a runtime `assertFixture` helper.
3. Implement `MetricsRecord` schema and a `makeMetricsRecord` builder.
4. Add a Vitest test that round-trips a small fixture through
   `assertFixture`.

**Done when:** `pnpm --filter explorer-ui test` passes and the new files
type-check.

### T2 — RendererAdapter contract

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add renderer adapter contract for bench harness`

**Files:**
- `apps/explorer-ui/src/bench/renderers/types.ts`

**Steps:**
1. Define `RendererAdapter` and `RendererController` interfaces.
2. Define `BenchConfig` interface.
3. Export from `apps/explorer-ui/src/bench/index.ts`.
4. Add a Vitest test that asserts the type contract via a fake adapter.

**Done when:** contract compiles and the test passes.

### T3 — Minimal fixture set

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `test(explorer-ui): commit stable bench fixtures across sizes and shapes`

**Files:**
- `apps/explorer-ui/src/bench/fixtures/call-graph-small.json`
- `apps/explorer-ui/src/bench/fixtures/call-graph-medium.json`
- `apps/explorer-ui/src/bench/fixtures/call-graph-large.json`
- `apps/explorer-ui/src/bench/fixtures/dependency-graph-small.json`
- `apps/explorer-ui/src/bench/fixtures/dependency-graph-medium.json`
- `apps/explorer-ui/src/bench/fixtures/architecture-c4-medium.json`
- `apps/explorer-ui/src/bench/fixtures/landing-overview-medium.json`
- `apps/explorer-ui/src/bench/fixtures/index.ts`

**Steps:**
1. Capture or hand-craft each fixture so it satisfies the `Fixture` schema.
2. Implement `loadFixture(id)` and `loadAllFixtures()` helpers.
3. Add a Vitest test that validates every fixture against `assertFixture`.

**Done when:** all fixtures load and validate.

### T4 — Cytoscape canvas adapter

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add cytoscape canvas renderer adapter for bench`

**Files:**
- `apps/explorer-ui/src/bench/renderers/cytoscape-canvas.ts`
- `apps/explorer-ui/src/bench/renderers/cytoscape-canvas.test.ts`

**Steps:**
1. Implement `CytoscapeCanvasAdapter` reusing
   `toCytoscapeElements` and `buildStylesheet`.
2. Add a Vitest smoke test that mounts a small fixture and asserts
   `node_count` and `edge_count`.

**Done when:** the adapter passes the smoke test.

### T5 — Cytoscape WebGL adapter

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add cytoscape webgl renderer adapter for bench`

**Files:**
- `apps/explorer-ui/src/bench/renderers/cytoscape-webgl.ts`
- `apps/explorer-ui/src/bench/renderers/cytoscape-webgl.test.ts`

**Steps:**
1. Implement `CytoscapeWebglAdapter` mirroring the canvas adapter and
   enabling the preview WebGL renderer.
2. Add a Vitest smoke test that mounts a small fixture.
3. Document the pinned cytoscape version in the adapter's `version` field.

**Done when:** the adapter passes the smoke test.

### T6 — Sigma proof-of-concept adapter

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add sigma proof-of-concept adapter gated by config`

**Files:**
- `apps/explorer-ui/src/bench/renderers/sigma-poc.ts`
- `apps/explorer-ui/src/bench/renderers/sigma-poc.test.ts`

**Steps:**
1. Implement `SigmaPocAdapter` using `graphology` and `sigma`.
2. Gate `isEnabled` on `BenchConfig.enable_sigma`.
3. Add a Vitest test that asserts `isEnabled({ enable_sigma: false })`
   returns `false`.

**Done when:** the gating test passes.

### T7 — Scenario runner

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add scenario runner with cold and warm run support`

**Files:**
- `apps/explorer-ui/src/bench/runner.ts`
- `apps/explorer-ui/src/bench/runner.test.ts`

**Steps:**
1. Implement `runBench(config)` and `runOnce(...)`.
2. Cover load, fit, pan, zoom, select, relayout.
3. Implement behavior checks (`selection_works`, `edge_highlight_works`,
   `layout_completed`).
4. Add a Vitest test that runs the runner with a fake adapter and
   asserts metrics shape.

**Done when:** the runner test passes.

### T8 — Report writer

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `feat(explorer-ui): add bench report writer for json and markdown`

**Files:**
- `apps/explorer-ui/src/bench/report.ts`
- `apps/explorer-ui/src/bench/report.test.ts`

**Steps:**
1. Implement JSON serialization of `MetricsRecord[]`.
2. Implement Markdown grouping by `renderer.id`, with timings and a
   regressions section.
3. Add a Vitest test using an in-memory output directory.

**Done when:** the report test passes.

### T9 — CLI entrypoint and package script

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `chore(explorer-ui): wire bench:renderer script and CLI entrypoint`

**Files:**
- `apps/explorer-ui/src/bench/cli.ts`
- `apps/explorer-ui/package.json`

**Steps:**
1. Implement `cli.ts` that parses `BENCH_ENABLE_SIGMA` and writes
   artifacts to the configured output directory.
2. Add `bench:renderer` script in `package.json`.
3. Run the script locally to confirm artifacts are produced.

**Done when:** `pnpm --filter explorer-ui bench:renderer` exits 0 and
artifacts exist on disk.

### T10 — End-to-end bench pass

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `chore(explorer-ui): run bench harness on cytoscape canvas and webgl`

**Steps:**
1. Run `bench:renderer` on the canvas adapter only (Sigma gated off).
2. Inspect `results.json` and `report.md`.
3. Run `bench:renderer` with `BENCH_ENABLE_SIGMA=1` to validate the gating.
4. Commit the artifacts under `artifacts/e7-renderer-bench/` as a snapshot.

**Done when:** both report files exist and the gating behaves as designed.

### T11 — Update ADR-041 status to Accepted

**Branch:** `feat/e7-renderer-scale-evaluation`
**Commit:** `docs: mark ADR-041 as Accepted once harness exists`

**Steps:**
1. Update `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md` to
   reflect that the benchmark harness is now in place.
2. Add a "Status: Accepted" line and a short rationale.

**Done when:** ADR-041 reflects the implemented evaluation method.

### T12 — Open follow-up ADR for the renderer decision

**Branch:** `feat/e7-renderer-scale-evaluation` (or follow-up branch)
**Commit:** `docs: add ADR-042 placeholder for renderer decision outcome`

**Steps:**
1. Add `docs/adr/ADR-042-renderer-decision.md` as a placeholder.
2. Reference the bench report path inside the new ADR.
3. Mark the ADR as `Proposed` until the report is reviewed.

**Done when:** ADR-042 exists and points at the report artifacts.

## Verification

After T9, the harness exists. After T10, there is at least one real report.
After T11 and T12, the architectural story is closed with a follow-up ADR
that selects one of the three branches from ADR-041 §8.

## References

- `sddk/e7-renderer-scale-evaluation/proposal.md`
- `sddk/e7-renderer-scale-evaluation/spec.md`
- `sddk/e7-renderer-scale-evaluation/design.md`
- `docs/adr/ADR-039-explorer-navigation-model.md`
- `docs/adr/ADR-041-explorer-renderer-scale-evaluation.md`
- `docs/explorer-roadmap.md`