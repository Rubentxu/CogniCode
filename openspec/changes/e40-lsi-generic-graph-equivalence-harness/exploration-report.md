# Exploration Report — e40 — GenericGraph equivalence harness

> Change: `e40-lsi-generic-graph-equivalence-harness` | Cycle: housekeeping | Date: 2026-09-15

## Problem statement

`RETIREMENT-LEDGER.md` (in `docs/CogniCode_Living_Software_Intelligence/`)
lists **GAP S2**:

> The M5 equivalence harness (e36) only covers `CallGraphProjection`. The
> `GenericGraphProjection` introduced in e37 design D5 has no oracle /
> equivalence coverage. The two projections have similar but distinct
> invariants; without coverage, regressions in D5 (sort order, dangling-free
> resolution, kind encoding) can ship undetected.

The LSI umbrella change lists this gap as one of the bounded housekeeping
slices eligible for direct execution without a formal design phase.

## Codebase reconnaissance

### Adapter (`cognicode-core`)

- `crates/cognicode-core/src/infrastructure/graph/generic_graph_projection.rs`
  - `FactGenericGraphProjection` (struct + `GenericGraphProjectionPort` impl)
  - `build_generic_projection(facts: &[Fact]) -> GenericProjection` —
    pure function, deterministic.
  - Gated behind `feature = "evidence-kernel"` AND `feature = "multimodal"`
    (see `crates/cognicode-core/src/infrastructure/graph/mod.rs:48`).

### Port (`cognicode-core`)

- `crates/cognicode-core/src/domain/ports/generic_graph_projection.rs`
  - `GenericProjection { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> }`
  - `#[derive(Debug, Clone)]` only — does NOT implement `Serialize`
    (intentional: keeps kernel types out of the projection boundary).

### Existing harness (`cognicode-core`)

- `crates/cognicode-core/tests/equivalence_harness.rs` (CallGraph coverage)
- `crates/cognicode-core/tests/equivalence_harness/` (submodules directory)

### Node aggregate (`cognicode-core`)

- `crates/cognicode-core/src/domain/aggregates/generic_graph.rs`
  - `GraphNode` derives `Debug + Clone + PartialEq + Serialize + Deserialize`
  - `GraphNode` has `created_at` and `updated_at` (wall-clock fields set by
    `GraphNode::builder` at construction — NOT part of the projection
    contract, must be excluded from any equivalence fingerprint).

### D5 invariants (declared in `generic_graph_projection.rs` doc-comment)

1. Determinism (ordered maps in → sorted vectors out).
2. Dangling-free (edges only when BOTH endpoints resolve to emitted nodes).
3. Self-loop-free (`GraphEdge::new` rejects self-loops).
4. Kind-multiset equivalence (E38.1 CP-2 single-codec).

## Solution shape

A sibling file `tests/equivalence_harness/generic_graph.rs` mirroring the
e37 harness style:

- Wired behind `#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`.
- 5 functional scenarios + 1 helper sanity.
- 1 pinned structural digest that excludes wall-clock fields.
- Comparison contract declared at file head BEFORE any comparison (per
  `projection-architecture` spec requirement "Derived projections are
  rebuildable").

The structural fingerprint used for equality is a sorted
`(NodeId, NodeKind, label, source_path)` tuple list for nodes + a sorted
`(source, target, kind, provenance)` tuple list for edges, hashed with
SHA-256. Excludes `GraphNode::created_at`/`updated_at` (wall-clock).

## Out of scope

- Multi-language projection corpus (TS/Go/Python) — separate cycle.
- Snapshot-stored projection persistence — separate cycle (M8).
- Property-based / fuzz testing — future work (post-M14).
