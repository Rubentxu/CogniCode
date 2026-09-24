# F3.W4: analyze_impact CLI/MCP equivalence corpus.

All symbols use `impact_*` prefix to avoid collisions with other
corpora (e.g., `unique_*` from query_index_equivalence, `outline_*`
from outline_equivalence, `top_level` from per_file_correctness,
`callee` from per_file_equivalence).

Three files establish a dependency chain:

- `lib.rs` defines `impact_target` (the symbol whose impact we analyze).
- `direct.rs` defines `impact_direct_caller`, which calls `impact_target`.
- `transitive.rs` defines `impact_transitive_caller`, which calls `impact_direct_caller`.

When `analyze_impact("impact_target")` runs:
- Direct dependents: `impact_direct_caller` (from direct.rs).
- Transitive dependents: `impact_transitive_caller` (from transitive.rs).
- Impacted files: both `direct.rs` and `transitive.rs`.

The corpus uses 3 files instead of one so the graph builder has
multiple files to traverse; a single-file corpus may produce 0
dependents depending on parser semantics.
