# e86 — Tasks (apply plan)

> Operational authority is `state.yaml`. This is the bounded task list the
> apply phase executes. Each task has one assertion.

## Sequencing rule

Tasks are sequential; the next task starts only when the previous one is
green. Tests are part of each task, never deferred to a "test pass".

## Task list

### T1 — `InstallerError` extensions

**Surface**: `crates/cognicode-cli/src/cmd/error.rs`

**Add 4 variants**:

```rust
#[error("platform {0:?} is not in the e85 Tier-1 surface; supported: {1}")]
PlatformNotInTier1(Platform, String),

#[error("{0}")]
DraftRelease(String),

#[error("no per-platform manifest asset for platform {0}; available: {1:?}")]
NoMatchingManifest(Platform, Vec<String>),

#[error("resolve failed: {0}")]
ResolveFailed(String),
```

**Assertion**: `cargo check -p cognicode-cli` succeeds; no caller updates
needed yet (variants are unused).

### T2 — `RollbackJournal` JSON shape

**Surface**: `crates/cognicode-cli/src/cmd/rollback_journal.rs`

**Add**:

- `pub enum SideEffect` gains `WroteTracker { path: PathBuf, previous: Option<String> }`.
- `impl SideEffect` gains a `kind()` discriminator for serialisation.
- `pub fn RollbackJournal::record_serialisable(&self) -> serde_json::Value`.
- `pub fn RollbackJournal::from_serialisable(value: serde_json::Value) -> Result<RollbackJournal>`.
- `reverse_one` for `WroteTracker`: writes `previous` back; if `previous`
  is `None`, removes the file.
- `remove SideEffect::WroteManifest`'s reverse (it already removes the file).

**Assertion**: existing `RollbackJournal` tests still pass; new tests
`test_serialise_round_trip`, `test_wrote_tracker_reverse`,
`test_wrote_tracker_no_previous_removes_file` pass.

### T3 — `lifecycle_journal` module

**Surface**: `crates/cognicode-cli/src/cmd/lifecycle_journal.rs` (new)

**Public surface**:

```rust
pub fn journal_path(version: &str) -> PathBuf; // ~/.cognicode/journal/<v>.json
pub fn write(journal: &RollbackJournal, manifest: &BundleManifest, previous_tracker: Option<&str>, path: &Path) -> Result<()>;
pub fn load(path: &Path) -> Result<RollbackJournal>;
pub fn list_committed(home: &CognicodeHome) -> Result<Vec<String>>; // returns versions with a journal
```

**Assertion**: tests `test_write_then_load_round_trip`,
`test_load_corrupt_json_fails_loudly`,
`test_list_committed_returns_only_versions_with_a_journal` pass.

### T4 — wire journal at `InstallerTransaction::commit`

**Surface**: `crates/cognicode-cli/src/cmd/installer_transaction.rs`

**Change**: at the end of `commit()`, after `journal.commit()`, write the
journal via `lifecycle_journal::write`. Track `previous_tracker` BEFORE
calling `tracker::write_version` — this requires `tracker::read_version`
to be called by `run_install` (currently does not happen). Add
`tracker::read_version() -> Result<Option<String>>` that returns `None`
if the tracker file is missing.

**Assertion**: `advance_skips_through_all_stages` and
`commit_writes_manifest_file` still pass; new test
`commit_writes_journal_next_to_install` passes; `test_clean_home_install`
still passes (the journal is written, the install completes).

### T5 — `lifecycle_resolver` module

**Surface**: `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs` (new)

**Public surface**:

```rust
pub struct ResolveRequest {
    pub host_platform: Platform,
    pub channel: Channel,
    pub version: VersionRequest, // Latest | Specific(String)
    pub base_url: Option<String>,
    pub staging_dir: Option<PathBuf>,
    pub api_token: Option<String>,
}

pub struct ResolvedRelease {
    pub version: String,
    pub tag: String,
    pub platform_token: String,
    pub manifest_url: String,
    pub manifest_sha256: String,
    pub published_at: Option<String>,
    pub release_url: String,
}

pub enum Channel { Stable, Preview }
pub enum VersionRequest { Latest, Specific(String) }

pub fn resolve_release(req: ResolveRequest) -> Result<ResolvedRelease, InstallerError>;
pub fn resolve_to_json(r: &ResolvedRelease) -> serde_json::Value;
```

The implementation:

1. `req.host_platform` must be in `TIER1_PLATFORMS` else
   `PlatformNotInTier1`.
