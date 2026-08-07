# corroboration-scoring Specification (NEW)

## Purpose

A pure, deterministic scoring function that quantifies how strongly a claim (a multimodal edge target) is **corroborated** by independent sources. The function reads `GraphEdge` aggregates (each carries `provenance` and `confidence ∈ [0.0, 1.0]`) and emits a `f64` score in `0.0..=1.0`. Scores are exposed as edge metadata in the rationale sub-graph response (`SubgraphResponse.corroboration_scores[edge_id]`). The function is invoked on-the-fly by the rationale builder — no materialization, no caching. This is the first half of the "how well is this claim backed?" answer; the second half (visualization) lives in `corroboration-styling`.

## Domain

| Term | Definition |
|------|------------|
| Provenance weight | Constant multiplier per `Provenance` variant: `Manual = 1.0`, `Extracted = 0.9`, `Inferred = 0.5`, `Tested = 0.85`. |
| Source | A unique `(provenance, target_node_id)` pair contributing to a target. A single edge contributes 1 source; multiple edges to the same target from the same provenance collapse. |
| Independent source | Two sources are independent if they differ in `provenance` OR in the canonical origin id (e.g., two `Doc` references with different `source_path`). |
| Score range | `0.0..=1.0`, clamped via `min(1.0, Σ(provenance_weight × confidence) / normalizer)`. |
| Edge score | Score for an individual `GraphEdge` based on a single source: `min(1.0, provenance_weight × confidence)`. |
| Target score | Aggregate score for a target node: `min(1.0, Σ(provenance_weight × confidence) for each independent source)`. |

## Files

| File | Change |
|------|--------|
| `crates/cognicode-core/src/domain/services/corroboration.rs` | New module: `provenance_weight`, `edge_score`, `target_score`, `score_subgraph` free functions + unit tests |
| `crates/cognicode-explorer/src/service.rs` | `build_rationale_graph()` calls `score_subgraph()` and populates `corroboration_scores` |
| `crates/cognicode-explorer/src/dto.rs` | No structural change — `corroboration_scores: HashMap<String, f64>` already added in `rationale-traversal` |

## Requirements

### Requirement: `provenance_weight` constant mapping

`fn provenance_weight(p: &Provenance) -> f64` MUST exist at `crates/cognicode-core/src/domain/services/corroboration.rs`. The function MUST be pure, total, and return:

| Input | Output |
|-------|--------|
| `Provenance::Manual` | `1.0` |
| `Provenance::Extracted` | `0.9` |
| `Provenance::Tested` | `0.85` |
| `Provenance::Inferred` | `0.5` |

Adding a new `Provenance` variant in the future MUST result in a compile error in this function (exhaustive match, no wildcard), forcing the team to assign a weight explicitly.

#### Scenario: Every variant returns the documented weight

- GIVEN each of the 4 `Provenance` variants
- WHEN `provenance_weight(&v)` is called
- THEN the result equals the table above

#### Scenario: Adding a new Provenance variant is a compile error

- GIVEN a hypothetical 5th `Provenance::Generated` variant
- WHEN `corroboration.rs` is compiled
- THEN the build fails with a non-exhaustive-match error pointing at `provenance_weight`

### Requirement: `edge_score` is a clamped product

`fn edge_score(edge: &GraphEdge) -> f64` MUST return `min(1.0, provenance_weight(&edge.provenance) * edge.confidence)`. The function MUST clamp both inputs: `confidence` is rejected at construction time when out of range (existing `GraphEdge` invariant), and the product is clamped to `1.0`. A `confidence == 0.0` MUST yield `0.0` regardless of weight.

#### Scenario: Single-source score is weight × confidence clamped

- GIVEN an edge with `provenance = Manual`, `confidence = 0.7`
- WHEN `edge_score(&e)` runs
- THEN the result equals `0.7`

#### Scenario: Score is clamped at 1.0

- GIVEN an edge with `provenance = Manual`, `confidence = 1.5` (constructor would reject; use a mock that bypasses)
- WHEN `edge_score(&e)` runs
- THEN the result equals `1.0`

#### Scenario: Zero confidence yields zero

- GIVEN an edge with `confidence = 0.0`
- WHEN `edge_score(&e)` runs
- THEN the result equals `0.0`

### Requirement: `target_score` aggregates independent sources

`fn target_score(target: &NodeId, edges: &[GraphEdge]) -> f64` MUST compute `min(1.0, Σ(score_per_source))` where each `score_per_source` is the maximum `edge_score` per `Provenance` variant (i.e., the four prov kinds act as 4 independent buckets; within a bucket, the highest-confidence edge wins). The function MUST be deterministic and pure. A target with no edges MUST return `0.0`.

#### Scenario: Two independent Manual sources approach 1.0

- GIVEN two edges `(target, Manual, 0.8)` and `(target, Manual, 0.9)`
- WHEN `target_score(target, &edges)` runs
- THEN the result is `min(1.0, 0.8 + 0.9) = 1.0` (clamped)

