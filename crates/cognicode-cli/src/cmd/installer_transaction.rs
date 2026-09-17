//! `cogh::installer_transaction` — Atomic install transaction state machine.
//!
//! Runs the full install pipeline through discrete stages:
//! ResolvingUrl → Downloading → VerifyingSha256 → Extracting → InstallingShims
//! → WritingManifest → Committed (or Failed).
//!
//! Uses a [`RollbackJournal`] to record side-effects so that a failed
//! install can be reversed cleanly.

use std::path::PathBuf;

use crate::bundle_manifest::{BundleManifest, Platform};
use crate::error::{BundleManifestError, InstallerError};
use crate::layout;
use crate::platform_adapter;
use crate::registry;
use crate::rollback_journal::{RollbackJournal, SideEffect};
use sha2::Digest;

/// Environment variable naming an explicit `bundle.yaml` to install from.
///
/// This is the seam that lets a release tool, a test, or a user point `cogh` at
/// an exact generated manifest instead of relying on any embedded fallback.
pub const ENV_BUNDLE_MANIFEST: &str = "COGNICODE_BUNDLE_MANIFEST";

/// Environment variable overriding the origin used to FETCH release artifacts.
///
/// Manifests always carry canonical `github.com` URLs, and the v2 contract
/// rejects anything else. This override changes only the fetch origin, never the
/// manifest. It is what makes mirrors, air-gapped installs, and deterministic
/// offline tests possible without weakening the canonical-URL rule.
pub const ENV_RELEASE_BASE_URL: &str = "COGNICODE_RELEASE_BASE_URL";

/// Rewrite a canonical component URL onto the configured fetch origin, if any.
pub fn resolve_download_url(canonical: &str) -> String {
    let Some(base) = std::env::var_os(ENV_RELEASE_BASE_URL) else {
        return canonical.to_string();
    };
    let base = base.to_string_lossy();
    let base = base.trim_end_matches('/');
    if base.is_empty() {
        return canonical.to_string();
    }
    match canonical.strip_prefix(crate::release_contract::RELEASE_DOWNLOAD_BASE) {
        Some(rest) => format!("{base}{rest}"),
        None => canonical.to_string(),
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
                let client = reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_secs(60))
                    .build()
                    .map_err(|e| InstallerError::Network("reqwest".into(), e.to_string()))?;
                let mut response = client
                    .get(resolve_download_url(&comp.url))
                    .send()
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
            // Extract each component
            let install_dir = layout::install_dir(&manifest.version);
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

            // Create shims for each binary
            let install_dir = layout::install_dir(&manifest.version);
            let adapter = platform_adapter::current_adapter();
            for comp in &manifest.components {
                let bin_path = install_dir.join(&comp.name).join("bin").join(&comp.name);
                if bin_path.exists() {
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

impl InstallerTransaction {
    /// Run the full install transaction for a given profile.
    ///
    /// Loads the bundle manifest (embedded or from disk), validates the
    /// version, then advances through each pipeline stage.
    /// Returns the path to the written install manifest on success.
    pub fn run(profile: &str) -> Result<PathBuf, InstallerError> {
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

        // Create journal and run through stages
        let journal = RollbackJournal::new();
        let mut tx = Self::Running {
            stage: InstallStage::ResolvingUrl,
            journal,
            manifest,
        };

        // Stage: ResolvingUrl → Downloading
        tx = tx.advance()?;

        // Stage: Downloading → VerifyingSha256
        tx = tx.advance()?;

        // Stage: VerifyingSha256 → Extracting
        tx = tx.advance()?;

        // Stage: Extracting → InstallingShims
        tx = tx.advance()?;

        // Stage: InstallingShims → WritingManifest
        tx = tx.advance()?;

        // Stage: WritingManifest → Committed
        tx = tx.commit()?;

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
    fn advance(self) -> Result<Self, InstallerError> {
        match self {
            Self::Running {
                stage,
                mut journal,
                manifest,
            } => {
                // Execute stage actions before transitioning
                if let Err(e) = advance_stage(stage, &mut journal, &manifest) {
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
    fn commit(self) -> Result<Self, InstallerError> {
        match self {
            Self::Running {
                mut journal,
                manifest,
                stage: _,
            } => {
                let manifest_path = layout::install_manifest_path(&manifest.version);

                // Serialize manifest to YAML
                let yaml = serde_yaml::to_string(&manifest)
                    .map_err(|e| InstallerError::Serialize(e.to_string()))?;

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
    #[serial]
    fn advance_skips_through_all_stages() {
        // Drive the real pipeline end to end against a real release that is
        // generated by the factory and served locally, so the Downloading,
        // VerifyingSha256 and Extracting stages genuinely execute.
        let _home = TempCognicodeHome::new();
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
            tx = tx.advance().unwrap();
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
        let result = committed.advance().unwrap();
        assert!(matches!(result, InstallerTransaction::Committed { .. }));

        // Failed
        let failed = InstallerTransaction::Failed {
            stage: InstallStage::Downloading,
            error: InstallerError::Unknown("test".into()),
        };
        let result = failed.advance().unwrap();
        assert!(matches!(result, InstallerTransaction::Failed { .. }));
    }

    #[test]
    #[serial]
    fn commit_writes_manifest_file() {
        // Writes to `cognicode_home()/install/<version>/manifest.yaml`.
        let _home = TempCognicodeHome::new();
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

        let result = tx.commit().unwrap();
        match result {
            InstallerTransaction::Committed { manifest_path } => {
                assert!(manifest_path.exists(), "manifest should be written");
                // Clean up
                let _ = std::fs::remove_file(&manifest_path);
            }
            other => panic!("expected Committed, got {:?}", other),
        }
    }

    // ----- e74 WU2: platform matching in the install pipeline -----

    /// Pin the e74 WU2 contract: `assert_host_platform` is part of
    /// the manifest-loading path. A bundle whose `platform` does not
    /// match the host's detected platform must be rejected before
    /// any install work happens, and the error message must be loud.
    ///
    /// This is the "no Windows-fallback-to-Linux" guarantee.
    #[test]
    fn installer_rejects_wrong_platform_bundle_with_loud_error() {
        // Simulate the wrong-platform scenario: parse the embedded
        // bundle, then point assert_host_platform at a non-matching
        // platform. The call must fail loudly with both the bundle
        // platform and the requested host platform in the error.
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
}
