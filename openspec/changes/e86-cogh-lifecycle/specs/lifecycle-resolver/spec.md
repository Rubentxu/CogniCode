# Spec: lifecycle-resolver

> e86 WU1. Operational authority is `state.yaml`.

## Purpose

`cogh` needs a deterministic, testable way to answer the question
"what is the latest published CogniCode release for this host, and where is
its BundleManifest v2?" without typing a URL or a version. The resolver is
the single seam between `cogh` and GitHub Releases; no command, no shell
script, no workflow ever composes a download URL.

## Scope

In scope:

- `cogh latest [plugin]` — print the latest published `vX.Y.Z` for the
  host platform.
- `cogh update [plugin]` — resolve, download the manifest, write it to
  `~/.cognicode/bundle.yaml`, then delegate to the existing
  `InstallerTransaction::run` pipeline.
- Channel selection: `--channel stable` (default; only real channel today).
  `--channel preview` returns a clear "no preview channel published" error.
- Draft-first safety: any release with `draft: true` or `prerelease: true`
  is refused.
- Non-Tier-1 platforms: refused with a clear "platform not in the e85
  Tier-1 surface" error.
- Override: `--staging <dir>` and `--base-url <url>` so tests and
  air-gapped installs can substitute a fake GitHub API.

Out of scope (deferred):

- A real `preview` channel backend.
- Cross-channel resolution (e.g. "promote a draft").
- A local cache of releases; the resolver hits the API and is done.

## Requirements

### REQ-LR-01 — host platform gating

The resolver MUST refuse to run when the host's detected `Platform` is not
in `TIER1_PLATFORMS`. The error message MUST list the supported tokens.

**When** the host is Linux x86_64 or Linux aarch64,
**then** the resolver proceeds.
**When** the host is any other platform,
**then** the resolver returns `InstallerError::PlatformNotInTier1` with the
detected platform and the list of supported tokens in the message.

### REQ-LR-02 — version resolution contract

The resolver MUST turn `host_platform + channel + requested_version` into:

```text
tag          = "v" + semver
version      = semver (no leading "v")
manifest_url = "{RELEASE_DOWNLOAD_BASE}/v{version}/bundle-{version}-{platform_token}.yaml"
digest       = SHA256SUMS line for that manifest filename
```

`requested_version` defaults to `"latest"`.

**When** `requested_version = "latest"`,
**then** the resolver calls `GET https://api.github.com/repos/Rubentxu/CogniCode/releases/latest`
and reads `tag_name`, stripping the leading `v`.
**When** `requested_version = "vX.Y.Z"` or `"X.Y.Z"`,
**then** the resolver calls `GET .../releases/tags/vX.Y.Z`.

### REQ-LR-03 — draft / prerelease rejection

**When** the resolved release has `draft: true` or `prerelease: true`,
**then** the resolver returns `InstallerError::DraftRelease` with the tag
and a message that includes "draft" so the failure is unambiguous.
This check MUST be performed even for `/releases/latest`, because a draft
newer than a published release may briefly exist.

### REQ-LR-04 — manifest asset presence

The resolver MUST verify the resolved release contains an asset named
exactly `bundle-{version}-{platform_token}.yaml`. The name MUST be derived
from the Rust contract (`bundle_manifest_filename`), never typed.

**When** the asset is absent,
**then** the resolver returns `InstallerError::NoMatchingManifest` with
the platform and the list of asset names actually present.

### REQ-LR-05 — GitHub auth precedence

The resolver MUST read the API token from `COGNICODE_GITHUB_TOKEN` first,
and fall back to anonymous when the variable is unset.

**When** `COGNICODE_GITHUB_TOKEN` is set,
**then** requests carry `Authorization: Bearer <token>`.
**When** the variable is unset,
**then** requests are anonymous.
The resolver MUST NOT read or write `~/.config/gh/hosts.yml`.

### REQ-LR-06 — `--base-url` override

The resolver MUST honour `--base-url <url>` for tests and air-gapped
installs. When set, the GitHub API base becomes `<url>` and the asset
download base becomes `<url>`.

**When** `--base-url` is set,
**then** every HTTP call goes to that base.
**When** `--base-url` is unset,
**then** the default is `https://api.github.com` for API calls and
`https://github.com/Rubentxu/CogniCode/releases/download` for asset URLs
(the e85 contract base).

