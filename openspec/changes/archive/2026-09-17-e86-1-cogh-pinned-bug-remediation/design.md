# e86.1 — Design

## Bug 1: rollback of populated `CreatedDir`

### Code change

`crates/cognicode-cli/src/cmd/rollback_journal.rs` (line 262):

```rust
SideEffect::CreatedDir(path) => {
    // Idempotent: a sibling reversal may have already removed
    // this dir (e.g. a later `CreatedDir(install_dir)` recorded
    // for the manifest parent dir uses `remove_dir_all`, which
    // removes this dir as a side effect). Only attempt removal
    // when the path still exists.
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|e| {
            InstallerError::Rollback(format!(
                "remove CreatedDir {}: {}",
                path.display(),
                e
            ))
        })?;
    }
}
```

### Reversal order: LIFO, why it works

Recording order in `installer_transaction.rs`:

```text
1.  CreatedDir(cache_dir)              ← outer dir
2.  Downloaded(dest)                   ← file inside cache_dir
3.  VerifiedSha256(path)               ← no-op on rollback
4.  CreatedDir(install_dir)            ← outer dir, recorded when creating install
5.  Extracted(component_dir)           ← dir inside install_dir
6.  CreatedDir(shims_dir)              ← outer dir
7.  CreatedSymlink                     ← file inside shims_dir
8.  CreatedDir(parent_of_manifest)     ← == install_dir (manifest_path = install_dir/manifest.yaml)
9.  WroteManifest(manifest_path)       ← file inside install_dir
10. WroteTracker { ... }               ← file in tracker_dir (sibling of install_dir)
```

Reversal (LIFO, 10→1):

- `WroteTracker` reversed (file removed or restored).
- `WroteManifest` reversed (file removed).
- `CreatedDir(install_dir)` (step 8). At this point, step 5
  (`Extracted`) is still in place, so install_dir is non-empty.
  With the new `remove_dir_all`, the dir is swept including the
  extracted component dir. ✓
- `CreatedSymlink` reversed.
- `CreatedDir(shims_dir)` reversed (now empty after symlink
  removal).
- `Extracted(component_dir)` reversed. Path doesn't exist (swept
  in step 8 reversal). The existing `if path.is_dir()` guard
  makes this a no-op. ✓
- `VerifiedSha256` no-op.
- `Downloaded(dest)` reversed.
- `CreatedDir(cache_dir)` (step 1). Path empty now (file removed
  in step 2 reversal). `remove_dir_all` succeeds. ✓
- `CreatedDir(install_dir)` (step 4). Path doesn't exist (swept
  in step 8 reversal). The new `if path.exists()` guard makes
  this a no-op. ✓

## Bug 2: stale-shim regression

### Code change

`crates/cognicode-cli/src/cmd/platform_adapter.rs` (LinuxAdapter
+ MacOsAdapter — Windows was already idempotent via `fs::copy`):

```rust
fn install_shim(&self, bin_path: &Path, shim_path: &Path) -> Result<ShimSideEffect> {
    if let Some(parent) = shim_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create shim parent {}", parent.display()))?;
    }
    // e86.1: idempotent on re-install. A previous install may have
    // left a symlink at `shim_path` (pointing at the previous
    // version's bin); `std::os::unix::fs::symlink` returns EEXIST
    // in that case. Detect and remove the existing link first;
    // record the same `CreatedSymlink` side-effect either way.
    if let Ok(existing_target) = std::fs::read_link(shim_path) {
        if existing_target == bin_path {
            // Already the correct link — no-op.
            return Ok(ShimSideEffect::Symlinked {
                link: shim_path.to_path_buf(),
                target: bin_path.to_path_buf(),
            });
        }
        std::fs::remove_file(shim_path).with_context(|| {
            format!("remove existing shim {}", shim_path.display())
        })?;
    } else if shim_path.exists() {
        // Not a symlink (e.g. a real file from a Windows-style
        // shim copy, or a corrupt path). Remove before relinking.
        std::fs::remove_file(shim_path).with_context(|| {
            format!("remove existing shim {}", shim_path.display())
        })?;
    }
    std::os::unix::fs::symlink(bin_path, shim_path).with_context(|| {
        format!("symlink {} -> {}", shim_path.display(), bin_path.display())
    })?;
    Ok(ShimSideEffect::Symlinked {
        link: shim_path.to_path_buf(),
        target: bin_path.to_path_buf(),
    })
}
```

