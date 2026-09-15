# Tasks — e41 — DFG conformance coverage

> Change: `e41-lsi-dfg-conformance-coverage` | Phase: tasks | Date: 2026-09-15

Single WU. ~43 LOC net. No review concerns; the file is conformance-only.

## Review Workload Forecast

```
Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: single-commit
400-line budget risk: Very Low (~43 LOC)
```

## WU1 — Register DFG fixtures + dispatch arm

**Goal**: Add 2 DFG fixtures to the conformance corpus and register
the `"dfg" => DFG.clone()` arm in the dispatch map, both gated by
`#[cfg(feature = "program-analysis-server")]`.

### Files

- `crates/cognicode-core/src/application/program_analysis/conformance.rs` (+43 / -2 LOC)

### Sub-steps

1. Add `#[cfg(feature = "program-analysis-server")] use ... DFG;` to the imports.
2. Add `"dfg" => DFG.clone()` arm in the dispatch map under the same feature gate.
3. Add 2 fixtures: `linear_def_use_chain` (4-statement chain) and
   `diamond_diamond_diamond` (5-statement diamond with multi-use convergence).
4. Both fixtures declare `cfg_digest` so `DfgParams::validate` passes.
5. Change dispatch-error format from `{e}` to `{e:?}` (debugging QoL;
   surfaces full `AnalyticsError` payload in future regressions).
6. Run `cargo test -p cognicode-core --lib --features program-analysis-server program_analysis`
   — 56 tests MUST pass (54 existing + 2 DFG fixtures).

### Verification

```
cargo test -p cognicode-core --lib --features program-analysis-server 'program_analysis'
# Expected: 56 passed; 0 failed
```

### Risk

- None. The DFG dispatch is unchanged; only the conformance corpus and the
  dispatch arm registration are touched.
- The `cfg_digest` parameter is dummy text — it doesn't gate any output
  behavior, but `DfgParams::validate` requires it to be present.

### Rollback

`git revert <commit>` — no migration, no schema, no runtime state.
