# L1 — Typed layout ownership helpers

> Status: **PASS** (no behaviour change)
> Closure date: 2026-09-18
> Apply commit: `24c38307`
> Initiative: `arch-canonical-layout`

## Origin

The architectural cycle `arch-canonical-layout` (post-E86.7
checkpoint) was opened to resolve the contradiction between the
canonical `versions/<v>/<component>/` layout (promoted by ADR-034,
ADR-035, `cognicode-cli/spec.md`, `cognicode-lifecycle/spec.md`,
`portable-skill-bundle/spec.md`) and the e74-era implementation
choice to write to `install/<v>/...` inside `InstallerTransaction`.

Phase A (ownership map) and Phase C (ADR) landed in
commit `fe63bbd3`. The ADR ratifies `versions/<v>/<component>/` as
the canonical layout and defines five typed helpers on
`CognicodeHome` as the L1 deliverable.

## What changed

`crates/cognicode-cli/src/cmd/layout.rs`:

* 5 new methods on `CognicodeHome`:

  | Method | Returns |
  |---|---|
  | `version_root(v)` | `<root>/versions/<v>/` |
  | `component_root(v, c)` | `<root>/versions/<v>/<c>/` |
  | `version_manifest(v)` | `<root>/versions/<v>/manifest.yaml` |
  | `skills_root(v)` | `<root>/versions/<v>/skills/` |
  | `skill_bundle(v, b)` | `<root>/versions/<v>/skills/<b>/` |

* 5 characterization tests (`t_l1_*`) pinning the exact path shape.
  The `version_manifest` test additionally asserts that the canonical
  path differs from the legacy `install_manifest_path` path — a
  regression guard against a future cycle collapsing the two.

## What did NOT change (deliberately)

* The install transaction still writes to `<root>/install/<v>/` via
  the existing `layout::install_manifest_path` helper.
* The IDE adapters (opencode, zcode, claude, codex) still read from
  `home.versions().join(version).join(plugin).join("skills")` (the
  existing `versions()` helper, not the new `version_root()`).
* The `install_manifest_path` method and free function are NOT
  retired yet — that is L5.

Net behaviour: zero. The on-disk layout and the user-visible CLI
output are identical to E86.7. The UAT Phase D reinstall would still
fail `link_or_copy` for the same reason it did before L1; L4 closes
that.

## Verification

* `cargo test -p cognicode-cli --bin cogh`:
  **223 passed / 0 failed / 1 ignored** (218 baseline + 5 new L1).
  No regressions on E86.x tests.
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 errors.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.

UAT re-run was deliberately skipped for L1 (no behaviour change).
The L4 cycle is the one that must demonstrate UAT Phase D reinstall
becoming GREEN.

## Cycle verdict

**PASS**. Bounded change: one file, 172 insertions, 0 deletions.
No new dependencies, no new crates, no new filesystem abstraction.
The five helpers are pure path arithmetic — no I/O, no side effects.

## Next cycle

**L2** — `InstallerTransaction` producer switches to the canonical
layout. The producer is the only writer today; after L2 it writes
to `<root>/versions/<v>/<component>/bin/<component>` instead of
`<root>/install/<v>/<component>/bin/<component>`. The journal envelope
records the same `SideEffect::CreatedDir` / `SideEffect::Extracted`
entries but against the new paths. Rollback remains in the journal.

L2 is bounded: ~3-line change in the Extracting stage (replace
`install_dir` derivation) + ~3-line change in the InstallingShims
stage (replace `install_dir.join(comp.name).join("bin")...` with
`home.version_root(v).join(comp.name).join("bin")...`) + ~3-line
change in the WritingManifest stage (use `home.version_manifest(v)`
instead of `layout::install_manifest_path(&v)`) + 1-3 RED tests
asserting the new on-disk shape.

If L2 surfaces a deeper contradiction (e.g. the bundle manifest
components don't match what the install transaction expects), STOP
and surface.
