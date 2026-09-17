# e86-followup — `cogh update` live install path

> Followup to e86 (M2 — cogh lifecycle). Operational authority: `state.yaml`.

## Why

e86 wired `cmd_latest` and `cmd_update --dry-run` to the new
`lifecycle_resolver`, but the **non-dry-run** path of `cmd_update` —
the one that downloads the manifest and runs `install::run_install`
end-to-end — has no test coverage. The verification report flagged it
explicitly:

> `cmd_update` real download (today dry-run + the resolve-then-write
> plumbing is fully tested; the e2e install pipeline is exercised by
> the live install test in `release_test_support`).

That claim was optimistic. The `release_test_support::LocalRelease`
helper exercises the `point_at` seam (`ENV_BUNDLE_MANIFEST` +
`ENV_RELEASE_BASE_URL`), which is the OLD install path. The NEW path
goes through `lifecycle_resolver::resolve_release` first and writes to
`~/.cognicode/bundle.yaml`. The two paths share the install backend
but have different upstreams, and the new upstream is unverified.

## Scope

In scope:

1. **`ResolverFixture` in `release_test_support.rs`** — a builder that
   produces a real `LocalRelease`, serves it over loopback, AND emits
   a `releases.json` fixture pointing at it. The fixture serves both
   shapes: `lifecycle_resolver::resolve_release(staging_dir=...)` and
   the existing `point_at` seam, so a future migration can drop the
   old path without re-doing the test infrastructure.

2. **End-to-end test of `cmd_update` non-dry-run** — point the resolver
   at the fixture, run `cmd_update`, assert:
   - `~/.cognicode/bundle.yaml` is written with the resolved manifest
   - `~/.cognicode/journal/<version>.json` is written
   - `~/.cognicode/tracker/version` is updated
   - `~/.cognicode/install/<version>/manifest.yaml` is written
   - `~/.cognicode/install/<version>/<component>` exists (the install
     actually placed the binaries)
   - the manifest's components match the published_components list

3. **End-to-end test of `cogh rollback` after a live install** —
   re-uses the install from step 2, runs rollback, asserts the
   install manifest and journal are gone.

Out of scope:

- Real GitHub API calls (the fixture covers the network shape).
- Tier-2 platforms (macOS, Windows) — Tier-1 only.
- Multi-component / multi-profile tests — single profile only.
- e87 (mise/aqua) — separate cycle.

## Exit gates

- `cargo test -p cognicode-cli --bin cogh` passes all of:
  - the existing 171 tests
  - 1 new ResolverFixture construction test
  - 1 new cmd_update live install test
  - 1 new cmd_rollback after live install test
- `cargo fmt --check --package cognicode-cli` exits 0
- `cargo check --workspace --all-targets` exits 0
- `just check-known-failures` exits 0
- Live smoke: `cogh update --base-url http://127.0.0.1:<port>` against
  the fixture completes a real install

## Apply plan

T1: Add `ResolverFixture::build(version)` in `release_test_support.rs`.
   - Reuses `LocalRelease::local_release()` for the real payloads.
   - Writes `<dir>/releases.json` in the shape the resolver expects,
     with the manifest asset URL rewritten to the loopback base.
   - Returns the staging dir + the LocalRelease (kept alive).

T2: Add `cmd_update_live_install_against_fixture` test.
   - Builds a `ResolverFixture`.
   - Sets `COGNICODE_HOME` to a temp dir (via `TempCognicodeHome`).
   - Calls `cmd_update` non-dry-run, passing the staging dir.
   - Asserts the side-effects listed above.

T3: Add `cmd_rollback_after_live_install` test.
   - Builds a `ResolverFixture`.
   - Runs the full install via `cmd_update`.
   - Calls `cmd_rollback`.
   - Asserts the install manifest and journal are gone, the tracker
     is gone (it was inside the journal's reversal scope).

T4: Exit gates (fmt, check, kf, smoke).

## Risk

`installer_transaction::load_bundle_manifest` reads `~/.cognicode/bundle.yaml`
first, falling back to the dev fixture. The new `cmd_update` writes to that
exact path. If `cmd_update` is interrupted after the write but before the
install starts, the next `cogh install` would re-run against this manifest.
That is benign (re-runs are idempotent at the install level) but the test
must be tolerant of the `install_lock::acquire_lock` ordering — a parallel
test that holds the lock will deadlock. Mitigation: `#[serial]` everywhere.
