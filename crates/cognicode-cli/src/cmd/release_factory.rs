//! e85 — Release factory: generation and the executable verification gate.
//!
//! This module turns a staging directory of produced payload archives into the
//! release's generated artifacts, and verifies them. It never touches the
//! network and never publishes: everything here runs locally, before any GitHub
//! Release exists, so the contract can be checked before anything is public.
//!
//! Pipeline (e84 WU8):
//!
//! ```text
//! staged payloads
//!   -> scan + hash          (ReleaseInventory)
//!   -> project Layer 1      (BundleManifest v2, one per platform)
//!   -> SHA256SUMS           (over payloads + manifests + inventory)
//!   -> verify               (R1-R9, the gate)
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::bundle_manifest::Platform;
use crate::bundle_manifest::{BUNDLE_API_VERSION, BundleComponent, BundleManifest, ProfileDef};
use crate::release_contract::{
    ArtifactKind, PUBLISHED_PROFILES, ProducedArtifact, ReleaseInventory, artifact_filename,
    artifact_url, build_inventory, bundle_manifest_filename, platform_token, published_components,
    release_inventory_filename, sha256_file,
};

/// Outcome of a successful generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerationReport {
    pub version: String,
    pub tag: String,
    pub payload_count: usize,
    pub manifest_count: usize,
    pub sha256sums_entries: usize,
}

/// Outcome of a successful verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyReport {
    pub version: String,
    pub tag: String,
    pub platforms: Vec<String>,
    pub payloads: usize,
    pub manifests: usize,
    pub checks: Vec<String>,
}

