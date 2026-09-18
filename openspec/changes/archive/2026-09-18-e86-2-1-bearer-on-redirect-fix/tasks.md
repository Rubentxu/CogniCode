# TASKS — E86.2.1 — Bearer Token Strip on Cross-Origin Redirect

> Owner: E86.2.1
> Spec: `openspec/changes/e86-2-1-bearer-on-redirect-fix/specs/bearer-redirect/spec.md`
> Design: `openspec/changes/e86-2-1-bearer-on-redirect-fix/design.md`
> Status: ready for `sdd-apply`

## Task 1 — Implement `download_with_bearer()` in installer_transaction.rs

**File**: `crates/cognicode-cli/src/cmd/installer_transaction.rs`

**Acceptance**: REQ-FIX-01, REQ-FIX-02, REQ-FIX-03, REQ-FIX-04 in spec.

Steps:

1. Add the helper (private fn) signature:
   ```rust
   fn download_with_bearer(
       client: &reqwest::blocking::Client,
       url: &str,
       bearer: Option<&str>,
   ) -> Result<reqwest::blocking::Response, InstallerError>
   ```
2. Implement the manual loop per design §3.1. Reuse
   `super::lifecycle_resolver::gh_trust_set` and
   `super::lifecycle_resolver::bearer_from_env` — both already
   exported as `pub fn`.
3. Add `MAX_DOWNLOAD_REDIRECTS: u8 = 5;` at the module level.
4. Replace lines 139–142 in `InstallStage::Downloading` with a
   `download_with_bearer()` call followed by the existing body-to-disk
   code (lines 143–153 unchanged).

Verification (locally before commit):

```bash
cargo build -p cognicode-cli --release
cargo test -p cognicode-cli --bin cogh
cargo fmt -p cognicode-cli --check
```

## Task 2 — Add three apply-phase tests in installer_transaction.rs::tests

**File**: `crates/cognicode-cli/src/cmd/installer_transaction.rs`
(append at the bottom inside `mod tests { ... }`).

**Acceptance**: REQ-FIX-01, REQ-FIX-02, REQ-FIX-03 in spec.

Tests:

- `e86_2_1_fix_01_bearer_propagates_across_redirect`
- `e86_2_1_fix_02_untrusted_redirect_host_returns_disallowed_error`
- `e86_2_1_fix_03_too_many_hops_returns_error`

Each test uses mini TCP listeners per the pattern already in
`lifecycle_resolver.rs::tests` (`bind_one_shot_302`, `bind_one_shot_ok`,
`header_value`). Either:

- (a) Promote the helpers to a shared module
  `crates/cognicode-cli/src/cmd/test_support/mod.rs` (one-time 25-line
  move + import in both test modules), OR
- (b) Duplicate the helpers into `installer_transaction.rs::tests`
  with `// SHARED-WITH crate::lifecycle_resolver::tests` comments.

Recommendation: do (a) — cleaner and the apply phase already has
multiple files to touch.

## Task 3 — Annotate the two RED tests as `// DEFECT-DOC`

**File**: `crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests`

Above `sc_fix_01_default_policy_strips_authorization_on_cross_host_redirect`
and `sc_fix_02_untrusted_redirect_target_is_not_followed`, add a
3-line block:

```rust
// DEFECT-DOC — empirical proof that reqwest 0.12.28's default redirect
// policy strips Authorization on cross-origin redirects. The fix lives
// in `installer_transaction.rs::download_with_bearer()` (manual loop).
// Kept after GREEN so any future reqwest upgrade that flips this
// behaviour will fail this test loudly.
```

## Task 4 — Re-run E86.2 disposable UAT

**Script**: `/tmp/cogh-uat-real-pc.sh` (already created).

**Acceptance**: REQ-FIX-05 in spec.

Steps:

1. `just build-server` (release binary).
2. Re-run the disposable UAT script with `COGNICODE_GITHUB_TOKEN="$(gh auth token)"`
   in the environment.
3. Verify Phase B exits with HTTP 200 (asset downloaded, mcp-server
   tarball under `$COGH_HOME/cache/`).
4. Verify `cogh list` shows `mcp-server@0.95.0`.
5. Verify Phase D uninstall ends with `install/mcp-server/0.95.0/`
   removed.
6. Update `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md`
   flipping Phase B from PARTIAL to GREEN. Add the new binary
   SHA-256 to the receipt.

## Task 5 — Run full test gates

**Acceptance**: REQ-FIX-06, REQ-FIX-07, REQ-FIX-08 in spec.

```bash
cargo test -p cognicode-cli --bin cogh
cargo fmt -p cognicode-cli --check
cargo clippy -p cognicode-cli --bin cogh --tests
```

All must be green or zero-warning on the touched files. Pre-existing
workspace-wide clippy warnings are out of scope.

## Task 6 — Apply receipt + commit

**File**: `openspec/changes/e86-2-1-bearer-on-redirect-fix/apply-receipt.md`

Required sections:

- test diff summary (was 186 → expect 189)
- UAT re-run GREEN evidence (binary SHA-256, exit codes, asset path)
- binary release SHA-256 captured from the rebuild (`sha256sum
  target/release/cogh` or, in this repo,
  `$CARGO_TARGET_DIR/release/cogh`)
- decision on the open question Q1 (private vs. public helper) — based
  on which side of trade-off landed

Commit message format (conventional commits, no AI trailer per
AGENTS.md):

```
fix(cogh installer): preserve bearer through cross-origin redirect (E86.2.1)

Manual redirect loop in installer_transaction.rs::Downloading re-attaches
Authorization on each hop, since reqwest 0.12.28 strips it on
cross-origin redirects regardless of redirect::Policy::custom behavior.

Closes parent finding in
openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md (Phase B PARTIAL).

Evidence:
- cargo test -p cognicode-cli --bin cogh: 189 passed, 0 failed
- disposable UAT Phase B: HTTP 200, mcp-server@0.95.0 installed
- RED defect-doc tests retained: lifecycle_resolver::tests::sc_fix_01_*
  and sc_fix_02_*
```

## Task 7 — Update E86.2 UAT receipt + related state files

Files to touch:

- `openspec/changes/e86-2-cogh-real-user-uat/uat-receipt.md` — flip
  Phase B from PARTIAL to GREEN, capture new binary SHA-256, capture
  duration of the asset download.
- `.agent/TESTING-STATE.md` — append a "Coverage patch E86.2.1" section
  noting: (a) which new tests cover the manual redirect loop; (b) which
  RED tests are now `// DEFECT-DOC`; (c) the empirical evidence that
  `Policy::custom` does not work for this case (mention
  reqwest issue #1040).

## Sequencer

- T1 → T2 → T3 → T4 → T5 → T6 → T7
- T1 and T2 are local to the apply agent (no external blockers).
- T4 requires a working network to a real GitHub Release; if
  `gh auth token` returns empty on the host, the UAT runs without
  auth and cannot claim GREEN for REQ-FIX-01/02 specifically; it can
  still claim GREEN for REQ-FIX-04 (no-token path) and a PARTIAL state
  for the rest.
- T6 cannot start until T1–T5 are GREEN.

## Open question — see proposal §Open question

**Q1.** Public vs. private `download_with_bearer()`. Lean: private.
Final decision recorded in the apply receipt.
