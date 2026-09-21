# Delta for program-analysis-conformance

> New capability; no existing main spec. Conformance fixtures, replay determinism gates, and perf envelope assertions for M5. Algorithm semantics live in `program-analysis-core`; this delta owns the fixture format, the digest-pin protocol, and the perf-budget wiring.

## ADDED Requirements

### Requirement: Canonical fixture corpus per language

The conformance suite MUST cover each algorithm kind (CFG, DFG, slicing, dominators, summaries, taint v1) with at least one canonical fixture per language in the existing provider fixture set (Rust, TypeScript, Java). Each fixture MUST declare its inputs (source, snapshot id, parameters) and expected output digest.

#### Scenario: Each algorithm has a canonical Rust fixture

- GIVEN the conformance suite
- WHEN it runs against the Rust fixture corpus
- THEN each algorithm kind has at least one fixture with a committed expected digest

#### Scenario: Java fixtures gate on jdtls availability

- GIVEN the conformance suite
- WHEN it runs against the Java fixture corpus
- THEN each algorithm kind has at least one fixture that runs serverless and one that declares an LSP dependency; the LSP-dependent fixture is skipped when jdtls is absent and passes when present

### Requirement: Digest pin per algorithm kind

Each algorithm kind MUST have a pinned canonical digest computed over the canonical fixtures. The pin is committed in the conformance harness and treated as a deterministic-replay contract: a regression that changes a digest fails the run unless the pin is explicitly re-pinned in a dedicated commit.

#### Scenario: First capture pins the digest

- GIVEN the harness with no committed pin for an algorithm
- WHEN the first capture completes
- THEN the harness emits a pin file with the captured digest and the run passes

#### Scenario: Silent drift fails the harness

- GIVEN a committed pin
- WHEN a code change alters the output digest without re-pinning
- THEN the harness fails with the algorithm kind, observed digest, and pinned digest

### Requirement: Replay determinism guard

The conformance suite MUST verify that running the same algorithm twice on the same snapshot yields byte-identical artifacts. The guard MUST run in CI and MUST NOT be skippable.

#### Scenario: Two runs produce byte-identical artifacts

- GIVEN a snapshot and an algorithm id
- WHEN the harness executes the algorithm twice in the same environment
- THEN the second artifact is byte-identical to the first and the digest matches the pin

#### Scenario: Replay guard fails on environment-dependent timing

- GIVEN an algorithm whose output embeds a timestamp
- WHEN the replay guard runs
- THEN the second artifact differs from the first and the guard fails with the algorithm kind

### Requirement: Tier-declared conformance for taint paths

Taint v1 conformance MUST declare the expected provenance tier per fixture (Extracted, Inferred, Ambiguous). A run MUST verify each emitted taint path's tier against its declaration, failing on contradiction.

#### Scenario: Contradicting tier fails the run

- GIVEN a fixture declaring Ambiguous for a taint path
- WHEN the run observes an Extracted or Inferred tier
- THEN the run fails, naming fixture, declared tier, and observed tier

#### Scenario: Ambiguous fixtures pass serverless

- GIVEN no live language servers
- WHEN the Ambiguous-declared fixtures run
- THEN they pass without server dependency

### Requirement: Perf envelope published and gated

The conformance suite MUST publish a baseline artifact per algorithm in `sandbox/results/lsi-program-analysis-baseline/` and MUST wire each algorithm into `perf-budget.toml`. A regression beyond the published budget MUST fail the perf gate.

#### Scenario: Baseline artifact is published

- GIVEN the benchmark harness
- WHEN the program-analysis baseline run completes
- THEN a baseline artifact exists in `sandbox/results/lsi-program-analysis-baseline/` with per-algorithm timings

#### Scenario: Regression beyond budget fails the gate

- GIVEN a committed budget for an algorithm
- WHEN a code change causes a regression beyond the budget
- THEN the perf gate fails with a per-algorithm delta report

### Requirement: WASM-clean algorithms

Algorithms exposed via the WASM build (browser target) MUST NOT depend on filesystem, network, or process I/O. Each WASM-exposed algorithm MUST compile under `wasm32-unknown-unknown` without feature flags that pull in I/O.

#### Scenario: WASM build succeeds for exposed algorithms

- GIVEN the WASM build target
- WHEN the program-analysis crate is compiled for `wasm32-unknown-unknown`
- THEN the build succeeds with no missing imports and no I/O symbols are referenced

#### Scenario: Non-WASM algorithms are feature-gated

- GIVEN an algorithm that requires I/O
- WHEN the conformance suite runs
- THEN the algorithm is excluded from the WASM build behind a feature flag and documented as such