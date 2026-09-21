# Tasks — roadmap-auto-discovery

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/roadmap-auto-discovery` |
| Path | a-min |
| Phase | tasks (advance to build) |
| Date | 2026-09-20 |

## Task dependency graph

```
T0 — Verify SDDK tooling + ledger state (DONE in this session)
T1 — Open cycle `archive-sync-lsi-m6-m7`
T2 — Execute cycle 1 (archive sync)
T3 — Open cycle `clippy-hygiene-181-bounded` (path b-direct)
T4 — Execute cycle 2a (84 mechanical warnings)
T5 — Execute cycle 2b (19 cognicode-core warnings)
T6 — Execute cycle 2c (72 dead_code) — STOP for user classification
T7 — Open cycle `release-v0.97.2-prep` (path a-min)
T8 — Execute cycle 3 (release prep, NO tag cut)
T9 — Verify initiative-level acceptance criteria
T10 — sddk-archive the umbrella initiative
```

## T0 — Verify SDDK tooling + ledger state (DONE)

- `sddk version` → 1.169.93 source: current, present: true
- `sddk adopt status` → complete, ledger in
  `~/.local/state/sddk/projects/p-c1fac1fea05615c6/ledger.sqlite`
- `sddk cycle status` (initial) → no active cycle
- Verified HEAD = `ba8897b5`, origin/main = `ba8897b5`
- Verified release tags `v0.94.11..v0.97.1`, workspace v0.97.1,
  lockstep rule active.
- Verified `.agent/h4.7-evidence/` + `openspec/changes/roadmap-auto-discovery/explore-report.md` exist.

## T1 — Open cycle `archive-sync-lsi-m6-m7`

```bash
sddk cycle start --root . --scope . \
  --name "archive-sync-lsi-m6-m7" \
  --path "b-direct" \
  --branch "main" \
  --base "ba8897b5"
```

**Cycle intent** (proposal already in this initiative's folder; this
sub-cycle will reuse umbrella's explore-report for context but produce
its own artifacts):

```text
product_objective: ensure every LSI M6/M7 closure change folder has an
  archive-manifest.md that reflects the D3-DEFER closure pattern.
scope: openspec/changes/{e65,e66,e67,e68,e69,e70,e71,e72,e73,e74,e75,e76,
  e77,e79,lsi-final-umbrella-reconciliation}/*/archive-manifest.md
allowed_components:
  - openspec/changes/*/archive-manifest.md (write only if missing or stale)
ordered_milestones:
  M1 — Enumerate closures needing manifest update (T1.1)
  M2 — For each, write/update archive-manifest.md (T1.2)
  M3 — Implement commit + receipt + archive commit
acceptance_criteria: every LSI M6/M7 folder has archive-manifest.md
required_tests: file existence enumeration only (no code change)
integration_policy: batched push at boundary 3
stop_conditions: code change attempted → STOP
final_deliverables: N file updates + 2 commits (impl + archive) + receipt
```

## T2 — Execute cycle 1

Inside `archive-sync-lsi-m6-m7`:
- `phase.explore.complete` (gate: exploration-sufficient; artifact:
  explore-report.md reused from umbrella or generated)
- `phase.design.complete` (skipped in b-direct)
- `phase.tasks.complete` (gate: tasks-testable; artifact: tasks.md)
- `phase.build.complete` (gate: implementation-complete; artifact:
  implementation-receipt.md)
- `phase.verify.complete` (gate: verification-passed; artifact:
  verification-report.md)
- `cycle.archive` (one archive commit + receipt)

Each cycle uses `sddk cycle transition` with appropriate `--gate-receipt`
and `--artifact` per the SDDK workflow.

## T3 — Open cycle `clippy-hygiene-181-bounded`

```bash
sddk cycle start --root . --scope . \
  --name "clippy-hygiene-181-bounded" \
  --path "b-direct"
```

Cycle intent:

```text
product_objective: reduce 181 cosmetic clippy warnings to 0 without
  waivers, allow attributes, or threshold changes.
scope:
  - crates/cognicode-cli/src/**/*.rs (153 warnings)
  - crates/cognicode-core/{src,tests}/**/*.rs (19 warnings)
  - other crates (~9 warnings)
allowed_components: as above
ordered_milestones:
  M1 — Sub-batch 2a: 84 mechanical warnings (committed)
  M2 — Sub-batch 2b: 19 cognicode-core warnings (committed)
  M3 — Sub-batch 2c: 72 dead_code → STOP for user classification
acceptance_criteria:
  - cargo clippy --workspace --all-targets EXIT=0 with 0 warnings
  - 0 #[allow(clippy::*)] added in this cycle
  - 0 clippy.toml/.clippy.toml thresholds changed
required_tests:
  - cargo test --workspace EXIT=0
  - python3 scripts/check_known_failures.py --tmpdir-root /tmp EXIT=0
  - H4 regression: continuation_e2e 5/5 PASS
