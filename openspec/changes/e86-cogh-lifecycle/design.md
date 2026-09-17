# e86 — Design

> e86 WU2. Operational authority is `state.yaml`.
> Decisions (D1..D8) are pinned and reviewed at apply time.

## Module map

```text
crates/cognicode-cli/src/cmd/
├── lifecycle_resolver.rs   (NEW) — version + platform -> BundleManifest v2
├── lifecycle_journal.rs    (NEW) — persistent journal + replay
├── lifecycle.rs            (EXTENDED with new tests; existing tests untouched)
├── layout.rs               (cmd_latest / cmd_update / cmd_rollback semantics)
├── installer_transaction.rs (EXTENDED: writes journal at commit; reads ~/.cognicode/bundle.yaml at load)
├── install.rs              (UNCHANGED)
├── install_lock.rs         (UNCHANGED, reused for cogh rollback)
├── error.rs                (EXTENDED: DraftRelease, PlatformNotInTier1, NoMatchingManifest, ResolveFailed)
├── rollback_journal.rs     (EXTENDED: WroteTracker variant; JSON serialise / deserialise)
├── release_contract.rs     (UNCHANGED)
├── bundle_manifest.rs      (UNCHANGED)
├── release_factory.rs      (UNCHANGED)
├── release_test_support.rs (EXTENDED: ResolverFixture helper)
└── mod.rs                  (EXTENDED: register the new modules)

crates/cognicode-cli/src/bin/cogh.rs (EXTENDED: --channel and --staging flags)
```

No edits outside `cognicode-cli`. No edits to the e85 release workflow
(`release.yml`) — e86 is consumer-side only.

## Decisions

### D1 — single resolver module

`lifecycle_resolver.rs` exposes one public function:

```rust
pub fn resolve_release(req: ResolveRequest) -> Result<ResolvedRelease, InstallerError>
```

`ResolveRequest` carries: `host_platform`, `channel`, `version`, optional
`base_url`, optional `staging_dir`, optional `api_token`. `ResolvedRelease`
carries: `version`, `tag`, `platform_token`, `manifest_url`,
`manifest_sha256`, `published_at`, `release_url`.

A second function `resolve_to_json` formats the result for `--json` output.

A third function `resolve_via_fixture(path)` is the test-only escape hatch
that reads `<dir>/releases.json` and picks the latest non-draft,
non-prerelease release with a matching asset.

### D2 — HTTP client policy

`lifecycle_resolver` uses `reqwest::blocking` (already a transitive
dependency through `installer_transaction`). One `Client` is built per
resolver call; we do not pool. The 60-second timeout from
`InstallerTransaction::Downloading` is the canonical timeout.

### D3 — draft-first safety as one branch

The check `if draft || prerelease { return Err(DraftRelease) }` lives in
`resolve_release` and is the only place it appears. Tests cover both
branches and the staging path (REQ-LDS-03).

### D4 — install path for `cogh update`

```text
cogh update
  → resolve_release(req)                              // new
  → download manifest to ~/.cognicode/bundle.yaml     // new (reqwest)
  → verify SHA256 against SHA256SUMS                  // new (reqwest)
  → install::run_install(home, profile)               // existing, unchanged
```

The manifest is fetched, verified, and written to disk BEFORE the install
runs. The install's `InstallerTransaction::load_bundle_manifest` reads
`~/.cognicode/bundle.yaml` (already the case today) and proceeds. The
existing dev-bundle fixture seam is preserved as a fallback.

### D5 — `cogh rollback` journal path

```text
~/.cognicode/journal/<version>.json
```

One file per committed install. The journal contains every `SideEffect` in
commit order. The file is read with `serde_json::from_str`; unknown
fields are ignored for forward-compat; unknown `type` values cause a
loud error.

### D6 — rollback reuses `RollbackJournal`

```rust
let journal = RollbackJournal::load(path)?;
let mut in_mem = RollbackJournal::from_serialised(&journal);
in_mem.rollback()?;
```

`from_serialised` is a new constructor on `RollbackJournal` that takes a
parsed JSON value and converts it back into the in-memory `Vec<SideEffect>`
form. We then call `rollback()` so the reversal logic is unchanged.

