# Exploration Report — cycle e49 — bundle version drift (root-cause fix)

> Cycle: A-min | Phase: explore | Date: 2026-09-15

## Trigger

Cycle e48 left 2 deterministic failures in `cargo test -p cognicode-cli`:

```
install failed: Err(install failed: version mismatch: manifest error:
bundle version `0.94.14` does not match cogh's CARGO_PKG_VERSION `0.94.15`;
you are running a mismatched installer (CLI upgrade required))
```

Affected tests:

- `lifecycle::tests::test_clean_home_install`
- `lifecycle::tests::test_install_with_ide_and_profile_dispatches_both`

These fail even under `--test-threads=1`, so they are deterministic
product defects, not test-race flakiness.

## Investigation

### Symptom

`assert_pkg_version()` in `crates/cognicode-cli/src/cmd/bundle_manifest.rs:178`
strictly compares the loaded bundle manifest's `version` field against
`env!("CARGO_PKG_VERSION")`. Any mismatch aborts the whole install
transaction before any stage executes.

### Where the manifest comes from

`InstallerTransaction::load_bundle_manifest()` in
`crates/cognicode-cli/src/cmd/installer_transaction.rs:254`:

```rust
fn load_bundle_manifest() -> Result<String, InstallerError> {
    let bundle_path = layout::bundle_yaml_path();   // ~/.cognicode/bundle.yaml
    if bundle_path.exists() {
        std::fs::read_to_string(&bundle_path)...
    } else {
        // Embedded fallback for distribution installers
        Ok(include_str!("../../../../bundles/v0.94.14/bundle.yaml").to_string())
    }
}
```

The `include_str!` is a **hard-coded path with a hard-coded version
segment `v0.94.14`** while the workspace version is `0.94.15`.

### On-disk state

```
bundles/v0.94.1/bundle.yaml    version: "0.94.1"
bundles/v0.94.11/bundle.yaml   version: "0.94.11"
bundles/v0.94.14/bundle.yaml   version: "0.94.14"
```

There is **no `bundles/v0.94.15/`**. On a clean home (tests use a temp
`HOME`, so `~/.cognicode/bundle.yaml` does not exist) the embedded
fallback resolves to `v0.94.14` → version mismatch → install aborts.

### History — this is a recurring maintenance trap

| Commit | What it did |
|--------|-------------|
| `5bbeaf94` | "fix(cogh): ... bundle version drift". Added `bundles/v0.94.14/bundle.yaml` and repointed the `include_str!` from `v0.94.11` → `v0.94.14`. |
| `eec319e2` | "chore(release): bump workspace version 0.94.14 → 0.94.15". Bumped `Cargo.toml` but **did not add a matching bundle** → drift reintroduced. |

The drift has now occurred at least twice (`0.94.11` and `0.94.14`).
The version bump and the bundle addition are two independent manual
steps, so they inevitably drift.

## Root cause

There are **two independent defects**, both of which must be fixed for
`test_install_with_ide_and_profile_dispatches_both` to pass:

### RC-1 — version-drifted embedded bundle (production defect)

The embedded bundle path is a **manually-maintained version literal**
that must be kept in lockstep with `CARGO_PKG_VERSION`, but nothing
enforces that lockstep. When it drifts, the failure is **silent until
runtime**: the crate still compiles, and only a fresh install on a
clean home surfaces the mismatch.

### RC-2 — stale binary in the lifecycle unit tests (test-infra defect)

`cogh_bin()` in `crates/cognicode-cli/src/cmd/lifecycle.rs:31`
hard-codes `<workspace>/target/debug/cogh`:

```rust
let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
workspace_root.join("target").join("debug").join("cogh")
```

