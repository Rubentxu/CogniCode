# SPEC — Bearer Token Strip on Cross-Origin Redirect (downloads)

> Owner: E86.2.1
> Source: `openspec/changes/e86-2-1-bearer-on-redirect-fix/proposal.md`
> Design: `openspec/changes/e86-2-1-bearer-on-redirect-fix/design.md`
> Status: DRAFT (will become executable via `sdd-spec` once apply starts)

## REQ-FIX-01: Bearer propagates through redirect chain

**Given** the installer `Downloading` arm
(`InstallStage::Downloading` in
`crates/cognicode-cli/src/cmd/installer_transaction.rs`)

**And** `COGNICODE_GITHUB_TOKEN` is set to a non-empty value

**And** the asset URL redirects (HTTP 30x with `Location` header) to
a host in `lifecycle_resolver::gh_trust_set()`

**When** `download_with_bearer(client, asset_url, Some(token))` is
called

**Then** each redirect hop issues a fresh `GET` to the next URL with
`Authorization: Bearer <token>` re-attached

**And** the final response body is delivered to the caller (write to
disk)

**And** no request leaves the helper without the bearer on redirect
hops inside the trust set.

## REQ-FIX-02: Hop to a host outside the trust set is rejected

**Given** a redirect chain where any hop's `Location` host is **not**
in `lifecycle_resolver::gh_trust_set()` (e.g. `attacker.example.com`)

**When** `download_with_bearer(client, asset_url, Some(token))` is
called

**Then** it returns `Err(InstallerError::Network("download",
format!("DisallowedRedirectHost {host}")))`

**And** no request is issued to the untrusted host (verified by TCP
listener receiving zero connections on that host).

## REQ-FIX-03: Redirect chain longer than budget is rejected

**Given** a redirect chain of 6 or more hops (synthetic 6-hop listener)

**When** `download_with_bearer(client, asset_url, Some(token))` is
called

**Then** it returns `Err(InstallerError::Network("download",
"TooManyRedirects (>5)"))` after the 6th hop's `Location` is observed.

## REQ-FIX-04: No bearer ⇒ no bearer attached (regression guard)

**Given** `COGNICODE_GITHUB_TOKEN` is unset or empty

**When** `download_with_bearer(client, asset_url, None)` is called

**Then** no `Authorization` header is sent on any hop

**And** the public-asset behaviour (HTTP 200 from a
`github.com` redirect to `objects.githubusercontent.com`) is preserved.

## REQ-FIX-05: End-to-end E86.2 UAT Phase B turns GREEN

**Given** the rebuilt `cogh` release binary with the E86.2.1 fix
applied

**And** `COGNICODE_GITHUB_TOKEN="$(gh auth token)"` set in the
disposable UAT environment

**When** `cogh install --version 0.95.0` runs in `/tmp/cogh-uat-real-pc-*/`

**Then** the asset HTTP exchange returns 200 (no 403)

**And** `cogh list` shows `mcp-server@0.95.0` after install

**And** the disposable home directory (`COGNICODE_HOME`) contains
`install/mcp-server/0.95.0/` populated with the expected shims and
manifest.

## Defect documentation (RED tests kept after fix lands)

`sc_fix_01_default_policy_strips_authorization_on_cross_host_redirect`
and `sc_fix_02_untrusted_redirect_target_is_not_followed` in
`crates/cognicode-cli/src/cmd/lifecycle_resolver.rs::tests` continue to
pass against reqwest 0.12.28's default policy. They are marked
`// DEFECT-DOC` and serve as regression guards against future reqwest
upgrades that might silently change the strip-on-redirect semantics.
