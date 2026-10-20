# F3.W4 + F3.W5 corpus: analyze_impact and get_call_hierarchy.

All symbols use `impact_*` prefix to avoid collisions with other
corpora (e.g., `unique_*` from query_index_equivalence, `outline_*`
from outline_equivalence, `top_level` from per_file_correctness,
`callee` from per_file_equivalence).

## F3.W4 (analyze_impact): callers/dependents of `impact_target`

`impact_target` (in `lib.rs`) is the analyzed symbol. The corpus
establishes a dependency chain:

- `direct.rs` defines `impact_direct_caller`, which calls
  `impact_target`.
- `transitive.rs` defines `impact_transitive_caller`, which calls
  `impact_direct_caller`.

When `analyze_impact("impact_target")` runs:

- Direct dependents: `impact_direct_caller` (from direct.rs).
- Transitive dependents: `impact_transitive_caller` (from transitive.rs).
- Impacted files: both `direct.rs` and `transitive.rs`.

## F3.W5 (get_call_hierarchy outgoing): callees of `impact_target`

`impact_target` delegates to two helpers, both defined in `lib.rs`:

- `impact_callee_a()` returns 1.
- `impact_callee_b()` returns 2.

When `get_call_hierarchy(symbol_name="impact_target",
direction=outgoing, depth=1)` runs:

- `calls` must contain `impact_callee_a` and `impact_callee_b`.
- `calls` must NOT contain `impact_direct_caller` (that's a
  caller of `impact_target`, not a callee).

The 3-file corpus exercises both directions of the dependency
graph: W4 walks callers, W5 walks callees.

## Why both tools share this corpus

W4 and W5 are the two halves of `build_graph` round-tripping:
- W4: who depends on me? (callers, both direct and transitive).
- W5: who do I depend on? (callees, direct only).

The corpus was originally built for W4. When designing W5, the
existing `direct.rs` and `transitive.rs` provided the "incoming"
side, but `lib.rs` had to be augmented so `impact_target` actually
calls helpers (the original `lib.rs::impact_target() { 42 }` had
no callees, which would make outgoing direction trivially empty).

The corpus is shared to minimize fixture surface area and keep
the dependency shape consistent across both tests.
