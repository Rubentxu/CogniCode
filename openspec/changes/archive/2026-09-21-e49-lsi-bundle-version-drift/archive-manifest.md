# Archive Manifest — e49 — bundle version drift (root-cause fix)

> Cycle: A-min | Phase: archive | Date: 2026-09-15

## Cycle summary

| Property | Value |
|----------|-------|
| Change ID | `e49-lsi-bundle-version-drift` |
| Path | A-min |
| Phases completed | explore → spec → tasks → apply (4 WUs) → verify |
| Final status | **ARCHIVED** |
| Base SHA | `dcca25ea` (post-e48 archive) |
| Diff stat | **+86 / -9** across **3 files** (implementation) + 5 OpenSpec artifacts |
| Verify verdict | **PASS** |

## Artifacts

| Artifact | Path |
|----------|------|
| Exploration report | `openspec/changes/e49-lsi-bundle-version-drift/exploration-report.md` |
| Spec | `openspec/changes/e49-lsi-bundle-version-drift/spec.md` |
| Tasks | `openspec/changes/e49-lsi-bundle-version-drift/tasks.md` |
| Verification report | `openspec/changes/e49-lsi-bundle-version-drift/verification-report.md` |
| Implementation | commit (this cycle) — `fix(cogh): derive embedded bundle path from CARGO_PKG_VERSION` |

## What was closed

Two independent defects that both produced a spurious
"you are running a mismatched installer (CLI upgrade required)" error on
any clean-home `cogh install`:

### RC-1 — version-drifted embedded bundle (production defect)

`InstallerTransaction::load_bundle_manifest()` embedded a hard-coded
`bundles/v0.94.14/bundle.yaml` while the workspace version was `0.94.15`.
`assert_pkg_version()` therefore aborted every clean install. This drift
had already recurred once (commit `5bbeaf94` fixed `0.94.11 → 0.94.14`;
commit `eec319e2` bumped to `0.94.15` without a bundle).

**Fix**: derive the embedded path from `CARGO_PKG_VERSION` at compile
time so the crate version is the single source of truth, and add the
missing `bundles/v0.94.15/bundle.yaml`. Drift now becomes a **compile
error** (proved in V-4 of the verification report).

### RC-2 — stale binary in lifecycle subprocess tests (test-infra defect)

`cogh_bin()` hard-coded `<workspace>/target/debug/cogh`. Under the
environment's custom `CARGO_TARGET_DIR` (`/var/home/rubentxu/cargo-targets`)
this resolved to a stale 2026-08-16 binary that still embedded the
drifted `v0.94.14` bundle, so the subprocess-based lifecycle tests
silently exercised the wrong binary and masked RC-1.

**Fix**: derive the binary path from `std::env::current_exe()` (the
unit-test executable lives in `<target-dir>/debug/deps/`, so the
companion `cogh` is two directories up).

### Regression test added

`embedded_bundle_version_matches_pkg_version` (in
`installer_transaction.rs::tests`) parses the embedded manifest and
asserts its version equals `CARGO_PKG_VERSION` and satisfies
`assert_pkg_version()`. Closes the gap where a bundle directory exists
but its internal `version:` field was not bumped.

## Verification outcome

| Metric | Pre-e49 (serial) | Post-e49 (serial) | Δ |
|--------|------------------|-------------------|---|
| `cargo test -p cognicode-cli -- --test-threads=1` | 98 passed; 2 failed; 1 ignored | **101 passed; 0 failed; 1 ignored** | ✅ −2 failures, +1 new test |
| integration suites | 34 passed | 34 passed | stable |
| `cargo test -p cognicode-cli ide::tests` | 20 passed; 0 failed | 20 passed; 0 failed | stable |

PASS verdict: all four spec requirements are satisfied; the CLI suite
is fully green under serial execution.

## Known residual (out of scope)

`cargo test -p cognicode-cli` under **default parallelism** still shows
3-4 variable failures in `install_lock::tests` and a few
`lifecycle::tests` / `ide::tests` entries. These are the pre-existing
process-global-state race (documented by e48): `#[serial]` only
serialises the annotated tests among themselves, so un-annotated tests
that mutate `HOME` / `COGNICODE_HOME` / the lock path still race. This is
orthogonal to the version drift fixed here and is a candidate for a
future cycle.

Also noted: pre-existing `cargo fmt --check` drift in
`tests/cogh_cli.rs` and `tests/cognicode_ide_adapter.rs` (unrelated).

## Why no tag

The user explicitly froze v1.0.0 tag cuts (`nada de tag 1.0.0, tenemos
que acabar el roadmap`). While this cycle fixes a production defect, it
is not a release: no public API change, no new capability, and the
roadmap is still in progress. A tag here would add noise without
information.

## No delta-spec

This cycle fixes behaviour to match the already-specified contract
(co-versioned bundle, `assert_pkg_version` as a hard gate). It does not
add or change a spec capability, so no delta-spec is required.

## Verification command

```
cargo test -p cognicode-cli -- --test-threads=1
```

Returns `101 passed; 0 failed; 1 ignored`. ✅
