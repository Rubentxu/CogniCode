# Verification report — e86-followup-live-update

**Cycle:** e86-followup-live-update
**Date:** 2026-09-17
**Author:** orchestrator (SDDK AUTO-RUN)
**Branch:** main
**HEAD at archive push:** `bfa819e64f38bfcdad3f3a4ad1a8e6b3ad4f5a1d`
**HEAD == origin/main:** verified at archive time.
**Cycle commit range:** `9d446974` (feat) → `de22c9ca` (archive) → `bfa819e6` (docs pin)

> The HEAD SHA above is the value of `main` at the moment this report
> was archived. Any subsequent commits (e.g. doc fixes after a re-read)
> will advance HEAD without invalidating this report — the cycle
> deliverables (174/174 tests passing, exit gates green, regression
> pinned) are all captured in commits `9d446974` and `de22c9ca`. The
> docs-pin commit `bfa819e6` is a follow-up edit only.

## Scope

The e86 verification report flagged one outstanding gap: the
non-dry-run path of `cmd update` had no test coverage. The legacy
seam (`release_test_support::LocalRelease` + `point_at`) exercises the
`COGNICODE_BUNDLE_MANIFEST` env var, NOT the `lifecycle_resolver` that
e86 introduced. This followup closed that gap.

## What changed

### T1 — `ResolverFixture::build(version)` in `release_test_support.rs`

- Added `pub serve_root: PathBuf` to `LocalRelease` so the fixture can
  place files under the served `v{version}/` URL prefix.
- New `pub struct ResolverFixture { release, staging_dir }` and
  `ResolverFixture::build(version) -> Result<Self>`:
  - Reuses `LocalRelease::local_release(version)` for the loopback
    payload.
  - Copies the canonical bundle manifest under
    `serve_root/v{version}/<manifest-name>` so `cmd update`'s GET on
    the manifest URL lands on 200. The HTTP server only serves
    `serve_root/v{version}/`; without this copy, the GET 404s and
    `cmd_update` aborts.
  - Writes a GitHub-API-shaped `releases.json` under the fixture's
    staging dir. The asset URL is rewritten to point at the loopback
    so the resolver's `--base-url` rewrite is not needed for staging
    tests.
- New `resolver_fixture_emits_valid_releases_json` test that runs
  `resolve_release` against the fixture and asserts the
  `manifest_url` starts with the loopback base.

### T2 — `cmd_update` live install

- New `cmd_update_live_install_against_fixture` in `layout::tests`.
- Uses three RAII env guards so the install runs in a hermetic
  environment without touching the host:
  - `TempCognicodeHome` (existing) — sets `COGNICODE_HOME` to a
    fresh tempdir.
  - `TempBaseUrl::set(url)` (new) — sets `COGNICODE_RELEASE_BASE_URL`
    so the install pipeline rewrites canonical github.com component
    URLs onto the loopback. **Also** clears `COGNICODE_BUNDLE_MANIFEST`
    on the way in and restores it on the way out. The clear is
    required because `installer_transaction::tests::advance_skips_through_all_stages`
    calls `point_at` without `unpoint`, leaving the env var pointing
    at a tempdir path that has been cleaned up. `load_bundle_manifest`
    prefers the env var over `bundle.yaml`, so a stale path crashes
    the next install.
  - `TempOpenCodeConfig::disable()` (new) — sets `OPENCODE_CONFIG` to
    a non-existent file so `ide::detect_opencode()` returns false.
    Without this, the install would try to symlink the freshly
    installed mcp-server into the user's real
    `~/.config/opencode/skills/`, polluting the host filesystem.
- Asserts post-install state:
  - `bundle.yaml` exists and contains "0.95.0".
  - `install_manifest_path("0.95.0")` exists.
  - `tracker_version()` reads "0.95.0".
  - `lifecycle_journal::journal_path("0.95.0")` exists.

### T3 — `cmd_rollback` after live install

- New `cmd_rollback_after_live_install` in `layout::tests`.
- Runs the live install from T2, then runs `cmd_rollback`.
- Asserts the rollback surfaces the populated-dir regression
  (`Directory not empty` / os error 39) and that the journal is left
  intact after the partial failure.
