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

fn default_kind() -> String {
    "Plugin".to_string()
}

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
        let m: PluginManifest = serde_yaml::from_str(s).with_context(|| "parse plugin.yaml")?;
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
            return Err(anyhow::anyhow!("plugin {} has no versions", self.name));
        }
        for v in &self.versions {
            if v.sha256.len() != 64 {
                return Err(anyhow::anyhow!(
                    "plugin {} version {}: sha256 must be 64 hex chars (got {})",
                    self.name,
                    v.r#ref,
                    v.sha256.len()
                ));
            }
            if !v.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(anyhow::anyhow!(
                    "plugin {} version {}: sha256 contains non-hex chars",
                    self.name,
                    v.r#ref
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

/// Resolve the BinaryName of the MCP server binary that the plugin
/// declares.
///
/// DEBT-3.f: replaces the hardcoded `"cognicode-mcp"` literal that
/// `ide.rs::cmd_ide_install` used to pass to `home.shim_path(...)`.
/// The literal silently coupled BinaryName to a specific plugin's
/// declared binary; if a future plugin declared a different binary
/// (or no binaries at all), the IDE integration would point at a
/// shim that didn't exist.
///
/// Fails loudly when:
/// * the plugin manifest cannot be read or parsed;
/// * the plugin declares no binaries (no source of truth for the
///   MCP server binary name).
pub fn plugin_mcp_binary_name(manifest_path: &Path) -> Result<String> {
    let manifest = PluginManifest::from_path(manifest_path).with_context(|| {
        format!(
            "failed to read plugin manifest at {}",
            manifest_path.display()
        )
    })?;
    manifest
        .binaries
        .first()
        .map(|b| b.name.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "plugin manifest at {} declares no binaries; cannot resolve MCP server binary name",
                manifest_path.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
apiVersion: cognicode/v1
kind: Plugin
name: mcp-server
description: CogniCode MCP server
homepage: https://github.com/Rubentxu/CogniCode/releases

versions:
  - ref: v0.92.0
    artifact: cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    url: https://github.com/Rubentxu/CogniCode/releases/download/v0.92.0/cognicode-mcp-0.92.0-x86_64-unknown-linux-gnu.tar.gz
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

    // ========================================================================
    // DEBT-3.f — strict T3 + T4 for :730 elimination
    //
    // These tests pin the contract that the `plugin_mcp_binary_name`
    // helper (which replaced the hardcoded `"cognicode-mcp"` literal
    // at `ide.rs:730`) reads the BinaryName from the plugin
    // manifest's `binaries[0].name` rather than from a hardcoded
    // literal. The legacy inline form silently coupled BinaryName
    // to a specific plugin's choice; if a future plugin declared a
    // different binary (or no binaries at all), the IDE
    // integration would point at a shim that didn't exist.
    //
    // The tests use three pairwise-distinct identity strings
    // (PluginId / BinaryName) so the assertions cannot green for
    // the wrong reason.
    // ========================================================================

    /// DEBT-3.f strict T3: `plugin_mcp_binary_name` reads the
    /// declared `binaries[0].name`, NOT a hardcoded literal. Plants
    /// a synthetic plugin manifest with a deliberately-divergent
    /// BinaryName and asserts the helper returns it verbatim.
    #[test]
    fn t_debt3f_strict_plugin_mcp_binary_name_uses_manifest_decl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_yaml = tmp.path().join("plugin.yaml");
        // Three pairwise-distinct identity strings:
        //   PluginId   = "alt-plugin"
        //   BinaryName = "renamed-mcp-binary"  (NOT "cognicode-mcp")
        // If the helper read the literal "cognicode-mcp" the test
        // would fail; only by consulting the manifest does it
        // return "renamed-mcp-binary".
        let yaml = r#"
apiVersion: cognicode/v1
kind: Plugin
name: alt-plugin
description: synthetic plugin with divergent binary name
versions:
  - ref: "v1.0.0"
    artifact: renamed-mcp-binary-1.0.0.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
binaries:
  - name: renamed-mcp-binary
    path: bin/renamed-mcp-binary
    description: MCP server with a different filename
"#;
        std::fs::write(&plugin_yaml, yaml).expect("plant plugin.yaml");

        let binary_name =
            plugin_mcp_binary_name(&plugin_yaml).expect("helper must find a declared binary");
        assert_eq!(
            binary_name, "renamed-mcp-binary",
            "DEBT-3.f strict T3: helper must read binaries[0].name from the manifest, \
             not a hardcoded literal; got: {binary_name}"
        );
        // Belt and suspenders: explicitly assert the helper did NOT
        // return the legacy literal. (If someone "fixes" the helper
        // to return a constant string, this assertion fires.)
        assert_ne!(
            binary_name, "cognicode-mcp",
            "DEBT-3.f strict T3: helper MUST NOT return the legacy literal; \
             the whole point of this commit is to derive the name"
        );
    }

    /// DEBT-3.f strict T4: fail loudly when the plugin manifest
    /// declares no binaries. The legacy inline form would have
    /// written `home.shim_path("cognicode-mcp")` regardless of
    /// the plugin's declaration, silently pointing at a shim that
    /// didn't exist if the plugin had no binaries. The helper
    /// must refuse to return a derived name and surface the
    /// missing-identity error to the caller.
    #[test]
    fn t_debt3f_strict_plugin_mcp_binary_name_fails_loudly_when_no_binaries() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let plugin_yaml = tmp.path().join("plugin.yaml");
        // Synthetic plugin with EMPTY binaries list — the source
        // of truth has no declared binary name.
        let yaml = r#"
apiVersion: cognicode/v1
kind: Plugin
name: bin-less-plugin
description: synthetic plugin with no binaries
versions:
  - ref: "v1.0.0"
    artifact: bin-less-1.0.0.tar.gz
    sha256: 0000000000000000000000000000000000000000000000000000000000000000
binaries: []
"#;
        std::fs::write(&plugin_yaml, yaml).expect("plant plugin.yaml");

        let result = plugin_mcp_binary_name(&plugin_yaml);
        let err = result.expect_err(
            "DEBT-3.f strict T4: helper MUST fail loudly when the plugin declares no binaries; \
             silently returning a default would re-introduce the heuristic the cycle is eliminating",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("declares no binaries"),
            "DEBT-3.f strict T4: error message must mention 'declares no binaries' \
             so the missing-identity is observable to the operator; got: {msg}"
        );
    }
}
