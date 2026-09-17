# Tasks — e86 followup live-update

## T1 — `ResolverFixture` builder + smoke test

Add `ResolverFixture` in `release_test_support.rs`. Promote
`LocalRelease.serve_root` to `pub`. Copy the canonical manifest under
`serve_root/v{version}/`. Write a GitHub-API-shaped `releases.json`
under the fixture's staging dir.

Add `resolver_fixture_emits_valid_releases_json` in
`release_test_support::tests`. Asserts the resolver accepts the
fixture's `releases.json` and returns a `ResolvedRelease` whose
`manifest_url` starts with the loopback base.

## T2 — `cmd_update` live install against the fixture

Add `cmd_update_live_install_against_fixture` in `layout::tests`. Uses
`TempCognicodeHome` + `TempBaseUrl` (new RAII guard) + the
`ResolverFixture`. Asserts post-install state: `bundle.yaml`,
`install/0.95.0/manifest.yaml`, tracker, journal.

Add `TempBaseUrl::set(url)` in `layout::test_support`. Clears
`COGNICODE_BUNDLE_MANIFEST` on the way in (see design D3).

## T3 — `cmd_rollback` after live install

Add `cmd_rollback_after_live_install` in `layout::tests`. Runs a live
install, then runs `cmd_rollback`. Asserts the rollback surfaces the
known populated-dir regression (`Directory not empty` / os error 39)
and that the journal is left intact (rollback aborted mid-way). This
pins the regression without blocking the cycle on a fix.

Add `TempOpenCodeConfig::disable()` in `layout::test_support`. Sets
`OPENCODE_CONFIG` to a non-existent file so `ide::detect_opencode()`
returns false without touching the host filesystem.

## T4 — Exit gates

- `cargo test -p cognicode-cli --bin cogh` — 177/177 (was 171 + 6 new).
- `cargo fmt --check --package cognicode-cli` — clean.
- `cargo check --workspace --all-targets` — exit 0.
- `just check-known-failures` — 41-entry baseline intact.
- Live `cogh latest --json` smoke green.

## T2b — Sequential install follow-through (added during re-read)

Added after the initial cycle closed. The auto-prompt asked for
"main workflows, edge cases, failure modes" — the obvious gap was
running `cogh update` twice in a row.

- New `cmd_update_sequential_installs_overwrite_cleanly` test.
- Asserts either idempotent overwrite OR the documented stale-shim
  regression surfaces.
- Surfaced a second pre-existing bug (LinuxAdapter::install_shim
  does not unlink a stale shim before re-symlinking). Pinned in
  verification-report.md, not fixed.

## T1b — ResolverFixture failure-mode follow-through (added during second re-read)

Added to exercise the most likely user-facing failure modes on the
staging-dir path: malformed `releases.json` and a list of
only-draft releases.

- New `resolver_fixture_rejects_malformed_releases_json` test.
- New `resolver_fixture_rejects_only_draft_releases_in_list` test.
- Both passed on the first run; the resolver's error surface for
  these failure modes is correct (returns `InstallerError::ResolveFailed`
  with an identifying message).
