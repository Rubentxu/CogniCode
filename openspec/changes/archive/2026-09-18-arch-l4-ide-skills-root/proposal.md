# L4 — IDE integration derives skill_path from skills_root

> Status: **PASS** (UAT Phase D reinstall now GREEN)
> Closure date: 2026-09-18
> Apply commit: `ab16e492`
> Initiative: `arch-canonical-layout`
> Binary SHA-256: `747d46b98138669ca1bc1d0992c7017077014bbf1c9258f5ca0197ab721e63ef`

## Origin

The UAT Phase D reinstall `link_or_copy failed` was the original
visible symptom that motivated the architectural cycle. Root cause:
`run_install` in `cmd/install.rs` hardcoded:

```rust
let skill_path = manifest_path
    .parent()
    .map(|p| p.join("mcp-server/skills"))
    .unwrap_or_else(|| PathBuf::from("~/.cognicode/skills"));
```

This pointed at:
- L2 path: the legacy `install/<v>/...` (no longer written by
  the producer).
- Component: `mcp-server` — a name the bundle manifest does not
  declare. The real bundle declares `cognicode` and `cognicode-mcp`.
- Layout mismatch: assumed a `skills/` subdir under the plugin
  root, contradicting both the bundle contract (`bin/<name>/<name>`)
  and the portable-skill-bundle spec (`versions/<v>/skills/<bundle>/`).

L4 closes that gap.

## What changed

* `cmd/install.rs::run_install` now derives the skill source from
  `home.skills_root(version)`. The integration is a no-op (with a
  warning) if the directory is empty.

* 1 new L4 test: `t_l4_install_emits_warning_when_no_skill_bundle_present`
  drives a real `run_install` against a temp home and asserts
  success even when no skill bundle is present. Pre-L4 the call
  surfaced `link_or_copy failed`.

## Verification

* `cargo test -p cognicode-cli --bin cogh`:
  **227 passed / 0 failed / 1 ignored** (226 baseline + 1 new L4).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 errors.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.

## Real-PC UAT

The UAT that previously failed Phase D reinstall now passes:

```
warning: no skill bundle found under /tmp/cogh-uat-real-pc-l4/cognicode-home/versions/0.95.0/skills; \
         skipping OpenCode integration
✓ OpenCode integration complete (no skill bundles to integrate)
Installed version 0.95.0 to /tmp/cogh-uat-real-pc-l4/cognicode-home/versions/0.95.0/manifest.yaml
```

Receipt status: `overall_status: ok`. Phase D events:

```
Phase D rollback:    ok
Phase D reinstall:   in_progress → ok (no link_or_copy failed)
Phase D uninstall:   ok
Phase D snapshot:    ok
```

The UAT-visible bug that started the architectural cycle is now
GREEN.

## Cycle verdict

**PASS**. Bounded change: 1 file, 83 insertions, 18 deletions.
The behavioural change is the `skills_root` derivation; the
empty-bundle path is the only behaviour shift from the user's
perspective.

## Next cycle

**L5** — Legacy install/ surface elimination.

Surface to retire (when no runtime consumer uses `install/`):

* Free fns: `layout::install_root()`, `layout::install_dir()`,
  `layout::install_manifest_path(version)`.
* Method: `CognicodeHome::install_manifest_path(version)`.
* E86.4 test that pins `home.install_manifest_path(v).starts_with(versions)`
  as a regression guard — that test pins the existence of the
  legacy path, so it must be retired alongside the method.

L5 is bounded: ~30 deletions (mostly removal of the `install_*`
helpers + their callers) + 1-2 RED tests asserting no `install/`
references remain.

After L5, the system has zero references to `install/<v>/...`
in source code. Any e74-era on-disk legacy trees are an
implementation detail of future migration (out of scope for L5;
filed separately).
