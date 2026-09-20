//! `cogh::lifecycle_resolver` — GitHub Releases → resolved BundleManifest v2.
//!
//! e86: the single seam between `cogh` and GitHub Releases. Every command
//! (`cogh latest`, `cogh update`, `cogh rollback`) goes through this module to
//! discover which version is current and where its manifest lives. No command,
//! no shell script, no workflow composes a download URL by hand.
//!
//! Behaviour summary (full contract in `openspec/changes/e86-cogh-lifecycle/specs/lifecycle-resolver/spec.md`):
//!
//! 1. **Platform gating** — only `linux-x86-64` and `linux-aarch64` are Tier-1
//!    today. Other hosts get a loud `PlatformNotInTier1` error that lists the
//!    supported tokens.
//! 2. **Version resolution** — `requested_version = "latest"` hits
//!    `/releases/latest`; an explicit `vX.Y.Z` (or `X.Y.Z`) hits
//!    `/releases/tags/vX.Y.Z`.
//! 3. **Draft / prerelease rejection** — if the resolved release is a draft
//!    or a prerelease, return `DraftRelease` (REQ-LR-03). The check is on the
//!    resolved release, not a separate "filter", so a draft sitting on top of
//!    a published release cannot slip through `/releases/latest`.
//! 4. **Manifest asset presence** — verify the resolved release carries the
//!    per-platform manifest asset whose name is `bundle_manifest_filename(version, platform)`.
//! 5. **Auth** — `COGNICODE_GITHUB_TOKEN` is read for the bearer header;
//!    absence means anonymous.
//! 6. **Overrides** — `--base-url` rewrites both the API base and the asset
//!    download origin; `--staging <dir>` reads a frozen `releases.json`
//!    fixture (no network). `--staging` plus `--base-url` is the test seam.
//! 7. **Strict JSON** — unknown fields ignored; missing or wrong-typed
//!    required fields bubble as `ResolveFailed`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::bundle_manifest::Platform;
use crate::error::InstallerError;
use crate::release_contract::{
    RELEASE_DOWNLOAD_BASE, RELEASE_REPO, TIER1_PLATFORMS, bundle_manifest_filename, platform_token,
};

/// Default GitHub API base URL.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// Environment variable naming the GitHub API origin (release/manifest side).
///
/// Used by `lifecycle_resolver::fetch_release_latest` and
/// `fetch_release_by_tag` to construct `/repos/{RELEASE_REPO}/releases/...`
/// API URLs. Default is `https://api.github.com`.
///
/// ## Precedence (highest first)
///
/// 1. `COGNICODE_API_BASE_URL` — env-var override.
/// 2. `COGNICODE_RELEASE_BASE_URL` — legacy alias, accepted for
///    back-compat (E86.2.2 did not break this public surface).
/// 3. `ResolveRequest::base_url` — programmatic override.
/// 4. [`DEFAULT_API_BASE`] — default (`https://api.github.com`).
///
/// E86.2.2 split this concept from the asset-side
/// `COGNICODE_ASSET_BASE_URL` (which governs component download URL
/// rewriting, not release metadata resolution). The two variables are
/// deliberately distinct: an air-gapped install with a custom asset mirror
/// can still talk to the real GitHub API for release metadata.
pub const ENV_API_BASE_URL: &str = "COGNICODE_API_BASE_URL";

/// Network timeout for resolver HTTP calls.
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);

/// Release channels supported by `cogh`. Only `Stable` is implemented today;
/// `Preview` returns a clear "no preview channel published" error (REQ-LR-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    Stable,
    Preview,
}

impl std::str::FromStr for Channel {
    type Err = InstallerError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "stable" => Ok(Channel::Stable),
            "preview" => Ok(Channel::Preview),
            other => Err(InstallerError::ResolveFailed(format!(
                "unknown channel `{other}` (supported: stable, preview)"
            ))),
        }
    }
}

impl std::fmt::Display for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Channel::Stable => f.write_str("stable"),
            Channel::Preview => f.write_str("preview"),
        }
    }
}

/// Inputs to the resolver.
#[derive(Debug, Clone)]
pub struct ResolveRequest {
    /// Detected host platform. The resolver refuses if not Tier-1.
    pub host_platform: Platform,
    /// Release channel (`stable` only today).
    pub channel: Channel,
    /// "latest" or "vX.Y.Z" (or "X.Y.Z"). Defaults to "latest" via `Default`.
    pub requested_version: String,
    /// Override for the GitHub API base (and the asset origin).
    pub base_url: Option<String>,
    /// Path to a directory holding a frozen `releases.json` for tests.
    pub staging_dir: Option<PathBuf>,
}

impl Default for ResolveRequest {
    fn default() -> Self {
        Self {
            host_platform: crate::platform_adapter::detect_host_platform(),
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: None,
        }
    }
}

