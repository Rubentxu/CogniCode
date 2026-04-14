# Exploration: scale-and-metrics

### Current State
`build_graph` is handled by `handle_build_graph` in
`src/interface/mcp/handlers.rs`, which calls
`AnalysisService::build_project_graph` in
`src/application/services/analysis_service.rs`. That method now uses Rayon to
parse files in parallel, but the crash seen by the sandbox is not a lock
deadlock: reproducing the MCP call with `RUST_BACKTRACE=1` shows the server
aborts with `thread '<unknown>' has overflowed its stack`, and the orchestrator
then reports EOF from `src/sandbox_core/mcp_core.rs`.

The likely trigger is recursive AST walking in
`src/infrastructure/parser/tree_sitter_parser.rs`
(`find_nodes_recursive_with_path`, `find_function_calls`, and
`find_calls_in_node`) while `build_project_graph` scans more than the intended
fixture sources. The scale workspaces include a local `target/` tree, and
`ignore::WalkBuilder::new(project_dir)` is not explicitly excluding `target/`,
so `build_graph` traverses generated/build artifacts in addition to `src/*.rs`.

For metrics, ground-truth support already exists in
`src/sandbox_core/ground_truth.rs` via `GroundTruth.usages` and
`GroundTruth.search_results`, while `find_usages` and `semantic_search` already
return structured data from `src/interface/mcp/handlers.rs` and
`src/interface/mcp/schemas.rs`. But `src/sandbox_core/scoring.rs` does not have
specialized matching for either tool, so `score_scenario` falls back to a
non-empty-response heuristic and sometimes assigns `75.0` instead of a real F1.

### Affected Areas
- `src/application/services/analysis_service.rs` — parallel `build_project_graph`, file enumeration, cache locking, and parser invocation.
- `src/infrastructure/parser/tree_sitter_parser.rs` — recursive tree walking that can overflow the stack on deep/generated syntax trees.
- `src/interface/mcp/handlers.rs` — `handle_build_graph`, `handle_find_usages`, and `handle_semantic_search` define the concrete response shapes.
- `src/interface/mcp/schemas.rs` — `FindUsagesOutput`, `UsageEntry`, `SemanticSearchOutput`, and `SearchResultDto` are the scoring input contracts.
- `src/sandbox_core/ground_truth.rs` — already contains `ExpectedUsage` / `ExpectedSearchResult`, but lacks dedicated parse+match helpers for these tools.
- `src/sandbox_core/scoring.rs` — contains the current fallback logic and the place to route tool-specific correctness scoring.
- `sandbox/manifests/scale/rust.yaml` — reproduces the `build_graph` crash and shows which scenarios depend on real correctness scoring.

### Approaches
1. **Constrain graph scanning and keep parallelism** — Exclude build/output directories before parsing, then keep Rayon for source files only.
   - Pros: Smallest change, addresses the reproduced overflow trigger, preserves recent performance work, likely fixes both crash risk and noisy graph quality.
   - Cons: Does not harden parser recursion by itself; a pathological source tree could still overflow.
   - Effort: Medium.

2. **Make parser traversal iterative or bounded** — Replace recursive tree-sitter walks with explicit stacks/iterators, optionally combined with directory filtering.
   - Pros: Fixes the root failure mode directly, safer for very deep ASTs, independent of workspace layout.
   - Cons: More invasive, touches core parser behavior, higher regression risk across symbol extraction and call-relationship detection.
   - Effort: High.

3. **Rollback or gate parallel parsing** — Revert `build_project_graph` to sequential parsing or make Rayon optional until parser safety is improved.
   - Pros: Simplest operational mitigation if thread stack size under Rayon is the amplifier, lowers concurrency-related uncertainty.
   - Cons: Loses performance gains, does not solve recursive overflow if it can also happen sequentially, and evidence currently points more to traversal depth than mutex contention.
   - Effort: Low.

4. **Add dedicated F1 matchers for `find_usages` and `semantic_search`** — Parse returned usages/results and compare against `GroundTruth.usages` / `GroundTruth.search_results` in `ground_truth.rs`, then route from `score_scenario`.
   - Pros: Uses existing schemas and ground-truth types, removes the `75.0` heuristic cleanly, keeps correctness scoring consistent with other tools.
   - Cons: Requires defining normalization rules for file paths, line/column indexing, and whether search matching is name-only or name+kind+file.
   - Effort: Medium.

5. **Replace fallback with count-based scoring only** — Remove the heuristic and score search tools by totals or presence/absence without full matching.
   - Pros: Small change.
   - Cons: Still not a real F1, does not satisfy the stated requirement, and would underuse already available ground truth.
   - Effort: Low.

### Recommendation
For the crash, start with **Approach 1** and pair it with a targeted hardening
slice from **Approach 2** if needed. The best first fix is to stop
`build_project_graph` from scanning fixture `target/` trees and other generated
directories, because that aligns with the reproduced stack overflow and is lower
risk than rewriting parser traversal immediately. If scale tests still fail
after filtering, then harden `TreeSitterParser` recursion.

For metrics, use **Approach 4**. The codebase already has the right ground truth
types and response structures; what is missing is the parser/matcher plumbing in
`ground_truth.rs` and dispatch in `score_scenario`. That is the cleanest way to
remove fallbacks and compute real F1 for both `find_usages` and
`semantic_search`.

### Risks
- Excluding too many directories in `build_project_graph` could hide legitimate source files in nonstandard layouts if the filter is too broad.
- Reworking parser traversal is sensitive because the same recursive helpers are used by symbol extraction and call-relationship logic.
- Search scoring needs explicit normalization decisions for relative vs absolute file paths and 0-indexed vs 1-indexed positions, or F1 will be artificially low.
- `semantic_search` currently returns ranking metadata (`score`, `match_type`), but ground truth only partially models ranking via optional `relevance_score`; exact scoring semantics must stay simple unless ranking accuracy is intentionally added.

### Ready for Proposal
Yes — the change is clear enough to propose as two scoped workstreams: (1)
stabilize `build_graph` scale runs by filtering scanned inputs and validating
parser safety, and (2) add real F1-based correctness scoring for `find_usages`
and `semantic_search` using existing ground-truth structures.
