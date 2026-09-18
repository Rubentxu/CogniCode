# e86.2.2 — Mirror-vs-download URL split + UAT bundle path

> Cycle: e86-2-2-mirror-vs-download-url-split
> Program: cognicode-distribution
> Parent: e86.2 (uat-receipt.md Phase REQ-UAT-01 PARTIAL after E86.2.1
>   closed the HTTP side; SHA256 mismatch is by-design on the dev
>   fixture, and a separate bug in `resolve_download_url` rewrites
>   canonical asset URLs onto `api.github.com` when
>   `COGNICODE_RELEASE_BASE_URL` is set to the API base, breaking
>   asset fetches).
> Phase: propose | Date: 2026-09-18
> Delivery: bounded A-lite (propose → spec → tasks → apply → verify →
>   debt-verify → release → archive).
> Severity: PARTIAL — the HTTP-side defect is already closed by E86.2.1
>   (`a506fc2d`); this cycle closes the residual URL-split + UAT
>   plumbing so REQ-UAT-01..04 can flip from PARTIAL/NOT-EVIDENCED to
>   PASS end-to-end.

## Intent

Two intertwined defects surfaced while the E86.2 UAT was being re-run
against the post-E86.2.1 release binary:

1. **`resolve_download_url` (`installer_transaction.rs:42`) rewrites
   canonical asset URLs onto whatever `COGNICODE_RELEASE_BASE_URL`
   points at, *including `https://api.github.com`*.** That env var
   originated as the resolver's API base (used by
   `lifecycle_resolver::fetch_release_latest`), but the same name now
   drives two semantically distinct things:
   - **API base** for resolver JSON metadata (`/repos/.../releases/latest`)
   - **asset fetch origin** for installer downloads
   - (which today happens to be the *same* host — `api.github.com`
     cannot serve `/releases/download/...`, only JSON)
   The result: when the UAT export `COGNICODE_RELEASE_BASE_URL=https://api.github.com`
   is set, the installer's `resolve_download_url` rewrites every
   `https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/<asset>`
   into `https://api.github.com/Rubentxu/CogniCode/releases/download/v0.95.0/<asset>`,
   which returns 403 (or 404). Detection of this rewrite failure mode
   was logged in `uat-receipt.md` addendum §"Conclusion".

2. **The reusable UAT script (`/tmp/cogh-uat-real-pc.sh`) cannot drive
   end-to-end Phase B without a real `bundle.yaml`.** The dev-only
   fixture (`crates/cognicode-cli/src/cmd/dev-bundle.yaml`) carries
   placeholder SHA256 sums by design (line 443–455 of
   `installer_transaction.rs`), so any install driven by it fails at
   the SHA256 stage by construction. To run a real end-to-end UAT we
   need the UAT script to fetch the v0.95.0 bundle from the release
   itself and pass it via `COGNICODE_BUNDLE_MANIFEST`. The script does
   not yet do that. Without that step REQ-UAT-02..04 cannot be
   evidenced.

These two defects are independent but both block the UAT GREEN
verdict, and both belong to the same bounded cycle because the fix to
(1) is what makes (2) achievable for subsequent automation runs.

## Approach

### Fix #1 — separate API base from asset download origin

Replace the single `COGNICODE_RELEASE_BASE_URL` env var with two:

- `COGNICODE_API_BASE_URL` — API resolution origin (was `..._RELEASE_...`).
  Default: `https://api.github.com`. Read in
  `lifecycle_resolver::fetch_release_latest`, `fetch_release_by_tag`.
- `COGNICODE_ASSET_BASE_URL` — asset download origin (new). Default:
  *empty* (no rewrite; canonical `github.com` URLs are used as-is).

`resolve_download_url` reads the new `COGNICODE_ASSET_BASE_URL`. If
unset or empty, canonical URLs pass through unchanged. The single
existing var name is **retired** with a one-time compatibility shim:
for one cycle, if `COGNICODE_RELEASE_BASE_URL` is set and the new asset
var is not, *and* the value's host is `api.github.com` or ends in
`.github.com`, the helper logs a clear deprecation warning and falls
back to canonical. After this cycle ships, only the new vars are
recognized (the deprecation shim is removed in E86.2.3 or E87).

Rationale: this makes the contract explicit, prevents the footgun
where mirrors and the API base silently share a host, and gives users a
clear opt-in for asset mirrors in the future without colliding with the
API base.

### Fix #2 — UAT script pre-fetches the real bundle

Extend `/tmp/cogh-uat-real-pc.sh` with a new step `phase_a_prefetch`
that:

1. Reads the requested `--version` (default `0.95.0`).
2. `curl -sSfL -H "Authorization: Bearer $(gh auth token)" "<canonical>/bundle-<version>-<platform>.yaml"`
   into `$UAT_ROOT/receipt/bundle.yaml`. The first real-GH round trip
   in the script; if it fails, Phase A aborts the same way the network
   check aborts.
