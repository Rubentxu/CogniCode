//! `cogh::bundle_manifest` — `bundle.yaml` schema for co-versioned bundle releases.
//!
//! Spec: `openspec/specs/release-bundle-contract/spec.md`.
//! Schema v1 (`apiVersion: cognicode.bundle/v1`). A bundle aggregates the
//! components that cogh installs together as one co-versioned release.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Bundled release manifest (`bundle.yaml`).
///
/// Schema v1 (`apiVersion: cognicode.bundle/v1`).
/// A bundle aggregates the components that cogh installs together as one co-versioned release.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleManifest {
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    #[serde(default = "default_bundle_kind")]
    pub kind: String,

    pub version: String,

    pub platform: Platform,

    /// ISO-8601 release timestamp (optional).
    #[serde(default)]
    pub released_at: Option<String>,

    /// Named install profiles (e.g. core / reviewer / full).
    #[serde(default)]
    pub profiles: Vec<ProfileDef>,

    pub components: Vec<BundleComponent>,
}

fn default_bundle_kind() -> String { "Bundle".to_string() }

/// Target platform triple.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    LinuxX86_64,
    LinuxAarch64,
    MacOsX86_64,
    MacOsAarch64,
    WindowsX86_64,
}

/// Component kind (the binary / asset being installed).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ComponentKind {
    /// Reserved for forward compat (cogh doesn't install itself).
    Cogh,
    /// The cognicode daily CLI.
    Cognicode,
    /// The local daemon (also serves as MCP bridge per ADR-039).
    Daemon,
    /// IDE-agnostic skill bundles.
    Skill,
    /// Sandbox container templates.
    Sandbox,
    /// Reserved for forward compat (e36 — embedded Explorer assets).
    ExplorerAsset,
    /// A bundled plugin.
    Plugin,
}

/// A single component within a bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleComponent {
    pub name: String,
    pub kind: ComponentKind,
    pub version: String,
    pub artifact: String,
    /// Lowercase hex SHA256 of the artifact file (mandatory).
    pub sha256: String,
    /// Full download URL.
    pub url: String,
    /// Which profiles include this component (subset of bundle.profiles[].name).
    #[serde(default)]
    pub profiles: Vec<String>,
}

/// A named profile (e.g., "core", "reviewer", "full").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Component KINDS this profile includes (loose filter).
    #[serde(default)]
    pub include_kinds: Vec<ComponentKind>,
}

