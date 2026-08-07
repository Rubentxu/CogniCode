# Delta for callgraph-petgraph-projection

> Companion to proposal `sdd/forward-reach-impact/proposal`. TDD RED gate
> is `test_find_forward_reach_direct_successor` — MUST fail to compile
> before implementation begins.

## ADDED Requirements

### Requirement: `find_forward_reach`

`pub fn find_forward_reach(&self, root: SymbolId, max_depth: usize) -> Vec<SymbolId>`
MUST return the **successors** of `root` reachable within `max_depth` forward
hops, traversing edges via `petgraph::Direction::Outgoing`. MUST exclude
`root` from the result. MUST use a `HashSet<NodeIndex>` visited-set to
guarantee termination on cycles. MUST return `vec![]` (no panic) when
`root` is missing from the projection, `max_depth == 0`, or the projection
is empty. The BFS is the symmetric counterpart of `find_impact_radius`
(mirroring it on `Direction::Outgoing`).

Direction semantics: `find_impact_radius` answers "what depends on X?"
(predecessors). `find_forward_reach` answers "what does X affect?"
(successors). Both MUST live on `CallGraphProjection` as sibling
read-only methods.

#### Scenario: Direct successor within depth 1 (RED gate)

- GIVEN graph `A → B`
- WHEN `find_forward_reach(A, 1)`
- THEN result equals `{B}` (any order)

#### Scenario: Transitive successor within depth 2

- GIVEN graph `A → B → C`, `A → D`
- WHEN `find_forward_reach(A, 1)` AND `find_forward_reach(A, 2)`
- THEN result equals `{B, D}` for depth 1 AND `{B, C, D}` for depth 2

#### Scenario: `max_depth == 0` returns empty

- GIVEN any non-empty graph `A → B`
- WHEN `find_forward_reach(A, 0)`
- THEN result is `vec![]`

#### Scenario: Missing root returns empty (no panic)

- GIVEN projection that does not contain `m`
- WHEN `find_forward_reach(m, 10)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: Cycle visited-set prevents infinite loop, root excluded

- GIVEN graph `A → B → C → A` (cycle includes root `A`)
- WHEN `find_forward_reach(A, usize::MAX)`
- THEN result equals `{B, C}` (order not asserted) AND no panic AND
  the BFS terminates in finite time AND `A` MUST NOT appear in the result

#### Scenario: Disconnected successor returns empty

- GIVEN graph `A → B` and a separate isolated node `Z`
- WHEN `find_forward_reach(Z, 5)`
- THEN result is `vec![]` (no panic, no global scan)

#### Scenario: Empty projection returns empty

- GIVEN projection built from `CallGraph::new()` (0 nodes)
- WHEN `find_forward_reach(any_id, 5)`
- THEN result is `vec![]` and no panic occurs

#### Scenario: `usize::MAX` depth sentinel returns all reachable successors

- GIVEN graph `A → B → C → D`
- WHEN `find_forward_reach(A, usize::MAX)`
- THEN result equals `{B, C, D}` and the BFS terminates

#### Scenario: Out-of-scope parameter does not break signature

- GIVEN any non-empty graph
- WHEN `find_forward_reach(A, 3)` where 3 is shallower than the longest
  reachable chain
- THEN the result is exactly the set of nodes within 3 forward hops
  from `A`, with no nodes beyond depth 3 and no duplicates

## TDD Acceptance — First Failing Test (RED gate)

The implementation MUST NOT begin until the following test fails to
compile:

```rust
// In crates/cognicode-core/src/infrastructure/graph/call_graph_projection.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_forward_reach_direct_successor() {
        // GIVEN: graph A → B
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new("A"));
        let b = g.add_symbol(Symbol::new("B"));
        g.add_dependency_with_provenance(a, b, ...);

        let p = CallGraphProjection::from_call_graph(&g);

        // WHEN: forward reach from A at depth 1
        let reach = p.find_forward_reach(a, 1);

        // THEN: result contains exactly B
        assert_eq!(reach, vec![b]);
    }
}
```

This test MUST fail to compile (`find_forward_reach` does not exist)
before the projection method is implemented. The implementation is
green only when the RED test and the 8 sibling scenarios above all pass.

## TDD Test Map — Behavior-First Order

| # | Test name | Verifies | Phase |
| - | --------- | -------- | ----- |
| 1 | `test_find_forward_reach_direct_successor` | R-direct, RED gate | red |
| 2 | `test_find_forward_reach_transitive_successor` | R-transitive | red→green |
| 3 | `test_find_forward_reach_zero_depth_returns_empty` | R-zero | red→green |
| 4 | `test_find_forward_reach_missing_root_returns_empty` | R-missing | red→green |
| 5 | `test_find_forward_reach_cycle_terminates_root_excluded` | R-cycle | red→green |
| 6 | `test_find_forward_reach_disconnected_returns_empty` | R-disconnected | red→green |
| 7 | `test_find_forward_reach_empty_projection` | R-empty | red→green |
| 8 | `test_find_forward_reach_max_usize_sentinel` | R-usize-max | red→green |
| 9 | `test_find_forward_reach_depth_boundary` | R-boundary | red→green |

> Tests 1–9 form the 7 unit tests mandated by the proposal (count: 7
> explicit scenarios above; the boundary test is added for coverage of
> the depth limit interaction with multi-fanout).
