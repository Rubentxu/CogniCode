//! `cogh::installer_transaction` — Atomic install transaction state machine.
//!
//! Runs the full install pipeline through discrete stages:
//! ResolvingUrl → Downloading → VerifyingSha256 → Extracting → InstallingShims
//! → WritingManifest → Committed (or Failed).
//!
//! Uses a [`RollbackJournal`] to record side-effects so that a failed
//! install can be reversed cleanly.

use std::path::PathBuf;

use super::lifecycle_resolver::{DEFAULT_API_BASE, bearer_from_env, gh_trust_set};
use crate::bundle_manifest::{BundleManifest, Platform};
use crate::error::{BundleManifestError, InstallerError};
use crate::layout;
use crate::platform_adapter;
use crate::registry;
use crate::rollback_journal::{RollbackJournal, SideEffect};
use sha2::Digest;
use std::io::Write;

/// Maximum number of redirect hops allowed during a download.
///
/// GitHub's release-asset redirect chain is at most 2 hops
/// (api.github.com → objects.githubusercontent.com). 5 provides
/// defensive slack while still protecting against redirect loops.
const MAX_DOWNLOAD_REDIRECTS: u8 = 5;

/// Environment variable naming an explicit `bundle.yaml` to install from.
///
/// This is the seam that lets a release tool, a test, or a user point `cogh` at
/// an exact generated manifest instead of relying on any embedded fallback.
pub const ENV_BUNDLE_MANIFEST: &str = "COGNICODE_BUNDLE_MANIFEST";

/// Environment variable for the asset download mirror.
///
/// When set and non-empty, `resolve_download_url` rewrites canonical
/// `RELEASE_DOWNLOAD_BASE/<rest>` URLs onto this origin instead.
/// This is the correct variable to set for asset mirrors.
///
/// The old `COGNICODE_RELEASE_BASE_URL` (now `COGNICODE_API_BASE_URL`)
/// was incorrectly used for asset rewriting, causing 403 on
/// api.github.com (which does not serve release assets).
pub const ENV_ASSET_BASE_URL: &str = "COGNICODE_ASSET_BASE_URL";

/// Rewrite a canonical component URL onto the configured fetch origin, if any.
///
/// ## Behaviour
///
/// - `COGNICODE_ASSET_BASE_URL` set + non-empty → rewrite
///   `https://github.com/Rubentxu/CogniCode/releases/download/<rest>` onto it.
/// - `COGNICODE_ASSET_BASE_URL` unset/empty AND
///   `COGNICODE_RELEASE_BASE_URL=https://api.github.com` → emit a
///   one-time deprecation warning on stderr; return canonical.
/// - Otherwise → return canonical unchanged.
///
/// URLs that do **not** begin with `RELEASE_DOWNLOAD_BASE` are returned
/// as-is regardless of env vars (REQ-86-2-2-01b — non-canonical URLs
/// are never rewritten).
pub fn resolve_download_url(canonical: &str) -> String {
    resolve_download_url_into(canonical, None)
}

