# e86 — cogh lifecycle: latest, update, rollback, remote bundle resolution

## Intent

Implement the lifecycle that the e85 release factory leaves for the consumer. e85
proved the producer and the consumer can speak the same artifact language
(BundleManifest v2). e86 makes `cogh` itself speak it, end-to-end, on a real
fresh Linux install.

```text
cogh install --profile core      # already works on a generated, locally served release (e85 e2e)
cogh latest                       # asks GitHub Releases: which v0.X.Y is current?
cogh update                       # resolves version + platform -> BundleManifest v2, then installs
cogh rollback                     # uses the existing RollbackJournal machinery (e84 characterized it absent)
```

e86 does NOT introduce mise, install.sh, aqua, Homebrew, Nix, Backstage, or any
non-cogh distribution channel. The scope is `cogh` talking to `cogh`'s own
release pipeline.

## The point

The current state:

- e85 produces BundleManifest v2, ReleaseInventory, SHA256SUMS, and attestations.
- `cogh install` already works **once** it has a manifest (the e85 e2e proves it:
  real generator, real installer, real HTTP, real install).
- `cogh latest / update / rollback` exist as **CLI stubs** (subcommands declared in
  `cogh.rs`, no semantics). The RollbackJournal machinery is real; the command is
  not (e84 WU characterization).

That gap is e86.

## Inputs treated as immutable

```text
e84 R1-R9           artifact contract, total and final
e85 WU0.1           ReleaseInventory / BundleManifest / cogh are three different things
e85 WU1-WU11        the contract the producer speaks
e85 WU12            Linux Tier 1 only on native runners; we do not advertise what is not built
e85 WU8             dev-bundle.yaml is dev-only; remote resolution of version+platform is e86's job
KEEP_CUSTOM_RELEASE  we own the release pipeline end-to-end
```

## Non-goals (explicit, not later)

```text
mise / install.sh / aqua / Homebrew / Nix        (e87)
fresh-Linux lifecycle UAT (real install + use)   (e88)
Backstage / Control Plane                         (CP0)
Explorer API as Layer-1 in a profile              (deferred until its own profile semantics are decided)
Adopting cargo-dist or any other release tool      (ADR-052 stands)
```

## Deliverable

1. `cogh latest` returns the latest published `vX.Y.Z` for the host platform.
2. `cogh update [plugin]` reads `~/.cognicode/tracker.json`, compares installed
   versions against `cogh latest`, and runs the existing installer pipeline
   pointed at the freshly resolved manifest URL.
3. `cogh rollback [plugin]` reads the RollbackJournal written by the previous
   install and reverses it on disk, with the same SHA256 gate the installer
   already uses.
4. `cogh update --channel stable` and `--channel preview` exist; "stable" is the
   only channel with a real backend today (preview returns a clear error: no
   preview channel published). No future-channel hooks.
5. Resolution contract:

   ```text
   host platform
   + requested version OR "latest"
   + channel
   -> GitHub Releases: https://api.github.com/repos/Rubentxu/CogniCode/releases/latest
   -> tag "v0.X.Y"
   -> asset filename: bundle-{v0.X.Y}-{platform_token}.yaml
   -> digest pinned by SHA256SUMS at the same tag
   ```

6. No edit to `cogh install`. The existing installer pipeline is the
   single backend for both install and update.
7. No edit to BundleManifest v2 or ReleaseInventory. Resolution reads them; it
   does not invent them.
8. The release workflow `release.yml` does not change. e86 is consumer-side.

## Exit gates (must all pass)

| Gate | Evidence |
|---|---|
| `cogh latest --json` on Linux x86_64 returns `{"version":"0.95.0","tag":"v0.95.0","platform_token":"x86_64-unknown-linux-gnu"}` | reproducible test against a locally served fake GitHub API |
| `cogh update` re-uses the same `InstallerTransaction` codepath | unit test that asserts both flows call `InstallerTransaction::load_bundle_manifest` with the same arguments |
| `cogh rollback` reverses the last install atomically (or refuses if journal is absent) | unit test on a synthetic home |
| Remote resolution refuses a non-Tier-1 platform token (aarch64 musl, darwin, windows) with a clear "platform not in the e85 Tier-1 surface" error | unit test |
| Resolution parses the BundleManifest v2 published at the resolved tag and rejects anything else | unit test using `BundleManifest::from_str` against real downloaded YAML |
| Resolution does NOT call out to GitHub if `--staging <dir>` is set | unit test |
| Draft-first safety: if the release is still a draft, `cogh update` refuses with `release v0.X.Y is a draft; refusing to install` | unit test with a synthetic draft |
| Attestation check: `cogh update` runs `gh attestation verify` against the resolved manifest before installing (warning-only when `gh` is absent, since install UAT environments may not have it) | unit test (with mock) + real call recorded |
| Workspace build | `cargo check --workspace --all-targets` exit 0 |
| Known-failure baseline | `python3 scripts/check_known_failures.py` exit 0, count unchanged |
| Architecture self-host | 3/3 zero drift |
| Format | `cargo fmt -p cognicode-cli --check` exit 0 |
| End-to-end on synthetic home: install v0.95.0 → update to v0.96.0 (synthetic) → rollback → state matches pre-update | `cargo test -p cognicode-cli lifecycle::tests` |

## What this cycle DOES NOT prove (deliberate)

- A real fresh-Linux install UAT (no Docker, no VM harness in this repo). e88.
- `mise` integration. e87.
- Cross-channel update (preview, nightly). Out of scope.
- Restoring `~/.cognicode` from a backup. Rollback is local, transactional.