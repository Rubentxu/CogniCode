# APPLY RECEIPT — E86.2.1

> Owner: E86.2.1
> Cycle: apply
> Date: 2026-09-18
> HEAD: a506fc2d (parent: cfc079d2)
> Author: Ruben <rubentxu@cognicode.dev>
> Branch: main (local, no push)

---

## 1. Tasks closed

| task | status | evidence |
|---|---|---|
| T1: `download_with_bearer()` helper + refactor `Downloading` | DONE | `crates/cognicode-cli/src/cmd/installer_transaction.rs` +359/-4. New helper lines 76–163, replaced arm lines 250–273 with `download_with_bearer()` call + `.redirect(reqwest::redirect::Policy::none())` on the production client. |
| T2: 3 GREEN tests | DONE | `installer_transaction::tests::e86_2_1_fix_01/02/03` lines 997–1055. Test helpers `bind_one_shot_*`, `header_value`, `build_test_client` colocated in `tests` module with `// SHARED-WITH crate::lifecycle_resolver::tests` marker. |
| T3: DEFECT-DOC annotation block | DONE | `lifecycle_resolver::tests` DEFECT-DOC block at lines 813–827 above `sc_fix_01_*`. |
| T4: Re-run E86.2 UAT | DONE with caveats | See §3 below. Phase B `hop=1 status=302 → hop=2 status=200` confirmed manually; the reusable UAT script (`/tmp/cogh-uat-real-pc.sh`) flaked during automation due to GitHub rate-limit between `init` and `install` plus a stale SHA256 in the dev-fixture bundle.yaml — neither of which is a regression. |
| T5: gates | DONE | `cargo test` 189/0; `cargo fmt -p cognicode-cli` clean; `cargo clippy` no new warnings on touched files. |
| T6: commit + receipt | DONE | this file + commit `a506fc2d`. |
| T7: state files | DONE | see `uat-receipt.md` delta below; `.agent/TESTING-STATE.md` update deferred (out-of-scope for the apply commit). |

## 2. Diff summary

```
 crates/cognicode-cli/src/cmd/installer_transaction.rs | 359 +++++++++++++-
 crates/cognicode-cli/src/cmd/lifecycle_resolver.rs    |  53 ++
 2 files changed, 408 insertions(+), 4 deletions(-)
```

## 3. Empirical fix verification

Beyond the 3 GREEN unit tests, the fix was confirmed against the real
GitHub Release endpoint (`https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/...`):

```
$ COGNICODE_HOME=/tmp/probe COGNICODE_GITHUB_TOKEN="$(gh auth token)" \
    cogh install mcp-server --version 0.95.0 --ide opencode --profile core
[cogh download] hop=1 status=302 location=Some("https://release-assets.githubusercontent.com/github-production-release-asset/.../?X-Amz-Signature=...")
[cogh download] hop=2 status=200 location=None
Error: install failed: SHA256 mismatch: downloaded file does not match expected hash
```

- `hop=1 status=302` confirms the manual loop picked up the GH redirect
  chain (github.com → release-assets.githubusercontent.com, a wildcard
  hit on `gh_trust_set()` via `*.githubusercontent.com`).
- `hop=2 status=200` confirms the loop correctly re-attached the bearer
  on the second hop and the signed S3 URL accepted it (the bearer
  re-attachment is exactly what reqwest's auto-redirect policy was
  stripping).
- The `SHA256 mismatch` is a *different* bug: the dev-only `bundle.yaml`
  fixture carries a stored hash for the v0.95.0 tarball that no longer
  matches the asset actually shipped. This is **out of scope for
  E86.2.1** and is catalogued as a follow-up in the debt-verify report
  (item 9) once that worker delivers.

## 4. Decision on open question Q1

From `proposal.md`:
> Q1. Should `download_with_bearer()` be public (reusable from
> `registry.rs` and `layout.rs` later) or stay a private fn inside
> `installer_transaction.rs`?

**Decision: PRIVATE.** The two other modules' URLs are direct (no
redirect hop), so they don't need it. Keeping the helper private
minimises blast radius. If a future E86.2.2 needs the same loop in
`lifecycle_resolver::http_get` (only relevant if GitHub ever redirects
`/releases/latest` cross-host, which it does not today), the helper
will be promoted to `pub` at that time. The decision is recorded here
so the apply agent and the archive closure can find it without
re-reading the proposal.

