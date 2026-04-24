//! Auto-installation of debug adapters

use std::path::{Path, PathBuf};

use crate::adapter::configs::Language;
use crate::adapter::registry::AdapterRegistry;
use crate::error::{DebugError, Result};

/// Installer for debug adapters
pub struct AdapterInstaller {
    registry: AdapterRegistry,
    install_dir: PathBuf,
}

impl AdapterInstaller {
    /// Create a new installer
    pub fn new() -> Self {
        let install_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rcode-debug")
            .join("adapters");

        Self {
            registry: AdapterRegistry::new(),
            install_dir,
        }
    }

    /// Create installer with custom directory
    #[allow(dead_code)]
    pub fn with_dir(install_dir: PathBuf) -> Self {
        Self {
            registry: AdapterRegistry::new(),
            install_dir,
        }
    }

    /// Get the installation directory
    #[allow(dead_code)]
    pub fn install_dir(&self) -> &PathBuf {
        &self.install_dir
    }

    /// Check if an adapter is already installed
    #[allow(dead_code)]
    pub fn is_installed(&self, language: &Language) -> bool {
        if let Some(config) = self.registry.get(language) {
            let adapter_path = self.get_adapter_path(&config.name);
            std::path::Path::new(&adapter_path).exists()
        } else {
            false
        }
    }

    /// Get the path where an adapter would be installed
    fn get_adapter_path(&self, name: &str) -> PathBuf {
        self.install_dir.join(name)
    }

