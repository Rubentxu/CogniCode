# Exploration Report — e42 — CP-3 cross-producer edge cases

> Change: `e42-lsi-cp3-cross-producer-edge-cases` | Cycle: housekeeping | Date: 2026-09-15

## Problem statement

The `RETIREMENT-LEDGER.md` lists **CP-3** as a resolved item from e38.1
(commit 6977d816):

> **CP-3 RESUELTO** (e38.1): `lsp_facts` resolve the reported container
> name against the extraction context (the `get_symbols` symbols of the
> walked files): exact name match, `SymbolKind::File` excluded, duplicate
> names tie-break on the lexicographically smallest FQN (the shared-
> resolver rule). A resolved container becomes the enclosing symbol's
> 1-based fact-side FQN.

The fix shipped two things:
1. The production code path (`lsp_facts::reference_subject`, line 213)
   with a documented fallback rule.
2. ONE happy-path test (`resolvable_container_normalizes_to_enclosing_symbol_fact_side_fqn`).

The fallback rule has **two corner cases** that the contract explicitly
documents but never tested:

1. `container = None` (LSP server omits enclosing-symbol metadata for
   module-level references).
2. `container = Some(name_that_does_not_exist_in_extraction_context)`
   (defensive: bridge MUST NOT fabricate an entity).

A future regression that breaks the fallback rule would NOT be caught by
the existing test suite.

## Codebase reconnaissance

### Production contract (`application/fact_bridge/lsp_facts.rs:213`)

```rust
fn reference_subject(reference: &Reference, indexes: &BTreeMap<String, SubjectIndex>) -> String {
    reference
        .container
        .as_deref()
        .and_then(|container| {
            indexes
                .get(reference.location.file())
                .and_then(|index| index.resolve(container))
        })
        .map_or_else(|| reference.location.file().to_string(), str::to_string)
}
```

The contract is clear: if either `container.is_none()` or
`index.resolve(container)` returns `None`, fall back to
`reference.location.file()`. Two corner cases, same fallback path.

### Existing tests (`application/fact_bridge/lsp_facts.rs:567`)

`resolvable_container_normalizes_to_enclosing_symbol_fact_side_fqn`
exercises only the **successful resolution** path. No test exercises
the fallback.

### Test coverage of CP-3 in other files

- `tests/cp5_tie_break.rs` — tests for cross-producer entity continuity
  (3 tests). All use `DeterministicAnalyzer` only; no LSP involvement.
- `src/application/fact_bridge/batch_builder.rs:615` —
  `lsp_reference_facts_join_tree_sitter_defines_entities` — happy-path
  join test, single case (container resolves to `main`).

Total existing CP-3 coverage: 2 happy-path tests, 0 fallback tests.

## Solution shape

Two RED-first tests added to `lsp_facts::tests`, mirroring the
`MainAndHelper` mock-provider pattern from the existing happy-path test:

1. `cp3_unreported_container_falls_back_to_file_path` — provider reports
   a reference with `container: None`. Asserts the pre-declared symbol
   entity's `callees` is empty (the call did NOT join it).

2. `cp3_unresolvable_container_falls_back_to_file_path` — provider
   reports a container name that does NOT match any symbol. Asserts the
   call fact's subject is DISTINCT from the pre-declared symbol entity
   (defensive against fabricated entities) AND the symbol's `callees`
   is empty.

Each test pre-declares a `core:defines` fact via `add_observation`
(simulating the DeterministicAnalyzer contribution to the same
snapshot), so the `SnapshotEntityView` has a known entity to compare
against.

## Out of scope

- Renegotiating the contract (e.g. rejecting references with no
  container outright instead of falling back to file path). The
  fallback is a *documented* contract — changing it is a design
  decision, not a test addition.
- Adding similar coverage for `tree_sitter_facts` (the
  DeterministicAnalyzer side has its own `core:contains` /
  `core:defines` grammar and is already covered by the
  `lsp_reference_facts_join_tree_sitter_defines_entities` test).
- Performance / determinism assertions on the fallback path
  (covered by `repeated_provider_collection_is_identical`).