integration_policy: batched push at boundary 3
stop_conditions:
  - any waiver/allow/threshold change attempted → STOP
  - cargo clippy --fix touched files outside scope → STOP and revert
final_deliverables:
  - 2a: 1 commit with mechanical fixes (≤ 84 changes)
  - 2b: 1 commit with cognicode-core fixes (≤ 19 changes)
  - 2c: STOPPED (awaiting user classification)
```

## T4 — Execute sub-batch 2a

- Identify 84 mechanical warnings via catalog
  (`.agent/h4.7-evidence/clippy-catalog-181-warnings.txt`).
- Apply corrections strictly per clippy's auto-suggestion.
- Verify with `cargo clippy --workspace --all-targets` and `cargo test`.
- Commit with conventional message.

## T5 — Execute sub-batch 2b

- Same pattern as 2a but in cognicode-core.
- Likely affects test code (low risk).

## T6 — Execute sub-batch 2c (BLOCKED)

- STOP at cycle boundary.
- Surface to user: "72 dead_code warnings need per-warning classification
  (delete vs. preserve vs. allow). Which do you want?"

## T7 — Open cycle `release-v0.97.2-prep`

```bash
sddk cycle start --root . --scope . \
  --name "release-v0.97.2-prep" \
  --path "a-min"
```

Cycle intent:

```text
product_objective: prepare the conditions for the next ceremonial release
  without cutting the tag (human gate reserved).
scope:
  - CHANGELOG.md (entry draft)
  - .sddk/releases/v0.97.2/ (template)
  - scripts/release_scorecard.py (read-only execution)
allowed_components: as above
ordered_milestones:
  M1 — Enumerate commits since v0.97.1 (`git log v0.97.1..HEAD`)
  M2 — Draft CHANGELOG entry from commit list
  M3 — Run scorecard pre-tag, capture receipt
  M4 — Produce release-receipt-v0.97.2.json (template, no apply)
acceptance_criteria:
  - CHANGELOG entry drafted
  - Scorecard receipt captured
  - Release receipt template ready
  - NO tag cut attempted
required_tests: scorecard + known_failures baseline
integration_policy: NO push, NO tag, all local-only documentation
stop_conditions:
  - any tag cut attempted → STOP
  - any push attempted → STOP
final_deliverables:
  - CHANGELOG.md draft
  - release-receipt template
  - scorecard receipt
```

## T8 — Execute cycle 3

Inside `release-v0.97.2-prep`:
- All phases local, no remote side effects.
- Final state: documents and receipts only, no tags.

## T9 — Verify initiative-level acceptance

Re-run all gates:
- `cargo fmt --check` EXIT=0
- `cargo check --workspace --all-targets` EXIT=0
- `cargo clippy --workspace --all-targets` EXIT=0 with ≤ 181 warnings
  (depending on cycle 2 progress)
- `cargo test --workspace` EXIT=0
- `python3 scripts/check_known_failures.py --tmpdir-root /tmp` EXIT=0
- H4 regression: `cargo test -p cognicode-mcp --test continuation_e2e` 5/5
- Working tree clean (except gitignored files)
- No AI attribution trailers, no waivers, no `allow` attributes
  introduced.

## T10 — Archive umbrella initiative

- `sddk sddk-archive` on `p-c1fac1fea05615c6/roadmap-auto-discovery`
- One archive commit for the umbrella.
- Surface final report to user.

## Boundary acknowledgements

| Boundary | Authorisation needed |
|----------|----------------------|
| B1 (initiative definition) | This proposal is the boundary. User authorised by saying "crea una iniciativa" |
| B2 (each cycle start) | One `sddk cycle start --name <X>` per cycle. **The orchestrator requests user GO before starting each** |
| B3 (batched push) | One `git push origin main` per batch. **User GO before each push** |
| Tag cut | Human gate, separate from this initiative |

## Parallelism

Cycles 1, 2, 3 are **sequential** by design:
- Cycle 1 modifies OpenSpec artifacts only.
- Cycle 2 modifies Rust source only.
- Cycle 3 modifies CHANGELOG and receipts.

There is no file-level conflict, but for safety they are run in order
so verification at T9 covers all changes.

## Stop / hold policy

- If any cycle's gate fails, STOP that cycle, document the failure,
  surface to user, wait for direction.
- If user explicitly cancels any cycle, archive it as superseded.
- If commits accumulate without user push authorisation, continue
  per initiative but never push without authorisation.

## Open questions for user

1. Cycle 2c (dead_code): include now or defer?
2. Cycle 3 (release prep): include in this initiative or separate?
3. Boundary 2: do you want a single GO for all 3 cycles, or per-cycle GO?
4. Push batching: after how many cycles do you want a push gate
   (suggestion: after each cycle, so push is small and reversible)?