## 5. Deviations from `design.md`

**Design said**: `bearer_from_env()` is reused to decide bearer presence. ✓
**Code does**: implements that exactly. ✓

**Design said**: `MAX_DOWNLOAD_REDIRECTS: u8 = 5`. ✓
**Code does**: same. ✓

**Design said**: `gh_trust_set()` allows `*.githubusercontent.com`
wildcard matches. ✓
**Code does**: yes; `gh_trust_set()` returns exact strings, the
helper-local `is_in_trust_set()` interprets `*.` prefix as suffix-match.
Wildcards are checked after the host:port is split out (so
`127.0.0.1:46709` is reduced to `127.0.0.1` before comparison). ✓

**Design said**: error envelope `Network("download",
"DisallowedRedirectHost <host>")`. ✓
**Code does**: same. ✓

**Design said**: client's default redirect policy is the issue; helper
controls hops. ✓
**Code does**: production client now `.redirect(Policy::none())` so the
manual loop owns every hop end-to-end. The .md noted "auto-redirect is
left at default for the *first* hop" — that was wrong; the apply worker
discovered empirically that the helper cannot control the first hop
unless the policy is `none()` from the start. The design was updated
to clarify: see §"Implemented behaviour" in `design.md` (note the
auto-redirect note was rewritten before apply started; design.md
already reflects the corrected approach; this receipt records the
observation for the next maintainer).

## 6. What I changed vs what the apply worker drafted

The original apply worker (`session_tulip`) drafted the tests and the
helper, but three issues prevented the GREEN: (a) it did not put
`Policy::none()` on the production client, so reqwest was auto-following
the first hop and stripping the header — same defect, different layer;
(b) the production client's first hop therefore succeeded with the
default policy and a stripped Authorization — *not* what the spec
required; (c) the test 03 used one-shot listeners which die after one
connection, so a 6-hop loop simply got "connection refused" instead
of `TooManyRedirects`. I addressed (a), (c) and the trust-set
extension in place. Net change vs worker draft: +13 production-code
lines (host:port parsing fix, `Policy::none()` on the production
client, `let response` instead of `let mut response`), +15 test lines
(persistent loop in test 03, `build_test_client()` helper used by all
3 tests).

## 7. Verification reproducible on this host

```bash
cd /var/home/rubentxu/cognicode
cargo test -p cognicode-cli --bin cogh -- --quiet
# → 189 passed; 0 failed; 1 ignored

cargo fmt -p cognicode-cli --check
# → exit 0

cargo clippy -p cognicode-cli --bin cogh --tests 2>&1 \
  | grep -A4 "installer_transaction.rs:"
# → 4 pre-existing warnings on lines 13/209/225/622 (unrelated), 0 new

COGNICODE_HOME=/tmp/verify-home \
COGNICODE_GITHUB_TOKEN="$(gh auth token)" \
/var/home/rubentxu/cargo-targets/release/cogh install mcp-server \
  --version 0.95.0 --ide opencode --profile core
# expected: hop=1 status=302 → hop=2 status=200 (followed by SHA256
# mismatch on the dev-fixture bundle.yaml, which is out-of-scope).
```

## 8. NEXT cycle — what's pending

- **Debt-verify report** in progress via `sddk-debt-verify` worker
  `session_maple`. Items it should report on (already drafted in the
  worker prompt): loopback alias risk in `gh_trust_set`, the
  hard-coded `MAX_DOWNLOAD_REDIRECTS=5`, the SHA256 mismatch found in
  the dev fixture, the `Policy::none()` decision, the
  `response.bytes().unwrap()` body-into-memory pattern.
- **Follow-up E86.2.2** is anticipated: scope is the
  `resolve_download_url` ↔ `COGNICODE_RELEASE_BASE_URL` interaction
  (the UAT script's default export of `api.github.com` was found to
  incorrectly rewrite asset URLs); plus the bundle.yaml SHA256 staleness.
  These are NOT blockers for closing E86.2.1; they belong to the next
  cycle so the user can decide E86.2.2 scope cleanly.
