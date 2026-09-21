# Verification Report — cycle e50 — CLI test parallel safety

## Summary

| Item | Value |
|------|-------|
| Cycle | e50 — CLI test parallel safety |
| Path | A-min |
| Branch | `main` |
| Base HEAD | `58266dcd` (post-e49 archive) |
| Type | test-only (+ `#[cfg(test)]` helper) |
| Verify verdict | **PASS** |

## Defects fixed

| ID | Defect | Fix |
|----|--------|-----|
| D-1 | Env-mutating tests not serialised → races on `HOME`/`COGNICODE_HOME` | `#[serial]` on every env-touching test (19 added this cycle; 35 total) + imports |
| D-2 | `registry` sha256 tests shared `temp_dir()/cogh-sha-<pid>.bin` | each uses its own `tempfile::tempdir()` |
| D-3 | IDE-adapter tests created + exec'd a shell wrapper → ETXTBSY | `Command::env("HOME", fake_home)` on the `cogh` binary; wrapper removed |
| D-4 | `install_lock` / `installer_transaction` tests used real `~/.cognicode` | shared `TempCognicodeHome` helper redirects `COGNICODE_HOME` to a temp dir |
| D-5 | `detect_opencode_finds_config` depended on the developer's real config | hermetic stub config under a private temp `HOME` |

## Verification commands executed

### V-1 — Repeated default-parallel runs (the acceptance gate)

```
cargo test -p cognicode-cli     # run 40 times
```

Result: **0 failures / 40 runs.**

Pre-e50 this command failed 3-6 tests per run (variable set). Post-e50
it is clean, repeatably.

### V-2 — Serial run (no regression)

```
cargo test -p cognicode-cli -- --test-threads=1
```

Result:

```
unit (cogh bin):             101 passed; 0 failed; 1 ignored
tests/cogh_cli:                7 passed
tests/portable_skill_bundle:   7 passed
tests/cognicode_lifecycle:     7 passed
tests/cognicode_plugin:        5 passed
tests/cognicode_ide_adapter:   8 passed
```

### V-3 — Targeted `ide::tests`

```
cargo test -p cognicode-cli ide::tests
```

Result: **20 passed; 0 failed.**

### V-4 — Isolation spot-checks

- `grep -rn "with-home.sh\|home_wrapper" crates/cognicode-cli/tests/` →
  **no matches** (wrapper gone).
- `install_lock` / `installer_transaction` fs tests now create every
  artefact under `TempCognicodeHome::path()`; they no longer resolve the
  real `~/.cognicode`.

## Per-module `#[serial]` coverage after this cycle

| Module | env-touching tests | annotated |
|--------|--------------------|-----------|
| `ide.rs` | 9 | 10 (incl. `detect_opencode_finds_config`) |
| `lifecycle.rs` | 20 | 20 |
| `layout.rs` | 1 | 1 |
| `tracker.rs` | 1 | 1 |
| `install_lock.rs` | 2 | 2 |
| `installer_transaction.rs` | 2 | 2 |

(Excludes pure tests that neither mutate nor read env-derived state.)

## Regression / no-regression

- `cargo test -p cognicode-cli --no-run` → compiles, 0 errors.
- `cargo fmt -p cognicode-cli --check` → changed files clean.
- No production behaviour change: the only non-test addition is the
  `#[cfg(test)] pub(crate) mod test_support` helper.

## Known residual (out of scope)

- Root cause class: `cognicode_home()` reads process env, so any future
  env-touching test must remember to be `#[serial]`. A structural fix
  (thread an explicit home through the API) is a larger refactor.
- Pre-existing `cargo fmt --check` drift in `tests/cogh_cli.rs`.
- Pre-existing clippy warnings in the crate.

## Conclusion

The `cognicode-cli` suite is now deterministic under default parallel
execution (0 failures / 40 runs) and remains green serially. All five
isolation defects are fixed; the flaky tests identified before this
cycle are stabilised by construction, not by re-running.