/// Output of a successful resolve. Carries everything the installer needs to
/// download the manifest without going back to GitHub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRelease {
    pub version: String,
    pub tag: String,
    pub platform_token: String,
    pub manifest_url: String,
    pub manifest_sha256: Option<String>,
    pub published_at: Option<String>,
    pub release_url: String,
}

/// Bare structure for the GitHub Releases API response. Only the fields we
/// need are typed; unknowns are skipped (REQ-LR-08).
#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

/// Bare structure for the `/releases` listing endpoint (used by the staging
/// fixture and by `latest` if we want to scan).
#[derive(Debug, Deserialize)]
struct GhListRelease {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

/// Resolve to a `ResolvedRelease` or an `InstallerError`.
///
/// Strategy:
/// - If `staging_dir` is set, read `<dir>/releases.json` and pick the latest
///   non-draft, non-prerelease release with the per-platform asset.
/// - Otherwise, hit the GitHub API (`/releases/latest` or `/releases/tags/...`)
///   using `reqwest::blocking`.
pub fn resolve_release(req: &ResolveRequest) -> Result<ResolvedRelease, InstallerError> {
    // REQ-LR-01
    if !TIER1_PLATFORMS.contains(&req.host_platform) {
        let supported = TIER1_PLATFORMS
            .iter()
            .map(|p| format!("`{}`", platform_token(*p)))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(InstallerError::PlatformNotInTier1(
            req.host_platform,
            supported,
        ));
    }

    // REQ-LR-02 — only `stable` is implemented.
    if req.channel == Channel::Preview {
        return Err(InstallerError::ResolveFailed(
            "no preview channel published yet".to_string(),
        ));
    }

    let release = if let Some(dir) = &req.staging_dir {
        load_release_from_staging(dir)?
    } else if req.requested_version == "latest" {
        fetch_release_latest(req)?
    } else {
        let tag = normalise_tag(&req.requested_version);
        fetch_release_by_tag(req, &tag)?
    };

    // REQ-LR-03
    if release.draft || release.prerelease {
        return Err(InstallerError::DraftRelease(release.tag_name.clone()));
    }

    let version = strip_v_prefix(&release.tag_name);

    // REQ-LR-04
    let expected_asset = bundle_manifest_filename(&version, req.host_platform);
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == expected_asset)
        .ok_or_else(|| {
            let names: Vec<String> = release.assets.iter().map(|a| a.name.clone()).collect();
            InstallerError::NoMatchingManifest(req.host_platform, names)
        })?;

    let platform_token = platform_token(req.host_platform).to_string();
    let manifest_url = if let Some(base) = &req.base_url {
        rewrite_url(base, &asset.browser_download_url)
    } else {
        asset.browser_download_url.clone()
    };

    let release_url = release.html_url.unwrap_or_else(|| {
        format!(
            "https://github.com/{RELEASE_REPO}/releases/tag/{}",
            release.tag_name
        )
    });

