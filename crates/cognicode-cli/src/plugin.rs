//! Plugin registry client — fetches plugins from GitHub Releases.
//!
//! Registry URL pattern: https://github.com/{owner}/{repo}/releases

use serde::{Deserialize, Serialize};
use serde_json;

/// Plugin manifest — defines a cognicode plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// Plugin name (e.g. "cognicode-mcp", "sandbox-templates")
    pub name: String,
    /// Plugin version (semver)
    pub version: String,
    /// Plugin type
    pub kind: PluginKind,
    /// Download URL (GitHub Release asset or direct URL)
    pub url: Option<String>,
    /// sha256 integrity hash (mandatory per ADR-035)
    pub sha256: Option<String>,
    /// Dependencies on other plugins
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Installation profile (core, reviewer, full)
    #[serde(default)]
    pub profiles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PluginKind {
    Binary,
    SkillBundle,
    SandboxTemplate,
    IdeAdapter,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("network error fetching {0}: {1}")]
    Network(String, String),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("failed to parse: {0}")]
    Parse(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("sha256 mismatch: expected {expected}, got {actual}")]
    Sha256Mismatch { expected: String, actual: String },
}

/// GitHub Release asset
#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// GitHub Release API response
#[derive(Debug, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<ReleaseAsset>,
}

/// Fetches the latest release for a GitHub repo.
pub fn fetch_latest_release(owner: &str, repo: &str) -> Result<Release, PluginError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let response = ureq::get(&url)
        .set("User-Agent", "cognicode-cli")
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| PluginError::Network(url.clone(), e.to_string()))?;

    if response.status() == 404 {
        return Err(PluginError::NotFound(format!("{}/{}", owner, repo)));
    }

    let release: Release = serde_json::from_str(&response.into_string()?)
        .map_err(|e| PluginError::Parse(e.to_string()))?;

    Ok(release)
}

/// Finds an asset by name pattern in a release.
pub fn find_asset<'a>(release: &'a Release, pattern: &str) -> Option<&'a ReleaseAsset> {
    release.assets.iter().find(|a| a.name.contains(pattern))
}

/// Parse owner/repo from a GitHub URL or shorthand
pub fn parse_github_shorthand(input: &str) -> Option<(String, String)> {
    // Handle full URLs: https://github.com/owner/repo
    if input.starts_with("https://github.com/") {
        let rest = &input["https://github.com/".len()..];
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() >= 2 {
            return Some((parts[0].to_string(), parts[1].to_string()));
        }
    }
    // Handle shorthand: owner/repo
    let parts: Vec<&str> = input.split('/').collect();
    if parts.len() == 2 {
        return Some((parts[0].to_string(), parts[1].to_string()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_deserialization() {
        let yaml = r#"
name: cognicode-mcp
version: 0.94.7
kind: binary
sha256: abc123
dependencies: []
profiles:
  - core
"#;
        let manifest: PluginManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.name, "cognicode-mcp");
        assert_eq!(manifest.kind, PluginKind::Binary);
        assert_eq!(manifest.version, "0.94.7");
    }

    #[test]
    fn test_manifest_with_all_fields() {
        let yaml = r#"
name: sandbox-templates
version: 0.1.0
kind: sandboxtemplate
url: https://github.com/Rubentxu/CogniCode/releases/download/v0.1.0/sandbox-templates.tar.gz
sha256: deadbeef
dependencies:
  - cognicode-mcp
profiles:
  - core
  - full
"#;
        let manifest: PluginManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.name, "sandbox-templates");
        assert_eq!(manifest.kind, PluginKind::SandboxTemplate);
        assert_eq!(manifest.dependencies, vec!["cognicode-mcp"]);
        assert_eq!(manifest.profiles, vec!["core", "full"]);
    }

    #[test]
    fn test_parse_github_shorthand() {
        assert_eq!(
            parse_github_shorthand("Rubentxu/CogniCode"),
            Some(("Rubentxu".to_string(), "CogniCode".to_string()))
        );
        assert_eq!(
            parse_github_shorthand("https://github.com/Rubentxu/CogniCode"),
            Some(("Rubentxu".to_string(), "CogniCode".to_string()))
        );
        assert_eq!(parse_github_shorthand("invalid"), None);
        assert_eq!(parse_github_shorthand("a/b/c"), None);
    }

    #[test]
    fn test_find_asset() {
        let release = Release {
            tag_name: "v1.0.0".to_string(),
            assets: vec![
                ReleaseAsset {
                    name: "cognicode-mcp-linux-x86.tar.gz".to_string(),
                    browser_download_url: "https://example.com/linux.tar.gz".to_string(),
                },
                ReleaseAsset {
                    name: "cognicode-mcp-darwin-arm64.tar.gz".to_string(),
                    browser_download_url: "https://example.com/darwin.tar.gz".to_string(),
                },
            ],
        };
        assert!(find_asset(&release, "linux").is_some());
        assert!(find_asset(&release, "darwin").is_some());
        assert!(find_asset(&release, "windows").is_none());
    }

    #[test]
    #[ignore = "requires network access"]
    fn test_fetch_latest_release() {
        let release = fetch_latest_release("Rubentxu", "CogniCode").unwrap();
        assert!(release.tag_name.starts_with("v"));
        assert!(!release.assets.is_empty());
    }
}