3. `export COGNICODE_BUNDLE_MANIFEST=$UAT_ROOT/receipt/bundle.yaml`
   so Phase B uses the real manifest, not the dev-only fixture.
4. Logs the `sha256sum` of the bundle for the receipt.

The `init` step no longer needs `COGNICODE_RELEASE_BASE_URL` to be set
(its only use was causing issue #1). We **remove the export** of that
variable from the script entirely.

### Tests (apply phase)

- **T-A-01**: `resolve_download_url()` with no env returns canonical.
- **T-A-02**: `resolve_download_url()` with `COGNICODE_ASSET_BASE_URL=https://my-mirror.example/assets`
  rewrites `https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/foo`
  to `https://my-mirror.example/assets/v0.95.0/foo` and **does NOT
  touch `https://api.github.com/repos/...` URLs** (those are resolver
  territory and pass through unchanged).
- **T-A-03**: deprecation shim — setting
  `COGNICODE_RELEASE_BASE_URL=https://api.github.com` with the new
  asset var unset triggers a `eprintln!` warning and falls back to
  canonical (no rewrite).
- **T-A-04**: `lifecycle_resolver::fetch_release_latest` now reads
  `COGNICODE_API_BASE_URL` (renamed); a regression test confirms
  existing resolve-by-tag tests still pass with the new var name.
- **T-A-05**: integration — a fresh UAT run with the script update
  completes Phase B with `cogh list` showing `mcp-server@0.95.0`,
  `cogh doctor` clean, and Phase D uninstall removing
  `~/.cognicode/install/mcp-server/0.95.0/`.

### Risk surface

- **Breaking change for users with custom asset mirrors today.** The
  deprecation shim covers `api.github.com` (the only documented
  `COGNICODE_RELEASE_BASE_URL` value in the UAT script). Anyone
  pointing at a real mirror via the old name will need to migrate to
  `COGNICODE_ASSET_BASE_URL`. Acceptable because the old name was
  never documented for asset mirroring (it was originally the API
  base); a deprecation warning in the cycle deliverables is enough.
- **No data-loss risk**: the fix only changes URL strings; the
  installer's transaction logic and the resolver's caching are
  untouched.
- **Test fragility**: the integration test depends on reaching real
  GitHub Releases. Risk mitigated by the script's existing
  preflight; if the network check fails the script aborts before the
  prefetch. Local-only tests (T-A-01..04) use loopback fixtures.

## Acceptance contract

| REQ | Observable |
|---|---|
| REQ-86-2-2-01 | `resolve_download_url` honours `COGNICODE_ASSET_BASE_URL`; does NOT rewrite onto `api.github.com` |
| REQ-86-2-2-02 | `lifecycle_resolver::http_get` honours `COGNICODE_API_BASE_URL` (renamed); existing tests pass |
| REQ-86-2-2-03 | Deprecation warning emitted when `COGNICODE_RELEASE_BASE_URL=api.github.com` is set; canonical URL used |
| REQ-86-2-2-04 | `cargo test -p cognicode-cli --bin cogh` is 0 failed (was 189; expect ~194) |
| REQ-86-2-2-05 | `cargo fmt -p cognicode-cli --check` clean |
| REQ-86-2-2-06 | `cargo clippy -p cognicode-cli --bin cogh --tests` — no new warnings on touched files |
| REQ-86-2-2-07 | UAT Phase A pre-fetches the v0.95.0 bundle; Phase B exits 0; Phases C/D run to completion |
| REQ-86-2-2-08 | `uat-receipt.md` REQ-UAT-01..04 flip from PARTIAL/NOT-EVIDENCED to PASS |

## Risks

- **External network dependency for T-A-05.** Mitigated by the
  preflight; if GH is unreachable the test fails cleanly with no
  partial state.
- **`gh auth token` expiry.** UAT script already handles this via the
  preflight (no token → preflight aborts).
- **Deprecation shim removal timing.** If we forget to remove the shim
  in E86.2.3/E87, the warning becomes noise. Logged in debt-verify as
  MED.

## Non-goals

- Adding per-component mirror configuration (over-engineering for one cycle).
- Supporting non-GitHub remotes — out of scope; E86.2.2 only fixes the
  intra-GitHub split.
- Switching from `include_str!("dev-bundle.yaml")` to a generated
  dev-only fixture. Out of scope; the existing fixture is by-design.

## Sequencer

- e86.2.2 is **blocked-by nothing** (E86.2.1 is closed at `a506fc2d`).
- Itself unblocks E86.2's `uat-receipt.md` REQ-UAT-01..04 → flip to PASS.
- Does NOT block E86.3 (uninstall coverage), which can start in a
  parallel bounded cycle once e86.2.2 is closed.