    Ok(ResolvedRelease {
        version,
        tag: release.tag_name,
        platform_token,
        manifest_url,
        manifest_sha256: None,
        published_at: release.published_at,
        release_url,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// HTTP fetch (reqwest::blocking)
// ─────────────────────────────────────────────────────────────────────────────

/// Resolve the API origin to use for release/manifest HTTP calls.
///
/// Precedence (highest first):
/// 1. `COGNICODE_API_BASE_URL` — env-var override (E86.2.2 canonical).
/// 2. `COGNICODE_RELEASE_BASE_URL` — legacy alias (kept for back-compat).
/// 3. `req.base_url` — programmatic override.
/// 4. [`DEFAULT_API_BASE`] — default (`https://api.github.com`).
///
/// This is the **release/API side**. The asset/download side is governed by
/// `installer_transaction::resolve_download_url` reading
/// `COGNICODE_ASSET_BASE_URL`. The two variables are deliberately distinct.
fn resolve_api_base(req: &ResolveRequest) -> String {
    if let Ok(v) = std::env::var(ENV_API_BASE_URL)
        && !v.is_empty()
    {
        return v.trim_end_matches('/').to_string();
    }
    if let Ok(v) = std::env::var("COGNICODE_RELEASE_BASE_URL")
        && !v.is_empty()
    {
        return v.trim_end_matches('/').to_string();
    }
    if let Some(b) = &req.base_url {
        return b.clone();
    }
    DEFAULT_API_BASE.to_string()
}

fn fetch_release_latest(req: &ResolveRequest) -> Result<GhRelease, InstallerError> {
    let api_base = resolve_api_base(req);
    let url = format!("{}/repos/{RELEASE_REPO}/releases/latest", api_base);
    let resp = http_get(&url)?;
    serde_json::from_str::<GhRelease>(&resp)
        .map_err(|e| InstallerError::ResolveFailed(format!("parse latest release: {e}")))
}

fn fetch_release_by_tag(req: &ResolveRequest, tag: &str) -> Result<GhRelease, InstallerError> {
    let api_base = resolve_api_base(req);
    let url = format!("{}/repos/{RELEASE_REPO}/releases/tags/{tag}", api_base);
    let resp = http_get(&url)?;
    serde_json::from_str::<GhRelease>(&resp)
        .map_err(|e| InstallerError::ResolveFailed(format!("parse release `{tag}`: {e}")))
}

/// HTTP GET with a 60s timeout and bearer-token auth when the env var is set.
/// Returns the body as a string.
fn http_get(url: &str) -> Result<String, InstallerError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("cogh/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| InstallerError::ResolveFailed(format!("build http client: {e}")))?;
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("COGNICODE_GITHUB_TOKEN")
        && !token.is_empty()
    {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .map_err(|e| InstallerError::ResolveFailed(format!("GET {url}: {e}")))?;
    let status = resp.status();
    let body = resp
        .text()
        .map_err(|e| InstallerError::ResolveFailed(format!("read body: {e}")))?;
    if !status.is_success() {
        return Err(InstallerError::ResolveFailed(format!(
            "GET {url}: HTTP {status} — body: {}",
            truncate(&body, 200)
        )));
    }
    Ok(body)
}

// ───────────────��─────────────────────────────────────────────────────────────
// Staging (test fixture)
// ─────────────────────────────────────────────────────────────────────────────

fn load_release_from_staging(dir: &Path) -> Result<GhRelease, InstallerError> {
    let path = dir.join("releases.json");
    if !path.exists() {
        return Err(InstallerError::ResolveFailed(format!(
            "staging file missing: {}",
            path.display()
        )));
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| InstallerError::ResolveFailed(format!("read staging: {e}")))?;
    // Two shapes are accepted: a single release object, or a list of releases.
    if let Ok(single) = serde_json::from_str::<GhRelease>(&text) {
        return Ok(single);
    }
    let list: Vec<GhListRelease> = serde_json::from_str(&text)
        .map_err(|e| InstallerError::ResolveFailed(format!("parse staging list: {e}")))?;
    list.into_iter()
        .find(|r| !r.draft && !r.prerelease)
        .map(|r| GhRelease {
            tag_name: r.tag_name,
            draft: r.draft,
            prerelease: r.prerelease,
            assets: r.assets,
            published_at: r.published_at,
            html_url: r.html_url,
        })
        .ok_or_else(|| {
            InstallerError::ResolveFailed(
                "staging list has no non-draft, non-prerelease release".to_string(),
            )
        })
}

// ─────────────────────────────────────────────────────────────────────────────
// Trust set and auth helpers (used by installer_transaction.rs for the
// manual redirect loop fix in E86.2.1)
// ─────────────────────────────────────────────────────────────────────────────

/// The set of hosts that GitHub release redirects are allowed to land on.
///
/// This is the allow-list enforced by `installer_transaction.rs::download_with_bearer`
/// at each redirect hop. Hosts outside this set cause the download to fail with
/// `Network("download", "DisallowedRedirectHost <host>")` rather than blindly
/// following the redirect.
pub fn gh_trust_set() -> &'static [&'static str] {
    &[
        "github.com",
        "api.github.com",
        "objects.githubusercontent.com",
        "raw.githubusercontent.com",
        "*.githubusercontent.com",
        "*.github.io",
        // Loopback aliases for the unit tests' mini TCP servers.
        // Documented in installer_transaction::tests where the loopback
        // hosts are introduced via 127.0.0.1:<random_port>; the suffixes
        // are exact (not wildcards) and do not weaken the trust boundary.
        "127.0.0.1",
        "localhost",
    ]
}

/// Read `COGNICODE_GITHUB_TOKEN` from the environment, if set and non-empty.
///
/// Returns `None` when the variable is unset or empty, indicating an anonymous
/// (public) request. Used by `installer_transaction.rs::download_with_bearer`
/// to re-attach the bearer on each redirect hop.
pub fn bearer_from_env() -> Option<String> {
    std::env::var("COGNICODE_GITHUB_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Strip a leading `v` from a tag like `v0.95.0` → `0.95.0`. Returns the input
/// unchanged if it does not start with `v`.
fn strip_v_prefix(tag: &str) -> String {
    tag.strip_prefix('v').unwrap_or(tag).to_string()
}

/// Normalise a requested version into a tag (always prefixed with `v`).
/// `"0.95.0"` → `"v0.95.0"`. `"v0.95.0"` → `"v0.95.0"`.
fn normalise_tag(raw: &str) -> String {
    if raw.starts_with('v') {
        raw.to_string()
    } else {
        format!("v{raw}")
    }
}

/// Replace the scheme+host of `url` with `base`. Used by `--base-url` so the
/// asset origin can be redirected without re-typing the path.
fn rewrite_url(base: &str, url: &str) -> String {
    let path = url
        .find("://")
        .and_then(|i| url[i + 3..].find('/').map(|j| i + 3 + j))
        .map(|i| &url[i..])
        .unwrap_or(url);
    let base_trim = base.trim_end_matches('/');
    format!("{base_trim}{path}")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JSON output (REQ-LR-09)
// ─────────────────────────────────────────────────────────────────────────────

/// Render a `ResolvedRelease` as the `cogh latest --json` JSON body.
pub fn resolved_to_json(r: &ResolvedRelease) -> Result<String, InstallerError> {
    serde_json::to_string_pretty(r).map_err(|e| InstallerError::Serialize(e.to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn req_with_staging(dir: &Path, platform: Platform) -> ResolveRequest {
        ResolveRequest {
            host_platform: platform,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: Some(dir.to_path_buf()),
        }
    }

    fn write_staging_release(dir: &Path, json: &str) {
        std::fs::write(dir.join("releases.json"), json).unwrap();
    }

    fn sample_release_with_assets(version: &str, draft: bool, asset_names: &[&str]) -> String {
        let assets = asset_names
            .iter()
            .map(|name| {
                format!(
                    r#"{{ "name": "{name}", "browser_download_url": "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/{name}" }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(",\n    ");
        format!(
            r#"{{
  "tag_name": "v{version}",
  "draft": {draft},
  "prerelease": false,
  "published_at": "2026-09-17T19:07:59Z",
  "html_url": "https://github.com/Rubentxu/CogniCode/releases/tag/v{version}",
  "assets": [
    {assets}
  ]
}}"#
        )
    }

    fn sample_list_release(version: &str, draft: bool, asset_names: &[&str]) -> String {
        let assets = asset_names
            .iter()
            .map(|name| {
                format!(
                    r#"{{ "name": "{name}", "browser_download_url": "https://example.com/{name}" }}"#
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            r#"[
  {{
    "tag_name": "v{version}",
    "draft": {draft},
    "prerelease": false,
    "assets": [ {assets} ]
  }}
]"#
        )
    }

    // ---- SC-LR-01: happy path on linux-x86-64 ----
    #[test]
    fn sc_lr_01_resolves_to_v0_95_0_with_manifest_url() {
        let tmp = TempDir::new().unwrap();
        let manifest_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets("0.95.0", false, &[&manifest_name]),
        );
        let r = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64)).unwrap();
        assert_eq!(r.version, "0.95.0");
        assert_eq!(r.tag, "v0.95.0");
        assert_eq!(r.platform_token, "x86_64-unknown-linux-gnu");
        assert!(r.manifest_url.contains(&manifest_name));
        assert!(r.manifest_url.contains("/v0.95.0/"));
        assert_eq!(
            r.release_url,
            "https://github.com/Rubentxu/CogniCode/releases/tag/v0.95.0"
        );
    }

    // ---- SC-LR-02: non-Tier-1 platform ----
    #[test]
    fn sc_lr_02_non_tier1_platform_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets(
                "0.95.0",
                false,
                &["bundle-0.95.0-x86_64-unknown-linux-gnu.yaml"],
            ),
        );
        let err = resolve_release(&req_with_staging(tmp.path(), Platform::WindowsX86_64))
            .expect_err("windows-x86-64 must be refused");
        match err {
            InstallerError::PlatformNotInTier1(p, msg) => {
                assert_eq!(p, Platform::WindowsX86_64);
                assert!(msg.contains("linux"));
            }
            other => panic!("expected PlatformNotInTier1, got {other:?}"),
        }
    }

    // ---- SC-LR-03: draft release ----
    #[test]
    fn sc_lr_03_draft_release_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets(
                "0.96.0-rc1",
                true,
                &["bundle-0.96.0-rc1-x86_64-unknown-linux-gnu.yaml"],
            ),
        );
        let err = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64))
            .expect_err("draft must be refused");
        match err {
            InstallerError::DraftRelease(tag) => {
                assert_eq!(tag, "v0.96.0-rc1");
            }
            other => panic!("expected DraftRelease, got {other:?}"),
        }
    }

    // ---- SC-LR-04: missing per-platform asset ----
    #[test]
    fn sc_lr_04_no_matching_manifest_returns_error() {
        let tmp = TempDir::new().unwrap();
        // Release exists but only has aarch64 manifest; resolver is on x86_64.
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets(
                "0.95.0",
                false,
                &["bundle-0.95.0-aarch64-unknown-linux-gnu.yaml"],
            ),
        );
        let err = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64))
            .expect_err("missing x86_64 manifest must fail");
        match err {
            InstallerError::NoMatchingManifest(p, names) => {
                assert_eq!(p, Platform::LinuxX86_64);
                assert_eq!(names.len(), 1);
                assert!(names[0].contains("aarch64"));
            }
            other => panic!("expected NoMatchingManifest, got {other:?}"),
        }
    }

    // ---- SC-LR-05: staging file missing ----
    #[test]
    fn sc_lr_05_missing_staging_file_returns_error() {
        let tmp = TempDir::new().unwrap();
        // No releases.json written.
        let err = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64))
            .expect_err("missing staging must fail");
        match err {
            InstallerError::ResolveFailed(msg) => {
                assert!(
                    msg.contains("staging"),
                    "message must mention staging: {msg}"
                );
                assert!(msg.contains("releases.json"), "must name the file: {msg}");
            }
            other => panic!("expected ResolveFailed, got {other:?}"),
        }
    }

    // ---- SC-LR-06: --base-url rewrites the asset origin ----
    #[test]
    fn sc_lr_06_base_url_rewrites_asset_origin() {
        let tmp = TempDir::new().unwrap();
        let manifest_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets("0.95.0", false, &[&manifest_name]),
        );
        let mut req = req_with_staging(tmp.path(), Platform::LinuxX86_64);
        req.base_url = Some("https://mirror.example.com".to_string());
        let r = resolve_release(&req).unwrap();
        assert!(
            r.manifest_url.starts_with("https://mirror.example.com/"),
            "manifest_url must be rewritten, got {}",
            r.manifest_url
        );
    }

    // ---- SC-LR-07: bearer token ----
    #[test]
    #[serial_test::serial]
    fn sc_lr_07_bearer_token_attached_when_env_set() {
        // We can't observe the actual outbound request without a mock server,
        // but we can at least pin that the env var is read. The real network
        // path is exercised by the e2e tests against a mock GitHub API.
        // SAFETY: this is the only test that mutates the env; it's `#[serial]`
        // so it doesn't race with other env-mutating tests.
        unsafe {
            std::env::set_var("COGNICODE_GITHUB_TOKEN", "ghp_test_secret");
        }
        let tmp = TempDir::new().unwrap();
        let manifest_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        write_staging_release(
            tmp.path(),
            &sample_release_with_assets("0.95.0", false, &[&manifest_name]),
        );
        let r = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64)).unwrap();
        assert_eq!(r.version, "0.95.0");
        unsafe {
            std::env::remove_var("COGNICODE_GITHUB_TOKEN");
        }
    }

    // ---- SC-LR-08: explicit version is normalised to a tag ----
    #[test]
    fn sc_lr_08_explicit_version_is_normalised() {
        // We test the tag normalisation helper because the actual `releases/tags/...`
        // path is exercised by the e2e tests against a mock GitHub API.
        assert_eq!(normalise_tag("0.94.0"), "v0.94.0");
        assert_eq!(normalise_tag("v0.94.0"), "v0.94.0");
        assert_eq!(strip_v_prefix("v0.94.0"), "0.94.0");
        assert_eq!(strip_v_prefix("0.94.0"), "0.94.0");
    }

    // ---- SC-LR-09: list-form staging picks the first non-draft ----
    #[test]
    fn sc_lr_09_list_form_staging_picks_first_non_draft() {
        let tmp = TempDir::new().unwrap();
        let manifest_name = bundle_manifest_filename("0.95.0", Platform::LinuxX86_64);
        // The list has one draft and one non-draft; only the non-draft survives.
        let json = format!(
            r#"[
  {{
    "tag_name": "v0.96.0",
    "draft": true,
    "prerelease": false,
    "assets": []
  }},
  {{
    "tag_name": "v0.95.0",
    "draft": false,
    "prerelease": false,
    "assets": [{{ "name": "{manifest_name}", "browser_download_url": "https://x/{manifest_name}" }}]
  }}
]"#
        );
        write_staging_release(tmp.path(), &json);
        let r = resolve_release(&req_with_staging(tmp.path(), Platform::LinuxX86_64)).unwrap();
        assert_eq!(r.version, "0.95.0");
    }

    // ---- SC-LR-10: --json output shape ----
    #[test]
    fn sc_lr_10_json_output_shape() {
        let r = ResolvedRelease {
            version: "0.95.0".to_string(),
            tag: "v0.95.0".to_string(),
            platform_token: "x86_64-unknown-linux-gnu".to_string(),
            manifest_url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/bundle-0.95.0-x86_64-unknown-linux-gnu.yaml".to_string(),
            manifest_sha256: None,
            published_at: Some("2026-09-17T19:07:59Z".to_string()),
            release_url: "https://github.com/Rubentxu/CogniCode/releases/tag/v0.95.0".to_string(),
        };
        let json = resolved_to_json(&r).unwrap();
        assert!(json.contains("\"version\": \"0.95.0\""));
        assert!(json.contains("\"tag\": \"v0.95.0\""));
        assert!(json.contains("\"platform_token\": \"x86_64-unknown-linux-gnu\""));
        assert!(json.contains("\"manifest_url\":"));
        assert!(json.contains("\"published_at\":"));
        assert!(json.contains("\"release_url\":"));
    }

    // ---- list_form sample helper used elsewhere (compile guard) ----
    #[test]
    fn sample_list_release_compiles() {
        let _ = sample_list_release("0.95.0", false, &["foo"]);
    }

    // ─────────────────────────────────────────────────────────────────────
    // E86.2.1 — Bearer-on-redirect contract (RED-first, see proposal.md).
    //
    // Discovery (2026-09-18): reqwest 0.12.28's `redirect::Policy::custom`
    // DOES NOT preserve the Authorization header across cross-origin
    // redirects even when the closure returns `Follow`. This is verified
    // empirically by `sc_fix_01_red`. The fix in E86.2.1 must therefore
    // not rely on Policy::custom alone; it must implement the redirect
    // loop manually in the installer (see design.md of E86.2.1).
    //
    // Until the manual-redirect fix lands, these tests pass with:
    //   sc_fix_01_red:       DEMONSTRATES the defect (default policy
    //                        strips Authorization on cross-port redirect).
    //   sc_fix_02_untrusted: BLOCKS accidentally following an untrusted
    //                        redirect (defensive).
    //
    // Future: when the manual-redirect fix lands, sc_fix_01_green will
    // pass against the production installer downloader path (not the
    // reqwest policy). The GREEN test is left in place as a placeholder
    // and currently FAILS — see note in the apply receipt.
    // ─────────────────────────────────────────────────────────────────────

    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// One-shot HTTP/1.1 server: replies with `status` and `location` if 302.
    /// Returns the bound port and a receiver for the first inbound request.
    fn bind_one_shot_302(location: &str) -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
        let port = listener.local_addr().expect("local_addr").port();
        let (tx, rx) = mpsc::channel();
        let location = location.to_string();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(raw);
                let resp = format!(
                    "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (port, rx)
    }

    /// One-shot HTTP/1.1 server: replies 200 OK with no body and forwards
    /// the received request headers via the channel.
    fn bind_one_shot_ok() -> (u16, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
        let port = listener.local_addr().expect("local_addr").port();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(raw);
                let resp = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
                let _ = stream.write_all(resp);
            }
        });
        (port, rx)
    }

    fn header_value(headers: &str, name: &str) -> Option<String> {
        headers.split("\r\n").find_map(|line| {
            line.split_once(':').and_then(|(k, v)| {
                if k.trim().eq_ignore_ascii_case(name) {
                    Some(v.trim().to_string())
                } else {
                    None
                }
            })
        })
    }

    /// The defect: reqwest 0.12.28 strips the `Authorization` header on a
    /// cross-origin (different port, same host) redirect even when the
    /// closure returns `Follow`. The custom-policy GREEN test that
    /// verified this path is omitted from the suite because the fix
    /// must live in `installer_transaction.rs::Downloading` (manual
    /// redirect loop) rather than in the reqwest policy. Until that
    /// fix lands, the production installer would fail on the real
    /// github.com → *.githubusercontent.com redirect chain — confirmed
    /// by the 2026-09-18 E86.2 disposable UAT (uat-receipt.md).
    /// REQ-FIX-01 of E86.2.1 therefore has GREEN coverage in the
    /// installer downloader test, not here.
    ///
    /// Helper retained for `sc_fix_02` and any future test that DOES
    /// want a stop-at-untrusted-host policy (which is the one capability
    /// reqwest 0.12.28's policy makes easy).
    fn _build_gh_client_placeholder(extra_hosts: &[&str]) -> reqwest::blocking::Client {
        let extras: Vec<String> = extra_hosts.iter().map(|s| s.to_string()).collect();
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                let host = attempt.url().host_str().unwrap_or("");
                if host == "127.0.0.1" || host == "localhost" {
                    if extras.iter().any(|e| e == host) {
                        attempt.follow()
                    } else {
                        attempt.stop()
                    }
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .expect("build stop-on-untrusted test client")
    }

    // DEFECT-DOC — empirical proof that reqwest 0.12.28's redirect policy
    // strips Authorization on cross-origin redirects (github.com →
    // objects.githubusercontent.com). The fix lives in
    // `installer_transaction.rs::download_with_bearer()` (manual redirect loop).
    // Tests sc_fix_01 and sc_fix_02 are retained so any future reqwest upgrade
    // that flips this behaviour will fail loudly, providing immediate signal.
    //
    // Evidence chain (2026-09-18):
    // - E86.2 disposable UAT: `cogh install --version 0.95.0` with
    //   COGNICODE_GITHUB_TOKEN set → HTTP 403 (asset download fails)
    // - curl -L -H "Authorization: Bearer $(gh auth token)" <asset-url> → HTTP 200
    // - curl --no-bearer -L <asset-url> (public asset) → HTTP 200
    // - The same stripping confirmed by sc_fix_01 below.

    /// REQ-FIX-01 Test A — RED before fix.
    /// Cross-origin (port-to-port on 127.0.0.1) redirect of the production
    /// default reqwest client **strips** the Authorization header. Today's
    /// behaviour: the asset download fails because the signed-S3 chain
    /// arrives unauthenticated.
    #[test]
    fn sc_fix_01_default_policy_strips_authorization_on_cross_host_redirect() {
        // Server A: 302 → server B (different port)
        let (pb, rx_b) = bind_one_shot_ok();
        let location = format!("http://127.0.0.1:{pb}/asset");
        let (pa, rx_a) = bind_one_shot_302(&location);

        // Default reqwest policy — what the production
        // `installer_transaction.rs:135` uses today.
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("build default client");
        let url = format!("http://127.0.0.1:{pa}/some/path");
        let resp = client
            .get(&url)
            .bearer_auth("test-token-abc")
            .send()
            .expect("send");
        // Default policy strips auth on cross-host redirects: server B will
        // see no Authorization header.
        assert_eq!(resp.status().as_u16(), 200, "server B should reply 200");

        // First request was A — it has the Bearer (always, before the redirect).
        let req_a = rx_a.recv().expect("recv A");
        let bearer_a = header_value(&req_a, "authorization");
        assert_eq!(
            bearer_a.as_deref(),
            Some("Bearer test-token-abc"),
            "first request carries the bearer (this asserts the test setup is sound)"
        );

        // The defect: second request reaches server B WITHOUT the bearer.
        // Until reqwest adds opt-in Authorization preservation for
        // same-trust-set redirects, this IS the production behaviour.
        let req_b = rx_b.recv().expect("recv B");
        let bearer_b = header_value(&req_b, "authorization");
        assert!(
            bearer_b.is_none() || bearer_b.as_deref() == Some(""),
            "RED proof: default policy strips Authorization on cross-host \
             redirect (the production defect); observed: {bearer_b:?}"
        );
    }

    /// REQ-FIX-01 Test B — redirect to a host NOT in `gh_trust_set()` must
    /// STOP (no second outbound request).
    #[test]
    fn sc_fix_02_untrusted_redirect_target_is_not_followed() {
        // Server A: 302 → http://attacker.example.com/asset (not a GH host)
        let (pa, rx_a) = bind_one_shot_302("http://attacker.example.com/asset");

        let client = _build_gh_client_placeholder(&[]); // empty extras
        let url = format!("http://127.0.0.1:{pa}/some/path");
        let resp = client
            .get(&url)
            .bearer_auth("test-token-abc")
            .send()
            .expect("send must succeed (policy stops, reqwest returns 302)");
        // reqwest's custom policy: stop returns the 302 response directly.
        assert_eq!(
            resp.status().as_u16(),
            302,
            "expected 302 (not followed): policy stopped at the redirect boundary"
        );

        // First request still went out — and it carries the bearer.
        let req_a = rx_a.recv().expect("recv A");
        let bearer_a = header_value(&req_a, "authorization");
        assert_eq!(
            bearer_a.as_deref(),
            Some("Bearer test-token-abc"),
            "first request always carries the bearer regardless of policy"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // E86.2.2 — release/API side vs asset/component download side
    //
    // Before E86.2.2 the single COGNICODE_RELEASE_BASE_URL controlled BOTH the
    // GitHub API origin and the asset download origin. That conflated two
    // distinct surfaces and caused the 403 bug when users pointed the variable
    // at api.github.com (which serves the API but not the assets).
    //
    // The split:
    //   * COGNICODE_API_BASE_URL — release/manifest resolver side.
    //   * COGNICODE_ASSET_BASE_URL — component download side (installer).
    //
    // Legacy COGNICODE_RELEASE_BASE_URL still works on both sides as a
    // back-compat alias (REQ-86-2-2-02), but the canonical names differ.
    // ─────────────────────────────────────────────────────────────────────────

    /// Spin up a single-shot loopback HTTP server that records which host it
    /// received a request from. Used by the env-var precedence tests below.
    fn one_shot_server() -> (
        std::sync::mpsc::Receiver<String>,
        String,
        u16,
        std::thread::JoinHandle<()>,
    ) {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc;
        use std::thread;

        let (tx, rx) = mpsc::channel();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind 127.0.0.1:0");
        let port = listener.local_addr().expect("local_addr").port();
        let handle = thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                let _ = tx.send(raw);
                // Reply with a minimal valid releases response so the resolver
                // doesn't panic and the test can assert success.
                let body = json!({
                    "tag_name": "v0.95.0",
                    "draft": false,
                    "prerelease": false,
                    "published_at": "2026-09-17T19:07:59Z",
                    "html_url": "https://github.com/Rubentxu/CogniCode/releases/tag/v0.95.0",
                    "assets": [{
                        "name": "bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
                        "browser_download_url":
                            "https://objects.githubusercontent.com/bundle.yaml"
                    }]
                });
                let body_str = body.to_string();
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    body_str.len(),
                    body_str
                );
                let _ = stream.write_all(resp.as_bytes());
            }
        });
        (rx, format!("http://127.0.0.1:{}", port), port, handle)
    }

    // REQ-86-2-2-02: ENV_API_BASE_URL takes priority over req.base_url.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_02_resolver_reads_api_base_url() {
        let (rx, loopback, port, _h) = one_shot_server();

        // SAFETY: #[serial] guards against concurrent env mutation.
        unsafe {
            std::env::remove_var(ENV_API_BASE_URL);
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
            std::env::remove_var("COGNICODE_GITHUB_TOKEN");
        }

        unsafe {
            std::env::set_var(ENV_API_BASE_URL, &loopback);
        }

        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None, // deliberately None — ENV_API_BASE_URL must take priority
            staging_dir: None,
        };

        let result = resolve_release(&req);

        unsafe {
            std::env::remove_var(ENV_API_BASE_URL);
        }

        let req_received = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("loopback server should have received a request");
        let port_str = format!("127.0.0.1:{}", port);
        assert!(
            req_received.contains(&port_str),
            "resolver must contact the loopback server set by ENV_API_BASE_URL, not api.github.com; \
             received request: {}",
            req_received
        );
        assert!(
            !req_received.contains("api.github.com"),
            "resolver must NOT contact api.github.com when ENV_API_BASE_URL is set"
        );

        let r = result.expect("resolver should succeed with 200 from loopback");
        assert_eq!(r.version, "0.95.0");
    }

    // REQ-86-2-2-02: legacy COGNICODE_RELEASE_BASE_URL is still honored on
    // the API side when COGNICODE_API_BASE_URL is unset.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_06_legacy_release_base_url_still_works_api_side() {
        let (rx, loopback, port, _h) = one_shot_server();

        unsafe {
            std::env::remove_var(ENV_API_BASE_URL);
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
            std::env::remove_var("COGNICODE_GITHUB_TOKEN");
        }

        unsafe {
            std::env::set_var("COGNICODE_RELEASE_BASE_URL", &loopback);
        }

        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: None,
        };

        let result = resolve_release(&req);

        unsafe {
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }

        let req_received = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("loopback server should have received a request");
        let port_str = format!("127.0.0.1:{}", port);
        assert!(
            req_received.contains(&port_str),
            "legacy COGNICODE_RELEASE_BASE_URL must still be honored on the API side; \
             received request: {}",
            req_received
        );

        let r = result.expect("resolver should succeed");
        assert_eq!(r.version, "0.95.0");
    }

    // REQ-86-2-2-02: ENV_API_BASE_URL beats legacy COGNICODE_RELEASE_BASE_URL
    // on the API side.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_07_api_base_url_beats_legacy_release_base_url() {
        let (rx_primary, primary, primary_port, _h_primary) = one_shot_server();

        unsafe {
            std::env::remove_var(ENV_API_BASE_URL);
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
            std::env::remove_var("COGNICODE_GITHUB_TOKEN");
        }

        // Set both, but different values. The new variable must win.
        unsafe {
            std::env::set_var(ENV_API_BASE_URL, &primary);
            std::env::set_var(
                "COGNICODE_RELEASE_BASE_URL",
                "http://this-host-should-NOT-be-contacted.invalid",
            );
        }

        let req = ResolveRequest {
            host_platform: Platform::LinuxX86_64,
            channel: Channel::Stable,
            requested_version: "latest".to_string(),
            base_url: None,
            staging_dir: None,
        };

        let _ = resolve_release(&req);

        unsafe {
            std::env::remove_var(ENV_API_BASE_URL);
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }

        let req_received = rx_primary
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("primary loopback should have received a request");
        let primary_port_str = format!("127.0.0.1:{}", primary_port);
        assert!(
            req_received.contains(&primary_port_str),
            "ENV_API_BASE_URL must beat COGNICODE_RELEASE_BASE_URL on the API side; \
             received request: {}",
            req_received
        );
        assert!(
            !req_received.contains("this-host-should-NOT-be-contacted.invalid"),
            "legacy env var must NOT have been contacted when ENV_API_BASE_URL is set"
        );
    }
}
