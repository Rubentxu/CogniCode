# L5 — Retire legacy `install/<v>/` surface

> Status: **PASS** (zero `install/<v>/` source pollution)
> Closure date: 2026-09-18
> Apply commit: `f86037c1`
> Initiative: `arch-canonical-layout`

## Origin

After L1–L4 every consumer and the producer converge on the
canonical `versions/<v>/<comp>/` layout per
`ADR-CANONICAL-LAYOUT-versions.md`. The legacy `install/<v>/...`
path-derivation surface was now dead code — it pointed at a
tree nothing wrote to and nothing read from. L5 retires it.

## What changed

* `crates/cognicode-cli/src/cmd/layout.rs`:
  - `pub fn install_root()` — deleted.
  - `pub fn install_dir(version: &str)` — deleted.
  - `pub fn install_manifest_path(version: &str)` — deleted.
  - `CognicodeHome::install_manifest_path(&self, version: &str)`
    method and its doc comment — deleted.
  - Block-level comment in the L1 helper group updated to reflect
    that L5 has actually retired the legacy surface.
  - 2 E86.4 tests (`t_e86_4_install_manifest_path_method_matches_install_layout`
    and `t_e86_4_install_manifest_path_method_under_versioned_subdir`)
    — deleted (they pinned the now-removed method).
  - The L1 character test `t_l1_version_manifest_matches_versions_layout`
    — assertion swapped from
    `version_manifest != install_manifest_path` (impossible after
    L5) to `version_manifest starts_with home.versions()`
    (the inverse semantics with a stable lifetime).
  - 1 new L5 test: `t_l5_zero_install_layout_source_pollution`.
    Walks every .rs source file in the workspace (skipping
    `cmd/layout.rs` itself) and fails if any of these patterns
    appear:
      - `pub fn install_root`
      - `pub fn install_dir`
      - `pub fn install_manifest_path`
      - `home.install_manifest_path`
    Local variables called `install_dir` (which now hold
    `home.version_root(...)` paths) and historical references
    in the two ADRs are explicitly NOT flagged — only the
    structural retargeting surface is pinned.

* `crates/cognicode-cli/src/cmd/installer_transaction.rs`:
  - 2 inverse-assertion tests (L2 and L3 legacy-`install/`-
    must-NOT-exist checks) updated to read
    `home.root.join("install").join(v)` directly instead of
    through the now-removed `home.install_manifest_path(v)`
    helper. The assertions themselves are unchanged.

## Verification

* `cargo test -p cognicode-cli --bin cogh`:
  **226 passed / 0 failed / 1 ignored**. The count rolls:
  L4 left 227 (226 baseline + 1 L4 test). L5 retires the 2
  E86.4 tests and adds 1 new L5 test. Net: -2 + 1 = -1 → 226.
* `cargo fmt --check`: clean.
* `cargo check --workspace`: clean.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 errors.
* `python3 scripts/check_known_failures.py`: 41 entries
  (cognicode-core lib), baseline unchanged.

## What did NOT change

* The 5 canonical layout helpers
  (`version_root`, `component_root`, `version_manifest`,
  `skills_root`, `skill_bundle`) introduced in L1 — unchanged.
* The install transaction producer — already writes to
  `versions/<v>/` after L2.
* `cmd_uninstall` — already reads `home.version_root(v)` after L3.
* `cmd/install.rs::run_install` — already derives skill path
  from `home.skills_root(version)` after L4.
* The IDE adapters (opencode / zcode / claude / codex) — already
  read from `versions/<v>/<plugin>/skills` per ADR-035.
* The two ADRs (`ADR-OWNERSHIP-MAP-install-vs-versions.md`,
  `ADR-CANONICAL-LAYOUT-versions.md`) — they explicitly reference
  the retired identifiers by name as historical record; they
  stay as they are.

## Cycle verdict

**PASS**. Bounded change: 2 files, 172 insertions, 133 deletions
(net ���39 deletions, the rest is the L5 zero-pollution test
scaffolding and updated comments).

The architectural cycle `arch-canonical-layout` is now
complete: the contradiction between
`versions/<v>/<component>/` (ADR-034/035 + three OpenSpec
specs) and the e74-era `install/<v>/...`
(InstallerTransaction) is fully resolved, with a permanent
regression guard against the legacy surface reappearing.

## Follow-ups filed separately (NOT part of this archive)

* Portable-skill-bundle modelling: the bundle manifest still
  models runtime components (cognicode, cognicode-mcp) but not
  portable skill bundles (cognicode-core, cognicode-mcp-driven).
  L4's "first directory under skills_root" heuristic remains
  the fallback until a separate ADR adds a skill bundle kind
  to ArtifactKind (or a parallel skill bundle manifest format).
* HOME config drift: `opencode.json` sha changed during test
  runs (likely from cargo test setting OPENCODE_CONFIG for some
  fixtures, or from `write_json_atomic` semantic no-op
  re-serialisation). Not L5-specific.
* Plugin-vs-component naming reconciliation:
  `mcp-server` vs `cognicode-mcp`. This is the same
  ambiguity L4 surfaced (the legacy `install/<v>/mcp-server/skills`
  hardcode happened because `mcp-server` was the assumed
  component name).
* Journal/uninstall retention policy: not addressed by L1-L5.
