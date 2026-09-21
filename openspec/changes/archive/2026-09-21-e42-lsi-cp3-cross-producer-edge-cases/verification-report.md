# Verification Report — e42 — CP-3 cross-producer edge cases

> Change: `e42-lsi-cp3-cross-producer-edge-cases` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | Unreported-container fallback test | COMPLIANT | `cp3_unreported_container_falls_back_to_file_path` exists at lsp_facts.rs; pre-declares `src/lib.rs:thing:1`, mock observer reports `container: None`, asserts 1 call fact, thing.callees empty, call subject != thing entity. |
| 2 | Unresolvable-container fallback test | COMPLIANT | `cp3_unresolvable_container_falls_back_to_file_path` exists at lsp_facts.rs; pre-declares `src/lib.rs:real_fn:1`, mock observer reports `container: Some("phantom_container")`, asserts 1 call fact, real_fn.callees empty, call subject != real_fn entity, no entity carries "phantom_container". |
| 3 | No production code change | COMPLIANT | `git diff 8f8e7ea3..f102655b -- crates/cognicode-core/src/application/fact_bridge/lsp_facts.rs` shows only test additions (no changes to `reference_subject`, `collect`, `collect_references`, `collect_hierarchy`). |

**Verdict: COMPLIANT (3/3 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-core --lib --features evidence-kernel \
    'lsp_facts::tests'
...
test application::fact_bridge::lsp_facts::tests::tier_provider_ids_match_the_provider_identity_constants ... ok
test application::fact_bridge::lsp_facts::tests::unresolved_symbol_query_records_site_and_exhausted_tiers_without_a_fact ... ok
test application::fact_bridge::lsp_facts::tests::unresolved_symbol_queries_name_the_symbol_fact_side_fqn ... ok
test application::fact_bridge::lsp_facts::tests::resolvable_container_normalizes_to_enclosing_symbol_fact_side_fqn ... ok
test application::fact_bridge::lsp_facts::tests::maps_reference_kinds_and_hierarchy_to_canonical_predicates ... ok
test application::fact_bridge::lsp_facts::tests::tier_decides_provenance_class ... ok
test application::fact_bridge::lsp_facts::tests::repeated_provider_collection_is_identical ... ok
test application::fact_bridge::lsp_facts::tests::cp3_unreported_container_falls_back_to_file_path ... ok
test application::fact_bridge::lsp_facts::tests::cp3_unresolvable_container_falls_back_to_file_path ... ok

test result: ok. 9 passed; 0 failed; 0 ignored
```

Also verified (no regression):

```
$ cargo test -p cognicode-core --lib --features evidence-kernel \
    'batch_builder::tests'
test result: ok. 9 passed; 0 failed; 0 ignored
```

## Diff summary

| Stat | Value |
|------|-------|
| Files | 1 (`lsp_facts.rs`) |
| LOC | +262 / -0 (tests only) |
| Commits | 1 (`f102655b`) |
| Head SHA | `f102655b` |
| Origin SHA | `f102655b` (verified via `git ls-remote origin main`) |

## Pre-existing failures (NOT introduced by this change)

The full `cargo test -p cognicode-core --lib` run on main had 39
failures in `application::services::file_operations` **before** this
change. Verified via `git stash` before commit `f102655b`. This
change does NOT touch `file_operations` and does NOT introduce any
new failure.

## Conformance matrix impact

Unchanged: `pct_verified=91.4% pct_triaged=92.4%`. This change is a
test-only addition that exercises an existing production contract;
no new spec content.

## Verdict

**PASS** — cycle is ready for archive.