impl VerifyReport {
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "release-verify: OK  version={} tag={}\n",
            self.version, self.tag
        ));
        out.push_str(&format!("  platforms : {}\n", self.platforms.join(", ")));
        out.push_str(&format!("  payloads  : {}\n", self.payloads));
        out.push_str(&format!("  manifests : {}\n", self.manifests));
        for c in &self.checks {
            out.push_str(&format!("  check     : {c}\n"));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// Project a platform's Layer-1 artifacts into a `BundleManifest` v2.
///
/// Layer 0 (`cogh`) and meta artifacts are excluded by construction: only
/// installable kinds may appear in a bundle.
pub fn generate_bundle_manifest(
    inventory: &ReleaseInventory,
    platform: Platform,
    released_at: Option<String>,
) -> Result<BundleManifest> {
    let mut components: Vec<BundleComponent> = Vec::new();

    for artifact in inventory.for_platform(platform).filter(|a| a.installable) {
        components.push(BundleComponent {
            name: artifact.component_name.clone(),
            kind: artifact.kind,
            version: artifact.version.clone(),
            artifact: artifact.filename.clone(),
            sha256: artifact.sha256.clone(),
            url: artifact.url(),
            profiles: artifact.profiles.clone(),
        });
    }

    if components.is_empty() {
        bail!(
            "no installable Layer 1 artifacts for platform `{platform}`; \
             refusing to generate an empty bundle"
        );
    }
    components.sort_by(|a, b| a.name.cmp(&b.name));

    let manifest = BundleManifest {
        api_version: BUNDLE_API_VERSION.to_string(),
        kind: "Bundle".to_string(),
        version: inventory.version.clone(),
        platform,
        released_at,
        profiles: PUBLISHED_PROFILES
            .iter()
            .map(|(name, description)| ProfileDef {
                name: (*name).to_string(),
                description: (*description).to_string(),
            })
            .collect(),
        components,
    };

    // The generator must produce something its own validator accepts. If it
    // cannot, the contract table and the generator disagree, which is a bug in
    // this code and not a bad input.
    manifest
        .validate()
        .context("the generator produced a manifest that fails v2 validation")?;

    Ok(manifest)
}

/// Generate the release artifacts into `out_dir`.
pub fn generate_release(
    staging: &Path,
    out_dir: &Path,
    version: &str,
    tag: &str,
    source_commit: &str,
    expected_platforms: &[Platform],
    released_at: Option<String>,
) -> Result<GenerationReport> {
    if tag != format!("v{version}") {
        bail!("tag `{tag}` must equal `v{version}` (e84 R8)");
    }
    std::fs::create_dir_all(out_dir)
        .with_context(|| format!("create out dir {}", out_dir.display()))?;

    // 1. Scan the staged payloads and hash them.
    let inventory = build_inventory(staging, version, tag, source_commit, expected_platforms)?;

    // 1b. Materialise a COMPLETE release directory: generated artifacts must sit
    //     next to the payloads they describe, so `out_dir` is self-contained and
    //     directly uploadable (and directly verifiable).
    for artifact in &inventory.artifacts {
        let src = staging.join(&artifact.filename);
        let dest = out_dir.join(&artifact.filename);
        if src != dest {
            std::fs::copy(&src, &dest)
                .with_context(|| format!("copy {} -> {}", src.display(), dest.display()))?;
        }
    }

    // 2. Project each platform's Layer 1 subset into a v2 manifest.
    let mut manifest_names: Vec<String> = Vec::new();
    for platform in expected_platforms {
        let manifest = generate_bundle_manifest(&inventory, *platform, released_at.clone())?;
        let name = bundle_manifest_filename(version, *platform);
        let dest = out_dir.join(&name);
        std::fs::write(&dest, manifest.to_yaml()?)
            .with_context(|| format!("write {}", dest.display()))?;
        manifest_names.push(name);
    }
    manifest_names.sort();

    // 3. The inventory itself.
    let inventory_name = release_inventory_filename(version);
    let inventory_json = inventory.to_json()?;
    let inventory_path = out_dir.join(&inventory_name);
    std::fs::write(&inventory_path, &inventory_json)
        .with_context(|| format!("write {}", inventory_path.display()))?;

    // 4. SHA256SUMS over payloads + manifests + inventory (never itself).
    let mut covered: Vec<String> = inventory
        .artifacts
        .iter()
        .map(|a| a.filename.clone())
        .collect();
    covered.extend(manifest_names.iter().cloned());
    covered.push(inventory_name.clone());

    let sums = sha256sums_for(out_dir, &covered)?;
    let sums_path = out_dir.join("SHA256SUMS");
    std::fs::write(&sums_path, &sums).with_context(|| format!("write {}", sums_path.display()))?;

    Ok(GenerationReport {
        version: version.to_string(),
        tag: tag.to_string(),
        payload_count: inventory.artifacts.len(),
        manifest_count: manifest_names.len(),
        sha256sums_entries: covered.len(),
    })
}

// ---------------------------------------------------------------------------
// SHA256SUMS (e84 WU10)
// ---------------------------------------------------------------------------

/// Build `SHA256SUMS` content for the named files, sorted by filename.
///
/// `sha256sum` format: lowercase hex, two spaces, filename. No timestamps, so
/// the output is byte-identical for identical inputs.
pub fn sha256sums_for(dir: &Path, filenames: &[String]) -> Result<String> {
    let mut sorted: Vec<&String> = filenames.iter().collect();
    sorted.sort();
    let mut out = String::new();
    for name in sorted {
        let path = dir.join(name);
        if !path.is_file() {
            bail!("cannot checksum `{name}`: file does not exist");
        }
        out.push_str(&format!("{}  {}\n", sha256_file(&path)?, name));
    }
    Ok(out)
}

/// Parse `SHA256SUMS` into filename -> digest, preserving strictness.
pub fn parse_sha256sums(content: &str) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    for (i, line) in content.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (digest, name) = line
            .split_once("  ")
            .with_context(|| format!("SHA256SUMS line {} is not `hex  filename`", i + 1))?;
        if digest.len() != 64 || !digest.chars().all(|c| c.is_ascii_hexdigit()) {
            bail!("SHA256SUMS line {} has a malformed digest", i + 1);
        }
        map.insert(name.to_string(), digest.to_ascii_lowercase());
    }
    if map.is_empty() {
        bail!("SHA256SUMS is empty");
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Verification gate (e84 WU9)
// ---------------------------------------------------------------------------

/// Verify a staged release. This is the executable form of R1–R9.
///
/// Runs locally against a staging directory; no GitHub Release required.
pub fn verify_release(
    staging: &Path,
    version: &str,
    tag: &str,
    expected_platforms: &[Platform],
) -> Result<VerifyReport> {
    let mut checks: Vec<String> = Vec::new();

    // R8: tag == v{version}
    if tag != format!("v{version}") {
        bail!("tag `{tag}` must equal `v{version}` (R8)");
    }
    checks.push("R8 tag equals v{version}".to_string());

    // R1/R2/R3/R5: scan, canonical names, one per component, no orphans,
    // non-placeholder digests recomputed from bytes.
    let inventory = build_inventory(staging, version, tag, "", expected_platforms)?;
    checks.push(format!(
        "R1/R2/R3/R5 {} canonical payloads, digests recomputed, no orphans, no placeholders",
        inventory.artifacts.len()
    ));

    // R9: Tier-1 platform set complete.
    for platform in expected_platforms {
        let present = inventory.for_platform(*platform).count();
        if present == 0 {
            bail!("platform `{platform}` has no payloads (R9)");
        }
    }
    checks.push(format!(
        "R9 platform set complete ({})",
        expected_platforms.len()
    ));

    // R6/R7: read each generated manifest, validate, and cross-check it against
    // the payloads.
    let mut manifest_count = 0usize;
    for platform in expected_platforms {
        let name = bundle_manifest_filename(version, *platform);
        let path = staging.join(&name);
        let text = std::fs::read_to_string(&path).with_context(|| {
            format!("missing generated manifest `{name}`; the release is incomplete (e84 R6)")
        })?;
        let manifest = BundleManifest::from_path(&path)
            .with_context(|| format!("`{name}` must be a valid BundleManifest v2"))?;

        if manifest.platform != *platform {
            bail!(
                "manifest `{name}` declares platform `{}` but is named for `{platform}`",
                manifest.platform
            );
        }
        if manifest.version != version {
            bail!(
                "manifest `{name}` declares version `{}` != `{version}`",
                manifest.version
            );
        }

        // No phantom: every manifest component must map to exactly one payload
        // with a matching digest.
        for component in &manifest.components {
            let payload = inventory
                .artifacts
                .iter()
                .find(|a| a.filename == component.artifact)
                .with_context(|| {
                    format!(
                        "phantom component: manifest `{name}` references `{}` \
                         which was not produced",
                        component.artifact
                    )
                })?;
            if payload.sha256 != component.sha256 {
                bail!(
                    "digest mismatch for `{}`: manifest says {}, recomputed {}",
                    component.artifact,
                    component.sha256,
                    payload.sha256
                );
            }
            if component.url != artifact_url(version, &component.artifact) {
                bail!("component `{}` url is not canonical", component.name);
            }
        }

        // No orphan: every installable payload for this platform must be in the
        // manifest exactly once.
        for artifact in inventory.for_platform(*platform).filter(|a| a.installable) {
            let matches = manifest
                .components
                .iter()
                .filter(|c| c.artifact == artifact.filename)
                .count();
            if matches != 1 {
                bail!(
                    "orphan payload `{}` appears {matches} times in manifest `{name}` (must be exactly 1)",
                    artifact.filename
                );
            }
        }

        // R7: no declared profile may be a no-op.
        for profile in manifest.profile_names() {
            if manifest.components_for_profile(profile).is_empty() {
                bail!("manifest `{name}` profile `{profile}` resolves to zero components (R7)");
            }
        }

        checks.push(format!("R6/R7 manifest `{name}` consistent with payloads"));
        manifest_count += 1;
    }

    // Layer separation: a bundle must never contain the bootstrap or metadata.
    for platform in expected_platforms {
        let name = bundle_manifest_filename(version, *platform);
        let manifest = BundleManifest::from_path(&staging.join(&name))?;
        for component in &manifest.components {
            if !component.kind.is_installable() {
                bail!(
                    "manifest `{name}` contains non-installable kind {:?} (e84 WU7)",
                    component.kind
                );
            }
            if component.name == ArtifactKind::Cogh.stem() {
                bail!("manifest `{name}` must not contain the Layer 0 bootstrap (e84 WU7)");
            }
        }
    }
    checks.push("Layer separation: no Layer 0 / meta kinds inside bundles".to_string());

    // The inventory must be complete and agree with what is on disk.
    let inventory_name = release_inventory_filename(version);
    let inventory_text = std::fs::read_to_string(staging.join(&inventory_name))
        .with_context(|| format!("missing `{inventory_name}`"))?;
    let published_inventory = ReleaseInventory::from_json(&inventory_text)?;
    let mut on_disk: Vec<String> = inventory
        .artifacts
        .iter()
        .map(|a| a.filename.clone())
        .collect();
    let mut listed: Vec<String> = published_inventory
        .artifacts
        .iter()
        .map(|a| a.filename.clone())
        .collect();
    on_disk.sort();
    listed.sort();
    if on_disk != listed {
        bail!(
            "release inventory does not match the staged payloads\n  on disk: {on_disk:?}\n  listed : {listed:?}"
        );
    }
    if published_inventory.version != version || published_inventory.tag != tag {
        bail!("release inventory version/tag do not match `{version}` / `{tag}`");
    }
    checks.push("ReleaseInventory matches the staged payload set".to_string());

    // SHA256SUMS must exist, cover everything, and agree with the manifests.
    let sums_text = std::fs::read_to_string(staging.join("SHA256SUMS"))
        .with_context(|| "missing `SHA256SUMS`")?;
    let sums = parse_sha256sums(&sums_text)?;

    let mut expected: Vec<String> = inventory
        .artifacts
        .iter()
        .map(|a| a.filename.clone())
        .collect();
    for platform in expected_platforms {
        expected.push(bundle_manifest_filename(version, *platform));
    }
    expected.push(inventory_name.clone());
    expected.sort();

    let mut listed_sums: Vec<String> = sums.keys().cloned().collect();
    listed_sums.sort();
    if expected != listed_sums {
        bail!(
            "SHA256SUMS does not cover exactly the release files\n  expected: {expected:?}\n  found   : {listed_sums:?}"
        );
    }
    for (name, digest) in &sums {
        let actual = sha256_file(&staging.join(name))?;
        if actual != *digest {
            bail!("SHA256SUMS digest mismatch for `{name}`: file {actual}, sums {digest}");
        }
    }
    checks.push(format!(
        "SHA256SUMS covers and matches {} files",
        sums.len()
    ));

    // The digests inside the manifest and inside SHA256SUMS must agree: two
    // independent encodings of the same fact must not diverge.
    for platform in expected_platforms {
        let manifest =
            BundleManifest::from_path(&staging.join(bundle_manifest_filename(version, *platform)))?;
        for component in &manifest.components {
            let from_sums = sums.get(&component.artifact).with_context(|| {
                format!(
                    "`{}` is in a manifest but not in SHA256SUMS",
                    component.artifact
                )
            })?;
            if *from_sums != component.sha256.as_str() {
                bail!(
                    "manifest and SHA256SUMS disagree on `{}`",
                    component.artifact
                );
            }
        }
    }
    checks.push("manifest digests and SHA256SUMS agree".to_string());

    // Expected payload set must match the published product surface exactly:
    // no unexpected archives.
    let unexpected: Vec<&ProducedArtifact> = inventory
        .artifacts
        .iter()
        .filter(|a| !published_components().any(|c| c.kind.stem() == a.component_name))
        .collect();
    if !unexpected.is_empty() {
        bail!(
            "unexpected payloads not in the published product surface: {:?}",
            unexpected.iter().map(|a| &a.filename).collect::<Vec<_>>()
        );
    }
    checks.push("no unexpected payloads beyond the declared product surface".to_string());

    Ok(VerifyReport {
        version: version.to_string(),
        tag: tag.to_string(),
        platforms: expected_platforms
            .iter()
            .map(|p| platform_token(*p).to_string())
            .collect(),
        payloads: inventory.artifacts.len(),
        manifests: manifest_count,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const VERSION: &str = "0.95.0";
    const TAG: &str = "v0.95.0";
    const TIER1: [Platform; 2] = [Platform::LinuxX86_64, Platform::LinuxAarch64];

    /// Stage canonical payloads for both Tier-1 platforms.
    fn stage() -> TempDir {
        let staging = TempDir::new().unwrap();
        for platform in TIER1 {
            for spec in published_components() {
                let name = artifact_filename(spec.kind.stem(), VERSION, platform);
                let body = format!("payload for {name}");
                fs::write(staging.path().join(&name), body).unwrap();
            }
        }
        staging
    }

    /// Stage and generate. The returned dir is the COMPLETE release directory.
    fn build() -> (TempDir, TempDir) {
        let staging = stage();
        let out = TempDir::new().unwrap();
        generate_release(
            staging.path(),
            out.path(),
            VERSION,
            TAG,
            "deadbeef",
            &TIER1,
            None,
        )
        .unwrap();
        (staging, out)
    }

    fn manifest_of(release: &Path, platform: Platform) -> BundleManifest {
        BundleManifest::from_path(&release.join(bundle_manifest_filename(VERSION, platform)))
            .unwrap()
    }

    /// Recompute SHA256SUMS over everything currently in the release dir, so a
    /// targeted adversarial test can isolate a check other than the sums.
    fn refresh_sums(release: &Path) {
        let mut covered: Vec<String> = fs::read_dir(release)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n != "SHA256SUMS")
            .collect();
        covered.sort();
        fs::write(
            release.join("SHA256SUMS"),
            sha256sums_for(release, &covered).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn generates_and_verifies_a_coherent_release() {
        let (_staging, out) = build();
        let report = verify_release(out.path(), VERSION, TAG, &TIER1).unwrap();
        assert_eq!(report.payloads, 6, "3 components x 2 platforms");
        assert_eq!(report.manifests, 2);
        assert!(report.render().contains("release-verify: OK"));
    }

    #[test]
    fn generated_manifest_contains_only_layer1_components() {
        let (_staging, out) = build();
        let m = manifest_of(out.path(), Platform::LinuxX86_64);
        let names: Vec<&str> = m.components.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["cognicode", "cognicode-mcp"]);
        assert!(
            !names.contains(&"cogh"),
            "Layer 0 must not be in the install plan"
        );
    }

    #[test]
    fn profile_core_and_reviewer_both_resolve() {
        let (_staging, out) = build();
        let m = manifest_of(out.path(), Platform::LinuxX86_64);
        assert_eq!(m.components_for_profile("core").len(), 1);
        assert_eq!(m.components_for_profile("reviewer").len(), 2);
    }

    #[test]
    fn sha256sums_is_sorted_and_timestamp_free() {
        let (_staging, out) = build();
        let sums = fs::read_to_string(out.path().join("SHA256SUMS")).unwrap();
        let names: Vec<&str> = sums
            .lines()
            .map(|l| l.split_once("  ").unwrap().1)
            .collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "SHA256SUMS must be filename-ascending");
        assert!(
            !sums.contains("2026") && !sums.contains("T00:"),
            "SHA256SUMS must contain no timestamps"
        );
    }

    // ---- WU17 adversarial cases ----

    #[test]
    fn reject_tag_not_matching_version() {
        let staging = stage();
        let out = TempDir::new().unwrap();
        let err = generate_release(
            staging.path(),
            out.path(),
            VERSION,
            "v9.9.9",
            "deadbeef",
            &TIER1,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("tag"), "got: {err}");
    }

    #[test]
    fn reject_missing_platform_lane() {
        let staging = stage();
        let out = TempDir::new().unwrap();
        fs::remove_file(staging.path().join(artifact_filename(
            "cognicode",
            VERSION,
            Platform::LinuxAarch64,
        )))
        .unwrap();
        let err = generate_release(
            staging.path(),
            out.path(),
            VERSION,
            TAG,
            "deadbeef",
            &TIER1,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("missing artifact"), "got: {err}");
    }

    #[test]
    fn reject_non_canonical_filename() {
        let staging = stage();
        let out = TempDir::new().unwrap();
        fs::write(
            staging.path().join("cognicode-0.95.0-linux-x86-64.tar.gz"),
            b"non canonical",
        )
        .unwrap();
        let err = generate_release(
            staging.path(),
            out.path(),
            VERSION,
            TAG,
            "deadbeef",
            &TIER1,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("not a canonical"), "got: {err}");
    }

    #[test]
    fn reject_wrong_version_in_filename() {
        let staging = stage();
        let out = TempDir::new().unwrap();
        fs::write(
            staging
                .path()
                .join("cognicode-0.94.0-x86_64-unknown-linux-gnu.tar.gz"),
            b"old",
        )
        .unwrap();
        let err = generate_release(
            staging.path(),
            out.path(),
            VERSION,
            TAG,
            "deadbeef",
            &TIER1,
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("declares version"), "got: {err}");
    }

    #[test]
    fn payload_modified_after_generation_fails_verification() {
        let (_staging, out) = build();
        verify_release(out.path(), VERSION, TAG, &TIER1).unwrap();

        let victim = out.path().join(artifact_filename(
            "cognicode",
            VERSION,
            Platform::LinuxX86_64,
        ));
        fs::write(&victim, b"tampered bytes").unwrap();

        let err = verify_release(out.path(), VERSION, TAG, &TIER1)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("mismatch"),
            "expected a digest mismatch, got: {err}"
        );
    }

    #[test]
    fn manifest_modified_after_sha256sums_fails_verification() {
        let (_staging, out) = build();
        let mpath = out
            .path()
            .join(bundle_manifest_filename(VERSION, Platform::LinuxX86_64));
        let mut text = fs::read_to_string(&mpath).unwrap();
        text.push_str("\n# tampered\n");
        fs::write(&mpath, text).unwrap();

        let err = verify_release(out.path(), VERSION, TAG, &TIER1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("digest mismatch"), "got: {err}");
    }

    #[test]
    fn reject_phantom_component_referencing_absent_archive() {
        let (_staging, out) = build();
        let mpath = out
            .path()
            .join(bundle_manifest_filename(VERSION, Platform::LinuxX86_64));
        let mut m = BundleManifest::from_path(&mpath).unwrap();
        let ghost = artifact_filename("explorer-api", VERSION, Platform::LinuxX86_64);
        m.components.push(BundleComponent {
            name: "explorer-api".to_string(),
            kind: ArtifactKind::ExplorerApi,
            version: VERSION.to_string(),
            artifact: ghost.clone(),
            sha256: crate::release_contract::ArtifactDigest::parse(
                "3b7e9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c",
            )
            .unwrap(),
            url: artifact_url(VERSION, &ghost),
            profiles: vec!["reviewer".to_string()],
        });
        fs::write(&mpath, m.to_yaml().unwrap()).unwrap();
        refresh_sums(out.path());

        let err = verify_release(out.path(), VERSION, TAG, &TIER1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("phantom"), "got: {err}");
    }

    #[test]
    fn reject_orphan_payload_absent_from_manifest() {
        let (_staging, out) = build();
        let mpath = out
            .path()
            .join(bundle_manifest_filename(VERSION, Platform::LinuxX86_64));
        let mut m = BundleManifest::from_path(&mpath).unwrap();
        m.components.retain(|c| c.name != "cognicode-mcp");
        fs::write(&mpath, m.to_yaml().unwrap()).unwrap();
        refresh_sums(out.path());

        let err = verify_release(out.path(), VERSION, TAG, &TIER1)
            .unwrap_err()
            .to_string();
        assert!(err.contains("orphan"), "got: {err}");
    }

    #[test]
    fn reject_x86_artifact_listed_in_arm_manifest() {
        let (_staging, out) = build();
        let mut m = manifest_of(out.path(), Platform::LinuxAarch64);
        for c in &mut m.components {
            c.artifact = artifact_filename(&c.name, VERSION, Platform::LinuxX86_64);
            c.url = artifact_url(VERSION, &c.artifact);
        }
        assert!(
            m.validate().is_err(),
            "x86 filenames must not validate in an ARM manifest"
        );
    }

    #[test]
    fn reject_placeholder_digest_in_manifest() {
        let (_staging, out) = build();
        let yaml = fs::read_to_string(
            out.path()
                .join(bundle_manifest_filename(VERSION, Platform::LinuxX86_64)),
        )
        .unwrap();
        for placeholder in [
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000000",
        ] {
            let spoofed = replace_hex_runs(&yaml, placeholder);
            assert!(
                BundleManifest::from_str(&spoofed).is_err(),
                "a placeholder digest must never parse"
            );
        }
    }

    /// Replace every 64-hex run in a document with `placeholder`.
    fn replace_hex_runs(yaml: &str, placeholder: &str) -> String {
        let mut out = String::new();
        let mut run = String::new();
        for ch in yaml.chars().chain(std::iter::once('\n')) {
            if ch.is_ascii_hexdigit() {
                run.push(ch);
                continue;
            }
            if run.len() == 64 {
                out.push_str(placeholder);
            } else {
                out.push_str(&run);
            }
            run.clear();
            out.push(ch);
        }
        out
    }
}
