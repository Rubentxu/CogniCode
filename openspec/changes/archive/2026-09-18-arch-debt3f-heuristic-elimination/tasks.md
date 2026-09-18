# Tasks: `arch/debt3f-heuristic-elimination`

> Arch debt cycle (DEBT-3.f). Eliminated identity-bridging
> heuristics in `cmd/ide.rs`, `cmd/install.rs`, and
> `cmd/installer_transaction.rs`. Catalogued `Blocked (DEBT-2)`
> sites with audit references.

## Site-by-site status

### `:423` (`installer_transaction.rs`) — DONE

* Extracted `locate_component_binary(home, version, component_id)`
  helper.
* Strict T1 planted (`t_debt3f_strict_locate_binary_with_divergent_filename`).
* Strict T1b planted (`t_debt3f_strict_locate_binary_legacy_shape_still_works`).
* Commits: `e710348f`, `88c9282f`.

### `:67` (`install.rs`) — DONE

* `home.shim_path("cognicode-mcp")` → `home.shim_path(daemon_cli_binary_name(&home.version_manifest(version))?)`.
* Strict T2a/T2b gate the helper.
* Commit: `11d6b3be`.

### `:730` (`ide.rs`) — DONE

* `home.shims().join("cognicode-mcp")` → `home.shim_path(plugin_mcp_binary_name(&home.plugin(plugin).join("plugin.yaml"))?)`.
* Strict T3/T4 gate the helper.
* Commit: `11d6b3be`.

### `:245, :266` (`ide.rs` opencode merge keys) — DONE

* Added `mcp_merge_key_from_command(mcp_command) -> Result<String>`
  helper.
* `integrate_opencode:245` uses the helper.
* `uninstall_opencode(version, binary_name)` signature updated.
* `cmd_ide_uninstall` resolves BinaryName once and threads it down.
* Commit: `c1e47f8a`.

### `:397, :422` (`ide.rs` zcode merge keys) — DONE

* Same pattern as opencode.
* Commit: `c1e47f8a`.

### `:494, :515` (`ide.rs` claude merge keys) — DONE

* Writes `~/.claude/mcp/<binary_name>.json`.
* Strict T (`t_debt3f_claude_integrate_uses_binary_name_stem`)
  asserts the file stem = BinaryName, NOT ComponentId.
* Commit: `c1e47f8a`.

### `:631, :665` (`ide.rs` codex merge keys) — DONE

* Inserts `[mcp_servers.<binary_name>]`.
* Strict T (`t_debt3f_codex_integrate_uses_binary_name_subtable`)
  asserts the subtable key = BinaryName.
* Commit: `c1e47f8a`.

### `:737` (`ide.rs::cmd_ide_install` opencode `skill_path`) — CATALOGUED

* `BLOCKED-BY-DEBT-2` annotation with audit reference.
* Follow-up: replace with `home.skill_bundle(version, &bundle_id)`
  when DEBT-2 introduces a portable-skill-bundle manifest.
* Commit: `2276f9c0`.

### `:367-374` (`integrate_zcode` skills source) — CATALOGUED

* Same shape as `:737`; same follow-up.
* Commit: `2276f9c0`.

### `:477-481` (`integrate_claude` skills source) — CATALOGUED

* Same.
* Commit: `2276f9c0`.

### `:593-597` (`integrate_codex` skills source) — CATALOGUED

* Same.
* Commit: `2276f9c0`.

### `:56-59` (`install.rs` "first directory under `skills_root`") — CATALOGUED

* `BLOCKED-BY-DEBT-2` annotation.
* Follow-up: read `bundle.id` from a portable-skill-bundle
  manifest instead of `read_dir().next()`.
* Commit: `2276f9c0`.

## Test fixtures updated

* `cmd_uninstall` characterization tests in `cmd/layout.rs` now
  plant a real bundle manifest with a `DaemonCli` component. The
  previous behaviour tolerated a manifest-less home; the new
  behaviour fails loudly when the manifest is missing or has no
  DaemonCli entry — that is the contract.
* Pre-existing tests that called `uninstall_*(version)` were
  updated to pass `"cognicode-mcp"` as the explicit `binary_name`
  argument.

## Out-of-scope follow-up (DEBT-2)

* The install transaction's manifest-writing path filters out the
  DaemonCli component for the `core` profile. `cmd_ide_uninstall`
  now strictly resolves the DaemonCli binary name from the manifest,
  which surfaces this gap. The round-trip test plants the DaemonCli
  metadata as a fixture; whether the install transaction should
  keep DaemonCli metadata in the manifest regardless of profile is
  a separate architectural decision.
* The five `BLOCKED-BY-DEBT-2` sites need a portable-skill-bundle
  manifest and a declared `bundle.id`.
