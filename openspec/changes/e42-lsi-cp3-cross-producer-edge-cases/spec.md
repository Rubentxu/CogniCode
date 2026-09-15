# Spec — e42 — CP-3 cross-producer edge cases

> Change: `e42-lsi-cp3-cross-producer-edge-cases` | Phase: specify | Date: 2026-09-15

## Scope

Add RED-first tests for the two corner cases of the CP-3
cross-producer join contract documented at
`crates/cognicode-core/src/application/fact_bridge/lsp_facts.rs:213`
(`reference_subject`). The contract explicitly falls back to the
reference site's file path when:

1. The reported `Reference::container` is `None`.
2. The reported container name does NOT resolve to any symbol in the
   extraction context.

Both paths were untested; this slice locks them down.

## Requirements

### Requirement: Unreported-container fallback test

**Given** an LSP observer that reports a reference with
`container: None`

**When** the bridge collects facts into a snapshot that also contains
a pre-declared `core:defines` fact for a different symbol

**Then** the test MUST verify:
- The bridge emits exactly one `core:calls` fact (the served reference).
- The pre-declared symbol entity's `callees` is empty (the call did
  NOT join it).
- The call fact's `subject` `EntityId` is DISTINCT from the
  pre-declared symbol entity's id (no silent fallback to the only
  declared symbol).

### Requirement: Unresolvable-container fallback test

**Given** an LSP observer that reports a reference with
`container: Some("phantom_container")` (a name that does NOT match
any symbol in the extraction context)

**When** the bridge collects facts into a snapshot that also contains
a pre-declared `core:defines` fact for `real_fn`

**Then** the test MUST verify:
- The bridge emits exactly one `core:calls` fact.
- `real_fn.callees` is empty (the call did NOT join `real_fn`).
- The call fact's `subject` `EntityId` is DISTINCT from `real_fn`'s
  `EntityId` (defensive: no fabricated entity under the bogus name).
- No entity in the recovered `SnapshotEntityView` carries the string
  `"phantom_container"` (defensive: no entity invented under the
  bogus name).

### Requirement: No production code change

**Given** the two new tests

**When** the test suite is run

**Then** no production code in `lsp_facts.rs` MUST be modified —
the tests exercise the existing `reference_subject` contract as a
regression net, not as a code change.

## Non-goals

- Renegotiating the CP-3 contract (changing the fallback rule is a
  design decision, not a test addition).
- Adding cross-producer tests for `tree_sitter_facts` (already
  covered by the `core:contains` / `core:defines` grammar tests).
- Performance / determinism assertions on the fallback path
  (covered by `repeated_provider_collection_is_identical`).
