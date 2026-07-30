# ADR-015: E28.6 Advanced Analytics Evidence Gate

**Status**: Accepted
**Date**: 2026-07-30
**Deciders**: SDD kernel orchestrator

## Context

E28.6 Advanced Analytics Evidence Gate requires admission decisions for
algorithms proposed for the analytics registry. Not every algorithm belongs
in every cohort — some require more evidence of correctness, performance, or
usefulness before admission.

The evidence gate is a structured admission process that evaluates algorithms
against three outcomes:

- **ADMIT**: Sufficient evidence of correctness, performance, and utility.
- **REJECT**: Evidence shows the algorithm is wrong, unscalable, or redundant.
- **DEFER**: Evidence is promising but insufficient for admission; rescheduled
  for future cohorts pending additional work.

## Decision

The following evidence gate ledger governs admission for E28.6 and future
cohorts.

### Evidence Gate Ledger

| Algorithm | Decision | Evidence | Rationale |
|-----------|----------|----------|-----------|
| **Personalized PageRank** | ADMIT | Extends standard PageRank with a well-understood personalization parameter (teleportation distribution). Correctness proved by matching standard PageRank when personalization is uniform. Cohort 3 maturity appropriate (Experimental). | Standard PageRank is already Stable (Cohort 1). Personalized variant adds a single new parameter without changing the core algorithm. |
| **Conductance** | ADMIT | Edge-cut measure for community detection. Computable from `build_directed_adjacency()`. O(V + E) per community. No circular dependency on other algorithms. | Needed for modularity computation and community quality scoring. Core structural metric with clear semantics. |
| **Modularity** | ADMIT | Newman/Girvan modularity using conductance as edge-cut. Computable from `build_directed_adjacency()` + Conductance. O(V²) naive, optimizable. | Primary community quality metric in the field. Direct dependency on Conductance (which is also ADMIT). |
| **k-shortest paths** | REJECT | Enumerates k shortest paths. Not the same as bounded shortest paths already admitted (Cohort 1). Produces large result sets with combinatorial explosion. | The existing `bounded_shortest_paths` already covers the bounded use case. k-shortest paths without a bound is an unbounded query that violates the "no unbounded variable-length paths" rule from ADR-014 §1. |
| **Multi-source reachability** | REJECT | Computes reachability from multiple source nodes. Equivalent to running BFS from each source independently. No advantage over running multiple single-source queries. | No algorithmic advantage over composition of existing single-source reachability. Redundant with the `traverse` operation already available via `GraphQueryPort`. |
| **Betweenness centrality** | DEFER | Requires all-pairs shortest paths (O(V³) Floyd-Warshart or O(V·E) Brandes). Memory pressure for large graphs (V² storage). Not yet profiled against plan limits. | Promising metric but requires significant performance validation before admission. DEFER to E28.7 pending performance profiling and possible sampling strategies. |
| **Leiden community detection** | DEFER | Improved Louvain algorithm. Requires iterative modularity maximization with guaranteed convergence properties. Implementation complexity is higher than modularity. | Promising but more complex than modularity. DEFER to E28.7 pending evidence that it converges better in practice for CogniCode graph shapes. |
| **Node similarity (Jaccard/Adamic-Adar)** | DEFER | Pairwise node similarity scores. O(V²) pairwise computation with large result matrices. Quality metrics require threshold tuning. | Scalability concerns for large graphs. DEFER to E28.7 pending edge filtering strategies and result size limits. |

## Consequences

### Positive

- Structured, evidence-based admission prevents premature or inappropriate algorithm inclusion.
- Rejected algorithms are documented with rationale, preventing repeated debate.
- Deferred algorithms have a clear path to future admission.

### Negative

- Deferred algorithms may frustrate users expecting complete coverage.
- "DEFER" is not a permanent rejection — requires tracking in the roadmap to avoid indefinite postponement.

## Review Schedule

- **ADMIT** algorithms: Ready for implementation in E28.6 PR1 (this change).
- **DEFER** algorithms: Re-evaluate in E28.7 with concrete performance evidence.
- **REJECT** algorithms: May be reconsidered if evidence changes (e.g., new optimization techniques, changed requirements).

## References

- ADR-014: MoldQL Pattern Profile and graph analytics platform
- E28.6 program: Advanced Analytics Evidence Gate
- `GraphBuilder::build_directed_adjacency()` — new trait method enabling conductance/modularity
- `personalized_pagerank` — pure algorithm function in `cognicode-graph-algos`
