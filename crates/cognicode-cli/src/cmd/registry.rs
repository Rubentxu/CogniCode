//! `cogh::registry` — Plugin registry client + integrity verification.
//!
// Spec: `openspec/specs/cognicode-plugin/spec.md` §"Plugin discovery
//! via GitHub registry" + "`sha256` integrity check".
//!
// Responsibilities:
//! - Resolve a plugin name + version ref into a downloadable artifact URL
//! - Download the artifact (tarball) with retry
//! - Verify the sha256 of the downloaded artifact
//! - Extract the tarball into `~/.cognicode/versions/<v>/<plugin>/`
//! - Write the plugin manifest to `~/.cognicode/plugins/<plugin>/plugin.yaml`

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use sha2::{Digest, Sha256};

use crate::manifest::PluginManifest;

pub const DEFAULT_REGISTRY: &str = "https://api.github.com";

#[derive(Debug, Clone)]
pub struct RegistryConfig {
    pub base_url: String,
    pub token: Option<String>,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            base_url: DEFAULT_REGISTRY.to_string(),
            token: std::env::var("GITHUB_TOKEN").ok(),
        }
    }
}

/// Verify the sha256 of a file against the expected hex string.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    let mut f = std::fs::File::open(path)
        .with_context(|| format!("open {} for sha256", path.display()))?;
    let mut buf = [0u8; 8192];
    loop {
        let n = std::io::Read::read(&mut f, &mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected.to_lowercase() {
        return Err(anyhow!(
            "sha256 mismatch: expected {}, got {} (file: {})",
            expected, actual, path.display()
        ));
    }
    Ok(())
}

/// Download a URL to a local path. Streams to disk.
pub fn download_to(url: &str, dest: &Path, cfg: &RegistryConfig) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .context("build reqwest client")?;
    let mut req = client.get(url);
    if let Some(t) = &cfg.token {
        req = req.bearer_auth(t);
    }
    let mut resp = req.send().with_context(|| format!("GET {url}"))?;
    if !resp.status().is_success() {
        return Err(anyhow!("download failed: HTTP {} for {}", resp.status(), url));
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create_dir {}", parent.display()))?;
    }
    let mut out = std::fs::File::create(dest)
        .with_context(|| format!("create {}", dest.display()))?;
    resp.copy_to(&mut out).with_context(|| format!("write {}", dest.display()))?;
    Ok(())
}

/// Resolve a plugin name + version against a manifest into a download URL.
pub fn resolve_url(manifest: &PluginManifest, version: &str) -> Result<(String, String)> {
    // 1. If version is "latest", pick the last entry (cogh registry convention).
    let v = if version == "latest" {
        manifest.versions.last()
    } else {
        manifest.find_version(version)
    };
    let v = v.ok_or_else(|| {
        anyhow!("version '{}' not found in plugin '{}'", version, manifest.name)
    })?;
    let url = v.url.clone().ok_or_else(|| {
        anyhow!(
            "version '{}' has no 'url' field; provide --from-url or registry",
            v.r#ref
        )
    })?;
    Ok((url, v.sha256.clone()))
}

/// Extract a `.tar.gz` archive into `dest`.
pub fn extract_targz(archive: &Path, dest: &Path) -> Result<()> {
    let f = std::fs::File::open(archive)
        .with_context(|| format!("open archive {}", archive.display()))?;
    let gz = flate2::read::GzDecoder::new(f);
    let mut tar = tar::Archive::new(gz);
    tar.unpack(dest).with_context(|| {
        format!(
            "unpack {} into {}",
            archive.display(),
            dest.display()
        )
    })?;
    Ok(())
}

/// Write a plugin manifest to disk (the registry copy).
pub fn write_manifest(manifest: &PluginManifest, home: &Path) -> Result<PathBuf> {
    let plugin_dir = home.join("plugins").join(&manifest.name);
    std::fs::create_dir_all(&plugin_dir)
        .with_context(|| format!("create {}", plugin_dir.display()))?;
    let yaml = serde_yaml::to_string(manifest).context("serialize manifest")?;
    let target = plugin_dir.join("plugin.yaml");
    std::fs::write(&target, yaml)
        .with_context(|| format!("write manifest {}", target.display()))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_empty_file() {
        // sha256 of zero bytes is
        // e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let tmp = std::env::temp_dir().join(format!("cogh-sha-{}.bin", std::process::id()));
        std::fs::write(&tmp, b"").unwrap();
        verify_sha256(
            &tmp,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        )
        .unwrap();
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn sha256_detects_mismatch() {
        let tmp = std::env::temp_dir().join(format!("cogh-sha-{}.bin", std::process::id()));
        std::fs::write(&tmp, b"hello").unwrap();
        assert!(verify_sha256(
            &tmp,
            "0000000000000000000000000000000000000000000000000000000000000000"
        )
        .is_err());
        let _ = std::fs::remove_file(&tmp);
    }

    #[test]
    fn resolve_url_latest() {
        let yaml = r#"
apiVersion: cognicode/v1
name: mcp-server
description: test
versions:
  - ref: "0.92.0"
    artifact: x.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
    url: https://example.com/0.92.0.tar.gz
  - ref: "0.93.0"
    artifact: x.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
    url: https://example.com/0.93.0.tar.gz
"#;
        let m = PluginManifest::from_str(yaml).unwrap();
        let (url, _) = resolve_url(&m, "latest").unwrap();
        assert_eq!(url, "https://example.com/0.93.0.tar.gz");
    }

    #[test]
    fn resolve_url_specific() {
        let yaml = r#"
apiVersion: cognicode/v1
name: mcp-server
description: test
versions:
  - ref: "0.92.0"
    artifact: x.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
    url: https://example.com/0.92.0.tar.gz
"#;
        let m = PluginManifest::from_str(yaml).unwrap();
        let (url, sha) = resolve_url(&m, "0.92.0").unwrap();
        assert_eq!(url, "https://example.com/0.92.0.tar.gz");
        assert!(sha.starts_with("0000"));
    }
}
