# SPEC — E86.2.2 — Mirror-vs-download URL split

> Owner: E86.2.2
> Source: openspec/changes/e86-2-2-mirror-vs-download-url-split/proposal.md
> Status: DRAFT — apply will land these as executable tests.

---

## REQ-86-2-2-01a — `resolve_download_url` honors ASSET var

**Given** `COGNICODE_ASSET_BASE_URL` is set to a custom origin (e.g.
`https://my-mirror.example/assets`) and a canonical URL like
`https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/mcp-server-0.95.0-x86_64-unknown-linux-gnu.tar.gz`

**When** `resolve_download_url(canonical)` is called

**Then** it returns `https://my-mirror.example/assets/v0.95.0/mcp-server-0.95.0-x86_64-unknown-linux-gnu.tar.gz`

**And** it never inspects `COGNICODE_API_BASE_URL` or
`COGNICODE_RELEASE_BASE_URL`.

## REQ-86-2-2-01b — `resolve_download_url` ignores API URLs

**Given** an input URL that is *not* rooted at the canonical release
base (e.g. `https://api.github.com/repos/X/Y/releases/latest`,
`https://example.com/some/asset`)

**When** `resolve_download_url(url)` is called

**Then** the URL is returned unchanged regardless of the value of
`COGNICODE_ASSET_BASE_URL`. The rewrite only applies to canonical
release-asset paths.

## REQ-86-2-2-01c — `resolve_download_url` defaults to canonical

**Given** neither `COGNICODE_ASSET_BASE_URL` nor any deprecation env
var is set

**When** `resolve_download_url(canonical)` is called

**Then** it returns the input unchanged.

## REQ-86-2-2-02 — Resolver reads API base var

**Given** `COGNICODE_API_BASE_URL=https://my-enterprise/api/v3` is set

**When** `lifecycle_resolver::fetch_release_latest` is called

**Then** the request URL is rooted at the configured API base, not at
the hardcoded `https://api.github.com`.

**And** when `COGNICODE_API_BASE_URL` is unset, the resolver still
defaults to `https://api.github.com`.

## REQ-86-2-2-03 — Deprecation shim for old var name

**Given** `COGNICODE_RELEASE_BASE_URL=https://api.github.com` is set
**and** `COGNICODE_ASSET_BASE_URL` is unset

**When** `resolve_download_url(canonical)` is called

**Then** it returns the input unchanged (no rewrite)

**And** it emits a single `eprintln!` warning of the form:
`warning: COGNICODE_RELEASE_BASE_URL is deprecated; use COGNICODE_ASSET_BASE_URL for asset mirrors or COGNICODE_API_BASE_URL for the API base.`

**And** the warning is suppressed if the value is empty / unset.

## REQ-86-2-2-04 — Test suite green

**Given** the apply changes

**When** `cargo test -p cognicode-cli --bin cogh` runs

**Then** it reports 0 failed (was 189 + new T-A-01..04 = ≥193). No
test that previously passed may now fail.

## REQ-86-2-2-05 — UAT prefetch step added to script

**Given** `/tmp/cogh-uat-real-pc.sh` is invoked with
`COGH_BIN=/path/to/release/cogh` and `UAT_ROOT=/tmp/foo`

**When** Phase A preflight runs

**Then** a new step `phase_a_prefetch`:
- Reads the requested version (default `0.95.0`).
- Computes the bundle URL on the canonical release base.
- `curl -sSfL -H "Authorization: Bearer $(gh auth token)" "$bundle_url" -o "$UAT_ROOT/receipt/bundle.yaml"`.
- Records the bundle's SHA-256 into the events log.
- Exports `COGNICODE_BUNDLE_MANIFEST=$UAT_ROOT/receipt/bundle.yaml` so Phase B uses the real manifest.
- Aborts Phase A with a clear message if the bundle cannot be fetched.

**And** the script no longer exports `COGNICODE_RELEASE_BASE_URL`.

## REQ-86-2-2-06 — End-to-end UAT completes all phases

**Given** the rebuilt release binary with the E86.2.2 fix and
`COGNICODE_GITHUB_TOKEN="$(gh auth token)"`

**When** `/tmp/cogh-uat-real-pc.sh` runs end-to-end with a disposable
HOME

**Then** Phase B exit code is 0 (install succeeds, asset SHA matches)

**And** Phase C `cogh list` reports `mcp-server@0.95.0`

**And** Phase D uninstall removes `~/.cognicode/install/mcp-server/0.95.0/`.

**And** `uat-receipt.md` REQ-UAT-01..04 flip from PARTIAL/NOT-EVIDENCED
to PASS.

---

## Defect documentation

The two RED defect-doc tests retained from E86.2.1
(`lifecycle_resolver::tests::sc_fix_01_*` and `sc_fix_02_*`) continue
to pass. They document the reqwest 0.12.28 strip-on-redirect behavior
the manual loop works around. No new defect-doc tests are introduced
in E86.2.2.
