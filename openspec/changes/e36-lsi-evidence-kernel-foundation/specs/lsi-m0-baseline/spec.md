# Delta for lsi-m0-baseline

> New capability introduced by this change; no existing main spec. Scope: M0 baseline tooling contracts only. Evidence-kernel behavior (facts, evidence, provenance, snapshots) is specified in the umbrella change `cognicode-living-software-intelligence` and is out of scope here.

## ADDED Requirements

### Requirement: Golden fixture byte-stability

Captured graph, symbol and impact outputs for the pinned multi-language fixture set MUST be serialized deterministically (stable element ordering, sorted map keys, no timestamps or environment-dependent paths) so that regenerating fixtures on unchanged code produces byte-identical artifacts. The capture harness MUST NOT silently overwrite committed fixtures when regenerated outputs differ.

#### Scenario: Regeneration is byte-identical

- GIVEN the pinned multi-language fixture set and a clean checkout
- WHEN the capture harness runs twice in the same environment
- THEN every generated fixture file is byte-identical to the committed fixture

#### Scenario: Kernel addition does not change consumer outputs

- GIVEN committed fixtures and the M1 evidence-kernel modules enabled
- WHEN the capture harness runs
- THEN every regenerated output is byte-identical to the committed fixtures

#### Scenario: Intended behavior change requires explicit re-baseline

- GIVEN a change that intentionally alters graph, symbol or impact output
- WHEN the capture harness detects a fixture diff
- THEN the run fails unless the diff is accepted into an explicitly committed new fixture baseline

### Requirement: Baseline coverage of critical surfaces

The fixture set SHOULD cover every critical consumer surface recorded in the M0 consumer inventory (MCP, CLI, Explorer graph/symbol/impact outputs), and the capture harness MUST report per-surface coverage.

#### Scenario: Inventory surfaces report coverage

- GIVEN the consumer inventory with N critical surfaces
- WHEN the capture harness reports coverage
- THEN each surface is listed as covered or uncovered with the fixture paths that exercise it

### Requirement: Benchmark baseline artifact

Before any M1 kernel code lands, the benchmark harness MUST produce a machine-readable baseline artifact recording per-benchmark timings and the execution environment. Kernel changes MUST be compared against this baseline, and the harness MUST report per-benchmark deltas.

#### Scenario: Baseline artifact is captured

- GIVEN the benchmark harness and no committed baseline artifact
- WHEN the baseline run completes
- THEN a machine-readable baseline artifact is written with per-benchmark results and environment metadata

#### Scenario: Kernel change is compared to baseline

- GIVEN a committed baseline artifact
- WHEN the benchmark harness runs after a kernel change
- THEN it emits a per-benchmark delta report against the baseline