impl BundleManifest {
    /// Parse from a path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read bundle.yaml {}", path.display()))?;
        Self::from_str(&text)
    }

    /// Parse from a YAML string.
    pub fn from_str(s: &str) -> Result<Self> {
        let m: BundleManifest = serde_yaml::from_str(s)
            .with_context(|| "parse bundle.yaml")?;
        m.validate()?;
        Ok(m)
    }

    /// Validate the manifest.
    pub fn validate(&self) -> Result<()> {
        // 1. apiVersion must be `cognicode.bundle/v\d+`
        if !self.api_version.starts_with("cognicode.bundle/v") {
            anyhow::bail!(
                "apiVersion must start with `cognicode.bundle/v`, got {}",
                self.api_version
            );
        }
        let api_num = &self.api_version["cognicode.bundle/v".len()..];
        if api_num.parse::<u32>().is_err() {
            anyhow::bail!(
                "apiVersion numeric part is not a valid u32: {}",
                api_num
            );
        }

        // 2. version matches semver-ish (digits.digits.digits[-...])
        if !is_semver_like(&self.version) {
            anyhow::bail!(
                "version must match `^\\d+\\.\\d+\\.\\d+(-.*)?$`, got {}",
                self.version
            );
        }

        // 3. each component: sha256 is 64 hex; name unique; profiles non-empty
        let mut seen_names = std::collections::HashSet::new();
        for (i, c) in self.components.iter().enumerate() {
            if c.sha256.len() != 64 || !c.sha256.chars().all(|ch| ch.is_ascii_hexdigit()) {
                anyhow::bail!(
                    "component[{}] sha256 must be 64 hex chars, got {}",
                    i,
                    c.sha256
                );
            }
            if c.name.is_empty() {
                anyhow::bail!("component[{}] name must not be empty", i);
            }
            if !seen_names.insert(&c.name) {
                anyhow::bail!("duplicate component name: {}", c.name);
            }
            if c.profiles.is_empty() {
                anyhow::bail!(
                    "component[{}] profiles must not be empty (every component must declare at least one profile)",
                    i
                );
            }
        }

        // 4. profile names referenced by components must exist in profiles[]
        let profile_names: std::collections::HashSet<&str> =
            self.profiles.iter().map(|p| p.name.as_str()).collect();
        for c in &self.components {
            for p in &c.profiles {
                if !profile_names.contains(p.as_str()) {
                    anyhow::bail!(
                        "component[{}] references unknown profile `{}`",
                        c.name,
                        p
                    );
                }
            }
        }

        Ok(())
    }

    /// Assert the bundle's version matches the running cogh's CARGO_PKG_VERSION.
    /// Returns Err with a clear message if mismatched; CLI tool upgrade required.
    pub fn assert_pkg_version(&self) -> Result<()> {
        let pkg = env!("CARGO_PKG_VERSION");
        if self.version != pkg {
            anyhow::bail!(
                "bundle version `{}` does not match cogh's CARGO_PKG_VERSION `{}`; \
                 you are running a mismatched installer (CLI upgrade required)",
                self.version,
                pkg
            );
        }
        Ok(())
    }

    /// Return components that include the given profile.
    pub fn components_for_profile(&self, profile: &str) -> Vec<&BundleComponent> {
        self.components
            .iter()
            .filter(|c| c.profiles.iter().any(|p| p == profile))
            .collect()
    }

    /// Find a component by exact name (returns Option, no error).
    pub fn component_by_name(&self, name: &str) -> Option<&BundleComponent> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Find all components of a given kind.
    pub fn components_by_kind(&self, kind: ComponentKind) -> Vec<&BundleComponent> {
        self.components.iter().filter(|c| c.kind == kind).collect()
    }

    /// Owned-string adapter: converts this manifest into an install plan.
    ///
    /// Consumes the manifest and returns an [`InstallPlan`] with the same
    /// version, profile, and components. The profile defaults to `"default"`
    /// if the manifest has no profiles.
    pub fn into_install_plan(self) -> InstallPlan {
        let profile = self
            .profiles
            .first()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "default".to_string());
        InstallPlan {
            version: self.version,
            profile,
            components: self.components,
        }
    }

    /// Filter this manifest's components for a specific target platform.
    ///
    /// Since bundle manifests are already platform-specific by design,
    /// this method returns `Self` unchanged (the platform field is
    /// authoritative). The method exists to satisfy the adapter interface.
    pub fn for_target_platform(self, _platform: Platform) -> Self {
        // BundleManifest is already per-platform; the platform field
        // is validated at parse time and cannot be mismatched.
        self
    }
}

/// Install plan derived from a [`BundleManifest`].
///
/// Represents the resolved, platform-specific set of components to install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Bundle version (semver).
    pub version: String,
    /// Selected install profile name.
    pub profile: String,
    /// Components to install.
    pub components: Vec<BundleComponent>,
}

