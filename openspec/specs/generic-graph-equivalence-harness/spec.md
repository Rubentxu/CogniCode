# Spec — e40 — GenericGraph equivalence harness

> Source: `openspec/changes/e40-lsi-generic-graph-equivalence-harness/spec.md`
> Archived: 2026-09-15
> This is a copy of the change spec for archival into `openspec/specs/`.

## Purpose

Extend the equivalence harness (`crates/cognicode-core/tests/equivalence_harness*`)
with coverage for `GenericGraphProjection` (e37 design D5). Close RETIREMENT-LEDGER
GAP S2 from `docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md`.

## ADDED Requirements

### Requirement: Equivalence harness covers GenericGraphProjection (REQ-EQGG-01)

The system MUST provide a sibling module at
`crates/cognicode-core/tests/equivalence_harness/generic_graph.rs` that
exercises the 4 invariants declared in e37 design D5.

The module MUST be wired behind
`#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]`.

### Requirement: Rebuild equivalence (REQ-EQGG-02)

Given a fixed synthetic fact corpus, the adapter MUST produce the same
structural fingerprint (SHA-256 over sorted `(NodeId, NodeKind, label,
source_path)` and `(source, target, kind, provenance)` tuples) when the
corpus is projected twice.

The fingerprint MUST exclude wall-clock fields
(`GraphNode::created_at`, `GraphNode::updated_at`) because they are set
by `GraphNode::builder` at construction time and are not part of the
projection contract.

### Requirement: Dangling-free contract (REQ-EQGG-03)

Given a fact corpus with one dangling reference (a `core:calls` fact
whose target does not resolve to any emitted node), the adapter MUST
drop the dangling edge without panicking and MUST NOT include the
unresolved target in any edge multiset.

### Requirement: Kind-multiset equivalence (REQ-EQGG-04)

Given a fact corpus with `core:defines` facts carrying
`kind=<SerdeName>` details (E38.1 CP-2 single-codec contract), the
multiset of emitted node kinds MUST match the multiset of kinds
declared in the input `core:defines` facts (plus file kinds for file
entities that contain symbols).

### Requirement: Empty input yields empty projection (REQ-EQGG-05)

Given an empty fact corpus, the adapter MUST produce a projection with
zero nodes and zero edges.

### Requirement: Pinned digest gate (REQ-EQGG-06)

Given the canonical sample corpus, the harness's pinned digest scenario
MUST compare the actual structural fingerprint against the constant
`PINNED_GENERIC_PROJECTION_DIGEST` =
`sha256:6d22cf734f1094a07df63aea4698e512ae5046f4fa732a322ea43b31678a1cc5`.

Any future change to the projection's deterministic output (sort order,
kind encoding, edge skipping policy) MUST re-pin this constant and the
change MUST be consciously committed.

### Requirement: Feature gating mirrors e37 (REQ-EQGG-07)

The new harness module MUST be excluded from compilation when either
`feature = "evidence-kernel"` OR `feature = "multimodal"` is absent.

### Requirement: Wired into parent harness (REQ-EQGG-08)

The parent test binary `equivalence_harness.rs` MUST include the new
module via `#[path = "equivalence_harness/generic_graph.rs"] mod generic_graph;`.

## Non-goals

- Property-based / fuzz testing of the projection — future cycle.
- Multi-language projection corpus (TS/Go/Python) — separate cycle.
- Coverage of any consumer of `GenericProjection` (e.g.
  `ExplorerApiHandler`) — covered by their respective cycles' e2e tests.
