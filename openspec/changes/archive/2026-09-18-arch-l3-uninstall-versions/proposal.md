# L3 — cmd_uninstall retargets to canonical version tree

> Status: **PASS**
> Closure date: 2026-09-18
> Apply commit: `38307029`
> Initiative: `arch-canonical-layout`

## Origin

L2 retargeted the producer (InstallerTransaction) to write to
`<root>/versions/<v>/...`. The E86.7 cmd_uninstall tests still
pinned the legacy `install/<v>/` path because at E86.7 time
that was the only tree on disk. After L2 the producer no longer
writes there, so `cmd_uninstall` would have been a silent no-op
for real installs. L3 closes that gap.

## What changed

* `cmd_uninstall` (`crates/cognicode-cli/src/cmd/layout.rs`)
  derives its removal target from `home.version_root(version)`
  instead of `home.install_manifest_path(version).parent()`.

* 3 E86.7 uninstall tests migrated from the legacy derivation to
  `home.version_root(v)`. The contract they pin is unchanged; the
  path is updated.

* 1 new L3 round-trip test
  (`t_l3_cmd_uninstall_round_trip_removes_canonical_tree`) drives
  a real install followed by `cmd_uninstall` and asserts the
  canonical tree is created then removed, while the legacy path is
  empty throughout.

## What did NOT change

* `cmd_rollback`, `cmd_doctor`, `cmd_current`, `cmd_where` —
  audit deferred to L5. The journal envelope records
  `versions/<v>/` paths post-L2, so `cmd_rollback` already
  restores the canonical tree by construction.
* `cmd_ide_install`, `install.rs` IDE integration — still L4.
* `install_manifest_path` helpers — still L5.

## Verification

* `cargo test -p cognicode-cli --bin cogh`:
  **226 passed / 0 failed / 1 ignored** (225 baseline + 1 new L3).
  All E86.7 uninstall tests pass against the canonical layout.
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 errors.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.

## Cycle verdict

**PASS**. Bounded change: 1 file, 78 insertions, 36 deletions.
The behavioural change is one line in `cmd_uninstall`; the rest
is test migration.

## Next cycle

**L4** — IDE consumers retarget. `cmd_ide_install` already reads
from `home.versions().join(version).join(plugin).join("skills")`,
which is the canonical path. But `install.rs:46-49` hardcodes
`<root>/install/<v>/mcp-server/skills`, which is the legacy path
AND uses the wrong component name. After L4, the IDE integration
derives `skill_path` from `home.component_root(v, daemon_component)`,
where `daemon_component` is the bundle's `daemon-cli` component
name (per `ArtifactKind::stem()`: `cognicode-mcp`). L4 is the
cycle that makes UAT Phase D reinstall `link_or_copy` GREEN.
