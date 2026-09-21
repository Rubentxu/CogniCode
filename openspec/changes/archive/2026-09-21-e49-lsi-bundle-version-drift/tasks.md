# Tasks — cycle e49 — bundle version drift (root-cause fix)

> Cycle: A-min | Phase: tasks | Date: 2025-09-15
> Change ID: `e49-lsi-bundle-version-drift`

## Work units

### WU-1 — Add `bundles/v0.94.15/bundle.yaml`

Copy `bundles/v0.94.14/bundle.yaml` to `bundles/v0.94.15/bundle.yaml`
and bump every `0.94.14` occurrence to `0.94.15` (top-level `version`,
component `version`, artifact names, and release URLs). Update
`released_at` to a current timestamp.

Rationale: the embedded path will be derived from `CARGO_PKG_VERSION`
(WU-2); a matching bundle must exist for the build to compile.

### WU-2 — Derive the embedded bundle path from `CARGO_PKG_VERSION`

In `crates/cognicode-cli/src/cmd/installer_transaction.rs`, in
`load_bundle_manifest()`, replace:

```rust
Ok(include_str!("../../../../bundles/v0.94.14/bundle.yaml").to_string())
```

with:

```rust
Ok(include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../bundles/v",
    env!("CARGO_PKG_VERSION"),
    "/bundle.yaml"
))
.to_string())
```

Rationale: single source of truth. A version bump without a matching
bundle becomes a compile error.

### WU-3 — Regression test: embedded bundle version == CARGO_PKG_VERSION

Add a unit test in `crates/cognicode-cli/src/cmd/installer_transaction.rs`
`mod tests` (or `bundle_manifest.rs` tests) that:
1. parses the embedded bundle via `BundleManifest::from_str`,
2. asserts `manifest.version == env!("CARGO_PKG_VERSION")`,
3. calls `manifest.assert_pkg_version()` and asserts `Ok`.

Rationale: the compile-time path check catches a missing directory, but
not a bundle whose internal `version:` field was forgotten. This test
closes that gap.

### WU-4 — Fix `cogh_bin()` to use `current_exe()`

In `crates/cognicode-cli/src/cmd/lifecycle.rs`, replace the hard-coded
`<workspace>/target/debug/cogh` resolution:

```rust
let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
workspace_root.join("target").join("debug").join("cogh")
```

with a `current_exe()`-relative resolution:

```rust
let exe = std::env::current_exe().expect("current_exe should be available in tests");
exe.parent()
    .and_then(|p| p.parent())
    .expect("test executable should live in <target-dir>/debug/deps")
    .join(format!("cogh{}", std::env::consts::EXE_SUFFIX))
```

Rationale: with a custom `CARGO_TARGET_DIR`, the hard-coded path resolves
to a stale binary and the subprocess lifecycle tests silently test it.

## Sequencing

Apply WU-1 → WU-2 → WU-3 → WU-4 in one commit.

## Acceptance gate

- `bundles/v0.94.15/bundle.yaml` exists with `version: "0.94.15"`.
- `installer_transaction.rs` contains no hard-coded `v0.94.` literal.
- `lifecycle.rs` contains no hard-coded `join("target")` path.
- `cargo build -p cognicode-cli` succeeds.
- New regression test passes.
- `cargo test -p cognicode-cli -- --test-threads=1`:
  **101 passed; 0 failed; 1 ignored** (previously 2 deterministic
  version-mismatch failures), all integration suites green.
- `cargo test -p cognicode-cli ide::tests` still 20/20.
- Conventional commit, no AI trailers.
