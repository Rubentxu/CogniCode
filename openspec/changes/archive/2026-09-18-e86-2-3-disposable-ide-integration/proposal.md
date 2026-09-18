# E86.2.3 — Disposable-aware IDE integration

> Status: **PASS**
> Closure date: 2026-09-18
> Apply commit: `c28f913f`
> Binary SHA-256: `9e63705ecfa7756026e5ffe1b7139936ed92a9a14c64b2b9e7a991194d4a1781`

## Origin

The E86.2.2 real-PC UAT (commit `4264ed81`) exercised the new
`bundle pre-fetch` step end-to-end and observed that the install bundle
core (download → SHA256 verify → extract → shim) succeeded against the
real GitHub Releases endpoint. The script then terminated non-zero on
`--ide opencode` because `ide::integrate_opencode` was symlinking skills
into the developer's real `~/.config/opencode/skills/` even though
`OPENCODE_CONFIG` was set to a disposable path.

That finding is what E86.2.3 closes.

## What changed

A single bug, but the same pattern repeated across three IDE adapters
(opencode, zcode, codex). E86.2.3 introduces one `*Paths::resolve()` per
adapter and routes every call-site through it:

* `OpenCodePaths` → driven by `OPENCODE_CONFIG`
* `ZCodePaths`   → driven by `ZCODE_CONFIG`
* `CodexPaths`   → driven by `CODEX_CONFIG`

Skills are now derived from the **same** ownership root the config file
lives in (`config_dir.join("skills")`), never from `$HOME`
independently. Claude was already correct.

The pre-existing `*_config_path()` / `*_skills_dir()` functions are kept
as thin shims around the resolver so call-sites don't churn.

## Behaviour matrix

| IDE    | config env          | config path          | skills path           |
|--------|---------------------|----------------------|-----------------------|
| opencode | `OPENCODE_CONFIG` | env or `$HOME/.config/opencode/opencode.json` | same root, `…/skills/` |
| zcode    | `ZCODE_CONFIG`     | env or `$HOME/.zcode/v2/config.json`           | same root, `…/skills/` |
| codex    | `CODEX_CONFIG`     | env or `$HOME/.codex/config.toml`              | same root, `…/skills/` |
| claude   | `CLAUDE_CONFIG`    | env or `$HOME/.claude`                          | same root, `…/skills/` (already correct) |

## Tests added (all #[serial])

* `t_e86_2_3_opencode_resolver_with_explicit_config` — T1.
* `t_e86_2_3_opencode_resolver_default` — T3 (default behaviour).
* `t_e86_2_3_opencode_skills_follow_opencode_config` — T2 (RED before
  the fix; pins the bug explicitly by setting both `OPENCODE_CONFIG`
  and a fake `$HOME` to two different disposable paths).
* `t_e86_2_3_opencode_uninstall_follows_opencode_config` — uninstall
  target respects the same root.

## Verification

* `cargo test -p cognicode-cli --bin cogh`: 202 pass / 0 fail / 1
  ignored (baseline unchanged).
* `test_install_with_ide_and_profile_dispatches_both` (E86.2.2
  regression guard): PASS.
* 9 E86.2.2 env-split tests: PASS.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo check --workspace`: clean.
* New clippy errors: 0 (the 38 pre-existing failures in `cognicode-core`
  are out of scope).

## Real-PC UAT (WU4)

Re-ran `/tmp/cogh-uat-real-pc.sh` against the rebuilt release binary
(`9e63705e…`) with the E86.2.2 + E86.2.3 changes in place. Snapshot of
the developer's real `~/.config/opencode/` before and after:

```
pre  symlink target: /home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills
post symlink target: /home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills
pre  config sha256:  501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737
post config sha256:  501c92bb75cf600c79d7ed56c1722281ee204b70dc9a31e1b40cc07c7134f737
pre  config mtime:   1789722950
post config mtime:   1789722950
```

Zero pollution. The disposable target at
`/tmp/cogh-uat-real-pc-8623/opencode-config/skills/cognicode-0.95.0`
was created during Phase B and removed during Phase D uninstall.

Overall UAT status: `ok` (Phases A/B/C/D all executed).

## Out of scope (filed for a later cycle)

1. `install.rs::run_install` reads `mcp-server/skills` from
   `manifest_path.parent().join(...)` which resolves to
   `cognicode-home/install/0.95.0/...`, but the real layout puts it at
   `cognicode-home/versions/0.95.0/...`. When the install runs without
   a prior `cogh init` that has already created the `versions/` tree,
   the OpenCode symlink source path does not exist and the integration
   fails. Symptom: `link_or_copy failed` on a fresh disposable UAT.
2. `ide::detect_opencode` uses `Path::exists()` which returns `true`
   for any path, including directories. After Phase B writes
   `OPENCODE_CONFIG`'s target file, Phase D reinstall (which does not
   pass `--ide opencode`) still triggers the integration step because
   `detect_opencode()` returns `true` on the now-existing file. Should
   be `Path::is_file()` (or check for the `mcp.cognicode-mcp` key
   specifically).
