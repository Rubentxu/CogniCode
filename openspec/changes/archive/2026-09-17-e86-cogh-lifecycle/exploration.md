# e86 — WU0 Exploration

> Operational authority is `state.yaml`. This file is the read-only investigation
> that precedes the WU1 spec work.

## Summary

`cogh` already has the single backend (`InstallerTransaction` + `run_install`)
that `cogh update` will reuse. `cmd_latest` and `cmd_update` are currently
no-op prints. The RollbackJournal machinery is real and tested, but is held
only in memory; for `cogh rollback` we need to persist a journal next to the
install so a later command can replay its reversal. The remote resolution is
trivial because GitHub's REST API returns the right shape and the
`bundle-{v0.X.Y}-{platform_token}.yaml` filename already follows the e85
contract.

## Findings

### 1. Backend re-use: trivial

`run_install(home, profile)` in `crates/cognicode-cli/src/cmd/install.rs:23`
already does:

- acquire advisory install lock,
- run `InstallerTransaction::run(profile)`,
- write `tracker::write_version(version)`,
- run OpenCode IDE integration,
- release the lock.

This is the **single install pipeline**. e86's `cogh update` will call it with
a different manifest source. No second install pipeline; no edits to
`install.rs` beyond optionally exposing its manifest_path return for the
update command to write a journal.

`InstallerTransaction::load_bundle_manifest()` already honours
`COGNICODE_BUNDLE_MANIFEST` first, then `~/.cognicode/bundle.yaml`, then
falls back to the dev fixture with a loud warning. e86's resolver will
**write the resolved manifest to `~/.cognicode/bundle.yaml`** so the existing
seam picks it up. That's the smallest change possible.

### 2. `cmd_latest` / `cmd_update` are no-ops

`crates/cognicode-cli/src/cmd/layout.rs:270-290` is literally:

```rust
pub fn cmd_latest(home, plugin, all) -> Result<()> {
    let _ = home;
    if all { println!("(latest --all: not yet implemented)") }
    ...
}
```

The CLI surface (`crates/cognicode-cli/src/bin/cogh.rs:110-122`) is already
correct: `Latest { plugin: Option<String>, --all }`, `Update { plugin: Option<String> }`.

For e86 the CLI will stay byte-for-byte the same; only the layout functions
gain semantics. The plugin-vs-component distinction that currently exists
in `layout::cmd_install` is **out of scope for e86**: e86's `cogh update`
takes no plugin name, it updates the runtime. The existing `Command::Update
{ plugin }` will be repurposed: `plugin` becomes optional and unused
(now the prompt is `cogh update` for "update the runtime"). We keep the
existing plugin semantics only for backward compatibility when a plugin
name is given — but in practice the only install path is the runtime. We
will document this and not break the surface.

### 3. `RollbackJournal` is in-memory only

`crates/cognicode-cli/src/cmd/rollback_journal.rs:40-43`:

```rust
pub struct RollbackJournal {
    effects: Vec<SideEffect>,
    committed: bool,
}
```

For `cogh rollback` to work as a separate command, the journal has to be
serialised next to the install it describes. e86 plan:

- Add `RollbackJournal::record_persistent(&self, path)` that writes JSON.
- Add `RollbackJournal::load(path) -> Self`.
- On commit, write `~/.cognicode/journal/<version>.json`.
- `cogh rollback` reads the latest journal, runs `rollback()`, removes the
  install dir, restores the tracker to its previous value (which is
  captured in the journal as a `SideEffect::WroteTracker { old }` we will
  add).

The current `SideEffect` enum does NOT cover "the install dir itself" nor
"the tracker file". e86 will add two new variants:

- `RemovedDir(PathBuf)` (already exists as a no-op reverse — we keep it
  as a marker so the journal records it).
- `WroteTracker { path, previous: Option<String> }` (new).

The reverse of `WroteTracker` is `std::fs::write(path, previous)`.

### 4. Remote resolution: GitHub API surface is exactly what we need

`GET https://api.github.com/repos/Rubentxu/CogniCode/releases/latest`
returns:

```json
{
  "tag_name": "v0.95.0",
  "draft": false,
  "prerelease": false,
  "published_at": "2026-09-17T19:07:59Z",
  "assets": [
    { "name": "bundle-0.95.0-x86_64-unknown-linux-gnu.yaml", ... },
    ...
  ]
}
```

