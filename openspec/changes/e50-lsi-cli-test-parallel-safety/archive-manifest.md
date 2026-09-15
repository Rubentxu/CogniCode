# Archive Manifest — e50 — CLI test parallel safety

> Cycle: A-min | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e50-lsi-cli-test-parallel-safety` |
| Path | A-min |
| Phases completed | explore → spec → tasks → apply (5 WUs) → verify |
| Final status | **ARCHIVED** |
| Base SHA | `58266dcd` (post-e49 archive) |
| Diff stat | **+141 / -83** across **8 files** (test-only) |
| Verify verdict | **PASS** |
| Acceptance | **0 failures / 40 default-parallel runs** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e50-lsi-cli-test-parallel-safety/exploration-report.md` |
| Spec | `openspec/changes/e50-lsi-cli-test-parallel-safety/spec.md` |
| Tasks | `openspec/changes/e50-lsi-cli-test-parallel-safety/tasks.md` |
| Verification report | `openspec/changes/e50-lsi-cli-test-parallel-safety/verification-report.md` |
| Implementation | commit (this cycle) — `test(cognicode-cli): isolate env-mutating tests for parallel safety` |

## What was closed

Five independent test-isolation defects that made
`cargo test -p cognicode-cli` flaky under default parallel execution:

1. **D-1** — env-mutating tests not serialised. Added 19 `#[serial]`
   annotations (35 total across the crate) so every test that mutates or
   reads process-global `HOME` / `COGNICODE_HOME` is mutually exclusive.
2. **D-2** — `registry` sha256 tests shared one fixed temp path. Each
   now uses its own `tempfile::tempdir()`.
3. **D-3** — IDE-adapter tests created and exec'd a shell wrapper, which
   intermittently returned `ETXTBSY` ("Text file busy"). Replaced with
   `Command::env("HOME", fake_home)`; the wrapper is gone.
4. **D-4** — `install_lock` and `installer_transaction` tests resolved the
   real `~/.cognicode`. A shared
   `layout::test_support::TempCognicodeHome` helper redirects
   `COGNICODE_HOME` to a temp dir, making them hermetic and race-free.
5. **D-5** — `detect_opencode_finds_config` asserted on the developer's
   real `~/.config/opencode/opencode.json`. Rewritten to build a stub
   config under a private temp `HOME`.

## Verification outcome

| Metric | Pre-e50 | Post-e50 | Δ |
|--------|---------|----------|---|
| `cargo test -p cognicode-cli` (parallel, per run) | 3-6 variable failures | **0 failures** | ✅ deterministic |
| 40 consecutive parallel runs | n/a | 0 failures total | ✅ |
| `cargo test -p cognicode-cli -- --test-threads=1` | 101 passed; 0 failed; 1 ignored | 101 passed; 0 failed; 1 ignored | stable |
| integration suites | 34 passed | 34 passed | stable |

## Why no tag

The user froze v1.0.0 tag cuts. This is a test-quality cycle: no public
API change, no new capability.

## No delta-spec

This cycle fixes test isolation to match already-specified behaviour. It
adds no spec capability, so no delta-spec is required.

## Verification command

```
cargo test -p cognicode-cli
```

Returns `101 passed; 0 failed; 1 ignored` (unit) with all integration
suites green, and is clean across 40 consecutive default-parallel runs. ✅
