# Design — ResolverFixture bridge + live install tests

## Context

`LocalRelease::local_release(version)` builds a real, installable
release on disk and serves it over a loopback HTTP server. Its
limitations:

1. The manifest is written to `release.dir/<manifest-name>` (the
   `out_dir` of `generate_release`), but the HTTP server only serves
   `release.serve_root/v{version}/<payload>`. The manifest is NOT
   served.
2. There is no `releases.json` for the `lifecycle_resolver` to consume.
3. `cmd_update` needs `COGNICODE_RELEASE_BASE_URL` set so the install
   pipeline rewrites canonical github.com URLs onto the loopback.

## Design

### D1 — Expose `serve_root` on `LocalRelease`

`LocalRelease` already constructs `serve_root = tmp.path().join("serve")`.
Promote it to a `pub` field. This lets `ResolverFixture::build` place
the canonical manifest under the served `v{version}/` directory.

### D2 — `ResolverFixture::build(version)`

```text
ResolverFixture::build(version: &str) -> Result<Self> {
    let release = local_release(version)?;

    // D1: ensure the manifest is served at v{version}/<manifest-name>.
    let manifest_name = bundle_manifest_filename(version, LinuxX86_64);
    let served = release.serve_root.join(format!("v{version}")).join(&manifest_name);
    std::fs::copy(&release.manifest_path, &served)?;

    // Write a GitHub-API-shaped releases.json the resolver can consume.
    let staging_dir = release._tmp.path().join("staging-resolver");
    std::fs::create_dir_all(&staging_dir)?;
    let manifest_url = format!("{}/v{}/{}", release.base_url, version, manifest_name);
    let json = format!(/* GhRelease shape with manifest asset URL */);
    std::fs::write(staging_dir.join("releases.json"), json)?;

    Ok(Self { release, staging_dir })
}
```

The `_tmp` field stays private; `ResolverFixture` is defined in the
same module so it can access it.

### D3 — Test env guards (in `layout::test_support`)

Three small RAII guards, each `#[serial]`-safe:

- `TempCognicodeHome` — already exists, sets `COGNICODE_HOME`.
- `TempBaseUrl::set(url)` — sets `COGNICODE_RELEASE_BASE_URL`, also
  clears `COGNICODE_BUNDLE_MANIFEST` on the way in and restores it on
  the way out. The clear is required because the legacy `point_at()`
  seam sets that env var and at least one existing test
  (`installer_transaction::tests::advance_skips_through_all_stages`)
  never calls `unpoint()`, so the stale path leaks between serialised
  tests.
- `TempOpenCodeConfig::disable()` — sets `OPENCODE_CONFIG` to a
  non-existent file. This makes `ide::detect_opencode()` return false
  without changing production code, so the install does not try to
  symlink the freshly installed mcp-server into the user's real
  `~/.config/opencode/skills/`.

### D4 — Tests

Two new tests in `layout::tests`:

- `cmd_update_live_install_against_fixture` — REQ-FU-02.
- `cmd_rollback_after_live_install` — REQ-FU-03. Pins the
  populated-dir rollback regression rather than blocking on it.

One new test in `release_test_support::tests`:

- `resolver_fixture_emits_valid_releases_json` — REQ-FU-01.

Total: 3 new tests (+1 helper test). 174 cogh tests after the change.

## What is NOT changed

- `LocalRelease::local_release` itself — kept stable, no public API
  break.
- `point_at` / `unpoint` — kept stable.
- `lifecycle_resolver` — no changes; it is exercised as-is.
- `cmd_update` / `cmd_rollback` — no changes; exercised as-is.
- `rollback_journal` — the populated-dir regression is pinned, not
  fixed.