#### Scenario: Same provenance twice does not double-count beyond the bucket max

- GIVEN two edges `(target, Extracted, 0.5)` and `(target, Extracted, 0.95)`
- WHEN `target_score(target, &edges)` runs
- THEN the result is `min(1.0, 0.9) = 0.9` (max within the Extracted bucket, not `0.45 + 0.855`)

#### Scenario: Mixed provenance buckets accumulate

- GIVEN edges `(target, Manual, 0.5)`, `(target, Extracted, 0.6)`, `(target, Inferred, 0.7)`
- WHEN `target_score(target, &edges)` runs
- THEN the result is `min(1.0, 0.5 + 0.54 + 0.35) ≈ 0.89`

#### Scenario: No edges returns zero

- GIVEN an empty `edges` slice
- WHEN `target_score(target, &[])` runs
- THEN the result equals `0.0`

### Requirement: `score_subgraph` populates the response map

`fn score_subgraph(nodes: &[GraphNode], edges: &[GraphEdge]) -> HashMap<String, f64>` MUST return a map keyed by `GraphEdge.id` (the same id used in the rationale sub-graph response). The value MUST be the `edge_score` of that edge. Edges with `id == ""` MUST be skipped (defensive — empty ids would collide). The function MUST be `O(edges.len())`.

#### Scenario: Every edge gets a score

- GIVEN a 4-edge rationale sub-graph
- WHEN `score_subgraph(&nodes, &edges)` runs
- THEN the result has 4 entries AND keys equal `edges[i].id`

#### Scenario: Empty-edge input returns empty map

- GIVEN `edges == []`
- WHEN `score_subgraph(...)` runs
- THEN the result equals `HashMap::new()`

#### Scenario: Edges with empty id are skipped

- GIVEN one edge with `id = ""` and one with `id = "e1"`
- WHEN `score_subgraph(...)` runs
- THEN the result has 1 entry keyed by `"e1"`

### Requirement: Integration in `build_rationale_graph`

`ExplorerService::build_rationale_graph(focus, max_depth, max_nodes)` in `crates/cognicode-explorer/src/service.rs` MUST call `score_subgraph()` on the result of `repo.rationale_subgraph()` and embed the map into `SubgraphResponse.corroboration_scores` before returning. The scoring call MUST NOT mutate the `GraphNode` or `GraphEdge` aggregates (no `provenance` rewrite, no confidence smoothing). On any error from `score_subgraph` the handler MUST return `500` with body `{"error":"scoring_failed"}` — scoring is total and should not fail in practice, so the error path is defensive.

#### Scenario: Rationale response carries scores

- GIVEN a rationale sub-graph with 3 edges `(Manual, 0.9)`, `(Extracted, 0.7)`, `(Inferred, 0.5)`
- WHEN `build_rationale_graph(focus, 3, 50)` runs
- THEN the returned `SubgraphResponse.corroboration_scores` map has 3 entries AND the values are `0.9`, `0.63`, `0.25`

## TDD RED Gate

These tests MUST be written FIRST and MUST FAIL before any implementation lands:

| Test | File | Asserts |
|------|------|---------|
| `provenance_weight_returns_table_values` | `corroboration.rs` (test) | All 4 variants return their weights |
| `provenance_weight_no_wildcard_arm` | `corroboration.rs` (compile test) | `match` is exhaustive (no `_ =>` in source) |
| `edge_score_weight_times_confidence` | `corroboration.rs` | `0.7` for `(Manual, 0.7)` |
| `edge_score_clamps_at_one` | `corroboration.rs` | `1.0` for `confidence > 1.0` |
| `edge_score_zero_confidence_yields_zero` | `corroboration.rs` | `0.0` |
| `target_score_two_manual_clamps_at_one` | `corroboration.rs` | `min(1.0, 1.6) = 1.0` |
| `target_score_within_bucket_takes_max` | `corroboration.rs` | `(Extracted, 0.5)` + `(Extracted, 0.95)` → `0.9` |
| `target_score_mixed_buckets_accumulate` | `corroboration.rs` | Sum of three buckets, clamped |
| `target_score_no_edges_returns_zero` | `corroboration.rs` | `0.0` |
| `score_subgraph_keys_are_edge_ids` | `corroboration.rs` | Map keys equal `edges[i].id` |
| `score_subgraph_empty_input_empty_map` | `corroboration.rs` | `HashMap::new()` |
| `score_subgraph_skips_empty_edge_ids` | `corroboration.rs` | 1 entry, not 2 |
| `build_rationale_graph_embeds_scores` | `service.rs` (integration) | `response.corroboration_scores` populated |

## Out of Scope (locked)

- Bayesian / probabilistic combination of sources
- Time-decay weighting (older evidence scores lower)
- Cross-space / federation corroboration
- Persisting scores to PG
- Scoring the focus node itself (scores attach to edges, not to nodes; target scores are computed on demand by the UI)
- Normalization across heterogeneous corpora (e.g., "0.7 in this workspace is not the same as 0.7 in another")
