# Spec — e40 — GenericGraph equivalence harness

> Source: `openspec/changes/e40-lsi-generic-graph-equivalence-harness/spec.md`
> Archived: 2026-09-15
> This is a copy of the change spec for archival into `openspec/specs/`
> at the next archive sync.

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

### REQ-EQGG-03 — Dangling-free contract (D5 invariant #2)

**Given** a fact corpus with one dangling reference
**When** the adapter projects it
**Then** the projection MUST drop the dangling edge without panicking.

### REQ-EQGG-04 — Self-loop-free contract (D5 invariant #3)

N/A — covered by `GraphEdge` unit tests (out-of-scope for this harness).

### REQ-EQGG-05 — Kind-multiset equivalence (E38.1 CP-2 / D5 invariant #4)

**Given** a fact corpus with `core:defines` facts carrying
`kind=<SerdeName>` details (E38.1 CP-2 single-codec contract)
**When** the adapter projects it
**Then** the multiset of emitted node kinds MUST match the multiset of
kinds declared in the input `core:defines` facts (plus file kinds for
file entities).

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

### REQ-EQGG-08 — Feature gating mirrors e37

The new harness module MUST be excluded from compilation when either
`feature = "evidence-kernel"` OR `feature = "multimodal"` is absent
(`#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`).

### REQ-EQGG-09 — Wired into parent harness

The parent test binary `equivalence_harness.rs` MUST include the new
module via `#[path = "equivalence_harness/generic_graph.rs"] mod generic_graph;`.
