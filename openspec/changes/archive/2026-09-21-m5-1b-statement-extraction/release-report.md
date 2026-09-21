# Release Report — M5.1b — Statement-level extraction

> Cycle: `p-c1fac1fea05615c6/m5-1b-statement-extraction`
> Path: **A-lite**
> Phase: **release**
> Released at: 2026-09-14T21:33:00Z
> CWD: `/var/mnt/DiscoChino2-fast/Proyectos/rust/CogniCode`

## Subject

| Field | Value |
|---|---|
| Base SHA (origin/main before cycle) | `90edff1f68d8d3e1219a648d27b48a88b86ce90f` |
| Candidate SHA (HEAD after cycle) | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| `git rev-parse HEAD` after release | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| `git rev-parse origin/main` after release | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| Commits in subject | `28dce397`, `6b8bee7f`, `23d7839e`, `448d81bd`, `47b25c86`, `2f1f638a`, `6bd72e78` (7 total) |
| Commits in subject (count) | 7 |
| Files changed | 15 (`git diff --stat 90edff1f..HEAD`) |
| Net diff | +2031 / -36 LOC |
| Diff digest (sha256 of `git diff --stat`) | `ac2ad5b9ae2cd4b95a70d24433d72f161e0d88a14e40c9dcdc58af4b1838e8ac` |
| HEAD digest (sha256 of full SHA) | `06731126030f04e3f80e65f92bfa9a6571414fd44bedc5abf33a15d2d5635a6b` |
| Branch | `main` |
| Remote | `origin = git@github.com:Rubentxu/CogniCode.git` |
| Tag | `m5-1b-statement-extraction` (annotated) |
| Tag peel SHA | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` (== HEAD) |
| Tag type | `tag` (annotated) |
| Author (subject SHA) | Ruben `<rubentxu@cognicode.dev>` at `2026-09-14T23:22:19+02:00` |

## Pre-conditions

| Check | Command | Result |
|---|---|---|
| Empty working tree | `git status --porcelain` | empty (exit 0) |
| HEAD == candidate SHA | `git rev-parse HEAD` | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| On trunk branch | `git branch --show-current` | `main` |
| No incoming changes | `git fetch --dry-run origin main` | no updates (only FETCH_HEAD ref advertisement) |
| HEAD == origin/main pre-push | `test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"` | NOT EQUAL (7 commits ahead, expected) |

## Push Evidence

| Step | Command | Exit | Result |
|---|---|---:|---|
| Direct trunk push | `git push origin main` | 0 | `90edff1f..6bd72e78  main -> main` |
| Post-push HEAD check | `git fetch origin main && git rev-parse origin/main` | 0 | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` |
| HEAD == origin/main equality test | `test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"` | 0 | EQUAL |

Push digest (sha256 of "HEAD: ... origin/main: ..." line):

```
sha256: 25063040e5aac70665faef761605d51cbc9d0358c0b1402c5c982f66eb47030d
```

No pre-push hook reformatted any committed change. Push ran on the first attempt with no interactive prompts.

## Tag Evidence

| Step | Command | Exit | Result |
|---|---|---:|---|
| Create annotated tag | `git tag -a m5-1b-statement-extraction 6bd72e789a04922e22e3bc282884c9f7d2af36a0 -m "Release M5.1b — statement-level extraction (A-lite cycle, 7 commits, PASS_WITH_WARNINGS)"` | 0 | tag created |
| Verify tag is annotated | `git cat-file -t refs/tags/m5-1b-statement-extraction` | 0 | `tag` |
| Verify tag peels to HEAD | `git rev-parse 'refs/tags/m5-1b-statement-extraction^{}'` | 0 | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` (= HEAD) |
| Push tag | `git push origin refs/tags/m5-1b-statement-extraction` | 0 | `* [new tag] m5-1b-statement-extraction -> m5-1b-statement-extraction` |
| Verify remote tag peel | `git ls-remote origin 'refs/tags/m5-1b-statement-extraction^{}'` | 0 | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` (= HEAD) |

Tag ref name: `m5-1b-statement-extraction`
Tag peel SHA: `6bd72e789a04922e22e3bc282884c9f7d2af36a0`
Annotated: `true` (`git cat-file -t` returns `tag`)

## Final Verify Pass (release re-verify)

Per `prompts/sddk/phases/release.md` Step 2 ("Run final local verify gates (one more pass)"), the same three commands from the verify phase were re-executed against the candidate SHA immediately before publication.

| # | Command | Exit | Output digest (sha256) | Result |
|---|---|---:|---|---|
| 1 | `cargo test -p cognicode-core --features program-analysis-server --lib program_analysis::ast_lift` | 0 | `2af5ae057bdc9b4567a85d009d26ad7462bdf75cf4884d666e2c43111a0b9118` | 21 / 21 passed (1754 filtered out) |
| 2 | `cargo test -p cognicode-core --features program-analysis-server --lib ingest::extractor::tests` | 0 | `f12a9327cda563f34a6fe8a3adca8625c74c478f1785448102eebd26d4d7d2bb` | 9 / 9 passed (1766 filtered out) |
| 3 | `cargo clippy -p cognicode-core --features program-analysis-server --lib --tests -- -D warnings` | 0 | `e052f08151a523a439df57c09c4c86dbe512e3beeaa2976d416a3a99de3a4f62` | clean — no warnings emitted |
| 4 | `cargo fmt --check` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | clean — no diffs (empty output) |

