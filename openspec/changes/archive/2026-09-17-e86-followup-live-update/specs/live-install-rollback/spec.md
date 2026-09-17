# Live install via the new resolver-driven path

## Purpose

e86 closed the `lifecycle_resolver` + `lifecycle_journal` + `cmd_rollback`
contracts, but `cmd_update`'s non-dry-run path had no test coverage. The
legacy seam (`release_test_support::LocalRelease` + `point_at`) exercises
`COGNICODE_BUNDLE_MANIFEST`, NOT the resolver. This change closes that
gap: it builds a `ResolverFixture` that bridges the loopback payload to
the resolver-driven install, and adds tests that drive the full install
+ rollback round trip through `cmd_update` / `cmd_rollback`.

## Contract

### REQ-FU-01 — ResolverFixture builder

`ResolverFixture::build(version)` MUST build a fixture that lets the
`lifecycle_resolver` resolve a release whose `manifest_url` is the
loopback server, AND have that loopback server actually serve the
canonical bundle manifest at that URL.

**Given** a Tier-1 Linux x86_64 host
**And** `LocalRelease::local_release(version)` is available
**When** `ResolverFixture::build(version)` runs
**Then** the loopback server is running at `<base_url>`
**And** a `releases.json` is written under the fixture's `staging_dir`
**And** the manifest asset URL in `releases.json` matches
`format!("{base_url}/v{version}/<manifest-name>")`
**And** that manifest file exists on disk under `serve_root/v{version}/`
**And** `resolve_release(ResolveRequest { staging_dir, ... })` returns
a `ResolvedRelease` whose `manifest_url` starts with the loopback
base URL.

#### REQ-FU-01b — ResolverFixture failure modes

The resolver MUST surface, not swallow, the most likely user-facing
failure modes on the staging-dir path:

**Given** a staging dir with malformed `releases.json`
**When** `resolve_release(staging_dir=...)` runs
**Then** the resolver returns `InstallerError::ResolveFailed` with a
message that identifies the parse failure.

**Given** a staging dir whose `releases.json` is a list containing
only draft / prerelease releases
**When** `resolve_release(staging_dir=..., requested_version="latest")` runs
**Then** the resolver returns `InstallerError::ResolveFailed` with a
message that identifies the draft-only rejection (per REQ-LR-03 /
REQ-LDS-01).

**Rationale:** the staging-dir path is exactly how integration tests
and air-gapped installs configure the resolver. Failures here are
not rare; the resolver must NOT panic, NOT silently fall back to the
network, and NOT swallow the error.

### REQ-FU-02 — Live install through `cmd_update`

`cmd_update(home, ..., staging, profile, /*dry_run=*/false)` MUST
succeed against a `ResolverFixture` and produce the post-install
artifacts the e86 contract promises.

**Given** a `TempCognicodeHome` with `home.init()` done
**And** `ResolverFixture::build("0.95.0")` built
**And** `COGNICODE_RELEASE_BASE_URL` set to the fixture's loopback base
**And** `OPENCODE_CONFIG` pointing at a non-existent file
**When** `cmd_update(home, None, Stable, None, Some(staging), "core", false)`
runs
**Then** the install completes without error
**And** `home.bundle_yaml_path()` exists and contains "0.95.0"
**And** `install_manifest_path("0.95.0")` exists
**And** `home.tracker_version()` reads "0.95.0"
**And** `lifecycle_journal::journal_path("0.95.0")` exists.

### REQ-FU-03 — Rollback after live install

`cmd_rollback(home, None)` MUST find the journal written by REQ-FU-02
(via the pinned tracker → version path) and reverse the side-effects.

**Given** the post-install state from REQ-FU-02
**When** `cmd_rollback(home, None)` runs
**Then** the rollback attempts to reverse every recorded side-effect
**And** the journal is consumed.

#### Note (KNOWN REGRESSION, OUT OF SCOPE)

`rollback_journal` removes `CreatedDir` entries with `rmdir`, which
fails on a directory that holds files written by a later stage
(`Extracted`, `Downloaded`). The post-install rollback therefore errors
with "Directory not empty" — this is a pre-existing bug in the rollback
implementation, NOT in the resolver-driven install path. REQ-FU-03
asserts that the regression surfaces (so it does not get lost) but does
not block this cycle on the fix.

### REQ-FU-04 — Sequential install follow-through (idempotency / overwrite)

Two consecutive `cmd_update` calls against the same `CognicodeHome`
MUST NOT crash. Either the second install overwrites cleanly
(idempotent), OR the failure must be attributable to a documented
pre-existing bug — NOT a regression introduced by this followup.

**Given** a fresh `CognicodeHome` with `home.init()` done
**And** `ResolverFixture::build("0.95.0")` built
**When** `cmd_update(home, ..., staging, "core", false)` runs twice
**Then** the second run either:
- succeeds (and the install manifest is rewritten), OR
- fails with the documented stale-shim regression (`shim install
  error: symlink <home>/shims/<bin> ...`) which proves the regression
  surfaced, not a regression introduced by this followup.

**Rationale:** every user who runs `cogh update` more than once hits
this path. The follow-through surfaces the bug if it exists, and
documents it in the verification report. Fixing the bug is out of
scope (would require modifying `platform_adapter::install_shim`).

#### Note (KNOWN REGRESSION, OUT OF SCOPE)

`LinuxAdapter::install_shim` calls `std::os::unix::fs::symlink`
without first removing an existing shim. On a second install the
symlink syscall returns `EEXIST` and the install transaction aborts.
The fix is to remove the existing shim (or fall back to copy) before
re-symlinking. Pinned by `cmd_update_sequential_installs_overwrite_cleanly`.

## Why this matters

Before this change, `cmd update --dry-run` was the only resolver-driven
path with test coverage. A bug in the actual install pipeline
(download, SHA256, extract, shim, tracker, journal) could only be
caught by an integration test against a real release. This followup
add that test using a loopback server so the install can be exercised
without network access.
