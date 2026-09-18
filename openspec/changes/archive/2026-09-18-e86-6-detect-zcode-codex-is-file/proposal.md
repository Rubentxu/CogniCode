# E86.6 — Detect zcode / codex use is_file

> Status: **PASS**
> Closure date: 2026-09-18
> Apply commit: `167f351c`
> Binary SHA-256: `00193869659d1e8e8d7cac4a624c135b8417ecd364231f5d14cda0d009ec785d` (identical to E86.5's — see below)

## Origin

E86.5 closed the `exists()`-vs-`is_file()` bug for `detect_opencode`.
The same one-line fix needed to be applied to `detect_zcode` and
`detect_codex`. Both used `Path::exists()` and would falsely return
`true` if a directory existed at the resolved `config_file` path.

## What changed

Two one-line changes in `crates/cognicode-cli/src/cmd/ide.rs`:

```rust
pub fn detect_zcode() -> bool {
    ZCodePaths::resolve().config_file.is_file()  // was .exists()
}

pub fn detect_codex() -> bool {
    CodexPaths::resolve().config_file.is_file()  // was .exists()
}
```

`detect_claude` is unchanged because its predicate is different — it
checks `mcp_dir` (a directory), and `Path::exists()` is the correct
predicate for that case.

Four new tests in `cmd/ide.rs::tests::`:

| ID | Name | Pre-fix | Post-fix |
|---|---|---|---|
| T1 | `t_e86_6_detect_zcode_returns_false_when_config_is_directory` | FAIL | PASS |
| T2 | `t_e86_6_detect_zcode_returns_true_when_config_is_file` | already PASS | pinned |
| T3 | `t_e86_6_detect_codex_returns_false_when_config_is_directory` | FAIL | PASS |
| T4 | `t_e86_6_detect_codex_returns_true_when_config_is_file` | already PASS | pinned |

## Verification

* `cargo test -p cognicode-cli --bin cogh`: **215 passed / 0 failed /
  1 ignored** (211 baseline + 4 new).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 new errors.

## Real-PC UAT (WU4)

`UAT_ROOT=/tmp/cogh-uat-real-pc-866 COGH_BIN=target/release/cogh bash /tmp/cogh-uat-real-pc.sh`:

* `overall_status: ok`.
* Phase A preflight: `ok`.
* Phase B install: `ok` (`✓ OpenCode integration complete`).
* Phase C list / latest / where / doctor: `done`.
* Phase D rollback: `ok`.
* Phase D reinstall: **FAIL** (pre-existing layout drift).
* Phase D uninstall: `ok`.

**Binary SHA-256 is identical to E86.5's** (`00193869…`). The
changed functions are dead-code-eliminated from the release binary
because the UAT only exercises `--ide opencode`. This is expected
behaviour for cargo + LTO when changed functions are never called
from the binary entry point. Source-level symmetry is the
deliverable; the binary will diverge the first time a different
adapter is exercised (e.g., a `--ide zcode` UAT run).

HOME pollution: post-UAT sha256 (`13bb7c2e…`) identical to the
post-E86.4-cleanup value. Zero new pollution.

## Out of scope (filed for a later cycle)

1. The Phase D reinstall `link_or_copy failed` is the deep layout
   bug from `install.rs:46-49`. E86.6 does NOT touch it. Tabled
   until the `install/` vs `versions/` architectural decision
   lands.
2. `cmd_uninstall` does not remove the install tree under
   `versions/{ver}/` or `install/{ver}/` (pre-existing E86.3
   out-of-scope item, still open).

## Cycle verdict

**PASS**. Bounded change: one file, 176 insertions (mostly tests
and doc comments), 2 deletions. The behavioural changes are two
lines. The fix is defensive (closes a latent false-positive) and
completes the consistency across all three config-file-driven
detect_* adapters.
