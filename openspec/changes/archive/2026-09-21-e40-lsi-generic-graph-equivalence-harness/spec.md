# Spec — e40 — GenericGraph equivalence harness

> Change: `e40-lsi-generic-graph-equivalence-harness` | Phase: specify | Date: 2026-09-15

## Scope

Extend the equivalence harness (`crates/cognicode-core/tests/equivalence_harness*`)
with coverage for `GenericGraphProjection` (e37 design D5). Close RETIREMENT-LEDGER
GAP S2 from `docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md`.

## Requirements

### REQ-EQGG-01 — Equivalence harness covers GenericGraphProjection

**Given** the `FactGenericGraphProjection` adapter introduced in e37
design D5 (gated behind `feature = "evidence-kernel"` +
`feature = "multimodal"`)

**When** the equivalence harness is run
**Then** a sibling module at
`crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` MUST
exist and exercise the 4 invariants declared in design D5.

### REQ-EQGG-02 — Rebuild equivalence (D5 invariant #1)

**Given** a fixed synthetic fact corpus
**When** the adapter projects the same corpus twice
**Then** the two `GenericProjection` values MUST yield the same structural
fingerprint (SHA-256 over sorted `(NodeId, NodeKind, label, source_path)`
and `(source, target, kind, provenance)` tuples).

The fingerprint MUST exclude wall-clock fields (`GraphNode::created_at`,
`GraphNode::updated_at`) because they are set by `GraphNode::builder` at
construction time and are not part of the projection contract.

### REQ-EQGG-03 — Dangling-free contract (D5 invariant #2)

**Given** a fact corpus with one dangling reference (a `core:calls` fact
whose target does not resolve to any emitted node)
**When** the adapter projects it
**Then** the projection MUST drop the dangling edge without panicking and
without including the unresolved target in any edge multiset.

### REQ-EQGG-04 — Self-loop-free contract (D5 invariant #3)

**Given** a fact corpus with a self-loop attempt (`source == target`)
**When** the adapter projects it
**Then** the projection MUST drop the self-loop without panicking
(`GraphEdge::new` returns `Err`; the harness must observe no such edge in
the output).

> *Note*: not exercised by an explicit scenario in this harness (the
> `build_generic_projection` implementation does not attempt self-loops on
> its own; self-loop rejection is `GraphEdge::new`'s job, separately
> covered by `GraphEdge`'s unit tests).

### REQ-EQGG-05 — Kind-multiset equivalence (E38.1 CP-2 / D5 invariant #4)

**Given** a fact corpus with `core:defines` facts carrying
`kind=<SerdeName>` details (E38.1 CP-2 single-codec contract)
**When** the adapter projects it
**Then** the multiset of emitted node kinds MUST match the multiset of
kinds declared in the input `core:defines` facts (plus file kinds for
file entities that contain symbols).

### REQ-EQGG-06 — Empty input ⇒ empty projection

**Given** an empty fact corpus
**When** the adapter projects it
**Then** the projection MUST contain zero nodes and zero edges.

### REQ-EQGG-07 — Pinned digest gate

**Given** the canonical sample corpus
**When** the harness runs the pinned digest scenario
**Then** the actual structural fingerprint MUST equal
`PINNED_GENERIC_PROJECTION_DIGEST` =
`sha256:6d22cf734f1094a07df63aea4698e512ae5046f4fa732a322ea43b31678a1cc5`.

Any future change to the projection's deterministic output (sort order,
kind encoding, edge skipping policy) MUST re-pin this constant and the
change MUST be consciously committed.

### REQ-EQGG-08 — Feature gating mirrors e37

**Given** the new harness module
**When** it is compiled without `feature = "evidence-kernel"` OR
without `feature = "multimodal"`
**Then** the module MUST be excluded from compilation
(`#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`).
This keeps the default build (single feature or none) lean.

### REQ-EQGG-09 — Wired into the parent harness

**Given** the new module file
**When** the parent test binary `equivalence_harness.rs` is built
**Then** it MUST include the new module via
`#[path = "equivalence_harness/generic_graph.rs"] mod generic_graph;`.

## Non-goals

- Property-based / fuzz testing of the projection — future cycle.
- Multi-language projection corpus (TS/Go/Python) — separate cycle.
- Coverage of any consumer of `GenericProjection` (e.g.
  `ExplorerApiHandler`) — covered by their respective cycles' e2e tests.

## Out-of-scope clarifications

The structural fingerprint deliberately excludes wall-clock fields. This
is a *contract* statement, not a bug: `GenericProjection` is the
projection contract; timestamps are bookkeeping noise set by the builder.
Any future spec change that introduces stable hashes over timestamps must
revisit REQ-EQGG-02 and REQ-EQGG-07 explicitly.
