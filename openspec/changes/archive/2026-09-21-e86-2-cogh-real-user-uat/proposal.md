# e86.2 — cogh Real User-PC UAT (proposal)

> Program: cognicode-distribution (umbrella: e84 contract + e85 release + e86 lifecycle)
> Milestone: M2.2 — real-Linux install UAT against the production v0.95.0 binary
> Phase: propose | Date: 2026-09-18
> Delivery: bounded cycle, single slice (this is mostly verification, not new code)

## Intent

Validate end-to-end that `cogh install mcp-server --ide opencode --profile core`
works against the REAL production binary `0.95.0` on the user's actual Linux
PC, not a TempDir/LocalRelease loopback mock.

This is the last missing acceptance gate named in E85's exit report ("e86.2:
real Linux install UAT against the production binary 0.95.0") and the only
thing standing between the current e86 coverage and a documented "install
on a real user PC has been exercised" claim.

## Scope

IN scope:
- Build release binary (`just build-release` / `cargo build --release -p cognicode-cli --bin cogh`).
- Run `cogh install mcp-server --ide opencode --profile core` against the
  release binary, in the user's actual HOME (no TempDir).
- Capture before/after state of:
  - `~/.cognicode/` (installed files + version tracker)
  - `~/.config/opencode/` (skills + MCP entry injected by the IDE adapter)
  - `~/.local/bin/cogh` (or platform-equivalent shim path)
- Run `cogh uninstall mcp-server 0.95.0` and verify it restores pre-install state.
- Run `cogh latest` against the real GitHub Releases endpoint to confirm
  resolution contract still hits production.
- Acceptance: every public cogh lifecycle command works (install, list,
  latest, where, init, rollback), doctor reports no issues.

OUT of scope (delegated to sister cycles):
- E86.3: unit-level uninstall coverage on the bugs e86.1 remediated (the UAT
  exercises uninstall but does not pin it with explicit regression tests).
- E86.4: rollback-to-version support (the UAT exercises `--to` only if E86.4
  lands first; otherwise current "last transaction only" rollback is verified).
- GPG signature, .pkg/.msi packaging, macOS/Windows real-hardware tests.

## Approach

Three phases. Most of the value is the report, not the code.

**Phase A — preflight (no source changes)**:
- `cargo build --release -p cognicode-cli --bin cogh` (release profile, ~3min).
- SHA256 the binary; record toolchain + commit SHA.
- Confirm no pre-existing `~/.cognicode/`; back it up if present.

**Phase B — install + verification (the UAT itself)**:
- `~/.cognicode/` empty (or backed up); `~/.local/bin/` empty.
- `./target/release/cogh init` → expect "initialized" + populated layout.
- `./target/release/cogh install mcp-server 0.95.0 --ide opencode --profile core`
  → expect Ok(0).
- Read `~/.cognicode/manifest.json`, `~/.cognicode/tracker.json`,
  `~/.config/opencode/skills/cognicode-mcp/SKILL.md`, MCP server entry in
  `~/.config/opencode/config.toml`. Confirm checksum-passed manifests.
- `./target/release/cogh list --installed` → expect mcp-server 0.95.0.
- `./target/release/cogh latest mcp-server` → expect 0.95.0.
- `./target/release/cogh where cognicode` → expect path under `~/.cognicode/bin`.
- `./target/release/cogh doctor` → expect clean (or known-warning list).
- `./target/release/cogh rollback` → expect Ok; manifest + tracker reverted.

**Phase C — uninstall (drives E86.3 motivation)**:
- `./target/release/cogh install mcp-server 0.95.0 --ide opencode --profile core`
  (re-install to have something to remove).
- `./target/release/cogh uninstall mcp-server 0.95.0 --ide opencode`
- Verify `~/.cognicode/` removed entries, MCP entry removed from
  `~/.config/opencode/config.toml`, skills removed.
- Walk the same three patterns e86.1 proved for install/rollback:
  uninstall with populated install dir, uninstall after a stale shim,
  uninstall on a zero-component profile (now EmptyInstall).

## Acceptance contract

| REQ | Observable |
|---|---|
| REQ-UAT-01 | `cogh install mcp-server 0.95.0 --ide opencode --profile core` exits 0 against the release binary built at HEAD |
| REQ-UAT-02 | After install: `~/.cognicode/manifest.json` lists mcp-server@0.95.0; `~/.local/bin/cogh` shim exists and points at the release binary; `~/.config/opencode/config.toml` has the MCP server entry; `~/.config/opencode/skills/cognicode-mcp/` is populated |
| REQ-UAT-03 | `cogh list`, `cogh latest`, `cogh where`, `cogh doctor`, `cogh rollback` all exit 0 against the release binary |
| REQ-UAT-04 | `cogh uninstall mcp-server 0.95.0 --ide opencode` reverses install completely (no orphan files) |
| REQ-UAT-05 | A documented failure of any of REQ-UAT-01..04 records the observed behaviour, expected behaviour, and an E86.3 task that pins the regression test |
| REQ-UAT-06 | The whole UAT reproduces from a single shell script that takes <10 min on the user's hardware |

## Risks

- **Permission surprises** — `--profile core` may attempt to write to
  `/etc` or `/usr/share` on some platforms; mitigated by running as a normal
  user with no `sudo` reliance, but if the contract requires elevated
  permissions the UAT must say so explicitly.
- **Real network access** — `cogh latest` will hit `https://api.github.com/`.
  If the user's network is proxied or air-gapped, the UAT must surface that,
  not silently substitute loopback.
- **Pre-existing state** — the user's HOME may already have a partial install;
  backup-and-restore is part of Phase A.

## Deliverables (this proposal -> apply phase)

1. `scripts/cogh-uat-real-pc.sh` — single shell entry point that runs
   Phases A->C non-interactively and writes a JSON receipt.
2. `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.{json,md}` —
   captured evidence per run (binary SHA, before/after filesystem state,
   observed vs expected per REQ).
3. `openspec/changes/e86-2-cogh-real-user-uat/specs/cogh-real-pc-uat/spec.md`
   — formalised Given/When/Then for REQ-UAT-01..06.
4. If REQ-UAT-05 fires, the discovered defect becomes an input to E86.3
   (`proposal.md` updated with the new task list driven by real failure).

## Non-goals

- Redo e86.1's three-bug regression suite in a separate test file (that
  is E86.3; E86.2 only proves the bugs do not fire end-to-end on real
  hardware).
- Add a CI job that runs the UAT on every PR (the UAT is too slow + too
  user-specific for that; CI stays on the existing release.yml + R-series
  guards).
- Bump the binary to 0.95.1 unless REQ-UAT-05 forces it.

## Sequencer

E86.2 -> E86.3 -> E86.4. E86.2 output may amend E86.3's task list if the
UAT finds a bug E86.3 should pin. E86.4 starts independently but uses
E86.3's regression suite as the safety net for its rollback-depth
changes.
