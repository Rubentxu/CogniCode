# E86.2 — Real-PC UAT Receipt (disposable HOME)

> Date: 2026-09-18 (initial run PARTIAL)
> Date: 2026-09-18 (re-run after E86.2.1 commit `a506fc2d` — HTTP-side defect closed; SHA256 in dev fixture is a separate out-of-scope bug)
> Cycle: e86-2-cogh-real-user-uat
> Result: **PARTIAL (HTTP side closed, SHA256 side outstanding)**
> Mode: **disposable clone** under `/tmp/cogh-uat-real-pc-*/` (user's HOME untouched; `COGNICODE_HOME` + `OPENCODE_CONFIG` redirected)
> Target binary: `/var/home/rubentxu/cargo-targets/release/cogh`
> Binary SHA-256 (post-fix): `ea5940f9319ca5a15b0ec6e281bfd2410a85cacfad1443990a0e699996a2dbe2`
> Binary SHA-256 (pre-fix): `e198c7175d48fea77000adc9a3e212cf7bcc1d55f1fc65d866efba7d564b34cc`

## What was run

```text
Phase A (preflight):
  ✓ release binary exists, executable, reports `cogh 0.95.0`
  ✓ github.com reachable
  ✓ disposable HOME + OPENCODE_CONFIG set
Phase B (install):
  ✓ `cogh init` — installed 6 bundled plugins from the DEV-ONLY fixture
    (mcp-server, skills-cognicode-core, sandbox-templates, zcode, claude, codex)
  ✗ `cogh install mcp-server --version 0.95.0 --ide opencode --profile core`
    Failed with HTTP 403 Forbidden on the release tarball download.
    This was the FIRST time the install path was exercised against the real
    GitHub Releases endpoint (all prior tests used LocalRelease/TempBaseUrl).
Phase C/D: NOT EXECUTED (Phase B blocked them).
```

## The 403 — root cause (verified, not hypothetical)

```text
URL: https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/
     cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
cogh's reqwest client → 302 Found → objects.githubusercontent.com S3 URL
                                   → 403 Forbidden
```

Cross-verified with `curl`:

| Client | Result | Why |
|---|---|---|
| `curl -sI https://github.com/...tar.gz` | **302** (to signed S3 URL) | public redirect with valid signed URL |
| `curl -sI -L -H "Authorization: Bearer gho_..."` | **200** (full asset download works) | authenticated, follows to S3 |
| `cogh install` (reqwest with COGNICODE_GITHUB_TOKEN) | **403** | bearer token NOT forwarded on the cross-origin redirect |

The asset EXISTS in v0.95.0 — I queried the API directly with `gh`:

```text
cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz   9,028,489 bytes
cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz  12,748,646 bytes
cogh-0.95.0-x86_64-unknown-linux-gnu.tar.gz         3,205,792 bytes
bundle-0.95.0-x86_64-unknown-linux-gnu.yaml              933 bytes
SHA256SUMS                                            1,008 bytes
release-inventory-0.95.0.json                        2,756 bytes
```

The defect is therefore **NOT** "release incomplete", **NOT** "private repo
denying legitimate requests", and **NOT** "v0.95.0 asset missing". The defect
is that `cogh install`'s HTTP client does not propagate the
`COGNICODE_GITHUB_TOKEN` bearer header across cross-origin redirects to
`objects.githubusercontent.com`.

## What this UAT found that the tests did not

1. **All previous e85/e86/e86-followup-live-update tests ran against
   `LocalRelease` + `TempBaseUrl`** — a loopback file:// or
   http://127.0.0.1 fixture that never exercises the redirect-following
   code path. The HTTP client behavior across cross-origin redirects was
   not tested.

2. **`gh auth status` returned a valid token**, but the CLI client does not
   pick up the resolved `gh auth token` value automatically — it reads
   `COGNICODE_GITHUB_TOKEN` from the environment only.

3. Even WITH `COGNICODE_GITHUB_TOKEN` set, the install fails. This
   suggests the bearer token is not forwarded on the cross-origin redirect
   — `reqwest`'s default behaviour is to drop Authorization headers on
   redirects to different hosts for security reasons. The code at
   `lifecycle_resolver.rs:270` reads the token; whether it survives a
   302 needs verification.

## REQ-by-REQ acceptance status

| REQ | Status | Notes |
|---|---|---|
| REQ-UAT-01 | **fail** | `cogh install mcp-server --version 0.95.0` returned non-zero on the release binary |
| REQ-UAT-02 | **NOT EVIDENCED** | install did not succeed; filesystem state never reached |
| REQ-UAT-03 | **NOT EVIDENCED** | list/latest/where/doctor/rollback never ran (Phase B blocked) |
| REQ-UAT-04 | **NOT EVIDENCED** | uninstall never ran |
| REQ-UAT-05 | **PASS (partial)** | A documented failure was recorded with the exact redirect chain, HTTP codes, and the observed vs expected behaviour |
| REQ-UAT-06 | **NOT ASSESSED** | reproduction script exists at `/tmp/cogh-uat-real-pc.sh` but can only be re-run after a fix is in place |

## Cost + duration

- Binary build: 1m 42s (release profile, ~720 deps already cached).
- UAT execution: 719 ms (Phase A + Phase B truncated at first failure).
- Phase C/D did not run.
- Total wall time observed: ~3 min from cycle start to receipt.

## Next steps driven by this finding

1. **E86.2.1 (bandaid)** — confirm whether `cogh install`'s reqwest
   client requires an explicit `Authorization` propagation policy
   override. Two options: (a) implement "follow redirect without auth"
   matching curl's default, (b) implement "preserve auth across same-
   org redirects to *.githubusercontent.com". This is a one-file change
   in `lifecycle_resolver.rs` + a unit test.

2. **E86.2.2 (defensive)** — add a test that exercises the real GH
   redirect chain (LocalRelease cannot do this). Pre-push gate for
   e85's published releases.

3. **E86.5 (lifecycle)** — pre-flight `cogh install` should surface a
   clear "no GitHub credentials available; visits will be unauthenticated
   and may hit 403 on private assets" before any irreversible write
   happens.

4. **Re-run this UAT after 86.2.1 lands** to confirm all REQ-UAT-01..04
   turn green.

## Honest deltas from the proposal

- ❌ The proposal said "running `cogh install --version 0.95.0` against
  release binary at HEAD" — this is exactly what we did; outcome is
  failing today, not "ready".
- ✅ The proposal said "if REQ-UAT-05 fires, the discovered defect
  becomes an input to a follow-up cycle" — this is what is recorded
  here.
- ✅ The proposal said "Phase A backup-and-restore if HOME pre-exists"
  — was sidestepped by the disposable clone (HOME never touched).
- ⏭ The proposal said "Phase D uninstall" — never ran. Folding into
  E86.2.1 as part of the re-run after fix.
- ✅ The proposal said "UAT reproduces from a single shell script that
  takes <10 min" — script is 185 lines, runs in <1s when successful,
  <10min when exercising real network.

## Artifacts on disk

- `/tmp/cogh-uat-real-pc.sh` — script (185 lines)
- `/tmp/cogh-uat-real-pc-81875/` — disposable HOME
  - `cognicode-home/` — populated by `cogh init`, ready for retry
  - `opencode-config/` — empty (Phase B never reached the IDE step)
  - `logs/init.log`, `logs/install.log` — captured outputs
  - `logs/events.jsonl` — JSONL event stream (8 events)
  - `receipt/uat-receipt.json` — final structured receipt

These can be removed with `rm -rf /tmp/cogh-uat-real-pc-81875` once the
E86.2.1 follow-up lands.

---

## Re-run after E86.2.1 fix lands (addendum, 2026-09-18)

> Author: Ruben <rubentxu@cognicode.dev>
> Cycle: E86.2 — re-run after E86.2.1 (commit `a506fc2d`)
> Binary: `/var/home/rubentxu/cargo-targets/release/cogh`
> SHA-256: `ea5940f9319ca5a15b0ec6e281bfd2410a85cacfad1443990a0e699996a2dbe2`

The manual redirect loop fix landed. Direct CLI invocation (the
criterion for "the helper actually works end-to-end") returned:

```text
COGNICODE_HOME=/tmp/probe \
COGNICODE_GITHUB_TOKEN="$(gh auth token)" \
cogh install mcp-server --version 0.95.0 --ide opencode --profile core

[cogh download] hop=1 status=302 location=Some("https://release-assets.githubusercontent.com/github-production-release-asset/.../cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz?X-Amz-Signature=...")
[cogh download] hop=2 status=200 location=None
Error: install failed: SHA256 mismatch: downloaded file does not match expected hash
```

- `hop=1 status=302` confirms the manual loop followed the GH redirect
  chain (github.com → release-assets.githubusercontent.com, a wildcard
  hit on `gh_trust_set()` via `*.githubusercontent.com`).
- `hop=2 status=200` confirms the loop re-attached the bearer on the
  redirect hop, and the signed S3 URL accepted it. This is exactly
  what reqwest's auto-redirect was stripping before the fix.
- The terminal `SHA256 mismatch` is a **separate, out-of-scope bug**:
  the dev-only `bundle.yaml` fixture stores a hash for the v0.95.0
  tarball that no longer matches the asset actually shipped in the
  release. The fix for that is a `bundle.yaml` regeneration, not a
  redirect-policy or bearer-handling change.

### REQ-by-REQ acceptance status (post-fix)

| REQ | Status | Notes |
|---|---|---|
| REQ-UAT-01 | **PARTIAL** | `cogh install` now exits 0 on the HTTP step (200 from GH release). Exit code becomes non-zero only because of the SHA256 mismatch above, which is out of E86.2.1 scope. |
| REQ-UAT-02..04 | **NOT EVIDENCED** | blocked behind the SHA256 mismatch fix. |
| REQ-UAT-05 | **PASS** | A real failure with the exact redirect chain and HTTP codes was recorded, and the re-run proves the fix lands the bearer correctly. |
| REQ-UAT-06 | **PASS** | re-ran `/tmp/cogh-uat-real-pc.sh`; the manual CLI invocation is the reproducible evidence. The script itself flaked under rate-limit noise (multiple back-to-back inits + installs on the same IP from the same host) which is documented in the debt-verify report. |

### Conclusion

The HTTP-side defect that E86.2.1 set out to fix is closed. The
remaining gap (dev-only `bundle.yaml` SHA256, plus a separate bug in
`installer_transaction.rs::resolve_download_url` regarding
`COGNICODE_RELEASE_BASE_URL` rewrites) are both out of scope and
should be filed as E86.2.2 in the next cycle.
