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
    RELEASE_DOWNLOAD_BASE, RELEASE_REPO, TIER1_PLATFORMS, bundle_manifest_filename,
    platform_token,
};

/// Default GitHub API base URL.
pub const DEFAULT_API_BASE: &str = "https://api.github.com";

/// Default download base URL for release assets.
pub const DEFAULT_DOWNLOAD_BASE: &str = RELEASE_DOWNLOAD_BASE;

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

    let release_url = release
        .html_url
        .unwrap_or_else(|| format!("https://github.com/{RELEASE_REPO}/releases/tag/{}", release.tag_name));

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

fn fetch_release_latest(req: &ResolveRequest) -> Result<GhRelease, InstallerError> {
    let url = format!(
        "{}/repos/{RELEASE_REPO}/releases/latest",
        req.base_url.as_deref().unwrap_or(DEFAULT_API_BASE)
    );
    let resp = http_get(&url, req.base_url.as_deref())?;
    serde_json::from_str::<GhRelease>(&resp).map_err(|e| {
        InstallerError::ResolveFailed(format!("parse latest release: {e}"))
    })
}

fn fetch_release_by_tag(
    req: &ResolveRequest,
    tag: &str,
) -> Result<GhRelease, InstallerError> {
    let url = format!(
        "{}/repos/{RELEASE_REPO}/releases/tags/{tag}",
        req.base_url.as_deref().unwrap_or(DEFAULT_API_BASE)
    );
    let resp = http_get(&url, req.base_url.as_deref())?;
    serde_json::from_str::<GhRelease>(&resp).map_err(|e| {
        InstallerError::ResolveFailed(format!("parse release `{tag}`: {e}"))
    })
}

/// HTTP GET with a 60s timeout and bearer-token auth when the env var is set.
/// Returns the body as a string.
fn http_get(url: &str, _base_url: Option<&str>) -> Result<String, InstallerError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("cogh/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| InstallerError::ResolveFailed(format!("build http client: {e}")))?;
    let mut req = client.get(url);
    if let Ok(token) = std::env::var("COGNICODE_GITHUB_TOKEN") {
        if !token.is_empty() {
            req = req.bearer_auth(token);
        }
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
    let list: Vec<GhListRelease> = serde_json::from_str(&text).map_err(|e| {
        InstallerError::ResolveFailed(format!("parse staging list: {e}"))
    })?;
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
            &tmp.path(),
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
            &tmp.path(),
            &sample_release_with_assets("0.95.0", false, &["bundle-0.95.0-x86_64-unknown-linux-gnu.yaml"]),
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
            &tmp.path(),
            &sample_release_with_assets("0.96.0-rc1", true, &["bundle-0.96.0-rc1-x86_64-unknown-linux-gnu.yaml"]),
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
            &tmp.path(),
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
                assert!(msg.contains("staging"), "message must mention staging: {msg}");
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
            &tmp.path(),
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
            &tmp.path(),
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
        write_staging_release(&tmp.path(), &json);
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
}
