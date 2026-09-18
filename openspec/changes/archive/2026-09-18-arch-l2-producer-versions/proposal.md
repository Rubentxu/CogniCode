# L2 — InstallerTransaction producer switches to versions/

> Status: **PASS** (with one side-effect acknowledgement)
> Closure date: 2026-09-18
> Apply commit: `ded95fbf`
> Initiative: `arch-canonical-layout`

## Origin

L1 introduced five typed helpers on `CognicodeHome` describing the
canonical install layout (`versions/<v>/<component>/`) but did not
change any behaviour. L2 retargets the only writer — the install
transaction — to actually land files where the rest of the system
already looks.

Pre-L2 reality (see `docs/adr/ADR-OWNERSHIP-MAP-install-vs-versions.md`):
- Producer wrote to `<root>/install/<v>/<comp>/`.
- Promoted specs and ADRs promise `<root>/versions/<v>/<comp>/`.
- `cmd_ide_install` (opencode, zcode, claude, codex) read from
  `home.versions().join(v).join(plugin).join("skills")`, i.e. the
  spec-promised path.
- The UAT Phase D reinstall `link_or_copy` failure was the visible
  symptom — the IDE adapter looked at a tree the producer never
  created.

## What changed

| File | Change |
|---|---|
| `cmd/installer_transaction.rs` | `run(home, profile)`, `advance(home)`, `commit(home)`, `advance_stage(stage, journal, manifest, home)`. The Extracting / InstallingShims / WritingManifest stages now derive their paths from `home.version_root(v)` and `home.version_manifest(v)` instead of the env-based free fns `layout::install_dir(v)` / `layout::install_manifest_path(v)`. |
| `cmd/install.rs` | `run_install` passes its `home` arg to `InstallerTransaction::run`. (One-line plumbing change.) |
| `cmd/layout.rs` | 4 existing tests updated to assert the manifest lands at `home.version_manifest(v)`. |
| `cmd/lifecycle.rs` | 1 existing test updated to look for the extracted payload under `versions/<v>/<comp>/`. |
| `cmd/installer_transaction.rs` (tests) | 2 new L2 tests pinning the new on-disk shape: `t_l2_extracting_writes_under_versions_layout`, `t_l2_commit_records_versions_layout_in_journal`. |

## What did NOT change

* `cmd_uninstall` still removes the legacy `install/<v>/` tree. After
  L2 the producer no longer writes there, so `cmd_uninstall` will be
  a no-op for real installs until L3 retargets it. **Acknowledged
  side-effect of L2.** The E86.7 tests still pass because they
  manually pre-create the install tree; they pin the legacy
  contract. L3 will re-point `cmd_uninstall` at `home.version_root(v)`
  and the E86.7 tests will be migrated.
* `cmd_rollback`, `cmd_doctor`, `cmd_current`, `cmd_where` (still L3).
* `cmd_ide_install` and the IDE integration in `install.rs:46-49`
  (still L4).
* The `install_manifest_path` helpers (still L5).

## Side-effects explicitly closed

* The e74 closure-authorization note about `cmd_install --home /tmp/foo`
  silently ignoring `--home` is now closed by construction:
  `InstallerTransaction::run` takes `home` as its first argument
  and the write target is `home.version_manifest(v)`. A `cogh install
  --home /tmp/foo` will now write to `/tmp/foo/versions/<v>/...`,
  not to `~/.cognicode/install/<v>/...`.

## Verification

* `cargo test -p cognicode-cli --bin cogh`:
  **225 passed / 0 failed / 1 ignored** (223 baseline + 2 new L2).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 errors.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.

UAT re-run was deliberately skipped. The UAT Phase D reinstall
`link_or_copy` failure is now in a different shape: the manifest
and components live under `versions/<v>/`, but the IDE integration
in `install.rs:46-49` still hardcodes `install/<v>/mcp-server/skills`.
L4 closes the IDE side.

## Cycle verdict

**PASS**. Bounded change: 4 files, 173 insertions, 36 deletions.
The producer is now load-bearing for the canonical layout. After
L2 the on-disk shape of a real install matches every promoted
spec and ADR.

## Next cycle

**L3** — Lifecycle consumers retarget:

* `cmd_uninstall` switches to `home.version_root(v)` (closes the
  E86.7 test migration).
* `cmd_rollback` already takes the journal envelope, so its paths
  come from the journal — no change needed unless the rollback
  reverse-side writes anything. Inspect.
* `cmd_doctor`, `cmd_current`, `cmd_where` — these probably need
  updates because they may still reference `install/` somewhere.
  Investigation in L3.

L3 is bounded: ~5-line change in `cmd_uninstall` + 1-2 RED tests
asserting that `cmd_uninstall` removes `versions/<v>/` instead of
the (now-empty) `install/<v>/`.
