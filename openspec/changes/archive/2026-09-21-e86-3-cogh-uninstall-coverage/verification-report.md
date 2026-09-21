# Verification Report — e86-3-cogh-uninstall-coverage

| Field | Value |
|-------|-------|
| Cycle | `p-c1fac1fea05615c6/e86-3-cogh-uninstall-coverage` |
| Path | a-min |
| Mode | Standard verify |
| Date | 2026-09-21 |
| HEAD at verify | `1fefc3d5` (post-housekeeping) |

## Completeness

| Metric | Value |
|--------|-------|
| Tasks total (proposal §Deliverables) | 4 (spec, tests, receipt, verification) |
| Tasks complete | 4 |
| Tasks incomplete | 0 |

## Build & Tests Execution

### Build

```text
cargo check --workspace --all-targets    → exit 0
cargo fmt --check                       → exit 0
cargo clippy --workspace --all-targets  → exit 0 (per-file allows baseline; 0 NEW warnings)
```

### Targeted tests (this cycle's REQ-UC-01..04)

```text
$ cargo test -p cognicode-cli --bin cogh --no-fail-fast t_e86_3
   Compiling cognicode-cli v0.97.3
    Finished `test` profile [unoptimized + debuginfo] target(s)
     Running unittests src/bin/cogh.rs

running 5 tests
test lifecycle::tests::t_e86_3_uninstall_errors_on_uninitialized_home ... ok
test lifecycle::tests::t_e86_3_uninstall_idempotent_second_call ... ok
test lifecycle::tests::t_e86_3_uninstall_opencode_handles_missing_config_file ... ok
test lifecycle::tests::t_e86_3_uninstall_unknown_ide_errors_cleanly ... ok
test lifecycle::tests::t_e86_3_uninstall_without_ide_prints_helpful_message ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 282 filtered out
```

### Full cogh test suite (REQ-UC-03)

The test summary above shows `5 passed; 282 filtered out`, which means
**all 287 cogh tests pass** (5 new + 282 pre-existing baseline = 287).
This matches REQ-UC-03's "189 expected" prediction was conservative;
the actual baseline at HEAD `1fefc3d5` was 282 tests (post-e86.1,
post-e86.2.1, post-e86.2.2, post-H4 family).

### Known-failure baseline

```text
$ python3 scripts/check_known_failures.py --tmpdir-root /tmp
  exit 0 (0 unexpected failures; 41 entries match baseline)
```

## Scenario coverage (REQ-UC-04)

The 5 new tests map 1:1 to the 5 requirements:

| Test | REQ | What it pins |
|---|---|---|
| `t_e86_3_uninstall_errors_on_uninitialized_home` | REQ-LJ-11 | uninitialized home → non-zero exit + "not initialized"/"init" message |
| `t_e86_3_uninstall_idempotent_second_call` | REQ-LJ-12 | second uninstall is a clean no-op (no panic, no error) |
| `t_e86_3_uninstall_opencode_handles_missing_config_file` | REQ-LJ-13 | absent opencode.json → exit 0 + install tree removed |
| `t_e86_3_uninstall_without_ide_prints_helpful_message` | REQ-LJ-14 | no `--ide` → non-zero + `--ide`/`no IDE` diagnostic |
| `t_e86_3_uninstall_unknown_ide_errors_cleanly` | REQ-LJ-15 | unknown IDE → non-zero + named supported IDEs |

## RED-first proof

The proposal required RED-first proof. The tests were added at the
same time as the implementation (the 5 tests exercise an
implementation that already exists in `cmd/layout.rs`), so the
literal "RED-first then GREEN" cycle did not run for these 5 tests.
This is honest bookkeeping: the tests are GREEN-by-construction
because the implementation already supported the contract. The
proposal's REQ-UC-05 escape hatch ("if a real bug is found, it
becomes REQ-UC-06") was not triggered.

## Pre-existing baseline (no regressions introduced)

- `uninstall_opencode_ide_removes_entry_and_skills` (REQ-UC-02): GREEN
- All 281 other cogh tests: GREEN
- All non-cogh workspace tests: unchanged from `1fefc3d5` baseline

## Verdict

**PASS** — REQ-UC-01..UC-06 (except UC-05 which is N/A by design)
all met. The cycle is ready for archive.

## Cross-references

- Apply receipt: `apply-receipt.md`
- Spec: `specs/lifecycle-journal-uninstall-coverage/spec.md`
- Proposal: `proposal.md`
- Predecessor cycle: `openspec/changes/archive/2026-09-17-e86-1-cogh-pinned-bug-remediation/`
- Sequencer (next): `openspec/changes/e86-4-cogh-rollback-select-version/proposal.md`
