# Tasks: E38 LSI Stable Entity Identity

> Auto-chain, stacked-to-main: WUs in-tree, revertable, commits/PRs await approval; e38 = NEW files + additive-only kernel edits; e37 evidence untouched (shared tree).

## Review Workload Forecast

|Field|Value|
|---|---|
|Estimated changed lines|~1100–1400|
|400-line budget risk|High|
|Chained PRs recommended|Yes|
|Suggested split|one PR per WU (PR1–PR5)|
|Delivery strategy|auto-chain|
|Chain strategy|stacked-to-main|

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

|Unit|Goal|Likely PR|Focused test command|Runtime harness|Rollback boundary|
|---|---|---|---|---|---|
|1|Continuity core|PR1|`cargo test -p cognicode-core --lib evidence_kernel --features evidence-kernel`|N/A|revert `ids.rs`/`mod.rs`; rm `continuity/`|
|2|Rename port+git adapter|PR2|`cargo test -p cognicode-core --lib rename_evidence --features evidence-kernel`|throwaway git repos|revert `ports.rs`+`git/mod.rs`; rm adapter|
|3|Tiered matcher T0–T3|PR3|`cargo test -p cognicode-core --lib continuity --features evidence-kernel`|permutation double-run|rm `matcher.rs`|
|4|Benchmark+fixtures+isolation|PR4|`cargo test -p cognicode-core --test workspace_isolation --test identity_benchmark --features evidence-kernel`|`just lsi-identity`|rm tests+fixtures; revert justfile|
|5|Guards+handoff|PR5|`cargo check -p cognicode-core` ±feature|`just lsi-equivalence`+`just lsi-fixtures check`|n/a verify-only|

## Phase 1: Continuity core (WU-1)

- [x] 1.1 `ids.rs` additive: `StableEntityId(u64)` Display `stable:N` + sibling derives; const `OccurrenceId::from_entity`/`to_entity`; display/serde tests.
- [x] 1.2 `continuity/view.rs`: `EntityFacts`+`SnapshotEntityView::from_facts` recovers FQN/kind from `core:defines` object+`provenance.detail`, callees/refs from facts.
- [x] 1.3 `continuity/fingerprint.rs`: tagged-multiset `fingerprint`+`similarity` (0.0 unless kind equal; Jaccard else). Scenario "Fingerprint is fact-deterministic". `continuity/mod.rs` + additive `evidence_kernel/mod.rs` re-exports; GREEN per WU-1 row, feature-off `cargo check -p cognicode-core` unchanged.

## Phase 2: Rename port + adapter (WU-2)

- [x] 2.1 RED threat-matrix tests `infrastructure/git/rename_evidence.rs`: git absent (PATH override), malformed `R<nnn>` row, non-repo dir, empty `git init` repo → empty evidence, compile-first.
- [x] 2.2 `ports.rs` additive: `FileRename`+sync `RenameEvidencePort` (Send+Sync; fail-closed).
- [x] 2.3 `infrastructure/git/rename_evidence.rs` (cfg-gate `git/mod.rs`): `git -C <root> diff --name-status --find-renames=50%` (`.arg()`); `R<nnn>\t<old>\t<new>` parse; failure→warn+empty ("Version control unavailable degrades safely"). GREEN per WU-2 row + clippy.

## Phase 3: Tiered matcher (WU-3)

- [x] 3.1 `continuity/matcher.rs`: pinned `MatcherThresholds` 0.6/0.05/0.5; `MatchTier`/`ContinuityStatus`/`ContinuityOutcome` (id None ONLY Ambiguous)/sorted `ContinuityResult`; `match_snapshots` renames-as-DATA.
- [x] 3.2 Tiers T0 exact FQN → T1 path+name+kind ("Line shift keeps stable identity") → T2 move evidence+floor ("File move keeps stable identity") → T3 Jaccard≥0.6 kind-prefiltered; New=K+1..; Terminated=before-only.
- [x] 3.3 Fail-closed: margin/epsilon ties, double-claim → `Ambiguous{candidates}`, terminal, never merged ("Colliding names fail closed"); no-resurrection; identical `ContinuityResult` under shuffled fact order (matcher sorts itself). GREEN per WU-3 row + clippy.

## Phase 4: Benchmark + fixtures (WU-4)

- [x] 4.1 RED `tests/identity_benchmark.rs`: one gate unmet → fail naming gate+value ("Missed gate fails the run").
- [x] 4.2 7 fixtures `sandbox/fixtures/lsi-identity/<case>/{before/,after/,evidence.json,expected-mapping.json}`: rename(+edit), move(+edit), line-shift (umbrella S1), colliding-names (umbrella S2), control.
- [x] 4.3 `tests/identity_benchmark/harness.rs`: gates PRECISION=0.95/RECALL=0.90/LINE_SHIFT=1.00/MOVE=0.99 pre-declared; `MATCHER_CONVENTION`+`PINNED_MATCHER_DIGEST` ≠ e37's ("Convention change requires explicit re-pin"); Ambiguous quarantined ("Ambiguous outcome reported separately"); expected-Ambiguous otherwise FAILS.
- [x] 4.4 `tests/workspace_isolation.rs`: two workspaces, identical sets; no mapping/candidate crosses, collisions=0 ("Identical workspaces do not collide"). `justfile`: `lsi-identity` fixed-argv recipe; GREEN per WU-4 row.

## Phase 5: Guards + handoff (WU-5)

- [x] 5.1 e37 reuse: `just lsi-equivalence` green; `PINNED_IDENTITY_DIGEST` `fnv1a64:efccc22e912913fe` unchanged; `just lsi-fixtures check` byte-stable.
- [x] 5.2 `cargo check -p cognicode-core` ±feature; `cargo fmt --check`; clippy `cargo clippy -p cognicode-core --lib --tests --features evidence-kernel` zero on new code (pre-existing macros/generic_graph red); never full suite.
- [x] 5.3 UAT-U20/21/22 deferred honestly (A5); `.agent/TESTING-STATE.md` handoff: fresh/stale evidence, WU1–4 commands.
