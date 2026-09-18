# E86.5 — Detect OpenCode uses is_file (not exists)

> Status: **PASS**
> Closure date: 2026-09-18
> Apply commit: `25420031`
> Binary SHA-256: `00193869659d1e8e8d7cac4a624c135b8417ecd364231f5d14cda0d009ec785d`

## Origin

`detect_opencode()` returned `Path::exists()` to decide whether the IDE
was configured. `Path::exists()` returns true for **both** files and
directories. If a developer's `$HOME/.config/opencode/` directory
exists but the `opencode.json` inside it does not, `detect_opencode()`
falsely returned `true` and the install pipeline tried to integrate:

* `Step::MergeJson` would create a fresh empty config (this is harmless
  but surprising).
* `Step::Symlink` would fail with `link_or_copy failed` because the
  source path (`<root>/install/<ver>/mcp-server/skills`, the deep
  layout bug from E86.4) does not exist either way.

The UAT script worked around the symptom by pointing `OPENCODE_CONFIG`
at a NON-EXISTENT file path inside a fresh tempdir (see
`cmd/layout.rs::TempOpenCodeConfig` comment, line 633). That kept the
UAT passing but did not address the underlying detection bug.

## What changed

One-line behavioural change in
`crates/cognicode-cli/src/cmd/ide.rs::detect_opencode`:

```rust
pub fn detect_opencode() -> bool {
    OpenCodePaths::resolve().config_file.is_file()  // was .exists()
}
```

`Path::is_file()` returns true only when the path is a regular file.
A directory at the resolved config path no longer falsely triggers
detection.

Two new tests in `cmd/ide.rs::tests::`:

| ID | Name | Pre-fix | Post-fix |
|---|---|---|---|
| T1 | `t_e86_5_detect_opencode_returns_false_when_config_is_directory` | FAIL (returns true) | PASS |
| T2 | `t_e86_5_detect_opencode_returns_true_when_config_is_file` | already PASS | PASS |

T1 is the bug-exhibiting test: it creates a DIRECTORY at the resolved
`config_file` path (so `Path::exists()` is true but `Path::is_file()`
is false). The sanity asserts at the top of the test explicitly pin
the shape being tested — `paths.config_file.exists()` is true,
`paths.config_file.is_file()` is false, `paths.config_file.is_dir()`
is true. Removing any of those asserts would weaken the test.

T2 is the happy-path contract pin.

## Verification

* `cargo test -p cognicode-cli --bin cogh`: **211 passed / 0 failed /
  1 ignored** (209 baseline + 2 new).
* `cargo fmt -p cognicode-cli`: clean.
* `cargo check --workspace`: clean.
* `python3 scripts/check_known_failures.py`: 41 entries, baseline
  unchanged.
* `cargo clippy -p cognicode-cli --bin cogh`: 0 new errors.

## Real-PC UAT (WU4)

`UAT_ROOT=/tmp/cogh-uat-real-pc-865 COGH_BIN=target/release/cogh bash /tmp/cogh-uat-real-pc.sh`:

* `overall_status: ok`.
* Phase A preflight: `ok`.
* Phase B install: `ok` (`✓ OpenCode integration complete`).
* Phase C list / latest / where / doctor: `done`.
* Phase D rollback: `ok`.
* Phase D reinstall: **FAIL** (pre-existing layout drift — see below).
* Phase D uninstall: `ok`.

## Pre-existing HOME pollution (no change vs E86.4)

Post-UAT sha256 of `~/.config/opencode/opencode.json`:
`13bb7c2e59ad9073936f75e30007136991a5aac44cf6dfe3153ed1ade45ef6e5`.
**Identical** to the post-E86.4-cleanup value (`13bb7c2e…`). E86.5 did
NOT introduce new pollution. The remaining delta vs the original
pre-E86.4 snapshot (`501c92bb…`) is the residual ordering/whitespace
difference from removing the pre-existing `cognicode-mcp` entry —
cleaned up in the E86.4 forensic step, unchanged here.

The symlink target at `~/.config/opencode/skills/cognicode-0.95.0` is
still pointing at the original
`/home/rubentxu/.cognicode/install/0.95.0/mcp-server/skills`.

## Out of scope (filed for a later cycle)

1. `detect_zcode` and `detect_codex` have the same
   `config_file.exists()` bug. Same fix shape (`.is_file()`); same
   one-line risk. Deliberately deferred to a follow-up cycle to keep
   E86.5 bounded.
2. The Phase D reinstall `link_or_copy failed` is the deep layout
   bug from `install.rs:46-49`. E86.5 does NOT touch it. Tabled until
   the `install/` vs `versions/` architectural decision lands.
3. `cmd_uninstall` does not remove the install tree under
   `versions/{ver}/` or `install/{ver}/` (pre-existing E86.3
   out-of-scope item, still open).

## Cycle verdict

**PASS**. Bounded change: one file, 127 insertions (mostly tests and
doc comments), 1 deletion. The behavioural change is one line. The
fix is defensive (closes a latent false-positive) rather than
symptom-driven (no observed UAT regression), but it removes a
class of failure that future UAT variants could trip.