Combined: 30 unit tests passed; clippy clean; formatting clean. Re-verify is consistent with the verify-report.md (also `21 / 21 + 9 / 9 + clippy clean + fmt clean`).

## UAT Waiver Reasoning

The `release-uat-approved` gate is **waived** for this cycle. The waiver rationale:

- **Backend-only library change.** M5.1b adds statement-level extraction to `cognicode-core` (`ExtractionResult.statements_by_function`, the `extract_statements_from_node` walker, and the `lift` Pass 4 that injects statements from the map). The change is a pure value transform consumed by the MCP server and the WASM module; there is no user-facing surface in this cycle (no UI change, no API endpoint addition, no schema migration, no contract change).
- **No user-visible behaviour change at the service boundary.** `ExtractionResult.statements_by_function` is `#[serde(default)]`; existing serializations deserialize with an empty map. The lift's new `FunctionLocalView.statements` field is additive — old views continue to deserialize, and consumers that don't read the new field are unaffected.
- **Precedent.** The M5.1 cycle (`p-c1fac1fea05615c6/m5-1`) used the same waiver per its verify-report; the M5.1b cycle preserves the same library-only scope and reuses the same precedent.
- **Coverage.** All 8 REQ-STMT-* requirements are bound to named passing tests (per `verify-report.md` L3) — the change is fully covered by the unit-test gate without an external UAT loop.

The waiver is not a bypass of evidence — the same `tests-pass` and `policy-compliant` gates still gate the transition. It is a classification of "no UAT surface exists in this cycle scope".

## Verify Verdict Recap

| Field | Value |
|---|---|
| Verify verdict | **PASS_WITH_WARNINGS** |
| Required scenarios | 8 / 8 COMPLIANT |
| Commands passed | 5 / 5 (cargo test ast_lift, cargo test extractor, cargo clippy, cargo fmt, forbidden-I/O grep) |
| Critical findings | 0 |
| Warnings | 1 (`ast-lift-loc-cap`, low, isolated to `#[cfg(test)]` submodules in `ast_lift.rs`) |
| Suggestions | 0 |
| Findings classified false_positive | 2 (`pre-existing-feature-gate`, `unused-import-conformance`) |

The lone warning is **not blocking**: the LOC breach (`ast_lift.rs` = 845 vs M5.1 cap ~545) is fully attributable to WU3's 5 conformance tests + 2 helpers under `#[cfg(test)]::tests::statement_lift` and `tests::real_source`, isolated from production code, and explicitly forecasted in `tasks.md`. Owner for follow-up split into `ast_lift_tests.rs`: `debt-verify` (post-archive, when test count grows further).

## Per-Commit Changelog

Per `prompts/sddk/phases/release.md` Step 5, the full subject-line changelog across the 7 commits (`90edff1f..HEAD`):

| # | SHA | Author | Date (ISO-8601) | Subject |
|---:|---|---|---|---|
| 1 | `28dce3972bd78079428f4fb25eff40f0666d2539` | Ruben | 2026-09-14T22:29:00+02:00 | `feat(core): WU1 — ExtractionResult.statement_by_function + Rust statement walker (M5.1b)` |
| 2 | `6b8bee7fa7077f833a5ec294cb85659de115e676` | Ruben | 2026-09-14T22:36:04+02:00 | `feat(core): WU2 — ast_lift injects statements from map + 2 module tests (M5.1b)` |
| 3 | `23d7839eef70e573dd6f661d9625b56d577aa7cd` | Ruben | 2026-09-14T22:50:20+02:00 | `feat(core): WU3 — 5 conformance tests for M5.1b statement extraction (M5.1b)` |
| 4 | `47b25c8662bb22a45b817655fdc737611385c7c1` | Ruben | 2026-09-14T22:50:44+02:00 | `docs: M5.1b — implementation-receipt + verification-report + openspec artifacts` |
| 5 | `448d81bd1520d81c0c92abedb7ba835eca65000a` | Ruben | 2026-09-14T23:06:52+02:00 | `fix(core): remove unused let-bindings in 2 conformance tests (M5.1b)` |
| 6 | `2f1f638a6712299abc26766dca47b2f5c8a8996f` | Ruben | 2026-09-14T23:07:10+02:00 | `docs(m5-1b): update implementation-receipt with orchestrator re-check` |
| 7 | `6bd72e789a04922e22e3bc282884c9f7d2af36a0` | Ruben | 2026-09-14T23:22:19+02:00 | `docs(m5-1b): SDDK verify report + findings (PASS_WITH_WARNINGS, 4/4 gates passed)` |

## Receipts and Gate Outputs

Gate evaluation and transition will be performed against this report (acting as `merge-receipt` evidence bundle) and the annotated remote tag (`release-receipt` evidence bundle).

| Receipt | Evidence basis | Gate(s) backed |
|---|---|---|
| `merge-receipt` | `git push origin main` exit 0 + `HEAD == origin/main` after push + digest `25063040…` | `no-pending-effects` (subset) |
| `release-receipt` | annotated tag `m5-1b-statement-extraction` peeling to `6bd72e78…`, verified remotely | `no-pending-effects` (subset) |

## Next Recommended Step

`sddk-archive` to consume this release-receipt, sync durable specs/knowledge, generate the closing HTML, and apply `archive.complete` with the archive manifest bound to the release-receipt (chain: `release-receipt` → `archive-manifest`).