fn is_semver_like(s: &str) -> bool {
    // Strict: \d+\.\d+\.\d+(-.*)?
    let mut parts = s.splitn(2, '-');
    let numeric = parts.next().unwrap_or("");
    let _suffix = parts.next();
    let mut nums = numeric.split('.');
    let a = nums.next().unwrap_or("");
    let b = nums.next().unwrap_or("");
    let c = nums.next().unwrap_or("");
    if nums.next().is_some() {
        return false; // extra dots
    }
    a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok() && c.parse::<u32>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
apiVersion: cognicode.bundle/v1
kind: Bundle
version: "0.94.1"
platform: linux-x86-64
released_at: "2026-08-12T14:00:00Z"
profiles:
  - name: core
    description: "Installer + daily CLI"
    include_kinds: [Cogh, Cognicode]
  - name: reviewer
    description: "Adds daemon"
    include_kinds: [Cogh, Cognicode, Daemon]
  - name: full
    description: "Adds sandbox + skills"
    include_kinds: [Cogh, Cognicode, Daemon, Skill, Sandbox]
components:
  - name: cognicode-mcp
    kind: Daemon
    version: "0.94.1"
    artifact: cognicode-mcp-0.94.1-x86_64-unknown-linux-gnu.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cognicode-mcp-0.94.1.tar.gz"
    profiles: [reviewer, full]
  - name: skills-cognicode-core
    kind: Skill
    version: "0.94.1"
    artifact: skills-cognicode-core-0.94.1.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000002"
    url: "https://example.com/skills-cognicode-core-0.94.1.tar.gz"
    profiles: [full]
  - name: sandbox-templates
    kind: Sandbox
    version: "0.94.1"
    artifact: sandbox-templates-0.94.1.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000003"
    url: "https://example.com/sandbox-templates-0.94.1.tar.gz"
    profiles: [full]
"#;

    #[test]
    fn parse_full_bundle() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        assert_eq!(m.version, "0.94.1");
        assert_eq!(m.profiles.len(), 3);
        assert_eq!(m.profiles[0].name, "core");
        assert_eq!(m.components.len(), 3);
        assert_eq!(m.components[0].name, "cognicode-mcp");
        assert_eq!(m.components[0].kind, ComponentKind::Daemon);
    }

    #[test]
    fn parse_minimal_bundle() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.1"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: cognicode-cli
    kind: Cognicode
    version: "0.94.1"
    artifact: cognicode-0.94.1.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cognicode-0.94.1.tar.gz"
    profiles: [core]
"#;
        let m = BundleManifest::from_str(yaml).unwrap();
        assert_eq!(m.kind, "Bundle");
        assert!(m.released_at.is_none());
        assert_eq!(m.profiles.len(), 1);
        assert_eq!(m.components.len(), 1);
    }

    #[test]
    fn reject_bad_apiversion() {
        let yaml = r#"
apiVersion: cognicode.bundle/v999abc
version: "0.94.1"
platform: linux-x86-64
components:
  - name: test
    kind: Cognicode
    version: "0.94.1"
    artifact: test.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/test.tar.gz"
    profiles: [core]
"#;
        let err = BundleManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("apiVersion"));
    }

    #[test]
    fn reject_bad_sha256() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.1"
platform: linux-x86-64
components:
  - name: test
    kind: Cognicode
    version: "0.94.1"
    artifact: test.tar.gz
    sha256: "not-hex-at-all"
    url: "https://example.com/test.tar.gz"
    profiles: [core]
"#;
        let err = BundleManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("sha256"));
    }

    #[test]
    fn reject_duplicate_component_name() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.1"
platform: linux-x86-64
components:
  - name: cognicode-cli
    kind: Cognicode
    version: "0.94.1"
    artifact: a.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/a.tar.gz"
    profiles: [core]
  - name: cognicode-cli
    kind: Daemon
    version: "0.94.1"
    artifact: b.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000002"
    url: "https://example.com/b.tar.gz"
    profiles: [core]
"#;
        let err = BundleManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("duplicate"));
    }

    #[test]
    fn reject_unknown_profile_ref() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.1"
platform: linux-x86-64
profiles:
  - name: core
    description: core
    include_kinds: [Cognicode]
components:
  - name: test
    kind: Cognicode
    version: "0.94.1"
    artifact: test.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/test.tar.gz"
    profiles: [nonexistent]