### Why this is idempotent without losing the journal entry

The `ShimSideEffect::Symlinked { link, target }` value is
unchanged regardless of whether the shim was a no-op, removed,
or newly created. The rollback path uses `remove_file(link)`
(line 287 of `rollback_journal.rs`), which is idempotent
(success for a missing file via the `if link.exists()` guard).

### Test additions

- `cmd_update_sequential_installs_succeed_cleanly` (new strict
  tripwire in `layout.rs:1377`).
- `cmd_update_sequential_installs_overwrite_cleanly` (kept for
  diagnostic value — the conditional match accepts either
  outcome).

## Bug 3: zero-component profile

### Code change

`crates/cognicode-cli/src/cmd/error.rs`:

```rust
/// The selected profile matched zero components in the bundle
/// manifest (e86.1 REQ-LJ-04). Without this, a typo'd profile
/// would silently install nothing while still pinning the
/// tracker and writing the lifecycle journal — the most
/// insidious masking failure mode in the install pipeline.
#[error("profile {0:?} matches no components in bundle manifest version {1}; refusing to install nothing")]
EmptyInstall(String, String),
```

`crates/cognicode-cli/src/cmd/installer_transaction.rs` (in
`run`):

```rust
// e86.1 REQ-LJ-04: refuse to install a zero-component profile
// silently. Without this, a typo'd `--profile core-typo` exits
// `Ok(())` while having installed nothing — the most insidious
// masking failure mode in the install pipeline.
if manifest.components.is_empty() {
    return Err(InstallerError::EmptyInstall(
        profile.to_string(),
        manifest.version.clone(),
    ));
}
```

### Why this is the right place

The check happens AFTER the manifest is parsed and validated
(host platform check, etc.) but BEFORE any stage runs. No
journal is created, no manifest is written, no tracker is
touched. The journal's `Drop` impl is a no-op because
`commit()` was never called.

### Test change

`cmd_update_zero_component_profile_does_not_pin_tracker` is
**renamed** to
`cmd_update_zero_component_profile_returns_empty_install_error`,
the `#[ignore]` is dropped, and the body is tightened to:

- `expect_err(...)` instead of `if result.is_ok() { ... }`.
- Assert the error message contains "EmptyInstall" or
  "matches no components" or the profile name.
- Assert the tracker, journal, AND manifest are absent.

## Test count summary

| | Before e86.1 | After e86.1 | Δ |
|---|---|---|---|
| `cargo test -p cognicode-cli --bin cogh` | 180 passed | 184 passed | +4 |
| `#[ignore]`d | 2 | 1 | -1 |

The +4 new tests:
- `cmd_update_zero_component_profile_returns_empty_install_error`
  (renamed from `..._does_not_pin_tracker`, `#[ignore]` dropped).
- `cmd_update_sequential_installs_succeed_cleanly` (strict
  tripwire).
- `test_rollback_reverses_populated_created_dir` (new unit).
- `test_rollback_is_idempotent_for_double_created_dir` (new
  unit).

The +1 ignored test that remains
(`test_cogh_install_runs_successfully`) is the pre-existing
bundle-version-mismatch pinning. It is unrelated to this cycle.

## Risks

- `remove_dir_all` for `CreatedDir` is a semantic shift. Any test
  that depended on the empty-only behaviour will break. Pre-audit:
  12 rollback_journal tests + 3 cmd_rollback tests; all pass on
  `HEAD` with the new code.
- The idempotent shim fix changes the `install_shim` semantics on
  Linux/macOS. Any test that depends on `symlink` failing on
  re-install will now pass without the expected error. Pre-audit:
  no test depends on this failure mode (the conditional match in
  `cmd_update_sequential_installs_overwrite_cleanly` accepts both
  outcomes, so it still passes).
- `InstallerError::EmptyInstall` is a new variant. Any caller that
  matches on `InstallerError` exhaustively will fail to compile.
  Audit: `cmd_update` uses `?` propagation, not exhaustive
  matching. No caller change needed.

## Out of scope (carried forward, not addressed)

- The pre-existing `boundary_tests.rs` fmt drift (`14a3a721`, e79).
- The pre-existing CLI help mis-descriptions (Class C from e84.1
  WU19).
- The `cmd_update` real download path (today dry-run + plumbing).
- The `ResolverFixture` shared fixture refactor.
- Clippy `-D warnings` (pre-existing).
