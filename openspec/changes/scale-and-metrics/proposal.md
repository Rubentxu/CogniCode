# Proposal: scale-and-metrics

## Intent

Two blocking issues prevent the `scale-and-metrics` sandbox from producing reliable
results. First, `build_graph` crashes the MCP server with a stack overflow whenever
scale fixtures include a `target/` tree, because `WalkBuilder` in
`build_project_graph` does not exclude build/generated directories, causing recursive
tree-sitter AST walks on thousands of generated `.rs` files. Second, `find_usages`
and `semantic_search` never compute real F1 — `score_scenario` falls through to a
non-empty-response heuristic that assigns `75.0`, masking actual correctness.

## Scope

### In Scope
- Add explicit exclusion of build/generated directories (`target/`, `node_modules/`,
  `.git/`, `dist/`, `build/`) in `build_project_graph` before Rayon parallel parsing.
- Add `parse_returned_usages` and `match_usages` helpers in `ground_truth.rs` to
  compare `FindUsagesOutput.usages` against `GroundTruth.usages` (`ExpectedUsage`),
  normalizing file paths to relative form and using line-level matching.
- Add `parse_returned_search_results` and `match_search_results` helpers in
  `ground_truth.rs` to compare `SemanticSearchOutput.results` against
  `GroundTruth.search_results` (`ExpectedSearchResult`), matching on name+kind.
- Wire both matchers into `score_scenario` under `"find_usages"` and
  `"semantic_search"` arms, removing the `75.0` fallback branch for those tools.
- Add new result types: `UsageMatchResult` and `SearchResultMatchResult` (parallel
  to `SymbolMatchResult`) with TP/FP/FN/precision/recall/F1 fields.

### Out of Scope
- Full iterative rewrite of `TreeSitterParser` recursive traversal (deferred;
  investigate only if filtering alone does not stop scale overflows).
- Ranking-accuracy scoring for `semantic_search` (the `score` / `match_type` fields
  in `SearchResultDto` are not modelled in ground truth — left for a future change).
- Adding scale fixture YAML files with `usages` / `search_results` ground truth
  entries (sandbox fixture authoring is a separate workstream).

## Capabilities

### New Capabilities
- `find-usages-f1-scoring`: real precision/recall/F1 scorer for `find_usages` tool
- `semantic-search-f1-scoring`: real precision/recall/F1 scorer for `semantic_search` tool

### Modified Capabilities
- `build-graph-stability`: `build_project_graph` directory scanning is extended to
  filter generated/build paths before handing files to Rayon.

## Approach

**Workstream 1 — Crash fix (low risk, small change):**
In `build_project_graph`, add a path-component filter after `WalkBuilder::build()`
that discards any entry whose path contains a blocklisted directory segment
(`target`, `node_modules`, `.git`, `dist`, `build`). This is a pure filter on the
already-walked `Vec<_>`, inserted before `into_par_iter()`. No parser code changes.
Blocked by nothing; safe to land independently.

**Workstream 2 — Real F1 for search tools (medium effort, bounded scope):**
1. Add `UsageMatchResult` and `SearchResultMatchResult` structs to `ground_truth.rs`.
2. Add `parse_returned_usages(response: &Value) -> Vec<ReturnedUsage>` — reads the
   `usages` array from `FindUsagesOutput` JSON, mapping each item to a
   `(file, line, col)` triple with the `file` field normalised to a relative path
   by stripping any common project prefix.
3. Add `match_usages(returned, expected) -> UsageMatchResult` — name-free, position-
   based F1: a TP is any returned usage whose `(file_basename, line)` matches an
   expected usage (column tolerance ±1 to absorb 0/1-indexing differences).
4. Add `parse_returned_search_results` / `match_search_results` — name+kind F1,
   ignoring `score` and `match_type` fields.
5. In `score_scenario` (`scoring.rs`), add explicit `"find_usages"` and
   `"semantic_search"` arms before the catch-all `_` branch, calling the new
   matchers when ground truth has the corresponding fields set.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `src/application/services/analysis_service.rs` | Modified | Add generated-dir filter before `into_par_iter()` in `build_project_graph` (lines ~169-188) |
| `src/sandbox_core/ground_truth.rs` | Modified | Add `UsageMatchResult`, `SearchResultMatchResult`, `ReturnedUsage`, `parse_returned_usages`, `match_usages`, `parse_returned_search_results`, `match_search_results` |
| `src/sandbox_core/scoring.rs` | Modified | Add `"find_usages"` and `"semantic_search"` arms in `score_scenario`; remove `75.0` fallback for those tools; import new match types |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Directory filter too broad, hiding legitimate `src/build/` folders | Low | Use path-component exact match, not substring; only block top-level known segments |
| File-path normalisation mismatch produces artificial F1=0 | Med | Use `file_basename` (last component) as primary key; document normalisation rule in code and spec |
| `target/` exclusion insufficient if fixture places sources inside it | Low | Scale manifests control workspace layout; document that `target/` must be outside `src/` in fixture docs |
| Stack overflow still occurs after filtering (deeply nested source AST) | Low | Monitor scale test results; if issue persists, open targeted iterative-parser ticket |

## Rollback Plan

- Workstream 1: revert the single `filter` line in `build_project_graph`; no API or
  schema changes, fully independent.
- Workstream 2: revert the two new `match` arms in `score_scenario` and remove the
  four new functions/structs in `ground_truth.rs`; the `75.0` fallback behaviour is
  preserved as-was in the `_` arm until removed.

## Dependencies

- No new external crates required.
- `ignore` crate (already in use) continues to handle `.gitignore` rules; the new
  filter is additive.

## Success Criteria

- [ ] `build_graph` scale test completes without EOF / stack overflow in the sandbox.
- [ ] `find_usages` correctness score is computed from real precision/recall F1 when
  `ground_truth.usages` is present (no longer defaults to `75.0`).
- [ ] `semantic_search` correctness score is computed from real name+kind F1 when
  `ground_truth.search_results` is present.
- [ ] All existing unit tests in `ground_truth.rs` and `scoring.rs` continue to pass.
- [ ] New unit tests for `match_usages` and `match_search_results` pass.