### REQ-LR-07 — `--staging` test override

`--staging <dir>` MUST exist solely for tests. When set, the resolver MUST
read the GitHub API response from `<dir>/releases.json` (a file in the shape
of `GET /releases`) and pick the latest non-draft, non-prerelease release
that has an asset for the host platform.

**When** `--staging` is set and `<dir>/releases.json` is missing,
**then** the resolver returns `InstallerError::ResolveFailed` with a clear
message naming the missing file.
**When** `--staging` is set and `--base-url` is also set,
**then** `--base-url` wins for the asset download origin (tests that want
to redirect asset downloads).

### REQ-LR-08 — JSON parsing strictness

The resolver MUST use `serde_json` to parse the GitHub API response.
Unknown fields are ignored. The required fields are:

- `tag_name: String`
- `draft: bool`
- `prerelease: bool`
- `assets: Vec<{ name: String, browser_download_url: String }>`

**When** any required field is missing or has the wrong type,
**then** the resolver returns `InstallerError::ResolveFailed` with the
field name and the parse error.

### REQ-LR-09 — output of `cogh latest`

`cogh latest --json` MUST print a JSON object with at least:

```json
{
  "version": "0.95.0",
  "tag": "v0.95.0",
  "platform_token": "x86_64-unknown-linux-gnu",
  "manifest_url": "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
  "manifest_sha256": "<hex>",
  "published_at": "2026-09-17T19:07:59Z",
  "release_url": "https://github.com/Rubentxu/CogniCode/releases/tag/v0.95.0"
}
```

Without `--json`, the resolver MUST print exactly:

```text
v0.95.0
```

(one line, the tag).

### REQ-LR-10 — output of `cogh update`

`cogh update` MUST:

1. Resolve to a (version, manifest_url, manifest_sha256) tuple via the
   resolver (REQ-LR-02..08).
2. Download the BundleManifest v2 to `~/.cognicode/bundle.yaml` over HTTPS
   using `reqwest`, verifying the digest from `SHA256SUMS` at the same
   tag.
3. Delegate to `install::run_install(home, profile)` for the full install
   pipeline (the existing single backend; no second pipeline).
4. On success, the install manifest, the tracker, and the journal are all
   written by the existing transaction; nothing new is written by e86
   beyond the manifest at step 2 and a possible journal at step 3.

**When** `cogh update --dry-run` is set,
**then** the resolver runs and prints the resolved tuple, but no download
or install happens.

## Scenarios

| ID | When | Then |
|---|---|---|
| SC-LR-01 | Host = linux-x86-64; latest release is `v0.95.0`, non-draft | Resolver returns tag `v0.95.0` and the manifest URL derived from the contract |
| SC-LR-02 | Host = windows-x86-64 | Resolver returns `PlatformNotInTier1` and lists `linux-x86-64, linux-aarch64` |
| SC-LR-03 | Latest release is a draft | Resolver returns `DraftRelease` with the tag |
| SC-LR-04 | Resolved release lacks the per-platform manifest asset | Resolver returns `NoMatchingManifest` with the asset list |
| SC-LR-05 | `--staging <dir>` is set and `releases.json` exists | Resolver reads from the file instead of GitHub |
| SC-LR-06 | `--base-url https://mirror.example.com` is set | Asset download URL is rewritten to `https://mirror.example.com/...` |
| SC-LR-07 | `COGNICODE_GITHUB_TOKEN` is set | Request includes `Authorization: Bearer <token>` |
| SC-LR-08 | `requested_version = "0.94.0"` | Resolver calls `/releases/tags/v0.94.0` |
| SC-LR-09 | Network call fails (DNS or HTTP error) | Resolver returns `ResolveFailed` with the underlying error |
| SC-LR-10 | `cogh update --dry-run` | Resolver prints the tuple; no HTTP download of the manifest; no install |

## Exit gates

This spec is satisfied when:

- `cargo test -p cognicode-cli --bin cogh lifecycle::tests::test_resolver_*`
  passes all of SC-LR-01..10.
- The existing 147 `cogh` tests still pass.
- `cargo check --workspace --all-targets` exits 0.
- `cargo fmt -p cognicode-cli --check` exits 0.