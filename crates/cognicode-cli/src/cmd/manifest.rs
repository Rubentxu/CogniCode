//! `cogh::manifest` — `plugin.yaml` schema for the plugin registry.
//!
// Spec: `openspec/specs/cognicode-plugin/spec.md`.
// The manifest is YAML. Schema is **versioned** (apiVersion: cognicode/v1).
// Each version entry MUST have a `sha256` for integrity verification.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top-level plugin manifest (`plugin.yaml`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    #[serde(default = "default_kind")]
    pub kind: String,

    pub name: String,

    pub description: String,

    #[serde(default)]
    pub homepage: Option<String>,

    #[serde(default)]
    pub repository: Option<String>,

    #[serde(default)]
    pub versions: Vec<PluginVersion>,

    #[serde(default)]
    pub binaries: Vec<PluginBinary>,
}

fn default_kind() -> String { "Plugin".to_string() }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginVersion {
    /// Human-readable version ref (e.g. `v0.92.0`, `0.92.0`, `latest`).
    pub r#ref: String,
    /// Filename of the artifact in the GitHub Release.
    pub artifact: String,
    /// sha256 of the artifact (mandatory).
    pub sha256: String,
    /// Full URL to download the artifact (overrides registry discovery).
    #[serde(default)]
    pub url: Option<String>,
    /// Minimum cogh version required (semver).
    #[serde(default)]
    pub min_cogh: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginBinary {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl PluginManifest {
    /// Parse a YAML manifest from a path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read manifest {}", path.display()))?;
        Self::from_str(&text)
    }

    /// Parse a YAML manifest from a string.
    pub fn from_str(s: &str) -> Result<Self> {
        let m: PluginManifest = serde_yaml::from_str(s)
            .with_context(|| "parse plugin.yaml")?;
        m.validate()?;
        Ok(m)
    }

    /// Validate the manifest (presence + sha256 format).
    pub fn validate(&self) -> Result<()> {
        if self.api_version != "cognicode/v1" {
            return Err(anyhow::anyhow!(
                "unsupported apiVersion: {} (expected cognicode/v1)",
                self.api_version
            ));
        }
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("plugin name is empty"));
        }
        if self.versions.is_empty() {
            return Err(anyhow::anyhow!(
                "plugin {} has no versions",
                self.name
            ));
        }
        for v in &self.versions {
            if v.sha256.len() != 64 {
                return Err(anyhow::anyhow!(
                    "plugin {} version {}: sha256 must be 64 hex chars (got {})",
                    self.name, v.r#ref, v.sha256.len()
                ));
            }
            if !v.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(anyhow::anyhow!(
                    "plugin {} version {}: sha256 contains non-hex chars",
                    self.name, v.r#ref
                ));
            }
        }
        Ok(())
    }

    /// Find a version by ref.
    pub fn find_version(&self, r#ref: &str) -> Option<&PluginVersion> {
        self.versions.iter().find(|v| v.r#ref == r#ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
apiVersion: cognicode/v1
kind: Plugin
name: mcp-server
description: CogniCode MCP server
homepage: https://github.com/Rubentxu/CogniCode-plugins/mcp-server

versions:
  - ref: v0.92.0
    artifact: cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    url: https://github.com/Rubentxu/CogniCode-plugins/mcp-server/releases/download/v0.92.0/cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
    min_cogh: ">=0.1.0"
  - ref: v0.91.1
    artifact: cognicode-mcp-0.91.1-x86_64-unknown-linux-gnu.tar.gz
    sha256: a3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855

binaries:
  - name: cognicode-mcp
    path: bin/cognicode-mcp
    description: MCP server
"#;

    #[test]
    fn parse_full_manifest() {
        let m = PluginManifest::from_str(SAMPLE_YAML).unwrap();
        assert_eq!(m.api_version, "cognicode/v1");
        assert_eq!(m.name, "mcp-server");
        assert_eq!(m.versions.len(), 2);
        assert_eq!(m.versions[0].r#ref, "v0.92.0");
        assert_eq!(m.versions[0].min_cogh.as_deref(), Some(">=0.1.0"));
        assert_eq!(m.binaries.len(), 1);
    }

    #[test]
    fn parse_minimal_manifest() {
        let yaml = r#"
apiVersion: cognicode/v1
name: test
description: minimal
versions:
  - ref: "1.0.0"
    artifact: test.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
"#;
        let m = PluginManifest::from_str(yaml).unwrap();
        assert_eq!(m.name, "test");
        assert_eq!(m.versions.len(), 1);
    }

    #[test]
    fn reject_invalid_apiversion() {
        let yaml = r#"
apiVersion: cognicode/v999
name: test
description: bad
versions:
  - ref: "1.0.0"
    artifact: t.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
"#;
        let err = PluginManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("apiVersion"));
    }

    #[test]
    fn reject_invalid_sha256() {
        let yaml = r#"
apiVersion: cognicode/v1
name: test
description: bad sha
versions:
  - ref: "1.0.0"
    artifact: t.tar.gz
    sha256: "not-hex"
"#;
        let err = PluginManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("sha256"));
    }

    #[test]
    fn reject_short_sha256() {
        let yaml = r#"
apiVersion: cognicode/v1
name: test
description: short sha
versions:
  - ref: "1.0.0"
    artifact: t.tar.gz
    sha256: abc123
"#;
        let err = PluginManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("64 hex"));
    }

    #[test]
    fn find_version_by_ref() {
        let m = PluginManifest::from_str(SAMPLE_YAML).unwrap();
        assert!(m.find_version("v0.92.0").is_some());
        assert!(m.find_version("v0.91.1").is_some());
        assert!(m.find_version("v9.9.9").is_none());
    }

    #[test]
    fn reject_empty_name() {
        let yaml = r#"
apiVersion: cognicode/v1
name: ""
description: empty name
versions:
  - ref: "1.0.0"
    artifact: t.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
"#;
        let err = PluginManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("name"));
    }
}
