//! `cogh::installer_transaction` — Atomic install transaction state machine.
//!
//! Runs the full install pipeline through discrete stages:
//! ResolvingUrl → Downloading → VerifyingSha256 → Extracting → InstallingShims
//! → WritingManifest → Committed (or Failed).
//!
//! Uses a [`RollbackJournal`] to record side-effects so that a failed
//! install can be reversed cleanly.

use std::path::PathBuf;

use crate::error::{BundleManifestError, InstallerError};
use crate::bundle_manifest::BundleManifest;
use crate::layout;
use crate::rollback_journal::{RollbackJournal, SideEffect};
use crate::registry;
use sha2::Digest;

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
    let mut file = std::fs::File::open(path)
        .map_err(|e| InstallerError::Io(path.to_path_buf(), e))?;
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
    Committed {
        manifest_path: PathBuf,
    },
    /// Transaction failed at a given stage with an error.
    Failed {
        stage: InstallStage,
        error: InstallerError,
    },
}

/// Execute the actions for a given stage.
fn advance_stage(stage: InstallStage, journal: &mut RollbackJournal, manifest: &BundleManifest) -> Result<(), InstallerError> {
    match stage {
        InstallStage::ResolvingUrl => {
            // Validate all component URLs are reachable (basic check)
            for comp in &manifest.components {
                if comp.url.is_empty() {
                    return Err(InstallerError::Network("empty URL".into(), "no URL provided".into()));
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
                let mut response = client.get(&comp.url)
                    .send()
                    .map_err(|e| InstallerError::Network(comp.url.clone(), e.to_string()))?;
                if !response.status().is_success() {
                    return Err(InstallerError::Network(comp.url.clone(), format!("HTTP {}", response.status())));
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
                verify_sha256(&path, &comp.sha256)?;
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
            // Create shims for each binary
            let install_dir = layout::install_dir(&manifest.version);
            for comp in &manifest.components {
                let bin_path = install_dir.join(&comp.name).join("bin").join(&comp.name);
                if bin_path.exists() {
                    let shim_path = layout::shims_dir().join(&comp.name);
                    if let Some(parent) = shim_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| InstallerError::Io(shim_path.clone(), e))?;
                    }
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&bin_path, &shim_path)
                        .map_err(|e| InstallerError::Io(shim_path.clone(), e))?;
                    #[cfg(not(unix))]
                    std::fs::copy(&bin_path, &shim_path)
                        .map_err(|e| InstallerError::Io(shim_path.clone(), e))?;
                    journal.record(SideEffect::CreatedSymlink {
                        link: shim_path,
                        target: bin_path,
                    });
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

        // Parse and assert version
        let mut manifest = BundleManifest::from_str(&yaml)
            .map_err(|e| InstallerError::ManifestParse(BundleManifestError(e)))?;
        manifest
            .assert_pkg_version()
            .map_err(|e| InstallerError::VersionMismatch(BundleManifestError(e)))?;

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

    /// Load bundle manifest from disk or fall back to embedded asset.
    fn load_bundle_manifest() -> Result<String, InstallerError> {
        let bundle_path = layout::bundle_yaml_path();
        if bundle_path.exists() {
            std::fs::read_to_string(&bundle_path)
                .map_err(|e| InstallerError::Io(bundle_path, e))
        } else {
            // Embedded fallback for distribution installers
            Ok(include_str!("../../../../bundles/v0.94.1/bundle.yaml").to_string())
        }
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
                    return Ok(Self::Failed {
                        stage,
                        error: e,
                    });
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
                let yaml =
                    serde_yaml::to_string(&manifest).map_err(|e| InstallerError::Serialize(e.to_string()))?;

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
    fn advance_skips_through_all_stages() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: cognicode-cli
    kind: Cognicode
    version: "0.94.0"
    artifact: cognicode-0.94.0.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cognicode-0.94.0.tar.gz"
    profiles: [core]
"#;
        let manifest = BundleManifest::from_str(yaml).unwrap();
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
    fn commit_writes_manifest_file() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: cognicode-cli
    kind: Cognicode
    version: "0.94.0"
    artifact: cognicode-0.94.0.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cognicode-0.94.0.tar.gz"
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
}