- The regression is **pinned, not fixed**. See "Out-of-scope findings"
  below.

## Test results

- `cargo test -p cognicode-cli --bin cogh` — **174 passed, 0 failed,
  1 ignored** (was 171 + 3 new = 174).
- `cargo fmt --check --package cognicode-cli` — clean.
- `cargo check --workspace --all-targets` — exit 0.
- `just check-known-failures` — 41-entry baseline intact.
- Live `cogh latest --json` smoke — green, returns v0.95.0.

## Observations / honesty

1. **Pre-existing diffs left untouched.** `docs/adr/README.md` and
   `openspec/changes/archive/2026-09-17-e86-cogh-lifecycle/state.yaml`
   both show diffs in `git status`. Neither was touched in this
   cycle. The README diff is the same one flagged in the e86
   verification report. The archive state.yaml diff is the e86 close.
   Both are not mine; not part of the cycle.

2. **Rollback populated-dir regression pinned, not fixed.** See the
   next section.

3. **`CognicodeHome::install_manifest_path` is inconsistent with the
   env-path version.** The method on `CognicodeHome` produces
   `<root>/<version>/manifest.yaml`, but the install transaction
   (which uses the env-based free fn `layout::install_manifest_path`)
   writes to `<root>/install/<version>/manifest.yaml`. The new T2
   test uses the free fn to stay pinned to the real install contract.
   Fixing the method to match the env path would require auditing
   every caller — out of scope here.

4. **`LocalRelease.serve_root` made `pub`** — promotes a private
   field to public API. `LocalRelease` is in `cmd::release_test_support`
   which is a test-support module, so the API surface expansion is
   contained.

## Out-of-scope findings (carried forward)

### Bug: rollback of a populated `CreatedDir` fails

`rollback_journal::RollbackJournal::rollback` reverses each recorded
`SideEffect`. For `CreatedDir(path)`, it calls `std::fs::remove_dir(path)`
— which fails on a non-empty directory. The legacy e86 T7 test
(`cmd_rollback_reverses_a_committed_install`) only recorded
`WroteManifest`, so `rmdir` succeeded on an empty `install/<version>/`.

A real install records additional side-effects:

- `Extracted(component_path)` — extracts a tarball into
  `install/<version>/<component>/`. The extraction itself is NOT
  journaled as individual `WroteFile` entries.
- `Downloaded(cache_path)` — leaves a tar.gz in `cache/`.

So when the rollback tries to `rmdir install/<version>/` and
`rmdir cache/`, both fail with `ENOTEMPTY`. The rollback aborts
mid-way and leaves the journal file on disk.

**Reproduction:** `cargo test -p cognicode-cli --bin cogh -- --nocapture cmd_rollback_after_live_install`.

**Fix (sketch, for a future cycle):**

1. Walk each `CreatedDir` recursively and remove contents before the
   `rmdir`.
2. OR record every write (`WroteFile(path)`) and every extraction as
   individual `WroteFile` entries, then roll back file-by-file.
3. OR keep `CreatedDir` only for directories that are guaranteed to be
   empty (i.e. drop `CreatedDir` from the journal entirely and rely on
   the recursive removal of the install root).

This is a real bug that affects every user running `cogh rollback`
after a successful install. It must be addressed before e86 can be
called complete.

## Exit gate verdict

| Gate | Status |
|------|--------|
| `ResolverFixture::build` emits a `releases.json` the resolver accepts | PASS |
| `cmd_update` non-dry-run installs binaries + writes journal + tracker | PASS |
| `cmd_rollback` after a live install surfaces the regression (pinned) | PASS (regression pinned, not fixed) |
| 174/174 cogh tests pass | PASS |
| `cargo fmt --check` on `cognicode-cli` | PASS |
| `cargo check --workspace --all-targets` | PASS |
| `just check-known-failures` (41-entry baseline) | PASS |
| Live `cogh latest --json` smoke | PASS |

**Cycle verdict:** scope met. The followup gap from the e86
verification report is closed: the new resolver-driven install path has
real end-to-end test coverage. One pre-existing rollback bug is pinned
for a future cycle.
