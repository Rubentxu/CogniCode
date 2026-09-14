# Delta for program-analysis-core

> New capability; no existing main spec. Umbrella owning CFG, DFG, slicing, per-function dominators, interprocedural summaries, and taint v1 over tree-sitter ASTs. Conformance fixtures, replay determinism, and perf envelope live in `program-analysis-conformance`. Replay contract inherits the evidence kernel (M1) and the determinism pin pattern of M2/M3/M4 (identity digest, golden fixtures, byte-stable serialization).

## ADDED Requirements

### Requirement: Per-function control-flow graph

The analyzer MUST produce a control-flow graph (CFG) for each function-shaped scope identified by the existing parser. The CFG MUST be representable as a deterministic graph: stable node ids, sorted outgoing edges, and a stable entry node. Output MUST round-trip through serialization without losing structure.

#### Scenario: Deterministic CFG for a function

- GIVEN a function source with branching and looping
- WHEN the analyzer extracts its CFG
- THEN every node carries a stable id, outgoing edges are sorted by target id, and a unique entry node is declared

#### Scenario: Empty function yields a single-node CFG

- GIVEN a function with no statements
- WHEN the analyzer extracts its CFG
- THEN the graph contains exactly one entry node and zero edges

### Requirement: Per-function data-flow graph

The analyzer MUST produce a data-flow graph (DFG) for each function-shaped scope, where nodes represent definitions and uses of values and edges represent definition-to-use relations within the same function. The DFG MUST be deterministic and MUST be computable from the CFG without re-parsing source.

#### Scenario: DFG derived from CFG without re-parse

- GIVEN a previously computed CFG and its source
- WHEN the analyzer computes the DFG
- THEN it consumes the CFG as input and emits a DFG without re-walking the tree-sitter AST

#### Scenario: DFG edge represents a definition reaching a use

- GIVEN an assignment followed by a use of the assigned variable
- WHEN the DFG is computed
- THEN exactly one edge connects the definition node to the use node

### Requirement: Forward and backward slicing

The analyzer MUST support both forward slicing (statements affected by a slicing criterion) and backward slicing (statements affecting a slicing criterion) over the CFG. Slicing criteria MUST be expressible as a variable-definition site. Output MUST list statements as a stable, sorted set.

#### Scenario: Backward slice of a tainted variable use

- GIVEN a variable introduced at one site and used at another, with no intervening reassignment
- WHEN a backward slice is requested from the use site
- THEN the result includes the introduction site and excludes unrelated statements

#### Scenario: Forward slice stops at reassignment

- GIVEN a variable defined, used, reassigned, and used again
- WHEN a forward slice is requested from the first definition
- THEN the result includes only the first use and excludes post-reassignment uses

### Requirement: Per-function dominators

The analyzer MUST compute dominators per function over its CFG. The result MUST list, for each node, its immediate dominator. Computation MUST be bounded by declared resource limits.

#### Scenario: Entry node dominates every other node

- GIVEN any function CFG with more than one node
- WHEN dominators are computed
- THEN every non-entry node has the entry node on its dominator chain

#### Scenario: Resource limit terminates dominator computation

- GIVEN a PlanLimits value capping nodes below the function size
- WHEN dominators are requested
- THEN the analyzer returns an outcome naming the limit, not a partial result

### Requirement: Interprocedural summaries

The analyzer MUST derive function summaries from per-function analysis and MUST join them at call sites to produce an interprocedural result. A summary MUST record inputs read, outputs written, and calls invoked. Joining two summaries MUST be deterministic given identical inputs.

#### Scenario: Caller summary references callee summary deterministically

- GIVEN two functions A and B where A calls B
- WHEN the interprocedural analyzer computes summaries
- THEN A's summary lists B by its stable summary id and joining yields the same result across runs

#### Scenario: Recursive call site does not expand unboundedly

- GIVEN a recursive function
- WHEN summaries are computed
- THEN recursion is recorded as a fixed point marker, not expanded to infinite depth

### Requirement: Taint v1 with declared sources and sinks

The analyzer MUST propagate a taint flag forward along the DFG from declared source patterns to declared sink patterns. Sources and sinks are declared per language as identifier-or-call patterns. A taint path MUST be emitted for each sink reached from a source without an intervening untaint operation. Taint v1 MUST classify each path by provenance tier: Extracted when type information comes from M4 S2 LSP, Inferred when derived from local resolver, Ambiguous when derived only from tree-sitter heuristics.

#### Scenario: Source-to-sink taint path is emitted

- GIVEN a variable assigned from a declared source pattern and passed to a declared sink pattern
- WHEN taint analysis runs
- THEN exactly one taint path is emitted, naming source, sink, and intermediate nodes

#### Scenario: Untainting operation breaks the path

- GIVEN a tainted variable passed through a declared untaint pattern
- WHEN taint analysis runs
- THEN no taint path is emitted across the untaint boundary

#### Scenario: Taint path declares its provenance tier

- GIVEN a taint path
- WHEN it is emitted
- THEN the path carries a tier label matching the provenance of its source identification (Extracted / Inferred / Ambiguous)

### Requirement: Replay-deterministic outputs

Every analyzer output (CFG, DFG, slice, dominator set, summary, taint path) MUST be reproducible byte-identical given the same snapshot id, the same algorithm id, the same parameters, and the same PlanLimits. A canonical digest MUST be computed and pinned per algorithm kind for the canonical fixtures. Changing a pin requires an explicit re-pin commit.

#### Scenario: Same inputs produce byte-identical CFG

- GIVEN a snapshot, algorithm id, and PlanLimits
- WHEN the CFG is computed twice in the same environment
- THEN both runs serialize to byte-identical artifacts and yield the same digest

#### Scenario: Intentional behavior change requires a re-pin commit

- GIVEN a pin digest for an algorithm
- WHEN a behavior change intentionally alters the output
- THEN the test harness fails until a new pin is committed

### Requirement: Bounded resource envelope

Every analyzer operation MUST accept a PlanLimits value bounding node count, edge count, wall-clock time, and memory. Exceeding any bound MUST yield an outcome naming the exceeded limit and the operation MUST NOT produce a partial result.

#### Scenario: Wall-clock bound terminates the operation

- GIVEN a PlanLimits wall-clock value that elapses during analysis
- WHEN the operation runs
- THEN it returns an outcome naming the wall-clock limit and emits no facts

#### Scenario: Node-count bound terminates dominator computation

- GIVEN a function whose CFG node count exceeds the configured cap
- WHEN dominators are requested
- THEN the analyzer returns an outcome naming the node-count limit and emits no dominator facts

### Requirement: Published performance envelope

Every analyzer algorithm MUST be benchmarked under the existing graph benchmark harness, and results MUST be published in `sandbox/results/lsi-program-analysis-baseline/` and recorded in `perf-budget.toml` as a per-algorithm budget. A change that regresses a benchmark beyond its published budget MUST fail the perf gate.

#### Scenario: Benchmark baseline artifact is captured

- GIVEN a fresh checkout with no committed program-analysis baseline
- WHEN the benchmark harness runs the program-analysis algorithms
- THEN a baseline artifact is written with per-algorithm timings and environment metadata

#### Scenario: Regression beyond budget fails the gate

- GIVEN a committed perf budget for an algorithm
- WHEN a code change causes a regression beyond the budget
- THEN the perf gate fails with a per-algorithm delta report