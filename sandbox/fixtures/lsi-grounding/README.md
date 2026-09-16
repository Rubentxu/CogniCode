# e67 LSI Grounding Fixture

Small Rust module that exercises the minimum canonical fact vocabulary needed
to demonstrate production grounding end-to-end.

## Shape

| Construct | Predicate emitted |
|---|---|
| `pub mod inner { pub fn helper() -> u32 { 42 } }` | `core:defines` (helper), `core:contains` (sample.rs → inner) |
| `pub fn greet() -> u32 { inner::helper() }` | `core:defines` (greet), `core:calls` (greet → inner::helper) |

## Expected predicates

`{core:defines, core:contains, core:calls}` (see `expected_facts.json`).

## Why this shape

- Minimum non-degenerate: 3 predicates, 1 calls edge, 1 contains edge.
- Real Rust syntax that `extract_file` parses cleanly.
- No `use` statements (keeps `core:imports` out of the predicate set) so the
  "bounded vocabulary" test in WU1 has a single, narrow target to assert.
- No references to other modules (keeps `core:references` out too).

## When to extend

If a future e67.x needs `core:imports` or `core:references` coverage, add
that construct here **and** extend `expected_facts.json` in the same commit.