But this environment builds to a custom `CARGO_TARGET_DIR`
(`/var/home/rubentxu/cargo-targets`, confirmed via `cargo metadata`).
`<workspace>/target/debug/cogh` therefore resolves to a **stale binary
from 2026-08-16** that still embeds the drifted `v0.94.14` bundle. Every
subprocess-based lifecycle test (`run_cogh`, `setup_temp_home`) silently
exercised that stale binary. The integration tests in `tests/` do not
have this bug — they correctly use `env!("CARGO_BIN_EXE_cogh")`.

RC-2 masked RC-1 in test output: the child process failed with the
version mismatch and the asserts only observed the missing side effect
(a missing tracker file), not the child's stderr.

## Why this matters beyond tests

This is a **production defect**, not just a test artifact:

- A user running `cogh install <profile>` on a machine without a
  pre-existing `~/.cognicode/bundle.yaml` (i.e. any first-time install
  from a distribution installer) is told "you are running a mismatched
  installer (CLI upgrade required)" even though they are running the
  correct, up-to-date CLI.
- The CLI is effectively un-installable from the embedded fallback
  whenever the workspace version has been bumped without a matching
  bundle.

## Landscape

| Path | Role |
|------|------|
| `crates/cognicode-cli/src/cmd/installer_transaction.rs:254-262` | embedded fallback loader (the defect site) |
| `crates/cognicode-cli/src/cmd/bundle_manifest.rs:178-190` | `assert_pkg_version` (the gate) |
| `crates/cognicode-cli/src/cmd/layout.rs:55` | `bundle_yaml_path()` = `~/.cognicode/bundle.yaml` |
| `bundles/v0.94.14/bundle.yaml` | previous co-versioned bundle |
| `crates/cognicode-cli/src/cmd/bundle_manifest.rs:500-524` | existing `assert_pkg_version` unit tests |

## Strategy (proposed)

**Fix the root cause, not the symptom.**

1. **Derive the embedded path from `CARGO_PKG_VERSION`** so the crate
   version is the single source of truth:

   ```rust
   include_str!(concat!(
       env!("CARGO_MANIFEST_DIR"),
       "/../../bundles/v", env!("CARGO_PKG_VERSION"), "/bundle.yaml"
   ))
   ```

   With this, a future version bump without a matching bundle produces
   a **compile error** (`couldn't read .../bundles/vX/bundle.yaml`),
   not a silent runtime failure. Drift becomes impossible.

2. **Add `bundles/v0.94.15/bundle.yaml`** (copy of `v0.94.14` with the
   version fields bumped) so the current build compiles.

3. **Add a regression test** asserting the embedded manifest's
   `version` equals `CARGO_PKG_VERSION`. This catches the case where a
   bundle directory exists but its internal `version:` field was not
   bumped (the compile-time path check would pass, but the runtime
   gate would still fail).

4. **Fix `cogh_bin()` (RC-2)** to derive the binary path from
   `std::env::current_exe()` instead of a hard-coded
   `<workspace>/target/debug` literal. The unit-test executable lives at
   `<target-dir>/debug/deps/cogh-<hash>`, so its companion binary is two
   directories up. This keeps the lifecycle subprocess tests honest under
   any `CARGO_TARGET_DIR`.

## Impact model

| Artifact | Impact | Confidence |
|----------|--------|-----------|
| `installer_transaction.rs` | production code (embedded loader) | KNOWN |
| `bundles/v0.94.15/bundle.yaml` | new artifact | KNOWN |
| `cargo test -p cognicode-cli` lifecycle tests | 2 deterministic failures expected to clear | LIKELY |
| conformance matrix | no spec delta; specs already verified | KNOWN |
| other crates | none (bundle logic is CLI-only) | LIKELY |

## Out of scope

- Garbage-collecting the stale `bundles/v0.94.1` and `bundles/v0.94.11`
  directories (historical; harmless).
- Un-ignoring `lifecycle::tests::test_cogh_install_runs_successfully`
  (it is `#[ignore]`d with a related reason; can be revisited once the
  version gate is proven, but is not required by this cycle).
- The residual shared-state race in `install_lock::tests` / other
  `lifecycle::tests` (that is a separate thread-safety concern).