/// Same as [`resolve_download_url`] but, when the deprecation shim fires,
/// writes the warning message to `warn_sink` instead of stderr. Tests use
/// this entry point to assert the warning is emitted; production code uses
/// [`resolve_download_url`] which routes to stderr.
///
/// ## Precedence (highest first)
///
/// 1. `COGNICODE_ASSET_BASE_URL` — canonical asset-side override.
/// 2. `COGNICODE_RELEASE_BASE_URL` — legacy back-compat alias (still
///    honored, with a deprecation warning emitted the first time it
///    triggers a rewrite).
/// 3. Canonical URL — unchanged.
///
/// ## Special case: api.github.com as legacy value
///
/// If the legacy var is set to exactly `DEFAULT_API_BASE` (`api.github.com`),
/// it is treated as **unset** (with a warning) and the canonical URL is
/// returned. This was the root cause of the E86.2 real-PC 403 bug: the
/// legacy var was being rewritten onto `api.github.com`, which serves the
/// API but not release assets.
pub fn resolve_download_url_into(canonical: &str, mut warn_sink: Option<&mut Vec<u8>>) -> String {
    let asset_base = std::env::var_os(ENV_ASSET_BASE_URL);

    // Case 1: ASSET_BASE_URL is set and non-empty → rewrite.
    if let Some(base) = asset_base {
        let base = base.to_string_lossy();
        let base = base.trim_end_matches('/');
        if !base.is_empty() {
            if let Some(rest) =
                canonical.strip_prefix(crate::release_contract::RELEASE_DOWNLOAD_BASE)
            {
                return format!("{base}{rest}");
            }
        }
    }

    // Case 2: ASSET_BASE_URL is unset/empty — fall back to the deprecated
    // old name COGNICODE_RELEASE_BASE_URL for back-compat with air-gapped
    // installs and mirrors predating E86.2.2.
    if let Ok(old) = std::env::var("COGNICODE_RELEASE_BASE_URL") {
        if !old.is_empty() {
            let trimmed = old.trim_end_matches('/');
            if trimmed == DEFAULT_API_BASE {
                // api.github.com does NOT serve release assets. The legacy
                // var being set to that value is the exact pattern that
                // caused the E86.2 403 bug — emit a warning and treat as
                // unset (return canonical).
                let msg = "warning: COGNICODE_RELEASE_BASE_URL is set to \
                           https://api.github.com, which does not serve release assets; \
                           use COGNICODE_ASSET_BASE_URL for asset mirrors \
                           or COGNICODE_API_BASE_URL for the API base.\n";
                if let Some(sink) = warn_sink.as_deref_mut() {
                    let _ = sink.write(msg.as_bytes());
                } else {
                    eprint!("{msg}");
                }
                return canonical.to_string();
            }
            // Any other non-empty value: rewrite (back-compat) but emit
            // a one-shot deprecation warning so users can migrate to
            // COGNICODE_ASSET_BASE_URL.
            if let Some(rest) =
                canonical.strip_prefix(crate::release_contract::RELEASE_DOWNLOAD_BASE)
            {
                let msg = "warning: COGNICODE_RELEASE_BASE_URL is deprecated; \
                           use COGNICODE_ASSET_BASE_URL for asset mirrors.\n";
                if let Some(sink) = warn_sink.as_deref_mut() {
                    let _ = sink.write(msg.as_bytes());
                } else {
                    eprint!("{msg}");
                }
                return format!("{trimmed}{rest}");
            }
        }
    }

    canonical.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Download helpers (E86.2.1 — bearer token preservation through redirects)
// ─────────────────────────────────────────────────────────────────────────────

/// Check whether `host` is within the allowed GitHub trust set.
///
/// Supports exact matches and wildcard suffixes (e.g. `*.githubusercontent.com`).
fn is_in_trust_set(host: &str) -> bool {
    let trust = gh_trust_set();
    trust.iter().any(|&trusted| {
        if trusted.starts_with("*.") {
            host.ends_with(&trusted[1..])
        } else {
            host == trusted
        }
    })
}

/// Download a URL with manual redirect following that re-attaches the Bearer
/// token on every hop.
///
/// reqwest 0.12.28's redirect policy **strips** the `Authorization` header
/// on cross-origin redirects (github.com → objects.githubusercontent.com),
/// regardless of `Policy::custom`. This helper works around that limitation
/// by handling redirects manually and re-attaching the bearer on each hop.
///
/// Returns the final response. Callers must check `response.status()` and
/// consume the body.
fn download_with_bearer(
    client: &reqwest::blocking::Client,
    url: &str,
    bearer: Option<&str>,
) -> Result<reqwest::blocking::Response, InstallerError> {
    let mut hop: u8 = 0;
    let mut current_url = url.to_string();
    let token = bearer;

    loop {
        hop += 1;
        if hop > MAX_DOWNLOAD_REDIRECTS {
            return Err(InstallerError::Network(
                url.to_string(),
                format!("TooManyRedirects (>{})", MAX_DOWNLOAD_REDIRECTS),
            ));
        }

        // Build and send the request
        let mut req_builder = client.get(&current_url);
        if let Some(t) = token {
            req_builder = req_builder.bearer_auth(t);
        }

        let response = req_builder
            .send()
            .map_err(|e| InstallerError::Network(current_url.clone(), e.to_string()))?;

        let status = response.status();
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        println!(
            "[cogh download] hop={} status={} location={:?}",
            hop,
            status.as_u16(),
            location
        );

        // Check if we need to follow a redirect
        match status {
            reqwest::StatusCode::MOVED_PERMANENTLY
            | reqwest::StatusCode::FOUND
            | reqwest::StatusCode::SEE_OTHER
            | reqwest::StatusCode::TEMPORARY_REDIRECT
            | reqwest::StatusCode::PERMANENT_REDIRECT => {
                let next_url = location.ok_or_else(|| {
                    InstallerError::Network(
                        current_url.clone(),
                        "redirect status with no Location header".to_string(),
                    )
                })?;

                // Validate the next host is in the trust set. Split off scheme, then
                // strip any `:port` suffix and any path so we compare only the
                // bare hostname against `gh_trust_set()`.
                let next_host = next_url
                    .strip_prefix("http://")
                    .or_else(|| next_url.strip_prefix("https://"))
                    .unwrap_or(next_url.as_str())
                    .split('/')
                    .next()
                    .unwrap_or("")
                    .split(':')
                    .next()
                    .unwrap_or("");

                if !is_in_trust_set(next_host) {
                    return Err(InstallerError::Network(
                        current_url.clone(),
                        format!("DisallowedRedirectHost {}", next_host),
                    ));
                }

                current_url = next_url;
                // Continue the loop — bearer will be re-attached on next iteration
            }
            // Non-redirect status — return to caller
            _ => return Ok(response),
        }
    }
}

/// Verifies a file against an expected sha256 hash.
pub fn verify_sha256(path: &std::path::Path, expected: &str) -> Result<(), InstallerError> {
    let actual = compute_sha256(path)?;
    if actual != expected {
        return Err(InstallerError::Sha256Mismatch);
    }
    Ok(())
}

fn compute_sha256(path: &std::path::Path) -> Result<String, InstallerError> {
    use std::io::Read;
    let mut file =
        std::fs::File::open(path).map_err(|e| InstallerError::Io(path.to_path_buf(), e))?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| InstallerError::Io(path.to_path_buf(), e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Install pipeline stages in execution order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStage {
    ResolvingUrl,
    Downloading,
    VerifyingSha256,
    Extracting,
    InstallingShims,
    WritingManifest,
    Committed,
    Failed,
}

/// Atomic install transaction.
#[derive(Debug)]
pub enum InstallerTransaction {
    /// Transaction still in progress at a given stage.
    Running {
        stage: InstallStage,
        journal: RollbackJournal,
        manifest: BundleManifest,
    },
    /// Transaction committed successfully; holds the manifest path.
    Committed { manifest_path: PathBuf },
    /// Transaction failed at a given stage with an error.
    Failed {
        stage: InstallStage,
        error: InstallerError,
    },
}

/// Execute the actions for a given stage.
fn advance_stage(
    stage: InstallStage,
    journal: &mut RollbackJournal,
    manifest: &BundleManifest,
    home: &crate::layout::CognicodeHome,
    profile: &str,
) -> Result<(), InstallerError> {
    match stage {
        InstallStage::ResolvingUrl => {
            // Validate all component URLs are reachable (basic check)
            for comp in &manifest.components {
                if comp.url.is_empty() {
                    return Err(InstallerError::Network(
                        "empty URL".into(),
                        "no URL provided".into(),
                    ));
                }
            }
            Ok(())
        }
        InstallStage::Downloading => {
            let cache_dir = layout::cache_dir();
            std::fs::create_dir_all(&cache_dir)
                .map_err(|e| InstallerError::Io(cache_dir.clone(), e))?;
            journal.record(SideEffect::CreatedDir(cache_dir.clone()));
            // Download each component
            for comp in &manifest.components {
                let dest = cache_dir.join(format!("{}.tar.gz", comp.name));
                // The client intentionally disables auto-redirect handling
                // (`Policy::none()`) because reqwest 0.12.28 strips
                // `Authorization` on cross-origin redirects, even under
                // `Policy::custom(|a| a.follow())`. The manual redirect loop
                // in `download_with_bearer` re-attaches the bearer on every
                // hop and enforces the GH trust set, so it owns the redirect
                // handling end-to-end. See E86.2.1 design and the
                // DEFECT-DOC tests in `lifecycle_resolver::tests`.
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|e| InstallerError::Network("reqwest".into(), e.to_string()))?;
                let bearer = bearer_from_env();
                let response = download_with_bearer(
                    &client,
                    &resolve_download_url(&comp.url),
                    bearer.as_deref(),
                )
                .map_err(|e| InstallerError::Network(comp.url.clone(), e.to_string()))?;
                if !response.status().is_success() {
                    return Err(InstallerError::Network(
                        comp.url.clone(),
                        format!("HTTP {}", response.status()),
                    ));
                }
                let mut file = std::fs::File::create(&dest)
                    .map_err(|e| InstallerError::Io(dest.clone(), e))?;
                std::io::copy(&mut response.bytes().unwrap().as_ref(), &mut file)
                    .map_err(|e| InstallerError::Io(dest.clone(), e))?;
                journal.record(SideEffect::Downloaded(dest));
            }
            Ok(())
        }
        InstallStage::VerifyingSha256 => {
            // Verify SHA256 for each downloaded file
            let cache_dir = layout::cache_dir();
            for comp in &manifest.components {
                let path = cache_dir.join(format!("{}.tar.gz", comp.name));
                verify_sha256(&path, comp.sha256.as_str())?;
                journal.record(SideEffect::VerifiedSha256(path));
            }
            Ok(())
        }
        InstallStage::Extracting => {
            // L2 (ADR-CANONICAL-LAYOUT): extract each component under
            // `<root>/versions/<v>/<comp>/`, NOT `<root>/install/<v>/<comp>/`.
            // The canonical install tree is owned by `home.version_root(v)`.
            let install_dir = home.version_root(&manifest.version);
            std::fs::create_dir_all(&install_dir)
                .map_err(|e| InstallerError::Io(install_dir.clone(), e))?;
            journal.record(SideEffect::CreatedDir(install_dir.clone()));
            let cache_dir = layout::cache_dir();
            for comp in &manifest.components {
                let src = cache_dir.join(format!("{}.tar.gz", comp.name));
                let dest = install_dir.join(&comp.name);
                registry::extract_targz(&src, &dest)
                    .map_err(|e| InstallerError::Unknown(e.to_string()))?;
                journal.record(SideEffect::Extracted(dest));
            }

            // DEBT-2b: extract each manifest-DECLARED skill bundle for the
            // active profile into the canonical skill namespace
            // `versions/<v>/skills/<SkillBundleId>/`. The SkillBundleId
            // comes from the manifest's `skill_bundles[]` section, which
            // survives the component profile filter — the namespaces are
            // orthogonal (see ADR-IDENTITY-MAP §3.5/§7). A declared bundle
            // missing from the cache fails loudly; nothing is derived from
            // a ComponentId or a directory scan.
            let skills_root = home.skills_root(&manifest.version);
            for bundle in manifest.skill_bundles_for_profile(profile) {
                let src = cache_dir.join(format!("{}.tar.gz", bundle.id));
                if !src.exists() {
                    return Err(InstallerError::Unknown(format!(
                        "bundle manifest declares skill bundle `{}` for profile \
                         `{profile}`, but {} is missing from the download cache; \
                         refusing to guess a substitute",
                        bundle.id,
                        src.display()
                    )));
                }
                let dest = skills_root.join(&bundle.id);
                std::fs::create_dir_all(&dest).map_err(|e| InstallerError::Io(dest.clone(), e))?;
                registry::extract_targz(&src, &dest)
                    .map_err(|e| InstallerError::Unknown(e.to_string()))?;
                journal.record(SideEffect::Extracted(dest));
            }
            Ok(())
        }
        InstallStage::InstallingShims => {
            // Ensure the shims directory exists even when the selected profile
            // has no matching components. The layout must materialize so
            // subsequent `cogh doctor` / `cogh where` calls find a well-formed
            // `~/.cognicode/shims/` path.
            let shims_dir = layout::shims_dir();
            std::fs::create_dir_all(&shims_dir)
                .map_err(|e| InstallerError::Io(shims_dir.clone(), e))?;
            journal.record(SideEffect::CreatedDir(shims_dir.clone()));

            // L2 (ADR-CANONICAL-LAYOUT): shim source is under the canonical
            // `<root>/versions/<v>/<comp>/bin/<comp>` (or wherever the
            // component bundle places the binary). If the archive has no
            // `bin/` (e.g. portable skill bundles), the `if bin_path.exists()`
            // guard below makes the shim stage a no-op for that component.
            let install_dir = home.version_root(&manifest.version);
            let adapter = platform_adapter::current_adapter();
            for comp in &manifest.components {
                // DEBT-3.f: bin-path resolution is delegated to
                // `locate_component_binary`, which tries the legacy
                // Cargo-style `bin/<comp>/<comp>` first and falls
                // back to scanning `<comp>/bin/` for any file. The
                // helper removes the inline `comp.name` coupling
                // that the legacy `bin/<comp>/<comp>` shape assumed.
                let bin_path = locate_component_binary(home, &manifest.version, &comp.name);
                if let Some(bin_path) = bin_path {
                    let shim_path = layout::shims_dir().join(&comp.name);
                    let effect = adapter
                        .install_shim(&bin_path, &shim_path)
                        .map_err(|e| InstallerError::ShimInstall(e.to_string()))?;
                    let (link, target) = match effect {
                        platform_adapter::ShimSideEffect::Symlinked { link, target } => {
                            (link, target)
                        }
                        platform_adapter::ShimSideEffect::Copied { dest, source } => (dest, source),
                    };
                    journal.record(SideEffect::CreatedSymlink { link, target });
                }
            }
            Ok(())
        }
        InstallStage::WritingManifest => {
            // Manifest writing is handled by commit()
            Ok(())
        }
        InstallStage::Committed | InstallStage::Failed => Ok(()),
    }
}

/// Locate the on-disk binary for a component under
/// `<root>/versions/<v>/<comp>/`. Returns the path to the first
/// candidate that exists.
///
/// DEBT-3.f: this helper centralises the bin-path resolution that
/// the install pipeline used to inline as
/// `install_dir.join(comp).join("bin").join(comp)`. The inline form
/// silently couples BinaryName to ComponentId by way of the
/// `comp.name == kind.stem()` invariant (BundleManifest enforces
/// `name == kind.stem()`). If a future component shipped multiple
/// binaries OR a binary whose filename diverged from the component
/// name, the inline form would silently miss it.
///
/// The strict T1 in this module's `tests` block pins the relaxed
/// contract that the relaxation now honours:
/// `locate_component_binary` must find any executable in
/// `<root>/versions/<v>/<comp>/bin/`, not just `<comp>` literally.
///
/// Order of attempts (legacy first, then relaxed):
/// 1. `<root>/versions/<v>/<comp>/bin/<comp>` — Cargo-style legacy.
/// 2. `<root>/versions/<v>/<comp>/<comp>` — root-level legacy alt.
/// 3. Scan `<root>/versions/<v>/<comp>/bin/` for any file (relaxed).
///
/// Returns `None` if no candidate exists. Callers must decide
/// whether to fail loudly or skip.
pub(crate) fn locate_component_binary(
    home: &crate::layout::CognicodeHome,
    version: &str,
    component_id: &str,
) -> Option<PathBuf> {
    let comp_root = home.component_root(version, component_id);
    let bin_dir = comp_root.join("bin");

    // 1. Legacy Cargo-style: bin/<comp>/<comp>.
    let legacy = bin_dir.join(component_id);
    if legacy.is_file() {
        return Some(legacy);
    }

    // 2. Legacy root-level alt: <comp>/<comp>.
    let root_level = comp_root.join(component_id);
    if root_level.is_file() {
        return Some(root_level);
    }

    // 3. Relaxed: scan bin/ for any file.
    //
    // DEBT-3.f strict T1: this leg makes the runtime resilient to
    // BinaryName ≠ ComponentId. A bundle that ships a binary
    // whose filename diverges from the component name (the case
    // the legacy `bin/<comp>/<comp>` heuristic silently broke)
    // is now found. The first regular file in `bin/` is returned;
    // deterministic ordering is not guaranteed by `read_dir` but
    // is sufficient as a last-resort fallback.
    //
    // If the directory does not exist or is empty, the helper
    // returns `None` and the caller decides whether to fail
    // loudly or skip the shim stage.
    let entries = std::fs::read_dir(&bin_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

impl InstallerTransaction {
    /// Run the full install transaction for a given profile.
    ///
    /// Loads the bundle manifest (embedded or from disk), validates the
    /// version, then advances through each pipeline stage.
    /// Returns the path to the written install manifest on success.
    pub fn run(
        home: &crate::layout::CognicodeHome,
        profile: &str,
    ) -> Result<PathBuf, InstallerError> {
        let yaml = Self::load_bundle_manifest()?;

        // Parse and validate the v2 contract.
        //
        // e85: there is deliberately NO lockstep between the bundle version and
        // this binary's version. Under e84's Layer 0 / Layer 1 split, the
        // installed runtime version is free to differ from the running `cogh`
        // bootstrap version, so `assert_pkg_version` was removed in favour of
        // letting the manifest validation decide what is installable.
        let mut manifest = BundleManifest::from_str(&yaml)
            .map_err(|e| InstallerError::ManifestParse(BundleManifestError(e)))?;
        // e74 WU2: refuse to load a wrong-platform bundle. No fallback
        // to a different platform's artifacts. The distribution matrix
        // is the contract; running the Linux bundle on Windows is the
        // bug we are explicitly preventing here.
        manifest
            .assert_host_platform(platform_adapter::detect_host_platform())
            .map_err(|e| InstallerError::ManifestParse(BundleManifestError(e)))?;

        // Filter components by profile
        let filtered_components: Vec<_> = manifest.components_for_profile(profile);
        manifest.components = filtered_components.into_iter().cloned().collect();

        // e86.1 REQ-LJ-04: refuse to install a zero-component profile
        // silently. Without this, a typo'd `--profile core-typo` exits
        // `Ok(())` while having installed nothing — the most insidious
        // masking failure mode in the install pipeline.
        if manifest.components.is_empty() {
            return Err(InstallerError::EmptyInstall(
                profile.to_string(),
                manifest.version.clone(),
            ));
        }

        // Create journal and run through stages
        let journal = RollbackJournal::new();
        let mut tx = Self::Running {
            stage: InstallStage::ResolvingUrl,
            journal,
            manifest,
        };

        // Stage: ResolvingUrl → Downloading
        tx = tx.advance(home, profile)?;

        // Stage: Downloading → VerifyingSha256
        tx = tx.advance(home, profile)?;

        // Stage: VerifyingSha256 → Extracting
        tx = tx.advance(home, profile)?;

        // Stage: Extracting → InstallingShims
        tx = tx.advance(home, profile)?;

        // Stage: InstallingShims → WritingManifest
        tx = tx.advance(home, profile)?;

        // Stage: WritingManifest → Committed
        tx = tx.commit(home)?;

        match tx {
            Self::Committed { manifest_path } => Ok(manifest_path),
            Self::Failed { error, .. } => Err(error),
            Self::Running { .. } => {
                // Should not happen: commit() always transitions out of Running
                Err(InstallerError::Unknown(
                    "commit() did not transition out of Running state".into(),
                ))
            }
        }
    }

    /// Resolve the bundle manifest `cogh` should install from.
    ///
    /// Precedence, highest first:
    ///
    /// 1. `COGNICODE_BUNDLE_MANIFEST` — an explicit path to a manifest.
    /// 2. `~/.cognicode/bundle.yaml` — a manifest placed in `COGNICODE_HOME`.
    /// 3. A **dev-only** embedded fixture, announced with a loud warning.
    ///
    /// ## Why the embedded fixture is not authoritative (e85 WU8)
    ///
    /// e84 requires manifests to be **generated from produced artifacts**, with
    /// digests computed from the packaged bytes. Such a manifest cannot honestly
    /// be committed before those artifacts exist, so a version-pinned embedded
    /// manifest can never be the production authority.
    ///
    /// Remote resolution of `version + platform -> published BundleManifest v2`
    /// is **e86**. Until then `cogh` can install only from an explicitly provided
    /// manifest. The fallback below exists solely so offline development and this
    /// crate's own tests have a well-formed v2 manifest to parse; its digests are
    /// not the digests of any real artifact, so an install driven by it fails at
    /// the SHA256 stage by construction rather than silently succeeding.
    fn load_bundle_manifest() -> Result<String, InstallerError> {
        if let Some(explicit) = std::env::var_os(ENV_BUNDLE_MANIFEST) {
            let path = PathBuf::from(explicit);
            return std::fs::read_to_string(&path).map_err(|e| InstallerError::Io(path, e));
        }

        let home_manifest = layout::bundle_yaml_path();
        if home_manifest.exists() {
            return std::fs::read_to_string(&home_manifest)
                .map_err(|e| InstallerError::Io(home_manifest, e));
        }

        eprintln!(
            "warning: no bundle manifest provided; falling back to the DEV-ONLY fixture.\n\
             Real installs must use a generated release manifest: set {ENV_BUNDLE_MANIFEST} \
             or place one at {}.\n\
             Remote resolution of version+platform is not implemented until e86.",
            home_manifest.display()
        );
        Ok(include_str!("dev-bundle.yaml").to_string())
    }

    /// Advance the transaction to the next stage.
    ///
    /// `profile` is threaded through so the Extracting stage knows which
    /// profiles' declared skill bundles to materialise (DEBT-2b).
    fn advance(
        self,
        home: &crate::layout::CognicodeHome,
        profile: &str,
    ) -> Result<Self, InstallerError> {
        match self {
            Self::Running {
                stage,
                mut journal,
                manifest,
            } => {
                // Execute stage actions before transitioning
                if let Err(e) = advance_stage(stage, &mut journal, &manifest, home, profile) {
                    return Ok(Self::Failed { stage, error: e });
                }

                let next_stage = match stage {
                    InstallStage::ResolvingUrl => InstallStage::Downloading,
                    InstallStage::Downloading => InstallStage::VerifyingSha256,
                    InstallStage::VerifyingSha256 => InstallStage::Extracting,
                    InstallStage::Extracting => InstallStage::InstallingShims,
                    InstallStage::InstallingShims => InstallStage::WritingManifest,
                    InstallStage::WritingManifest => InstallStage::Committed,
                    InstallStage::Committed => {
                        // Already at terminal — return as-is
                        return Ok(Self::Running {
                            stage,
                            journal,
                            manifest,
                        });
                    }
                    InstallStage::Failed => {
                        return Ok(Self::Failed {
                            stage,
                            error: InstallerError::Unknown("Already failed".into()),
                        });
                    }
                };
                Ok(Self::Running {
                    stage: next_stage,
                    journal,
                    manifest,
                })
            }
            Self::Committed { .. } | Self::Failed { .. } => Ok(self),
        }
    }

    /// Commit the transaction: write the install manifest and finalize the journal.
    fn commit(self, home: &crate::layout::CognicodeHome) -> Result<Self, InstallerError> {
        match self {
            Self::Running {
                mut journal,
                manifest,
                stage: _,
            } => {
                // L2 (ADR-CANONICAL-LAYOUT): the manifest now lands at
                // `<root>/versions/<v>/manifest.yaml`, not `install/<v>/`.
                let manifest_path = home.version_manifest(&manifest.version);

                // Serialize manifest to YAML
                let yaml = serde_yaml::to_string(&manifest)
                    .map_err(|e| InstallerError::Serialize(e.to_string()))?;

                // Capture the tracker value BEFORE the install overwrites it,
                // so a future rollback can restore it (e86 REQ-LJ-02).
                let previous_tracker = crate::tracker::read_version_optional();
                let tracker_path = crate::layout::tracker_dir().join("version");

                // Ensure parent directory exists (record for rollback)
                if let Some(parent) = manifest_path.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| InstallerError::Io(parent.into(), e))?;
                    journal.record(SideEffect::CreatedDir(parent.into()));
                }

                // Write manifest file
                std::fs::write(&manifest_path, yaml)
                    .map_err(|e| InstallerError::Io(manifest_path.clone(), e))?;
                journal.record(SideEffect::WroteManifest(manifest_path.clone()));

                // Persist the journal BEFORE we mark it committed, so the
                // on-disk record reflects the side-effects even if the
                // tracker write below fails. Errors are non-fatal for the
                // install (the install is correct); we surface them as a
                // warning on stderr and the next `cogh rollback` will report
                // "nothing to roll back" (REQ-LJ-01).
                let journal_path = crate::lifecycle_journal::journal_path(&manifest.version);
                if let Err(e) = crate::lifecycle_journal::write(
                    &journal,
                    &manifest,
                    previous_tracker.as_deref(),
                    &journal_path,
                ) {
                    eprintln!(
                        "warning: failed to persist install journal at {}: {}",
                        journal_path.display(),
                        e
                    );
                }

                // Record the tracker write for the in-memory journal so that
                // an in-process rollback (e.g. a test that fails after commit)
                // can still restore it.
                journal.record(SideEffect::WroteTracker {
                    path: tracker_path,
                    previous: previous_tracker,
                });

                // Commit journal (no-op, but marks as non-rollbackable)
                journal.commit();

                Ok(Self::Committed { manifest_path })
            }
            Self::Committed { .. } | Self::Failed { .. } => Ok(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::test_support::TempCognicodeHome;
    use serial_test::serial;

    /// The dev-only fixture must be a well-formed v2 manifest.
    ///
    /// NOTE: the fixture is explicitly **not** authoritative and is not a
    /// published release manifest (e85 WU8). This test pins only that it parses,
    /// that it is v2, and that no declared profile is a no-op.
    #[test]
    fn dev_fixture_is_well_formed_v2() {
        let yaml = include_str!("dev-bundle.yaml");
        let manifest = BundleManifest::from_str(yaml).expect("dev fixture must parse as v2");

        assert_eq!(
            manifest.api_version,
            crate::bundle_manifest::BUNDLE_API_VERSION
        );
        for profile in manifest.profile_names() {
            assert!(
                !manifest.components_for_profile(profile).is_empty(),
                "dev fixture profile `{profile}` must not resolve to zero components"
            );
        }
    }

    #[test]
    fn install_stage_ordering() {
        use std::fmt::Display;
        let stages = [
            InstallStage::ResolvingUrl,
            InstallStage::Downloading,
            InstallStage::VerifyingSha256,
            InstallStage::Extracting,
            InstallStage::InstallingShims,
            InstallStage::WritingManifest,
            InstallStage::Committed,
        ];
        for (i, &s) in stages.iter().enumerate() {
            assert_eq!(s as i32, i as i32);
        }
        // Failed should be last
        assert_eq!(InstallStage::Failed as i32, 7);
    }

    #[test]
    #[serial_test::serial]
    fn advance_skips_through_all_stages() {
        // Drive the real pipeline end to end against a real release that is
        // generated by the factory and served locally, so the Downloading,
        // VerifyingSha256 and Extracting stages genuinely execute.
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);

        let manifest = BundleManifest::from_path(&release.manifest_path).unwrap();
        let journal = RollbackJournal::new();

        let mut tx = InstallerTransaction::Running {
            stage: InstallStage::ResolvingUrl,
            journal,
            manifest,
        };

        for _ in 0..5 {
            tx = tx.advance(&home, "core").unwrap();
        }

        // Should be at WritingManifest, ready to commit
        match tx {
            InstallerTransaction::Running {
                stage: InstallStage::WritingManifest,
                ..
            } => {}
            other => panic!("expected WritingManifest, got {:?}", other),
        }
    }

    #[test]
    fn advance_is_noop_for_terminal_states() {
        // Committed
        let committed = InstallerTransaction::Committed {
            manifest_path: PathBuf::from("/tmp/manifest.yaml"),
        };
        // The terminal-state branches do not touch the home at all, but the
        // argument type still requires a CognicodeHome. We pass a dummy that
        // is never dereferenced.
        let home = crate::layout::CognicodeHome::resolve(None).expect("resolve home for dummy arg");
        let result = committed.advance(&home, "core").unwrap();
        assert!(matches!(result, InstallerTransaction::Committed { .. }));

        // Failed
        let failed = InstallerTransaction::Failed {
            stage: InstallStage::Downloading,
            error: InstallerError::Unknown("test".into()),
        };
        let result = failed.advance(&home, "core").unwrap();
        assert!(matches!(result, InstallerTransaction::Failed { .. }));
    }

    #[test]
    #[serial_test::serial]
    fn commit_writes_manifest_file() {
        // L2 (ADR-CANONICAL-LAYOUT): writes to `<root>/versions/<v>/manifest.yaml`,
        // not `<root>/install/<v>/manifest.yaml`.
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");
        let yaml = r#"
apiVersion: cognicode.bundle/v2
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: cognicode
    kind: cognicode
    version: "0.94.0"
    artifact: cognicode-0.94.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.94.0/cognicode-0.94.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        let manifest = BundleManifest::from_str(yaml).unwrap();
        let journal = RollbackJournal::new();

        let tx = InstallerTransaction::Running {
            stage: InstallStage::WritingManifest,
            journal,
            manifest,
        };

        let result = tx.commit(&home).unwrap();
        match result {
            InstallerTransaction::Committed { manifest_path } => {
                assert!(manifest_path.exists(), "manifest should be written");
                assert_eq!(
                    manifest_path,
                    home.version_manifest("0.94.0"),
                    "L2: manifest must land at versions/<v>/manifest.yaml per ADR"
                );
                // The persisted journal MUST be on disk next to the install
                // (e86 REQ-LJ-01) so a later `cogh rollback` can replay it.
                let journal_path = crate::lifecycle_journal::journal_path("0.94.0");
                assert!(
                    journal_path.exists(),
                    "persisted journal should be written next to install at {}",
                    journal_path.display()
                );
                // Clean up
                let _ = std::fs::remove_file(&manifest_path);
                let _ = std::fs::remove_file(&journal_path);
            }
            other => panic!("expected Committed, got {:?}", other),
        }
    }

    // ----- e86 WU4: commit persists the journal next to the install -----
    // (The `commit_writes_manifest_file` test above already pins this; the
    // dedicated test below re-asserts it as a behaviour contract and adds the
    // round-trip through `lifecycle_journal::load`.)

    #[test]
    #[serial_test::serial]
    fn commit_writes_journal_next_to_install() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");
        let yaml = r#"
apiVersion: cognicode.bundle/v2
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: cognicode
    kind: cognicode
    version: "0.94.0"
    artifact: cognicode-0.94.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.94.0/cognicode-0.94.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
"#;
        let manifest = BundleManifest::from_str(yaml).unwrap();
        let tx = InstallerTransaction::Running {
            stage: InstallStage::WritingManifest,
            journal: RollbackJournal::new(),
            manifest,
        };
        let result = tx.commit(&home).expect("commit must succeed");
        let manifest_path = match result {
            InstallerTransaction::Committed { manifest_path } => manifest_path,
            other => panic!("expected Committed, got {:?}", other),
        };

        let journal_path = crate::lifecycle_journal::journal_path("0.94.0");
        assert!(
            journal_path.exists(),
            "persisted journal must exist at {}",
            journal_path.display()
        );

        let envelope = crate::lifecycle_journal::load_envelope(&journal_path).expect("load");
        assert_eq!(envelope.version, "0.94.0");
        assert!(
            !envelope.effects.effects().is_empty(),
            "journal must carry effects"
        );
        assert!(
            envelope
                .effects
                .effects()
                .iter()
                .any(|e| matches!(e, crate::rollback_journal::SideEffect::WroteManifest(_))),
            "persisted journal must include WroteManifest, got {:?}",
            envelope.effects.effects()
        );

        let _ = std::fs::remove_file(&manifest_path);
        let _ = std::fs::remove_file(&journal_path);
    }

    // ----- e74 WU2: platform matching in the install pipeline -----

    /// Pin the e74 WU2 contract: `assert_host_platform` is part of
    /// the manifest-loading path. A bundle whose `platform` does not
    /// match the host's detected platform must be rejected before
    /// any install work happens, and the error message must be loud.
    ///
    /// This is the "no Windows-fallback-to-Linux" guarantee.
    ///
    /// DEBT-4: `#[serial]` — this test READS process env
    /// (`load_bundle_manifest` consults `COGNICODE_BUNDLE_MANIFEST`),
    /// so it must not run concurrently with env-mutating tests.
    #[test]
    #[serial_test::serial]
    fn installer_rejects_wrong_platform_bundle_with_loud_error() {
        // Simulate the wrong-platform scenario: parse the embedded
        // bundle, then point assert_host_platform at a non-matching
        // platform. The call must fail loudly with both the bundle
        // platform and the requested host platform in the error.
        //
        // DEBT-4: make the test hermetic against env leaks from earlier
        // serial tests (`point_at` leaves `COGNICODE_BUNDLE_MANIFEST`
        // pointing at a dropped tempdir). Clear the overrides so the
        // embedded dev fixture is the deterministic input.
        crate::release_test_support::unpoint();
        let yaml = InstallerTransaction::load_bundle_manifest()
            .expect("embedded bundle manifest must be readable");
        let manifest =
            BundleManifest::from_str(&yaml).expect("embedded bundle manifest must parse");
        let host = platform_adapter::detect_host_platform();

        // First, the matching case must succeed. (This is the same
        // assertion as `embedded_bundle_version_matches_pkg_version`
        // but for platform, kept here so this test stands alone.)
        manifest
            .assert_host_platform(host)
            .expect("current host must match embedded bundle platform");

        // Then, force a mismatch by picking a different triple.
        let wrong_host = match host {
            Platform::LinuxX86_64 => Platform::WindowsX86_64,
            Platform::LinuxAarch64 => Platform::MacOsX86_64,
            Platform::MacOsX86_64 => Platform::LinuxX86_64,
            Platform::MacOsAarch64 => Platform::LinuxAarch64,
            Platform::WindowsX86_64 => Platform::LinuxX86_64,
        };
        let err = manifest
            .assert_host_platform(wrong_host)
            .expect_err("wrong-platform bundle must be rejected");
        let msg = format!("{err}");
        assert!(
            msg.contains("no fallback") || msg.contains("wrong-platform"),
            "error must explain the no-fallback policy: {msg}"
        );
        assert!(
            msg.contains(&format!("{:?}", wrong_host)),
            "error must mention the rejected host: {msg}"
        );
        assert!(
            msg.contains(&format!("{:?}", manifest.platform)),
            "error must mention the bundle platform: {msg}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // E86.2.1 — Bearer token preservation through redirects (GREEN tests)
    //
    // These tests verify that `download_with_bearer()` correctly re-attaches
    // the Authorization header on each redirect hop, and enforces the trust set.
    //
    // The test helpers (bind_one_shot_*, header_value) are duplicated from
    // `crate::lifecycle_resolver::tests` to keep the test module self-contained.
    // See: // SHARED-WITH crate::lifecycle_resolver::tests
    // ─────────────────────────────────────────────────────────────────────────

    // SHARED-WITH crate::lifecycle_resolver::tests
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// One-shot HTTP/1.1 server: replies with `status` and `location` if 302.
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

    /// Bind a blocking reqwest client with **no** auto-redirect handling.
    ///
    /// The manual redirect loop in `download_with_bearer` needs to inspect
    /// each `Location` header itself to re-attach the bearer; if reqwest's
    /// built-in policy follows redirects automatically, it strips
    /// `Authorization` on cross-origin hops and the helper never gets the
    /// chance to re-attach. The client therefore has `.redirect(Policy::none())`
    /// and the helper owns every hop.
    fn build_test_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("build test client with no auto-redirect")
    }

    // ===== L2 producer retarget tests (ADR-CANONICAL-LAYOUT) =====
    //
    // Pinned by the L2 cycle. The producer must write to
    // `<root>/versions/<v>/...` instead of `<root>/install/<v>/...`.
    // These tests assert the on-disk shape after a successful commit
    // against a real release staged by `release_test_support`.

    /// L2 T1 (was RED before fix): after a real commit, the manifest
    /// lands at `<root>/versions/<v>/manifest.yaml` and the component
    /// tree lands at `<root>/versions/<v>/<component>/`. Pre-L2 it
    /// landed at `<root>/install/<v>/...`.
    #[test]
    #[serial_test::serial]
    fn t_l2_extracting_writes_under_versions_layout() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);

        // Drive the full pipeline end-to-end and commit.
        let result = InstallerTransaction::run(&home, "core");
        let manifest_path = match result {
            Ok(p) => p,
            Err(e) => panic!("install transaction must succeed; got {e:?}"),
        };

        // The manifest must be at versions/<v>/manifest.yaml per ADR.
        assert!(
            manifest_path.starts_with(home.version_root(env!("CARGO_PKG_VERSION"))),
            "L2: manifest must be under versions/<v>/; got {}",
            manifest_path.display()
        );
        assert_eq!(
            manifest_path,
            home.version_manifest(env!("CARGO_PKG_VERSION")),
            "L2: manifest path must equal home.version_manifest(v)"
        );
        assert!(
            manifest_path.exists(),
            "L2: manifest file must exist on disk"
        );

        // The legacy install/<v>/ tree must NOT exist. We assert
        // by checking the on-disk directory directly rather than via
        // `home.install_manifest_path` (which L5 retired). After L5
        // nothing in the source tree references `install/<v>/`.
        let legacy_install = home.root.join("install").join(env!("CARGO_PKG_VERSION"));
        assert!(
            !legacy_install.exists(),
            "L2+L5: legacy install/<v>/ must NOT exist after the canonical-layout cycle; got {}",
            legacy_install.display()
        );

        // Clean up
        let _ = std::fs::remove_file(&manifest_path);
        let version_root = home.version_root(env!("CARGO_PKG_VERSION"));
        let _ = std::fs::remove_dir_all(&version_root);
        let journal_path = crate::lifecycle_journal::journal_path(env!("CARGO_PKG_VERSION"));
        let _ = std::fs::remove_file(&journal_path);
    }

    // ========================================================================
    // DEBT-3.f — strict T1 (gate for :423 heuristic elimination)
    //
    // Adversarial contract: the runtime must locate a component's
    // binary even when the binary's filename deliberately diverges
    // from the ComponentId. The legacy inline heuristic
    // `install_dir.join(comp).join("bin").join(comp)` looked for a
    // file named exactly `<comp>` and silently missed anything
    // else. The strict T1 below plants a binary named
    // `<comp>-v2` (clearly NOT `<comp>`) inside
    // `<root>/versions/<v>/<comp>/bin/` and asserts the runtime
    // still finds it.
    //
    // The helper's third leg (`scan bin/ for any file`) is what
    // makes T1 green. The first two legs (legacy `bin/<comp>/<comp>`
    // and legacy `<comp>/<comp>`) are preserved for back-compat
    // and pinned by T1b.
    //
    // The test deliberately does NOT place a `<comp>` file alongside
    // the `<comp>-v2` file. If both were present, the legacy
    // helper would coincidentally return the `<comp>` file and T1
    // would green for the wrong reason. The single-planted-file
    // shape forces the runtime to choose: find it, or fail.
    // ========================================================================

    /// DEBT-3.f strict T1 (gate): the binary for a component can be
    /// located by name that is NOT equal to the ComponentId, as long
    /// as it lives in the canonical `<comp>/bin/` directory.
    ///
    /// This pins the invariant that "install / extract / shim
    /// resolution / component lookup" work even when BinaryName ≠
    /// ComponentId (the case the legacy `bin/<comp>/<comp>`
    /// heuristic silently broke).
    #[test]
    #[serial_test::serial]
    fn t_debt3f_strict_locate_binary_with_divergent_filename() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");

        // Three deliberately distinct identity strings. None of
        // these match each other:
        //   - PluginId      = "mcp-server"        (legacy plugin world)
        //   - ComponentId   = "alpha-daemon"      (the bundle component)
        //   - BinaryName    = "alpha-daemon-v2"   (the actual on-disk binary)
        //
        // The runtime must derive the shim target from the
        // ComponentId (alpha-daemon) but locate the binary by
        // BinaryName (alpha-daemon-v2). It must NOT silently fall
        // back to a heuristic like `comp.name == binary.name` or
        // a hardcoded `cognicode-mcp` literal.
        let plugin_id = "mcp-server";
        let component_id = "alpha-daemon";
        let binary_name = "alpha-daemon-v2";
        assert_ne!(plugin_id, component_id);
        assert_ne!(component_id, binary_name);
        assert_ne!(plugin_id, binary_name);

        // Plant the synthetic on-disk shape.
        let version = "0.95.0";
        let comp_root = home.component_root(version, component_id);
        let bin_dir = comp_root.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("create <comp>/bin/");
        let planted = bin_dir.join(binary_name);
        std::fs::write(&planted, b"#!/bin/sh\necho alpha-daemon-v2 dev-fixture\n")
            .expect("plant binary file");

        // The strict gate: the helper must find the planted file.
        let located = locate_component_binary(&home, version, component_id).expect(
            "DEBT-3.f strict T1: locate_component_binary must find \
                         a binary even when its filename diverges from the \
                         ComponentId; legacy heuristic hardcodes `bin/<comp>/<comp>` \
                         and silently misses everything else",
        );
        assert_eq!(
            located, planted,
            "DEBT-3.f strict T1: located path must equal the planted file"
        );

        // Clean up.
        let _ = std::fs::remove_dir_all(home.version_root(version));
    }

    /// DEBT-3.f strict T1b (gate, legacy still works): the helper
    /// also finds the legacy `bin/<comp>/<comp>` shape so existing
    /// bundles continue to install correctly after the relaxation.
    /// This is the "back-compat" half of the relaxation contract:
    /// the helper's first leg still prefers `<comp>` literally if
    /// present, falling back to the relaxed scan only when the
    /// legacy shape is absent.
    #[test]
    #[serial_test::serial]
    fn t_debt3f_strict_locate_binary_legacy_shape_still_works() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");

        let component_id = "alpha-daemon";
        let version = "0.95.0";
        let legacy_path = home
            .component_root(version, component_id)
            .join("bin")
            .join(component_id);

        // Plant the legacy shape (binary named exactly `<comp>`).
        std::fs::create_dir_all(legacy_path.parent().unwrap()).expect("create bin/");
        std::fs::write(&legacy_path, b"#!/bin/sh\necho legacy\n").expect("plant legacy binary");

        let located = locate_component_binary(&home, version, component_id)
            .expect("legacy `bin/<comp>/<comp>` shape must still resolve");
        assert_eq!(located, legacy_path);

        // Clean up.
        let _ = std::fs::remove_dir_all(home.version_root(version));
    }

    /// L2 T2: the journal's `WroteManifest` SideEffect points at the
    /// canonical `versions/<v>/manifest.yaml`, not `install/<v>/...`.
    /// The journal is the authoritative undo log; if it points at the
    /// legacy layout, a future `cogh rollback` would re-create the
    /// legacy tree.
    #[test]
    #[serial_test::serial]
    fn t_l2_commit_records_versions_layout_in_journal() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");
        let release = crate::release_test_support::local_release(env!("CARGO_PKG_VERSION"))
            .expect("stage a local release");
        crate::release_test_support::point_at(&release);

        let result =
            InstallerTransaction::run(&home, "core").expect("install transaction must succeed");

        let journal_path = crate::lifecycle_journal::journal_path(env!("CARGO_PKG_VERSION"));
        let envelope =
            crate::lifecycle_journal::load_envelope(&journal_path).expect("load journal envelope");
        assert_eq!(envelope.version, env!("CARGO_PKG_VERSION"));

        // The journal must record at least one WroteManifest effect,
        // and that effect's path must be under versions/<v>/.
        let wrote = envelope
            .effects
            .effects()
            .iter()
            .find_map(|e| match e {
                crate::rollback_journal::SideEffect::WroteManifest(p) => Some(p.clone()),
                _ => None,
            })
            .expect("journal must contain a WroteManifest effect");
        assert!(
            wrote.starts_with(home.version_root(env!("CARGO_PKG_VERSION"))),
            "L2: journal WroteManifest path must be under versions/<v>/; got {}",
            wrote.display()
        );

        // Clean up
        let _ = std::fs::remove_file(&result);
        let _ = std::fs::remove_dir_all(home.version_root(env!("CARGO_PKG_VERSION")));
        let _ = std::fs::remove_file(&journal_path);
    }

    /// REQ-FIX-01 GREEN — Bearer propagates across a cross-origin redirect.
    ///
    /// Sets up: server A (127.0.0.1:portA) → 302 → server B (127.0.0.1:portB).
    /// Verifies that the second request to server B carries the Bearer token.
    /// This was the defect: reqwest 0.12.28's redirect policy stripped it.
    #[test]
    fn e86_2_1_fix_01_bearer_propagates_across_redirect() {
        // Server B: terminal destination
        let (port_b, rx_b) = bind_one_shot_ok();
        // Server A: redirects to server B
        let location = format!("http://127.0.0.1:{port_b}/asset");
        let (port_a, rx_a) = bind_one_shot_302(&location);

        let client = build_test_client();

        let result = super::download_with_bearer(
            &client,
            &format!("http://127.0.0.1:{port_a}/some/path"),
            Some("test-token-abc"),
        );

        // The helper should follow the redirect and return 200
        let response = result.expect("download_with_bearer should succeed");
        assert_eq!(response.status().as_u16(), 200, "server B should reply 200");

        // First hop: server A received the bearer (soundness check)
        let req_a = rx_a.recv().expect("recv from A");
        assert_eq!(
            header_value(&req_a, "authorization").as_deref(),
            Some("Bearer test-token-abc"),
            "first hop must carry the bearer"
        );

        // Second hop: server B also receives the bearer (THE FIX)
        let req_b = rx_b.recv().expect("recv from B");
        assert_eq!(
            header_value(&req_b, "authorization").as_deref(),
            Some("Bearer test-token-abc"),
            "GREEN: bearer must propagate to the redirect target"
        );
    }

    /// REQ-FIX-02 GREEN — Redirect to a host outside the trust set returns
    /// `DisallowedRedirectHost` error instead of blindly following.
    #[test]
    fn e86_2_1_fix_02_untrusted_redirect_host_returns_disallowed_error() {
        // Server A: redirects to attacker.example.com (NOT in gh_trust_set)
        let (port_a, _rx_a) = bind_one_shot_302("http://attacker.example.com/asset");

        let client = build_test_client();

        let result = super::download_with_bearer(
            &client,
            &format!("http://127.0.0.1:{port_a}/some/path"),
            Some("test-token-abc"),
        );

        let err = result.expect_err("should fail for untrusted redirect host");
        let msg = format!("{err}");
        assert!(
            msg.contains("DisallowedRedirectHost"),
            "error must contain 'DisallowedRedirectHost', got: {msg}"
        );
        assert!(
            msg.contains("attacker.example.com"),
            "error must name the untrusted host, got: {msg}"
        );
    }

    /// REQ-FIX-03 GREEN — More than 5 redirect hops returns `TooManyRedirects`.
    ///
    /// Builds a 6-listener redirect loop. Each listener must answer many
    /// connections (we send up to 7 requests round-trip in the worst case
    /// because the helper may count hops before terminating); therefore
    /// each thread loops over `listener.incoming()` instead of using a
    /// one-shot pattern.
    #[test]
    fn e86_2_1_fix_03_too_many_hops_returns_error() {
        let n_hops: usize = 6;
        let mut ports: Vec<u16> = Vec::with_capacity(n_hops);

        // Pre-allocate ports first so each thread can resolve the next port.
        let mut pre_bound: Vec<TcpListener> = Vec::with_capacity(n_hops);
        for _ in 0..n_hops {
            let l = TcpListener::bind("127.0.0.1:0").expect("bind");
            l.set_nonblocking(false).ok();
            ports.push(l.local_addr().expect("local_addr").port());
            pre_bound.push(l);
        }

        // Spawn persistent redirect handlers. Each one accepts connections
        // until the test ends and redirects every one of them to the next
        // port in the cycle, producing an infinite redirect loop. The helper
        // must detect the loop within `MAX_DOWNLOAD_REDIRECTS` and bail out.
        for hop in 0..n_hops {
            let next_port = ports[(hop + 1) % ports.len()];
            let listener = pre_bound.remove(0);
            thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    use std::io::{Read, Write};
                    let mut stream = stream;
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    let location = format!("http://127.0.0.1:{next_port}/hop-{}", hop + 1);
                    let resp = format!(
                        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                }
            });
        }

        // Give threads time to start accepting
        thread::sleep(std::time::Duration::from_millis(50));

        let client = build_test_client();

        let result = super::download_with_bearer(
            &client,
            &format!("http://127.0.0.1:{}/start", ports[0]),
            Some("test-token"),
        );

        let err = result.expect_err("should fail after 5 hops");
        let msg = format!("{err}");
        assert!(
            msg.contains("TooManyRedirects"),
            "error must contain 'TooManyRedirects', got: {msg}"
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // E86.2.2 — Asset-base vs API-base URL split
    //
    // Before E86.2.2, COGNICODE_RELEASE_BASE_URL was the single env var for both
    // the GitHub API origin and the asset download origin. That caused a 403 bug:
    // when COGNICODE_RELEASE_BASE_URL=https://api.github.com, the resolver rewrote
    // canonical github.com asset URLs onto api.github.com, which does not serve
    // release assets.
    //
    // E86.2.2 splits this into:
    //   COGNICODE_API_BASE_URL   — GitHub API origin (resolver path, default unchanged)
    //   COGNICODE_ASSET_BASE_URL — asset download origin (installer path, default empty)
    //
    // resolve_download_url now rewrites ONLY when COGNICODE_ASSET_BASE_URL is set.
    // ─────────────────────────────────────────────────────────────────────────

    // REQ-86-2-2-01a: ASSET_BASE_URL is set → rewrite canonical asset URL.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_01a_resolve_download_url_honors_asset_base() {
        // SAFETY: #[serial] guards against concurrent env mutation.
        unsafe {
            std::env::set_var("COGNICODE_ASSET_BASE_URL", "https://my-mirror.example.com");
        }
        let result = super::resolve_download_url(
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
        );
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
        }
        assert_eq!(
            result,
            "https://my-mirror.example.com/v0.95.0/bundle-0.95.0-x86_64-unknown-linux-gnu.yaml",
            "canonical asset URL must be rewritten onto ASSET_BASE_URL"
        );
    }

    // REQ-86-2-2-01b: non-canonical URLs pass through unchanged regardless of env.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_01b_resolve_download_url_passes_through_non_canonical() {
        // SAFETY: #[serial] guards against concurrent env mutation.
        unsafe {
            std::env::set_var("COGNICODE_ASSET_BASE_URL", "https://my-mirror.example.com");
        }
        let result = super::resolve_download_url("https://example.com/whatever/thing.tar.gz");
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
        }
        assert_eq!(
            result, "https://example.com/whatever/thing.tar.gz",
            "non-canonical URL must not be rewritten even when ASSET_BASE_URL is set"
        );
    }

    // REQ-86-2-2-01c: ASSET_BASE_URL unset → return canonical unchanged.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_01c_resolve_download_url_canonical_when_unset() {
        // SAFETY: #[serial] guards; clear any stale old-name variable.
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }
        let result = super::resolve_download_url(
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz",
        );
        assert_eq!(
            result,
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz",
            "canonical URL must pass through when ASSET_BASE_URL is unset"
        );
    }

    // REQ-86-2-2-03: old COGNICODE_RELEASE_BASE_URL=https://api.github.com
    // emits a deprecation warning to stderr.
    //
    // The warning is emitted from `resolve_download_url` which writes to stderr.
    // To test this without a subprocess, we redirect stderr to a temp file using
    // a helper binary approach: write a small shim to a temp file that redirects
    // stderr and run it.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_03_deprecation_warning_on_old_var_with_api_host() {
        // SAFETY: #[serial] guards against concurrent env mutation.
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
            std::env::set_var("COGNICODE_RELEASE_BASE_URL", DEFAULT_API_BASE);
        }

        let mut sink: Vec<u8> = Vec::new();
        let result = super::resolve_download_url_into(
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/pkg.tar.gz",
            Some(&mut sink),
        );

        // SAFETY: cleanup; env state must not leak into other tests.
        unsafe {
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }

        assert_eq!(
            result, "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/pkg.tar.gz",
            "old RELEASE_BASE_URL=api.github.com must NOT rewrite (would cause 403)"
        );

        let warning = String::from_utf8_lossy(&sink);
        assert!(
            warning.contains("COGNICODE_RELEASE_BASE_URL"),
            "warning must name the deprecated variable, got: {warning}"
        );
        assert!(
            warning.contains("COGNICODE_ASSET_BASE_URL")
                && warning.contains("COGNICODE_API_BASE_URL"),
            "warning must name both new env vars to guide migration, got: {warning}"
        );
    }

    // REQ-86-2-2-04: legacy COGNICODE_RELEASE_BASE_URL is still honored on
    // the asset side when COGNICODE_ASSET_BASE_URL is unset.
    //
    // This is the back-compat tripwire: air-gapped installs / mirrors set
    // COGNICODE_RELEASE_BASE_URL today. After E86.2.2 split the variable,
    // those setups must still work without an explicit migration step.
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_04_legacy_release_base_url_still_works_asset_side() {
        // SAFETY: #[serial] guards against concurrent env mutation.
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }
        unsafe {
            std::env::set_var(
                "COGNICODE_RELEASE_BASE_URL",
                "https://my-legacy-mirror.example.com",
            );
        }

        let result = super::resolve_download_url(
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/pkg.tar.gz",
        );

        unsafe {
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }

        assert_eq!(
            result, "https://my-legacy-mirror.example.com/v0.95.0/pkg.tar.gz",
            "legacy COGNICODE_RELEASE_BASE_URL must still rewrite on the asset side"
        );
    }

    // REQ-86-2-2-05: COGNICODE_ASSET_BASE_URL beats legacy
    // COGNICODE_RELEASE_BASE_URL on the asset side (no env var ambiguity).
    #[test]
    #[serial_test::serial]
    fn t_e86_2_2_05_asset_base_url_beats_legacy_release_base_url() {
        // SAFETY: #[serial] guards.
        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }
        unsafe {
            std::env::set_var("COGNICODE_ASSET_BASE_URL", "https://new-mirror.example.com");
            std::env::set_var(
                "COGNICODE_RELEASE_BASE_URL",
                "https://legacy-mirror.example.com",
            );
        }

        let result = super::resolve_download_url(
            "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/pkg.tar.gz",
        );

        unsafe {
            std::env::remove_var("COGNICODE_ASSET_BASE_URL");
            std::env::remove_var("COGNICODE_RELEASE_BASE_URL");
        }

        assert_eq!(
            result, "https://new-mirror.example.com/v0.95.0/pkg.tar.gz",
            "ASSET_BASE_URL must beat RELEASE_BASE_URL on the asset side"
        );
    }

    /// A valid manifest body declaring one skill bundle
    /// (`skills-for-claude`, pairwise-distinct from the ComponentId
    /// `cognicode-mcp`) plus a `reviewer`-only DaemonCli component.
    fn skill_bundle_tx_manifest() -> BundleManifest {
        BundleManifest::from_str(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-01-01T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
  - name: reviewer
    description: MCP bridge
skill_bundles:
  - id: skills-for-claude
    version: "0.95.0"
    profiles: [core, reviewer]
components:
  - name: cognicode
    kind: cognicode
    version: "0.95.0"
    artifact: cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core, reviewer]
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [reviewer]
"#,
        )
        .expect("fixture manifest must be valid")
    }

    /// DEBT-2b strict T1: the INSTALLED manifest written by commit()
    /// must preserve the `skill_bundles[]` declarations, even though
    /// the install filtered components to the active profile. The
    /// SkillBundleId namespace is orthogonal to component profiles:
    /// uninstall/integrate time needs the declarations regardless of
    /// which profile's components were extracted.
    ///
    /// Identities planted pairwise-distinct:
    ///     ComponentId   = "cognicode-mcp" (reviewer-only)
    ///     SkillBundleId = "skills-for-claude"
    #[test]
    #[serial_test::serial]
    fn t_debt2b_commit_preserves_skill_bundle_declarations() {
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");

        let manifest = skill_bundle_tx_manifest();
        assert!(
            manifest.skill_bundles_for_profile("core").len() == 1,
            "gate validity: manifest declares the skill bundle for core"
        );
        // Sanity: the install will filter out the reviewer-only DaemonCli.
        assert!(manifest.component_by_name("cognicode-mcp").is_some());

        let journal = RollbackJournal::new();
        let tx = InstallerTransaction::Running {
            stage: InstallStage::WritingManifest,
            journal,
            manifest,
        };

        let result = tx.commit(&home).expect("commit must succeed");
        let manifest_path = match result {
            InstallerTransaction::Committed { manifest_path } => manifest_path,
            other => panic!("expected Committed, got {other:?}"),
        };

        let installed =
            BundleManifest::from_path(&manifest_path).expect("installed manifest must parse");
        let bundles = installed.skill_bundles_for_profile("core");
        assert_eq!(
            bundles.len(),
            1,
            "installed manifest must retain skill_bundles declarations"
        );
        assert_eq!(bundles[0].id, "skills-for-claude");
        assert_ne!(
            bundles[0].id, "cognicode-mcp",
            "SkillBundleId must not be conflated with ComponentId"
        );

        let _ = std::fs::remove_file(&manifest_path);
        let _ = std::fs::remove_file(crate::lifecycle_journal::journal_path("0.95.0"));
    }

    /// DEBT-2b strict T2: the Extracting stage must materialise each
    /// declared skill bundle at
    /// `versions/<v>/skills/<SkillBundleId>/` from
    /// `cache_dir/<SkillBundleId>.tar.gz`, honouring the profile
    /// filter. The skill namespace lives under `skills/`, disjoint
    /// from `versions/<v>/<ComponentId>/` — a bundle id that happens
    /// to equal a component name must NOT collide with it.
    ///
    /// Identities planted:
    ///     ComponentId   = "cognicode-mcp" (reviewer-only)
    ///     SkillBundleId = "skills-for-claude"
    #[test]
    #[serial_test::serial]
    fn t_debt2b_extracting_materialises_declared_skill_bundles() {
        use crate::rollback_journal::SideEffect;
        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");

        // Stage the skill bundle tarball in the cache, as Downloading would.
        let cache_dir = layout::cache_dir();
        std::fs::create_dir_all(&cache_dir).unwrap();
        let payload_dir = tempfile::tempdir().unwrap();
        std::fs::write(payload_dir.path().join("SKILL.md"), "---\nname: x\n---\n").unwrap();
        let tarball = cache_dir.join("skills-for-claude.tar.gz");
        {
            let f = std::fs::File::create(&tarball).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut tar = tar::Builder::new(enc);
            tar.append_path_with_name(payload_dir.path().join("SKILL.md"), "SKILL.md")
                .unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }

        // Also stage the `cognicode` component tarball (core's only
        // component) so the component extraction loop succeeds.
        let comp_tarball = cache_dir.join("cognicode.tar.gz");
        {
            let f = std::fs::File::create(&comp_tarball).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut tar = tar::Builder::new(enc);
            tar.append_dir_all("bin", payload_dir.path()).unwrap();
            tar.into_inner().unwrap().finish().unwrap();
        }

        let mut manifest = skill_bundle_tx_manifest();
        // The install filters components to the profile BEFORE extraction
        // (core install strips the reviewer-only DaemonCli), but the
        // skill bundle declared for core must still be extracted.
        let filtered: Vec<_> = manifest.components_for_profile("core");
        manifest.components = filtered.into_iter().cloned().collect();
        assert!(
            manifest.component_by_name("cognicode-mcp").is_none(),
            "gate validity: core profile strips the DaemonCli"
        );

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(home.version_root("0.95.0")));

        let tx = InstallerTransaction::Running {
            stage: InstallStage::Extracting,
            journal,
            manifest,
        };

        let tx = match tx.advance(&home, "core") {
            Ok(t) => t,
            other => panic!("advance failed: {other:?}"),
        };
        if let InstallerTransaction::Failed { error, .. } = &tx {
            panic!("advance returned Failed: {error:?}");
        }
        // Drain the journal from the Running state we just advanced into.
        // Instead of poking internals, verify the on-disk outcome.
        let skills_dir = home.skill_bundle("0.95.0", "skills-for-claude");
        assert!(
            skills_dir.join("SKILL.md").exists(),
            "declared skill bundle must be extracted to {} (id from manifest, \
             never derived from a ComponentId)",
            skills_dir.display()
        );

        // And the disjointness invariant: the skill path is NOT the
        // component path even for coincident names.
        let comp_dir = home.version_root("0.95.0").join("skills-for-claude");
        assert_ne!(
            skills_dir, comp_dir,
            "skill namespace lives under skills/, not the component root"
        );

        match tx {
            InstallerTransaction::Running { stage, .. } => {
                assert_eq!(
                    stage,
                    InstallStage::InstallingShims,
                    "advance must move Extracting → InstallingShims"
                );
            }
            other => panic!("expected Running, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(home.version_root("0.95.0"));
    }

    /// DEBT-2b strict T3 (round-trip): extract-then-integrate with NO
    /// test-side fixture planting of the skill directory. The install
    /// transaction materialises the declared bundle; the IDE integrator
    /// must then resolve it purely from the installed manifest.
    ///
    /// Identities planted:
    ///     ComponentId   = "cognicode-mcp" (reviewer-only, absent)
    ///     SkillBundleId = "skills-for-claude"
    #[test]
    #[serial_test::serial]
    fn t_debt2b_round_trip_extract_then_integrate() {
        use crate::rollback_journal::SideEffect;
        use std::path::Path;
        use tempfile::TempDir;

        let _temphome = TempCognicodeHome::new();
        let home =
            crate::layout::CognicodeHome::resolve(None).expect("resolve home from COGNICODE_HOME");

        // 1. Cache the payloads as Downloading would.
        let cache_dir = layout::cache_dir();
        std::fs::create_dir_all(&cache_dir).unwrap();
        let staging = TempDir::new().unwrap();
        std::fs::write(
            staging.path().join("SKILL.md"),
            "---\nname: roundtrip\n---\nbody",
        )
        .unwrap();

        let make_tarball = |name: &str, src: &Path| {
            let f = std::fs::File::create(cache_dir.join(format!("{name}.tar.gz"))).unwrap();
            let enc = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
            let mut tar = tar::Builder::new(enc);
            for entry in std::fs::read_dir(src).unwrap() {
                let e = entry.unwrap();
                let name = e.file_name().to_string_lossy().to_string();
                tar.append_path_with_name(e.path(), name).unwrap();
            }
            tar.into_inner().unwrap().finish().unwrap();
        };
        make_tarball("skills-for-claude", staging.path());

        // `cognicode` component payload (bin/ layout) for the shim stage.
        let bin_dir = staging.path().join("bin");
        std::fs::create_dir_all(&bin_dir).unwrap();
        std::fs::write(bin_dir.join("cognicode"), "#!/bin/sh\n").unwrap();
        let comp_f = std::fs::File::create(cache_dir.join("cognicode.tar.gz")).unwrap();
        let enc = flate2::write::GzEncoder::new(comp_f, flate2::Compression::fast());
        let mut tar = tar::Builder::new(enc);
        tar.append_dir_all(".", &bin_dir).unwrap();
        tar.into_inner().unwrap().finish().unwrap();

        // 2. Run the transaction's stage actions manually from Extracting
        // (download/network is not the SUT here); advance through shims.
        let mut manifest = skill_bundle_tx_manifest();
        let filtered: Vec<_> = manifest.components_for_profile("core");
        manifest.components = filtered.into_iter().cloned().collect();

        let mut journal = RollbackJournal::new();
        journal.record(SideEffect::CreatedDir(home.version_root("0.95.0")));
        let mut tx = InstallerTransaction::Running {
            stage: InstallStage::Extracting,
            journal,
            manifest,
        };
        tx = tx.advance(&home, "core").unwrap(); // Extracting
        tx = tx.advance(&home, "core").unwrap(); // InstallingShims
        tx = tx.advance(&home, "core").unwrap(); // WritingManifest
        let tx = tx.commit(&home).expect("commit must succeed");
        let manifest_path = match tx {
            InstallerTransaction::Committed { manifest_path } => manifest_path,
            other => panic!("expected Committed, got {other:?}"),
        };

        // 3. Integrate with NO test-side skill fixture: the integrator
        // must find `versions/<v>/skills/skills-for-claude/` purely via
        // the installed manifest.
        let skill_sources = crate::bundle_manifest::declared_skill_bundle_dirs(
            &home.skills_root("0.95.0"),
            &manifest_path,
            "core",
        )
        .expect("integration resolution must succeed post-install");
        assert_eq!(
            skill_sources.len(),
            1,
            "the installed manifest must drive exactly one bundle resolution"
        );
        assert!(skill_sources[0].join("SKILL.md").exists());

        let _ = std::fs::remove_dir_all(home.version_root("0.95.0"));
        let _ = std::fs::remove_file(crate::lifecycle_journal::journal_path("0.95.0"));
    }
}