### D7 — journal written at commit time

`InstallerTransaction::commit` gains a tail call:

```rust
journal.commit();                              // existing
let journal_path = journal_path_for(&manifest.version);
crate::lifecycle_journal::write(&journal, &manifest, &journal_path)?;
```

The journal is written AFTER the manifest, so a crash mid-write leaves
the install committable but without a journal — the next `cogh rollback`
will report "nothing to roll back", which is the honest answer.

### D8 — flags on the CLI

```rust
Latest {
    plugin: Option<String>,
    #[arg(long)]
    all: bool,
    #[arg(long, default_value = "stable")]
    channel: Channel,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    staging: Option<PathBuf>,
    #[arg(long)]
    json: bool,
},
Update {
    plugin: Option<String>,
    #[arg(long, default_value = "stable")]
    channel: Channel,
    #[arg(long)]
    base_url: Option<String>,
    #[arg(long)]
    staging: Option<PathBuf>,
    #[arg(long, default_value = "core")]
    profile: String,
    #[arg(long)]
    dry_run: bool,
},
Rollback {
    plugin: Option<String>,
},
```

`Channel` is a small enum: `Stable`, `Preview` (the latter returns a clear
"no preview channel published" error).

## Test plan

| Test | Source module | Asserts |
|---|---|---|
| `resolver_rejects_non_tier1` | `lifecycle_resolver::tests` | `PlatformNotInTier1` for windows / darwin |
| `resolver_returns_v095_for_staging_fixture` | `lifecycle_resolver::tests` | `ResolvedRelease{tag="v0.95.0", ...}` |
| `resolver_rejects_draft_release` | `lifecycle_resolver::tests` | `DraftRelease` with WU13 message |
| `resolver_rejects_prerelease` | `lifecycle_resolver::tests` | `DraftRelease` with prerelease message |
| `resolver_rejects_release_without_manifest_asset` | `lifecycle_resolver::tests` | `NoMatchingManifest` |
| `resolver_honors_base_url_override` | `lifecycle_resolver::tests` | manifest_url rewritten |
| `resolver_honors_staging_dir` | `lifecycle_resolver::tests` | reads `<dir>/releases.json` |
| `resolver_sends_auth_header_when_token_set` | `lifecycle_resolver::tests` | mocked server receives the header |
| `resolver_handles_network_error` | `lifecycle_resolver::tests` | `ResolveFailed` |
| `resolver_parses_specific_version` | `lifecycle_resolver::tests` | hits `/releases/tags/vX.Y.Z` |
| `update_dry_run_prints_tuple_no_download` | `lifecycle::tests` | no HTTP download, no install |
| `update_full_round_trip` | `lifecycle::tests` | staging resolver → install via existing pipeline |
| `journal_is_written_on_commit` | `lifecycle_journal::tests` | file exists, JSON-valid, contains `WroteTracker` |
| `rollback_with_no_journal_is_noop` | `lifecycle_journal::tests` | exit 0 |
| `rollback_reverses_last_install` | `lifecycle_journal::tests` | install dir removed, tracker restored |
| `rollback_is_idempotent` | `lifecycle_journal::tests` | second call is a no-op |
| `rollback_holds_install_lock` | `lifecycle_journal::tests` | concurrent install waits |
| `rollback_with_corrupt_journal_fails_loudly` | `lifecycle_journal::tests` | clear error, install dir untouched |
| `cogh_e2e_full_cycle` | `lifecycle::tests` | install v0.95.0 → update to v0.96.0 (synthetic) → rollback → state matches pre-update |

## Carry-forward

- `cogh install` is the single backend. No second install pipeline.
- `release.yml` is not touched.
- `bundle_manifest.rs`, `release_contract.rs`, `release_factory.rs` are
  not touched.
- The e85 adversarial suite (35 tests) still passes.

## What this design does NOT cover

- A real `--channel preview` backend.
- Cross-channel resolution or "promote a draft" semantics.
- A local cache of releases (the resolver hits the API every call).
- The mise install.sh from e87.
- The fresh-Linux lifecycle UAT from e88.

These are explicitly deferred.