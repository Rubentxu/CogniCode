# Proposal: `arch/debt3f-heuristic-elimination`

> Arch debt cycle (DEBT-3.f). Eliminates the heuristic identity
> bridges catalogued in `docs/adr/ADR-IDENTITY-MAP-distribution.md`
> §9.4-11. No public manifest formats change. No new dependencies.

## Why

`DEBT-3` (taxonomy adoption) was closed at `e61faa67`. The DEBT-3
ADR identified a residual set of *stringly-typed identity-bridging
literals* in the production CLI that are still coupled to specific
identity strings — typically `"cognicode-mcp"` or `"mcp-server"` —
where the bridge should come from an explicit source declared in the
manifest. Those residuals are catalogued in the §9 inference audit and
classified `Eliminate` or `Blocked (DEBT-2)`.

This cycle eliminates every `Eliminate` site and catalogues every
`Blocked (DEBT-2)` site with a pointer.

## Scope

| Site | Source | Classification | Action |
|---|---|---|---|
| `:423` | `installer_transaction.rs:423` (legacy binary location) | Eliminate | Replaced with `locate_component_binary(home, version, component_id)` helper in `installer_transaction.rs`. Strict T1 (`t_debt3f_strict_locate_binary_with_divergent_filename`) planted with binary stem ≠ ComponentId; T1b (`t_debt3f_strict_locate_binary_legacy_shape_still_works`) covers the legacy-shape path. |
| `:67` | `install.rs:67` (literal `"cognicode-mcp"` shim path) | Eliminate | Replaced with `home.shim_path(daemon_cli_binary_name(&home.version_manifest(version))?)`. Strict T2a/T2b gate the new helper. |
| `:730` | `ide.rs:730` (literal `"cognicode-mcp"` shim path) | Eliminate | Replaced with `home.shim_path(plugin_mcp_binary_name(&home.plugin(plugin).join("plugin.yaml"))?)`. Strict T3/T4 gate the new helper. |
| `:245, :266` | `ide.rs` opencode merge keys | Eliminate | Replaced with `mcp_merge_key_from_command(mcp_command)`. `uninstall_opencode` signature updated to take `binary_name: &str`. |
| `:397, :422` | `ide.rs` zcode merge keys | Eliminate | Same as above. `uninstall_zcode` signature updated. |
| `:494, :515` | `ide.rs` claude merge keys | Eliminate | Same as above. `uninstall_claude` signature updated; the IDE integration writes `~/.claude/mcp/<binary_name>.json`. |
| `:631, :665` | `ide.rs` codex merge keys | Eliminate | Same as above. `uninstall_codex` signature updated; the IDE integration inserts `[mcp_servers.<binary_name>]`. |
| `:737` | `ide.rs::cmd_ide_install` opencode `skill_path` | Blocked (DEBT-2) | Catalogued with `BLOCKED-BY-DEBT-2` pointer. Requires portable-skill-bundle manifest. |
| `:367-374` | `ide.rs::integrate_zcode` skills source | Blocked (DEBT-2) | Catalogued. |
| `:477-481` | `ide.rs::integrate_claude` skills source | Blocked (DEBT-2) | Catalogued. |
| `:593-597` | `ide.rs::integrate_codex` skills source | Blocked (DEBT-2) | Catalogued. |
| `:56-59` | `install.rs` "first directory under `skills_root`" | Blocked (DEBT-2) | Catalogued. |

## Strict T gates (RED → GREEN)

All gates are adversarial: three pairwise-distinct identities
(`PluginId = "mcp-server"`, `ComponentId = "cognicode-mcp"`,
`BinaryName ≠ ComponentId`) and the failure paths must surface the
identity gap explicitly.

