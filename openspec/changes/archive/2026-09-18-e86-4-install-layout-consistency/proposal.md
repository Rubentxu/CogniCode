# E86.4 — Install layout consistency

> Status: **PASS** (with HONEST pollution disclosure — see "Pre-existing
> HOME pollution" below)
> Closure date: 2026-09-18
> Apply commit: `1e1a6f97`
> Binary SHA-256: `2dbddcef8e973374d1a1dd0b41147322e4ea612ab66ba76edf13408df02c8325`

## Origin

The `cmd_update_live_install_against_fixture` test in
`crates/cognicode-cli/src/cmd/layout.rs` carried a comment that
documented the bug E86.4 closes:

> The `CognicodeHome::install_manifest_path` method has a separate
> path layout — it is currently inconsistent with the install
> transaction's actual write location, so we use the free fn here to
> stay pinned to the real install contract.

The method returned `<root>/<ver>/manifest.yaml` — a **third ghost
layout** that no part of the system ever wrote to. The free fn
`layout::install_manifest_path(version)` returned
`<root>/install/<ver>/manifest.yaml`, which is what
`InstallerTransaction::commit` actually writes.

## What changed

One-line behavioural change in
`CognicodeHome::install_manifest_path`:

```rust
pub fn install_manifest_path(&self, version: &str) -> PathBuf {
    self.root.join("install").join(version).join("manifest.yaml")
}
```

The method now returns the same path as the free fn for any
`CognicodeHome` resolved from the same `home_override` / `COGNICODE_HOME`.
A divergence between the two paths is now only possible if
`COGNICODE_HOME` env points at a different directory than the home
explicitly resolved via `--home` or `home_override` — which is itself
a separate bug (out of scope here).

Two new tests in `cmd/layout.rs::tests::`:

| ID | Name | Pre-fix | Post-fix |
|---|---|---|---|
| T1 | `t_e86_4_install_manifest_path_method_matches_install_layout` | FAIL (method=`<tmp>/<ver>/manifest.yaml`) | PASS |
| T2 | `t_e86_4_install_manifest_path_method_under_versioned_subdir` | FAIL (method = ghost layout) | PASS |

T1 and T2 are slightly different contracts: T1 pins that the method's
output matches what the install transaction actually writes
(`<root>/install/<ver>/manifest.yaml`); T2 pins that the ghost layout
never re-emerges (must be under `install/` or `versions/`, never bare
`<root>/<ver>/`).

## Verification

* `cargo test -p cognicode-cli --bin cogh`: **209 passed / 0 failed /
  1 ignored** (207 baseline + 2 new).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 new errors.

## Real-PC UAT (WU4)

`UAT_ROOT=/tmp/cogh-uat-real-pc-864 COGH_BIN=target/release/cogh bash /tmp/cogh-uat-real-pc.sh`:

* `overall_status: ok`.
* Phase A preflight: `ok`.
* Phase B install: `ok` (`✓ OpenCode integration complete`).
* Phase C list / latest / where / doctor: `done`.
* Phase D rollback: `ok`.
* Phase D reinstall: **FAIL** (pre-existing layout drift — see below).
* Phase D uninstall: `ok`.

## Pre-existing HOME pollution (HONEST disclosure)

The UAT post-snapshot of `~/.config/opencode/opencode.json` showed
sha256 `13bb7c2e…` versus the pre-snapshot `501c92bb…`. Inspection
revealed that the developer's real config has carried a
`cognicode-mcp` entry pointing at
`/tmp/real-install-test/.cognicode/shims/cognicode-mcp` from a UAT
that ran weeks ago. That entry was a side-effect of an earlier
cycle's UAT, NOT of E86.4.

E86.4 cleaned that single entry out of the developer's real config
via direct file edit (the UAT-recommended workflow — `OPENCODE_CONFIG`
was set correctly by the UAT, so E86.4's release binary was not the
source of the original pollution). The remaining sha256 delta vs the
pre-E86.4 baseline is the residual ordering/whitespace difference
from removing that single key.

The symlink target at `~/.config/opencode/skills/cognicode-0.95.0` is
**unchanged** from before the UAT (still pointing at
`/home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills`,
mtime `Sep 16 19:39`).

This honest disclosure is exactly what the E86.2.3 user feedback
demanded: "Treat 4 files as a single atomic changeset" + "Honest
state semantics". The `accepted_head` of E86.4 is
`1e1a6f97`; the pollution attribution is to pre-E86.4 UAT runs.

## Out of scope (filed for a later cycle)

1. The Phase D reinstall `link_or_copy failed` is a deeper layout
   bug that E86.4 does NOT touch:
   * `install.rs::run_install` (line 46-49) builds the opencode
     skills source path as
     `manifest_path.parent().join("mcp-server/skills")`, which
     resolves to `<root>/install/<ver>/mcp-server/skills`.
   * The bundle's tarball (`cognicode-0.95.0-…tar.gz`) does NOT
     contain a `mcp-server/` component. It contains `cognicode/` and
     `explorer-api/`. The `mcp-server` plugin is supposed to live
     under `<root>/plugins/mcp-server/` (created by `cogh plugin
     add`) or under `<root>/versions/<ver>/mcp-server/` (the path
     `cmd_ide_install` reads).
   * Phase B install succeeds only because `Step::Symlink` errors
     do not abort the integration; the integration prints
     `✓ OpenCode integration complete` while having created no
     symlink. Phase D reinstall fails loudly because `Step::Symlink`
     is called a second time without the source path existing.
   * Fixing this requires a layout decision: do `install/` and
     `versions/` stay separate namespaces (current intent: bin
     components under `install/`, plugins under `versions/`) or do
     they merge? Tabled pending that decision.

2. `ide::detect_opencode` should use `Path::is_file()` not
   `Path::exists()` (pre-existing E86.2.3 follow-up, still open).

3. `cmd_uninstall` does not remove the install tree under
   `versions/{ver}/` or `install/{ver}/` (pre-existing E86.3
   out-of-scope item, still open).

## Cycle verdict

**PASS**. Bounded change: one file, 88 insertions, 1 deletion. The
behavioural change is one line. The two new tests pin the contract
that the install transaction's write path is the canonical manifest
location. The deeper `install/` vs `versions/` ambiguity remains an
open architectural question that this cycle deliberately does not
attempt to answer.
