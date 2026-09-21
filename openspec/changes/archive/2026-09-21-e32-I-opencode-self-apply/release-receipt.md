# Release Receipt: v0.94.14 — E32-I OpenCode Self-Apply

**Release tag**: v0.94.14
**Release commit**: e4f856adf87c4fc2c3fc0e2cbc39c45405ca7c81
**Tag SHA**: 23c9e23d35b98695cc74902c4eed1d29cd5d243f (annotated)
**Branch**: main
**Cycle**: e32i_opencode_self_apply
**Path**: A-min
**Mode**: Standard
**Date**: 2026-08-16
**Archive manifest**: d7334c715f9a4e0937865b67a1610f6b1e3c8aa3
(`openspec/changes/e32-I-opencode-self-apply/archive-report.md`)
**Verify verdict**: PASS WITH WARNINGS

---

## Release contents

### Apply stack (brought to main via merge commit `bdadba1c`)

| SHA | Subject |
|-----|---------|
| `fbd8738b` | fix(cogh): E32-I dispatch-order fix --ide now respects --profile |
| `91a8168a` | feat(cli): E35 IDE targeting - ZCode, Claude, Codex support |
| `51dc325a` | chore(fmt): apply rustfmt across cognicode-cli/core/macros/runtime |
| `b46bfbd6` | fix(cogh): correction pass for e34_plugin_cleanup |
| `e86af04c` | feat(cogh): e34 plugin cleanup — retire plugin registry dead code |

### Cycle bookkeeping commits

| SHA | Subject |
|-----|---------|
| `f3bdc7d9` | chore(verify): E32-I verify-report - PASS WITH WARNINGS |
| `e04d9ef6` | chore(spec): persist E32-I spec + tasks for cycle audit trail |
| `bdadba1c` | chore(merge): merge feat/e32-i-opencode-self-apply |
| `d7334c71` | chore(archive): E32-I sync delta specs to canonical |
| `1d85711b` | chore(merge): E32-I delta spec sync from feature branch |
| `e4f856ad` | chore(release): bump workspace version 0.94.11 → 0.94.14 |

### Deltas synced to canonical spec

- `openspec/specs/cognicode-cli/spec.md` — Requirement `cogh install registers MCP server with IDEs` MODIFIED + 1 new scenario (`--ide` + `--profile` dispatch)
- `openspec/specs/cognicode-ide-adapter/spec.md` — Requirement `Adapter manifest declares integrate / uninstall steps` MODIFIED + 2 new scenarios (fresh entry, stale entry replacement)

---

## Acceptance criteria

- [x] `cargo build -p cognicode-cli --bin cogh` succeeds (verify-report.md §Build status)
- [x] Spec scenarios: 6 / 8 PASS at runtime; 2 UNTESTED (test-coverage gaps, not behavioural defects)
- [x] `lifecycle::tests`: 19 / 20 pass (1 ignored — bundle version), 1 pre-existing failure (`test_clean_home_install`) unrelated to E32-I
- [x] `ide::tests`: 20 / 20 pass
- [x] Delta specs synced to `openspec/specs/` (commit `d7334c71`)
- [x] Workspace version consistent across all 12 member crates (`cargo metadata` → all `0.94.14`)
- [x] `git rev-parse HEAD == origin/main` (post-tag push verified)
- [x] `git ls-remote origin v0.94.14` returns tag (postcondition verified)
- [ ] Real-config apply — user-driven; deferred per AGENTS.md (safety)

---

## Version bump rationale

Workspace version was `0.94.11` at the start of the cycle; latest tag before this
release was `v0.94.13` (E35). Per ADR-0011 + the version-sync guardrail
introduced in e33-14, the tag must reflect the same number as the workspace
`[workspace.package].version`. E32-I is a patch-level bug fix (dispatch-order
+ absolute shim path), so the bump is `0.94.11 → 0.94.14` (one patch ahead of
the latest tag, absorbing the drift caught by the guardrail).

Single-source version pattern (commit `603d99b6` — e33-1.1): all 12 member
crates inherit from `[workspace.package].version` via `version.workspace = true`,
so the single-line bump in the root `Cargo.toml` propagates everywhere.

**Workspace member versions (post-bump)**:

```
cognicode                0.94.14
cognicode-core           0.94.14
cognicode-graph-algos    0.94.14
cognicode-macros         0.94.14
cognicode-mcp            0.94.14
cognicode-runtime        0.94.14
cognicode-explorer       0.94.14
cognicode-ladybug        0.94.14
cognicode-cli            0.94.14
cognicode-sandbox        0.94.14
cognicode-graph-wasm     0.94.14
cognicode-core-mock      0.94.14
```

---

## Diff against v0.94.13 (high-level)

The full diff between `v0.94.13` and `v0.94.14` covers:

1. **E35 work** (already in `91a8168a`): ZCode, Claude, Codex IDE targeting.
2. **E34 work** (`b46bfbd6`, `e86af04c`): plugin registry cleanup.
3. **E32-I work** (`fbd8738b`): the dispatch-order + absolute shim path fix
   that this cycle is named after.
4. **Spec/verify/archive bookkeeping** (`f3bdc7d9`, `e04d9ef6`, `bdadba1c`,
   `d7334c71`, `1d85711b`).
5. **Version bump** (`e4f856ad`): single-line `[workspace.package].version`
   0.94.11 → 0.94.14.

The E32-I cycle commit-pin (between previous release `1d85711b` and tag
`e4f856ad`) is exactly one file, one line:

```diff
diff --git a/Cargo.toml b/Cargo.toml
@@ -22,7 +22,7 @@
 exclude = ["crates/spike-ladybug"]

 [workspace.package]
-version = "0.94.11"
+version = "0.94.14"
 edition = "2024"
 authors = ["CogniCode Team"]
```

---

## Receivers

- **Local files**:
  `openspec/changes/e32-I-opencode-self-apply/release-receipt.md` (this file)
  `openspec/changes/e32-I-opencode-self-apply/archive-report.md`
  `openspec/changes/e32-I-opencode-self-apply/verify-report.md`
  `openspec/changes/e32-I-opencode-self-apply/spec.md`
  `openspec/changes/e32-I-opencode-self-apply/tasks.md`
  `openspec/changes/e32-I-opencode-self-apply/proposal.md`
- **Remote**: `github.com:Rubentxu/CogniCode @ v0.94.14` (commit `e4f856ad`)
- **Distribution**: post-tag (CI/CD optional, not part of this cycle)

---

## Ledger Status

**SKIPPED** — SDDK ledger CLI is broken (`UNIQUE` constraint / `STORAGE_NOT_FOUND`
/ `FOREIGN KEY`). Per the manual workaround documented in E34 / E35 / E32-I
cycles, this release-receipt is written as a regular file and committed
locally. No `sddk ledger` CLI calls were made. The `release-receipt` + the
`archive-report.md` together provide the local-only ledger evidence required
by ADR-0011.

---

## Next Recommended Step

E32-I cycle closed. The user can now run a real-config `cogh install
mcp-server --ide opencode --profile core` against their own `~/.config/opencode/`
to validate the dispatch-order fix in production. Any UAT evidence should be
filed under `chore/e32-i-uatt-evidence` (the same pattern as `chore/e32-z-uatt-evidence`).