| Test | What it pins | RED → GREEN commit |
|---|---|---|
| `installer_transaction::tests::t_debt3f_strict_locate_binary_with_divergent_filename` | Helper returns the divergent binary path, not a ComponentId alias | `e710348f` (helper) + `88c9282f` (relaxed 3rd leg) |
| `installer_transaction::tests::t_debt3f_strict_locate_binary_legacy_shape_still_works` | Legacy-shape path still resolves | `88c9282f` |
| `bundle_manifest::tests::t_debt3f_strict_daemon_cli_binary_name_uses_manifest_decl` | Helper reads the manifest, not a literal | `11d6b3be` |
| `bundle_manifest::tests::t_debt3f_strict_daemon_cli_binary_name_fails_loudly_when_no_daemon_cli` | Helper errors when no DaemonCli, doesn't fall back | `11d6b3be` |
| `manifest::tests::t_debt3f_strict_plugin_mcp_binary_name_uses_manifest_decl` | Helper reads `binaries[0].name`, not a literal | `11d6b3be` |
| `manifest::tests::t_debt3f_strict_plugin_mcp_binary_name_fails_loudly_when_no_binaries` | Helper errors on empty `binaries: []` | `11d6b3be` |
| `ide::tests::t_debt3f_merge_key_renamed_binary` | Helper returns BinaryName verbatim when ≠ ComponentId | `c1e47f8a` |
| `ide::tests::t_debt3f_merge_key_legacy_basename` | Legacy-shape binary returns verbatim, no special-case | `c1e47f8a` |
| `ide::tests::t_debt3f_merge_key_empty_command_fails_loudly` | Empty `mcp_command` errors | `c1e47f8a` |
| `ide::tests::t_debt3f_merge_key_unusable_filename_fails_loudly` | Path with no `file_name()` errors | `c1e47f8a` |
| `ide::tests::t_debt3f_claude_integrate_uses_binary_name_stem` | e2e: file stem = BinaryName; legacy stem NOT written | `c1e47f8a` |
| `ide::tests::t_debt3f_zcode_integrate_uses_binary_name_key` | e2e: JSON key = BinaryName; legacy key NOT present | `c1e47f8a` |
| `ide::tests::t_debt3f_codex_integrate_uses_binary_name_subtable` | e2e: TOML subtable key = BinaryName; legacy table NOT present | `c1e47f8a` |

## Architecture invariants

* No public manifest format is changed. `BundleManifest` and
  `PluginManifest` schemas are untouched.
* No new crate or dependency is added.
* No `SkillBundleId`, no new `ArtifactKind`, no `skill_bundles[]`
  manifest field is introduced. (DEBT-2 territory.)
* No silent aliases: every identity resolution that previously fell
  back to a literal now fails loudly.
* `cognicode-release` binary continues to compile without `layout.rs`
  (helper signatures take `&Path`, not `&CognicodeHome`, where the
  helper is reachable from both binaries).

## Acceptance evidence

* `cargo test -p cognicode-cli --bins` → 242 passed, 0 failed
  (37 in `cognicode-release`).
* `cargo check -p cognicode-cli --tests` → clean.
* `cargo fmt --check -p cognicode-cli` → only pre-existing
  DEBT-3 WU5 layout.rs drift remains (out of scope; L5 lesson).
* `cargo clippy -p cognicode-cli --no-deps --bin cogh --tests` →
  no new errors introduced by this cycle.
* `python3 scripts/check_known_failures.py` → 41 entries,
  unchanged baseline.

## Commits (in dependency order)

1. `e710348f` — `arch(debt3f): extract locate_component_binary helper (legacy shape)`
2. `88c9282f` — `arch(debt3f): relax locate_component_binary third leg to scan bin/`
3. `11d6b3be` — `arch(debt3f): derive BinaryName from manifest, eliminate :67/:730 literals`
4. `c1e47f8a` — `arch(debt3f): derive IDE merge keys from BinaryName, not literal`
5. `2276f9c0` — `arch(debt3f): catalog BLOCKED-BY-DEBT-2 sites in install/ide`

## Open follow-up (DEBT-2)

* The install transaction's manifest-writing path filters out the
  DaemonCli component for the `core` profile. `cmd_ide_uninstall`
  now strictly resolves the DaemonCli binary name from the manifest,
  which surfaces this gap (the round-trip test plants the
  DaemonCli metadata as a fixture). Whether the install transaction
  should keep DaemonCli metadata in the manifest regardless of
  profile is a separate architectural decision and is tracked as
  DEBT-2 territory.
* The five `BLOCKED-BY-DEBT-2` sites cannot be retargeted until a
  portable-skill-bundle manifest is modelled and a `bundle.id` is
  declared there.
