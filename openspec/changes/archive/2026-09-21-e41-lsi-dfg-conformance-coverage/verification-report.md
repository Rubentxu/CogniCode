# Verification Report — e41 — DFG conformance coverage

> Change: `e41-lsi-dfg-conformance-coverage` | Phase: verify | Date: 2026-09-15

## Scope verification

| REQ | Title | Verdict | Evidence |
|-----|-------|---------|----------|
| 1 | Conformance corpus includes DFG fixtures | COMPLIANT | `linear_def_use_chain` + `diamond_diamond_diamond` registered in `canonical_corpus()` (gated by `program-analysis-server`). |
| 2 | DFG dispatch arm registered | COMPLIANT | `"dfg" => DFG.clone()` arm added in `run_corpus()` dispatch map (gated). |
| 3 | DFG fixtures declare required params | COMPLIANT | Both fixtures include `function_id` and `cfg_digest`. |
| 4 | DFG dispatch exercised by replay guard | COMPLIANT | `replay_guard_is_byte_identical_for_entire_corpus` (and `run_corpus_produces_digest_per_fixture`) now include the 2 DFG fixtures. |

**Verdict: COMPLIANT (4/4 REQs).**

## Verification commands run

```
$ cargo test -p cognicode-core --lib \
    --features program-analysis-server 'program_analysis'
...
test application::program_analysis::acceptance_evidence::replay_guard_is_byte_identical_for_entire_corpus ... ok
test application::program_analysis::conformance::tests::run_corpus_produces_digest_per_fixture ... ok
test application::program_analysis::conformance::tests::replay_guard_is_byte_identical ... ok
test application::program_analysis::conformance::tests::perf_envelope_reports_median_and_p95 ... ok
[... 52 other program_analysis tests ...]
test result: ok. 56 passed; 0 failed; 0 ignored
```

## Diff summary

| Stat | Value |
|------|-------|
| Files | 1 (`conformance.rs`) |
| LOC | +43 / -2 |
| Commits | 1 (`9dda3e89`) |
| Head SHA | `9dda3e89` |
| Origin SHA | `9dda3e89` (verified via `git ls-remote origin main`) |

## Conformance matrix impact

Unchanged: `pct_verified=91.4% pct_triaged=92.4%` (the change is a
sub-spec coverage of an existing requirement under
`openspec/specs/program-analysis-conformance/spec.md` — does not
add new REQs to the matrix).

## Pre-existing failures (NOT introduced by this change)

The `cargo test -p cognicode-core --lib` (no features) run on main
already had 14 failing tests in `application::services::file_operations`
**before** this change. Verified via `git stash` + re-run + `git stash
pop` on commit `93d57115`. This change does NOT touch `file_operations`
and does NOT introduce any new failure.

## Verdict

**PASS** — cycle is ready for archive.
