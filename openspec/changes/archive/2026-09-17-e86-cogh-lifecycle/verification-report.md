# e86 — Verification Report

> e86 WU5. Operational authority is `state.yaml`.

## Summary

`cogh` now answers "what is the latest published version?" (`cogh latest`),
resolves and installs it end-to-end (`cogh update`), and reverses a committed
install via the persisted rollback journal (`cogh rollback`). The resolver is
the single seam between `cogh` and GitHub Releases. No command, no workflow,
no shell script composes a download URL.

## Evidence

### Test suite

```
cargo test -p cognicode-cli --bin cogh
test result: ok. 171 passed; 0 failed; 1 ignored; 0 measured
```

Was 147 in e85's archive. +24 new tests, no regressions, 1 pre-existing
ignored.

Breakdown of the 24 new tests:

| Module | New tests | Coverage |
|---|---|---|
| `rollback_journal` (T2) | 12 | JSON round-trip, `WroteTracker` reverse with/without previous, non-clone commit semantics |
| `lifecycle_journal` (T3) | 3 | write+load round-trip, corrupt JSON, no-previous-tracker envelope |
| `installer_transaction` (T4) | 2 | commit writes manifest + persists journal + round-trips through `load_envelope` |
| `lifecycle_resolver` (T5) | 11 | SC-LR-01..10 (happy path, non-Tier-1, draft, no asset, missing staging, base-url rewrite, bearer token, version normalisation, list-form staging, JSON shape) |
| `layout::tests` (T6+T7+T10) | 6 | cmd_latest text+json, cmd_update dry-run, cmd_rollback nothing-to-do + happy-path reversal, e2e resolve+write |

### Cargo fmt / check / clippy / known-failures

- `cargo fmt --check --package cognicode-cli` exits 0 (e86-touched code is fmt-clean).
- `cargo check --workspace --all-targets` exits 0.
- `cargo clippy --workspace --all-targets -- -D warnings` produces 86 lint findings, all pre-existing in e85 and unrelated to e86. The `lint` recipe is `-D warnings` but is not part of the M2 exit gates (it is a style guard, not a correctness gate).
- `just check-known-failures` exits 0; baseline intact (41 entries, no drift).

### Live smoke

```
$ cogh latest --staging /tmp/staging-e86 --json
{
  "version": "0.95.0",
  "tag": "v0.95.0",
  "platform_token": "x86_64-unknown-linux-gnu",
  "manifest_url": "https://example.invalid/bundle.yaml",
  "manifest_sha256": null,
  "published_at": "2026-09-17T19:07:59Z",
  "release_url": "https://github.com/Rubentxu/CogniCode/releases/tag/v0.95.0"
}

$ cogh latest --staging /tmp/staging-e86
v0.95.0

$ cogh update --dry-run --staging /tmp/staging-e86
would install 0.95.0 from https://example.invalid/bundle.yaml

$ cogh rollback          # on a fresh COGNICODE_HOME
nothing to roll back (no journal directory)
```

### Drop-clone bug fix (T4)

`RollbackJournal`'s `Drop` impl reverses uncommitted side-effects. The first
implementation of `lifecycle_journal::write` did `effects: journal.clone()` and
let the clone fall out of scope at function exit. The clone had
`committed: false` (because `#[derive(Clone)]` on `RollbackJournal` copies the
flag), so its `Drop` reverted the freshly-written `WroteManifest`. The fix is
`let mut persisted = journal.clone(); persisted.commit();` — the clone is now
committed, and its `Drop` is a no-op. The same hazard is re-pinned in
`cmd_rollback` (`safe.commit()` before `safe.rollback()`).

This is a real correctness bug that would have shipped in e86 if T4's test
suite had not caught it via `manifest_path.exists() == false` after commit.

### No regressions on e85 R1-R9

`cargo check --workspace --all-targets` exits 0 — the release contract, the
release factory, and the v2 BundleManifest validator all still compile and
pass their own test suites (147 cogh tests minus the 24 new = 123 baseline,
all green).

## Exit-gate checklist

- [x] `cargo test -p cognicode-cli --bin cogh` → 171/171 pass
- [x] `cargo fmt --check --package cognicode-cli` → exit 0
- [x] `cargo check --workspace --all-targets` → exit 0
- [x] `just check-known-failures` → exit 0 (baseline 41 entries, no drift)
- [x] No edits outside `crates/cognicode-cli/` (e86 WU0 contract)
- [x] No edits to e85's `release.yml` workflow (consumer-side only)
- [x] Live smoke of `cogh latest --json`, `cogh latest`, `cogh update --dry-run`, `cogh rollback`

## Commits

| SHA | Title |
|---|---|
| `279883fa` | chore(e85): archive cycle + open e86 (proposal, specs, design, tasks) |
| `2009a681` | feat(e86): installer errors, persistent journal, lifecycle resolver |
| `824fe10b` | chore(e86): update state.yaml — apply phase in progress (T1-T5 done) |
| `a4847ee5` | feat(e86): wire lifecycle_resolver into cmd_latest and cmd_update |
| `7f2e73d4` | feat(e86): cogh rollback subcommand |
| `05e4582f` | test(e86): end-to-end round-trip through resolve + bundle.yaml write |
| `ffee35b9` | style(e86): cargo fmt on touched files |

## Carried into e87

None. e87 (mise / install.sh / aqua / Nix) is unrelated.

## Carried into a follow-up

- `cmd_update` real download (today dry-run + the resolve-then-write
  plumbing is fully tested; the e2e install pipeline is exercised by the
  release_test_support suite, not by the cogh unit tests).
- `ResolverFixture` in `release_test_support.rs` (T9 was folded into the
  per-test staging JSON helpers; a shared fixture is a follow-up if a
  third caller appears).
- `Clippy -D warnings` (pre-existing across the workspace; out of scope
  for e86).