    /// Install an adapter for a language
    #[allow(dead_code)]
    pub async fn install(&self, language: &Language) -> Result<PathBuf> {
        let config = self.registry.get(language)
            .ok_or_else(|| DebugError::UnsupportedLanguage(format!("{:?}", language)))?;

        let download_config = config.download_url.as_ref()
            .ok_or_else(|| DebugError::Configuration(format!(
                "Adapter {} is not auto-installable", config.name
            )))?;

        // Check if already installed
        let install_path = self.get_adapter_path(&config.name);
        if install_path.exists() {
            tracing::info!("{} already installed at {:?}", config.name, install_path);
            return Ok(install_path);
        }

        // Create install directory
        tokio::fs::create_dir_all(&self.install_dir).await
            .map_err(|e| DebugError::Io(e))?;

        // Download the file
        tracing::info!("Downloading {} from {}", config.name, download_config.url);
        let temp_file = self.download_file(&download_config.url).await?;

        // Verify checksum if provided
        if let Some(expected_sha256) = &download_config.sha256 {
            self.verify_checksum(&temp_file, expected_sha256).await?;
        }

        // Extract archive
        tracing::info!("Extracting {} to {:?}", config.name, self.install_dir);
        self.extract_archive(&temp_file, &download_config.extract_to).await?;

        // Make executable
        if install_path.exists() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = tokio::fs::metadata(&install_path).await
                    .map_err(|e| DebugError::Io(e))?;
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                tokio::fs::set_permissions(&install_path, perms).await
                    .map_err(|e| DebugError::Io(e))?;
            }
        }

        // Clean up temp file
        tokio::fs::remove_file(&temp_file).await.ok();

        tracing::info!("Successfully installed {} to {:?}", config.name, install_path);
        Ok(install_path)
    }

    /// Download a file from URL
    async fn download_file(&self, url: &str) -> Result<PathBuf> {
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join(format!("rcode_debug_download_{}", std::process::id()));

        let response = reqwest::get(url).await
            .map_err(|e| DebugError::Configuration(format!("Failed to download: {}", e)))?;

        if !response.status().is_success() {
            return Err(DebugError::Configuration(format!(
                "Download failed with status: {}", response.status()
            )));
        }

        let bytes = response.bytes().await
            .map_err(|e| DebugError::Configuration(format!("Failed to read response: {}", e)))?;

        let mut file = tokio::fs::File::create(&temp_file).await
            .map_err(|e| DebugError::Io(e))?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes).await
            .map_err(|e| DebugError::Io(e))?;
        tokio::io::AsyncWriteExt::flush(&mut file).await
            .map_err(|e| DebugError::Io(e))?;

        Ok(temp_file)
    }

    /// Verify SHA256 checksum of a file
    async fn verify_checksum(&self, file_path: &Path, expected: &str) -> Result<()> {
        use sha2::{Sha256, Digest};

        let data = tokio::fs::read(file_path).await
            .map_err(|e| DebugError::Io(e))?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let result = hasher.finalize();

        let actual = hex::encode(result);

        if actual != expected {
            return Err(DebugError::Configuration(format!(
                "Checksum mismatch. Expected: {}, Got: {}", expected, actual
            )));
        }

        Ok(())
    }

    /// Extract an archive to a directory
    async fn extract_archive(&self, archive_path: &Path, extract_to: &str) -> Result<()> {
        let extension = archive_path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let target_dir = if extract_to.starts_with("/") {
            PathBuf::from(extract_to)
        } else {
            self.install_dir.join(extract_to)
        };

        tokio::fs::create_dir_all(&target_dir).await
            .map_err(|e| DebugError::Io(e))?;

        match extension {
            "vsix" | "zip" => {
                // Use zip crate for extraction
                self.extract_zip(archive_path, &target_dir).await?;
            }
            "tar" | "gz" | "tgz" => {
                // Use tar command for tar archives
                let status = tokio::process::Command::new("tar")
                    .args(["-xzf", &archive_path.to_string_lossy()])
                    .current_dir(&target_dir)
                    .status()
                    .await
                    .map_err(|e| DebugError::Io(e))?;

                if !status.success() {
                    return Err(DebugError::Configuration(
                        "Failed to extract tar archive".to_string()
                    ));
                }
            }
            _ => {
                // Assume it's a binary - just copy to install dir
                let dest = self.install_dir.join(
                    archive_path.file_name()
                        .ok_or_else(|| DebugError::Configuration("Invalid filename".to_string()))?
                );
                tokio::fs::copy(archive_path, &dest).await
                    .map_err(|e| DebugError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Extract a ZIP archive using the zip crate
    async fn extract_zip(&self, archive_path: &Path, target_dir: &Path) -> Result<()> {

        // Read file into memory synchronously (zip crate is sync)
        let file_data = std::fs::read(archive_path)
            .map_err(|e| DebugError::Io(e))?;

        let cursor = std::io::Cursor::new(file_data);
        let mut archive = zip::ZipArchive::new(cursor)
            .map_err(|e| DebugError::Configuration(format!("Invalid ZIP archive: {}", e)))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)
                .map_err(|e| DebugError::Configuration(format!("Failed to read ZIP entry: {}", e)))?;

            let outpath = match file.enclosed_name() {
                Some(path) => target_dir.join(path),
                None => continue,
            };

            if file.name().ends_with('/') {
                std::fs::create_dir_all(&outpath)
                    .map_err(|e| DebugError::Io(e))?;
            } else {
                if let Some(parent) = outpath.parent() {
                    if !parent.exists() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| DebugError::Io(e))?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(|e| DebugError::Io(e))?;
                std::io::copy(&mut file, &mut outfile)
                    .map_err(|e| DebugError::Io(e))?;
            }
        }

        Ok(())
    }

    /// Verify that an installed adapter works
    #[allow(dead_code)]
    pub async fn verify(&self, language: &Language) -> Result<bool> {
        let config = self.registry.get(language)
            .ok_or_else(|| DebugError::UnsupportedLanguage(format!("{:?}", language)))?;

        let adapter_path = self.get_adapter_path(&config.name);

        let output = tokio::process::Command::new(&adapter_path)
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                tracing::info!("{} is installed and working: {}",
                    config.name,
                    String::from_utf8_lossy(&out.stdout).trim()
                );
                Ok(true)
            }
            Ok(out) => {
                tracing::warn!("{} returned error: {}",
                    config.name,
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                Ok(false)
            }
            Err(e) => {
                tracing::warn!("{} not found or not executable: {}",
                    config.name,
                    e
                );
                Ok(false)
            }
        }
    }
}

impl Default for AdapterInstaller {
    fn default() -> Self {
        Self::new()
    }
}