2. `req.channel` must be `Stable`; `Preview` returns
   `ResolveFailed("preview channel not published yet")`.
3. Resolve via `staging_dir` if set, else real HTTP.
4. Find the per-platform bundle manifest asset by exact name from
   `bundle_manifest_filename(version, host_platform)`. If missing,
   `NoMatchingManifest`.
5. Refuse `draft || prerelease` with `DraftRelease`.
6. Fetch `SHA256SUMS` at the same tag, find the line for the manifest
   filename, parse the digest. Reject placeholders by reusing
   `ArtifactDigest::parse` from `release_contract`.

**Assertion**: 10 unit tests from the design's test plan all pass.

### T6 — wire resolver into `cogh latest / update`

**Surface**: `crates/cognicode-cli/src/cmd/layout.rs` (replace stubs)

**Change**:

- `cmd_latest` parses flags from CLI args (need to thread the struct).
  Easiest: change `cmd_latest` to take a `LatestArgs` struct from `cogh.rs`.
- `cmd_update` parses flags and calls `resolve_release` then
  `download_manifest` then `run_install`.
- New `cogh rollback` handler in `cogh.rs` calls `cmd_rollback`.

**Helper** in `lifecycle_resolver.rs`:

```rust
pub fn download_manifest(r: &ResolvedRelease, dest: &Path, base_url: Option<&str>) -> Result<()>;
```

Downloads the manifest to `dest`, verifies against `r.manifest_sha256`.

**Assertion**: `test_update_full_round_trip` passes;
`test_update_dry_run_prints_tuple_no_download` passes;
`test_cogh_e2e_full_cycle` passes.

### T7 — `cogh rollback` command

**Surface**: `crates/cognicode-cli/src/cmd/layout.rs` + new CLI subcommand.

**Change**:

- Add `Command::Rollback` to `cogh.rs`.
- `cmd_rollback` reads `tracker::read_version`, derives
  `journal_path(version)`, loads the journal, calls
  `RollbackJournal::rollback()`, removes `install_dir(version)`, restores
  the tracker.

**Assertion**: 5 rollback tests from design pass.

### T8 — CLI flag surface

**Surface**: `crates/cognicode-cli/src/bin/cogh.rs`

**Change**: extend `Latest`, `Update` with `channel`, `base_url`,
`staging`, `json`, `dry_run`, `profile` flags per D8.

**Assertion**: `cargo build -p cognicode-cli --bin cogh` succeeds.

### T9 — `release_test_support::ResolverFixture`

**Surface**: `crates/cognicode-cli/src/cmd/release_test_support.rs`

**Change**: add

```rust
pub struct ResolverFixture { pub dir: PathBuf, pub base_url: String, ... }
pub fn resolver_fixture(version: &str) -> ResolverFixture;
```

Starts a tiny HTTP server that mimics `https://api.github.com/repos/.../releases/latest`
and `.../releases/tags/vX.Y.Z`. Useful for `cogh update --dry-run` tests.

**Assertion**: `test_resolver_via_fixture_*` tests pass.

### T10 — full E2E

**Surface**: `crates/cognicode-cli/src/cmd/lifecycle.rs` (new test)

**Change**: `test_cogh_e2e_full_cycle`:

1. `local_release("0.94.0")` → install v0.94.0.
2. `local_release("0.95.0")` → resolver picks it, install v0.95.0.
3. `cogh rollback` → state matches post-step-1.

**Assertion**: the test passes; the install dir for v0.94.0 is gone; the
tracker reads `0.94.0`.

### T11 — exit gates

**Assertion**:

- `cargo test -p cognicode-cli --bin cogh` — all tests pass, including
  the 147 existing tests.
- `cargo test -p cognicode-cli --bin cognicode-release` — all 35 tests
  still pass (no regression on R1-R9).
- `cargo check --workspace --all-targets` exit 0.
- `cargo fmt -p cognicode-cli --check` exit 0.
- `python3 scripts/check_known_failures.py` exit 0, count unchanged.
- `cargo test -p cognicode-core --test architecture_self_host_e2e`
  3/3 pass.
- `bash scripts/check-release-matrix.sh` RESULT: OK.

## Forecast

- New code: ~700-900 LOC across 2 new modules + extensions.
- New tests: ~22 (10 resolver + 6 rollback + 3 journal + 1 e2e + 2 misc).
- Files touched: 8.
- Forecast confidence: high (the surface is small, the e85 tests are a
  safety net).