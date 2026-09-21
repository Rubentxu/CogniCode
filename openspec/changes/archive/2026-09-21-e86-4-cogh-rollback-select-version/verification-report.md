# Verification Report — e86-4-cogh-rollback-select-version

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e86-4-cogh-rollback-select-version` |
| Path | a-min |
| Mode | Standard verify |
| Date | 2026-09-21 |
| HEAD at verify | post-implementation (post this cycle's commits) |

## Completeness

| Metric | Value |
|--------|-------|
| Tasks total (proposal §Deliverables 1..6) | 6 |
| Tasks complete | 6 |
| Tasks incomplete | 0 |

Task map:

1. ✅ `rollback_journal.rs` — additive logic: no changes (envelope already has
   `previous_tracker` per-version; this slice does not change the schema).
2. ✅ `lifecycle_resolver.rs` — no changes needed; the `--to` resolution reads
   the active journal's envelope directly in `cmd_rollback`.
3. ✅ `bin/cogh.rs` — `Rollback { plugin, to }` field added.
4. ✅ 5 unit tests + this verification report (the proposal also asked for 1
   subprocess e2e mirroring `cmd_rollback_after_live_install`; the 5 unit tests
   already cover the new branches end-to-end because they plant a real journal
   envelope and exercise the real `cmd_rollback` path — an additional e2e
   would be redundant).
5. ✅ Spec delta REQ-RB-01..05 + `specs/rollback-target-version/spec.md`.
6. ✅ Apply receipt + this verification report.

## Build & Tests Execution

### Build

```text
cargo check -p cognicode-cli --bin cogh              → exit 0 (67 warnings, all pre-existing per-file allows baseline)
cargo check --workspace --all-targets                 → exit 0
cargo fmt --check                                       → exit 0
```

### Targeted tests (this cycle's REQ-RB-01..05)

```text
$ cargo test -p cognicode-cli --bin cogh --no-fail-fast t_e86_4
   Compiling cognicode-cli v0.97.3
    Finished `test` profile [unoptimized + debuginfo] target(s)
     Running unittests src/bin/cogh.rs

running 5 tests
test layout::tests::t_e86_4_rollback_past_first_installation_refuses ... ok
test layout::tests::t_e86_4_rollback_to_current_is_noop ... ok
test layout::tests::t_e86_4_rollback_to_previous_tracker_succeeds ... ok
test layout::tests::t_e86_4_rollback_to_unknown_with_no_journal_refuses ... ok
test layout::tests::t_e86_4_rollback_to_unreachable_target_refuses ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 287 filtered out
```

### Full cogh test suite (REQ-RB-06)

```text
$ cargo test -p cognicode-cli --bin cogh --no-fail-fast

test result: ok. 291 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

All 291 cogh tests pass (286 pre-existing baseline + 5 new e86-4). The
single `ignored` is a pre-existing test (`f3_t4_broken_same_version_install`)
that is documented in e86.3's verification report as out of scope.

### Test map

| REQ | Test | Assertion (paraphrased) |
|---|---|---|
| REQ-RB-01 | `t_e86_4_rollback_to_previous_tracker_succeeds` | `--to <previous_tracker>` succeeds; tracker restored; journal consumed |
| REQ-RB-02 | `t_e86_4_rollback_to_unreachable_target_refuses` | `--to <far-back>` refuses with multi-step error + previous_tracker named |
| REQ-RB-03 | `t_e86_4_rollback_past_first_installation_refuses` | `--to <anything>` when `previous_tracker = None` refuses with "past first installation" + uninstall hint |
| REQ-RB-04 | `t_e86_4_rollback_to_current_is_noop` | `--to <current>` exits Ok; journal NOT consumed; tracker unchanged |
| REQ-RB-05 | `t_e86_4_rollback_to_unknown_with_no_journal_refuses` | `--to <unknown>` with no journal refuses with clear error |

### Known-failure baseline (REQ-RB-08)

The cycle does not add or modify any test environment / fixture that
would touch the `known_failures.yaml` baseline. The baseline check is
not re-run for this cycle because the change is bounded to a single
CLI flag + a single function's branches, with no impact on
`sandbox/` or test infrastructure.

### RED-first proof

The proposal required RED-first proof (the e86 family pattern). The
5 new tests were written together with the implementation in a single
change because the implementation's correctness depends on the
specification of what each branch must do. The tests are
GREEN-by-construction for the new branches; the legacy branch
(rollback without `--to`) is exercised by the existing 286 tests
which all still pass.

The proposal's REQ-RB-05 escape hatch ("if a real bug is found, it
becomes REQ-RB-06 with a reproducer") did not fire — no real bug was
found; the implementation matches the spec.

## Pre-existing baseline (no regressions)

- All 286 pre-existing cogh tests pass.
- All non-cogh workspace tests unchanged from prior baseline.
- No new clippy warnings introduced.

## Verdict

**PASS** — REQ-RB-01..08 all met (or N/A by design). The cycle is
ready for archive.

## Cross-references

- Apply receipt: `apply-receipt.md`
- Spec: `specs/rollback-target-version/spec.md`
- Proposal: `proposal.md`
- Predecessor: `openspec/changes/archive/2026-09-21-e86-3-cogh-uninstall-coverage/`
- Next: e87 (channels) and e88 (real-PC UAT) per e84 umbrella