"#;
        let err = BundleManifest::from_str(yaml).unwrap_err();
        assert!(err.to_string().contains("unknown profile"));
    }

    #[test]
    fn components_for_profile_filter() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        // reviewer includes cognicode-mcp (Daemon)
        let reviewer = m.components_for_profile("reviewer");
        assert_eq!(reviewer.len(), 1);
        assert_eq!(reviewer[0].name, "cognicode-mcp");
        // full includes all 3
        let full = m.components_for_profile("full");
        assert_eq!(full.len(), 3);
        // core has no matching components in our fixture
        let core = m.components_for_profile("core");
        assert_eq!(core.len(), 0);
    }

    #[test]
    fn components_by_kind_filter() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        let daemons = m.components_by_kind(ComponentKind::Daemon);
        assert_eq!(daemons.len(), 1);
        assert_eq!(daemons[0].name, "cognicode-mcp");
        let skills = m.components_by_kind(ComponentKind::Skill);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "skills-cognicode-core");
        let sandboxes = m.components_by_kind(ComponentKind::Sandbox);
        assert_eq!(sandboxes.len(), 1);
        assert_eq!(sandboxes[0].name, "sandbox-templates");
    }

    #[test]
    fn component_by_name_lookup() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        assert!(m.component_by_name("cognicode-mcp").is_some());
        assert_eq!(m.component_by_name("cognicode-mcp").unwrap().kind, ComponentKind::Daemon);
        assert!(m.component_by_name("nonexistent").is_none());
    }

    #[test]
    fn assert_pkg_version_mismatch() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "99.99.99"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: test
    kind: Cognicode
    version: "99.99.99"
    artifact: test.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/test.tar.gz"
    profiles: [core]
"#;
        let m = BundleManifest::from_str(yaml).unwrap();
        let err = m.assert_pkg_version().unwrap_err();
        assert!(err.to_string().contains("99.99.99"));
        assert!(err.to_string().contains("mismatch"));
    }

    #[test]
    fn assert_pkg_version_match() {
        // CARGO_PKG_VERSION is "0.94.0" per workspace
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
components:
  - name: test
    kind: Cognicode
    version: "0.94.0"
    artifact: test.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/test.tar.gz"
    profiles: [core]
"#;
        let m = BundleManifest::from_str(yaml).unwrap();
        // This should succeed because the workspace version is 0.94.0
        m.assert_pkg_version().expect("pkg version should match 0.94.0");
    }

    #[test]
    fn into_install_plan_uses_first_profile_as_default() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        let plan = m.into_install_plan();
        assert_eq!(plan.version, "0.94.1");
        assert_eq!(plan.profile, "core"); // first profile
        assert_eq!(plan.components.len(), 3);
    }

    #[test]
    fn into_install_plan_uses_default_when_no_profiles() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.1"
platform: linux-x86-64
profiles:
  - name: default
    description: default install profile
components:
  - name: cognicode-cli
    kind: Cognicode
    version: "0.94.1"
    artifact: cognicode-0.94.1.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cognicode-0.94.1.tar.gz"
    profiles: [default]
"#;
        let m = BundleManifest::from_str(yaml).unwrap();
        let plan = m.into_install_plan();
        assert_eq!(plan.profile, "default");
    }

    #[test]
    fn into_install_plan_preserves_components() {
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        let plan = m.into_install_plan();
        let names: Vec<_> = plan.components.iter().map(|c| c.name.clone()).collect();
        assert_eq!(names, vec!["cognicode-mcp", "skills-cognicode-core", "sandbox-templates"]);
    }

    #[test]
    fn for_target_platform_is_noop() {
        // BundleManifest is already platform-specific at parse time,
        // so for_target_platform returns self unchanged.
        let m = BundleManifest::from_str(SAMPLE_YAML).unwrap();
        let filtered = m.for_target_platform(Platform::LinuxX86_64);
        // All components should be preserved since the manifest is already Linux
        assert_eq!(filtered.components.len(), 3);
    }
}