Empirical confirmation: the endpoint already filters drafts and prereleases
at the server side, returning v0.95.0 as the only non-draft. **However** we
still check `draft == false` and `prerelease == false` in the client because
drafts CAN appear via the full `/releases` endpoint and our resolver must
not regress if we ever broaden the channel set.

The asset filename for the per-platform bundle manifest is exactly
`bundle-{version}-{platform_token}.yaml`, derivable from the Rust contract.
We never type it manually.

### 5. Draft-first safety

The e85 release workflow creates a DRAFT, uploads the assets, attests,
re-verifies, then `gh release edit --draft=false`. The window between
"assets uploaded" and "draft published" is real (seconds to minutes).
`cogh update` must refuse during that window with a clear error.

### 6. Platform allow-list

`Platform` has 5 variants. e85 publishes only `LinuxX86_64` and
`LinuxAarch64`. `cogh update` must refuse any other host platform with
"platform not in the e85 Tier-1 surface". The non-Tier-1 token check lives
in the resolver, BEFORE we hit GitHub's API — saves a round-trip for hosts
that have no published manifest anyway.

### 7. Test seam

`crates/cognicode-cli/src/cmd/release_test_support.rs` already provides:

- `local_release(version)` — full Tier-1 fixture, served over loopback.
- `point_at(release)` / `unpoint()` — sets `COGNICODE_BUNDLE_MANIFEST` and
  `COGNICODE_RELEASE_BASE_URL`.
- `TempCognicodeHome` — temporary `~/.cognicode` rooted in a temp dir.

e86 tests will reuse this verbatim. The only addition is a **resolver
fixture**: a way to inject a fake GitHub API response (JSON) so resolver
tests do not hit the network.

## Module ownership for e86

| New module | Purpose |
|---|---|
| `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs` | `version + platform -> BundleManifest v2` resolution; fetches GitHub Releases API; rejects drafts / prereleases / non-Tier-1 platforms |
| `crates/cognicode-cli/src/cmd/lifecycle_journal.rs` | persists and replays `RollbackJournal` for `cogh rollback` |
| `crates/cognicode-cli/src/cmd/lifecycle.rs` | gains new tests; existing tests untouched |

`release_contract.rs`, `bundle_manifest.rs`, `release_factory.rs`,
`installer_transaction.rs`, `install.rs`, `release.yml` are **not edited**
in e86 except for one extension: `InstallerError` gains
`DraftRelease`, `PlatformNotInTier1`, `NoMatchingManifest`,
`ResolveFailed` variants. (These could live in `error.rs`; we add them
there.)

## Open questions deferred to design

1. What `--channel` values do we accept today? (proposal says only `stable`,
   and `--channel preview` returns a clear "no preview channel published"
   error.)
2. Should `cogh update` re-write the tracker unconditionally, or only if the
   new version is greater? Semver ordering from the GitHub response is
   trivial; we will pin the rule "always write if a newer version is
   installed" with a unit test.
3. Where does the resolver read `GITHUB_TOKEN` from? `~/.config/gh/hosts.yml`
   like `gh` itself, OR from `COGNICODE_GITHUB_TOKEN` env var. We pick
   `COGNICODE_GITHUB_TOKEN` first, then fall back to anonymous (rate-limited
   to 60 req/h per IP — fine for `cogh latest` once per session).

## Non-goals reminder

Confirmed by inspection:

- mise, install.sh, aqua, Homebrew, Nix → out (e87)
- Cross-channel / preview / nightly → out (deferred)
- explorer-api as Layer-1 → out (its own decision)
- cargo-dist → out (ADR-052 stands)
- Backstage / Control Plane → out (CP0)

## Exit conditions for WU0

- [x] Confirmed `run_install` is the only real backend.
- [x] Confirmed `cmd_latest` / `cmd_update` are no-op prints, no semantics.
- [x] Confirmed `RollbackJournal` is in-memory; persistence needed.
- [x] Confirmed GitHub API response shape and `/releases/latest` draft filter.
- [x] Confirmed test seam (`release_test_support.rs`) is reusable.
- [x] Identified module ownership and one cross-cutting change
  (`InstallerError` variants).
- [x] Captured open questions for WU2 design.