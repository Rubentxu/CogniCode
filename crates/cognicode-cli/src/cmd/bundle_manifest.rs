//! `cogh::bundle_manifest` — `bundle.yaml` schema **v2** for co-versioned bundles.
//!
//! e85 implements the contract decided in e84
//! (`openspec/changes/e84-cognicode-distribution-artifact-contract/wu2-*`).
//!
//! ## Changes from v1
//!
//! * `apiVersion` is exactly `cognicode.bundle/v2`. There is no open-ended
//!   `vN` acceptance: the code supports the schema it understands.
//! * `profiles[].include_kinds` is **removed**. `components[].profiles` is the
//!   single, authoritative encoding of profile membership (e84 R7). v1 had two
//!   encodings, one of them inert, and they disagreed — the default profile
//!   resolved to zero components.
//! * `sha256` is parsed into an [`ArtifactDigest`], which rejects placeholders
//!   (e84 R5). v1 accepted `0000…0001` because it only checked length and hexness.
//! * artifact filename and URL are **derived** from the component name, version
//!   and platform, and must match exactly (e84 R1).
//! * a component's `kind` must be Layer 1 (e84 WU7): a bundle is `cogh`'s install
//!   plan, so it must not contain `cogh` itself, `SHA256SUMS`, or the inventory.
//! * every declared profile must resolve to at least one component.
//! * **`assert_pkg_version` is gone.** Under e84's Layer 0 / Layer 1 split, the
//!   installed runtime version is free to differ from the running `cogh`
//!   bootstrap version. Tying them together was the v1 assumption that made an
//!   embedded, version-pinned manifest look authoritative.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::release_contract::{
    ArtifactDigest, ArtifactKind, artifact_filename, artifact_url, platform_token,
};

/// The only `apiVersion` this code understands.
pub const BUNDLE_API_VERSION: &str = "cognicode.bundle/v2";

/// Bundled release manifest (`bundle.yaml`), schema v2.
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

    /// Named install profiles (e.g. core / reviewer).
    #[serde(default)]
    pub profiles: Vec<ProfileDef>,

    pub components: Vec<BundleComponent>,
}

fn default_bundle_kind() -> String {
    "Bundle".to_string()
}

/// Target platform triple.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    LinuxX86_64,
    LinuxAarch64,
    MacOsX86_64,
    MacOsAarch64,
    WindowsX86_64,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Platform::LinuxX86_64 => "linux-x86-64",
            Platform::LinuxAarch64 => "linux-aarch64",
            Platform::MacOsX86_64 => "mac-os-x86-64",
            Platform::MacOsAarch64 => "mac-os-aarch64",
            Platform::WindowsX86_64 => "windows-x86-64",
        })
    }
}

/// A single installable component within a bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleComponent {
    /// Canonical component name. Must equal `kind.stem()`.
    pub name: String,
    /// Layer 1 artifact kind. Layer 0 and meta kinds are rejected.
    pub kind: ArtifactKind,
    /// Component version. Must equal the bundle version.
    pub version: String,
    /// Derived artifact filename. Must equal the canonical name.
    pub artifact: String,
    /// Computed SHA256 of the artifact. Placeholders are rejected.
    pub sha256: ArtifactDigest,
    /// Derived download URL. Must equal the canonical URL.
    pub url: String,
    /// Profiles that include this component. The authoritative encoding.
    #[serde(default)]
    pub profiles: Vec<String>,
}

/// A named profile (e.g., "core", "reviewer").
///
/// v2 has no `include_kinds`: membership lives on the component, where it
/// travels with the artifact and cannot drift.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProfileDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
}

impl BundleManifest {
    /// Parse from a path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read bundle.yaml {}", path.display()))?;
        Self::from_str(&text)
    }

    /// Parse from a YAML string and validate.
    pub fn from_str(s: &str) -> Result<Self> {
        let m: BundleManifest = serde_yaml::from_str(s).with_context(|| "parse bundle.yaml")?;
        m.validate()?;
        Ok(m)
    }

    /// Serialise back to YAML, canonical field order.
    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Validate every v2 contract invariant.
    pub fn validate(&self) -> Result<()> {
        // 1. Exactly the schema this code understands.
        if self.api_version != BUNDLE_API_VERSION {
            bail!(
                "apiVersion must be exactly `{BUNDLE_API_VERSION}`, got `{}`; \
                 this build understands only that schema",
                self.api_version
            );
        }

        // 2. Semver-ish version.
        if !is_semver_like(&self.version) {
            bail!(
                "version must match `^\\d+\\.\\d+\\.\\d+(-.*)?$`, got {}",
                self.version
            );
        }

        // 3. Profiles: non-empty, unique names.
        if self.profiles.is_empty() {
            bail!("bundle must declare at least one profile");
        }
        let mut profile_names: HashSet<&str> = HashSet::new();
        for (i, p) in self.profiles.iter().enumerate() {
            if p.name.trim().is_empty() {
                bail!("profile[{i}] name must not be empty");
            }
            if !profile_names.insert(p.name.as_str()) {
                bail!("duplicate profile name: {}", p.name);
            }
        }

        // 4. Components.
        if self.components.is_empty() {
            bail!("bundle must declare at least one component");
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for (i, c) in self.components.iter().enumerate() {
            if c.name.trim().is_empty() {
                bail!("component[{i}] name must not be empty");
            }
            if !seen.insert(c.name.as_str()) {
                bail!("duplicate component name: {}", c.name);
            }

            // 4a. A bundle is cogh's install plan: Layer 1 only.
            if !c.kind.is_installable() {
                bail!(
                    "component[{}] `{}` has kind {:?} which is not a Layer 1 \
                     installable component; a bundle must not contain the \
                     bootstrap, checksums or release metadata",
                    i,
                    c.name,
                    c.kind
                );
            }

            // 4b. One name per component.
            if c.name != c.kind.stem() {
                bail!(
                    "component[{}] name `{}` must equal its kind stem `{}`",
                    i,
                    c.name,
                    c.kind.stem()
                );
            }

            // 4c. Version lockstep.
            if c.version != self.version {
                bail!(
                    "component `{}` version `{}` != bundle version `{}`",
                    c.name,
                    c.version,
                    self.version
                );
            }

            // 4d. Derived artifact name.
            let expected_artifact = artifact_filename(&c.name, &self.version, self.platform);
            if c.artifact != expected_artifact {
                bail!(
                    "component `{}` artifact `{}` is not canonical; expected `{}`",
                    c.name,
                    c.artifact,
                    expected_artifact
                );
            }

            // 4e. Derived URL.
            let expected_url = artifact_url(&self.version, &c.artifact);
            if c.url != expected_url {
                bail!(
                    "component `{}` url `{}` is not canonical; expected `{}`",
                    c.name,
                    c.url,
                    expected_url
                );
            }

            // 4f. Profile membership: non-empty, all declared.
            if c.profiles.is_empty() {
                bail!("component `{}` must declare at least one profile", c.name);
            }
            for p in &c.profiles {
                if !profile_names.contains(p.as_str()) {
                    bail!("component `{}` references unknown profile `{}`", c.name, p);
                }
            }
        }

        // 5. No profile may be a no-op.
        for name in &profile_names {
            if self.components_for_profile(name).is_empty() {
                bail!(
                    "profile `{name}` resolves to zero components; \
                     a profile that installs nothing is a contract violation"
                );
            }
        }

        // 6. The platform token must be a known one (guards against a future
        //    variant added to the enum without a mapping).
        if platform_token(self.platform).is_empty() {
            bail!(
                "platform `{}` has no canonical Rust target token",
                self.platform
            );
        }

        Ok(())
    }

    /// Assert that this manifest's `platform` matches the host's platform.
    ///
    /// Deterministic host → platform matching. Wrong-platform bundles fail
    /// loudly, with no fallback to another platform's artifacts.
    pub fn assert_host_platform(&self, host: Platform) -> Result<()> {
        if self.platform != host {
            bail!(
                "bundle platform `{:?}` does not match host platform `{:?}`; \
                 refusing to load a wrong-platform bundle \
                 (no fallback to another platform's artifacts)",
                self.platform,
                host
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

    /// Find a component by exact name.
    pub fn component_by_name(&self, name: &str) -> Option<&BundleComponent> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Find all components of a given kind.
    pub fn components_by_kind(&self, kind: ArtifactKind) -> Vec<&BundleComponent> {
        self.components.iter().filter(|c| c.kind == kind).collect()
    }

    /// The profile names, in declaration order.
    pub fn profile_names(&self) -> Vec<&str> {
        self.profiles.iter().map(|p| p.name.as_str()).collect()
    }

    /// Convert into an install plan, defaulting to the first declared profile.
    pub fn into_install_plan(self) -> InstallPlan {
        let profile = self
            .profiles
            .first()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "core".to_string());
        InstallPlan {
            version: self.version,
            profile,
            components: self.components,
        }
    }
}

/// Install plan derived from a [`BundleManifest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Bundle version (semver).
    pub version: String,
    /// Selected install profile name.
    pub profile: String,
    /// Components to install.
    pub components: Vec<BundleComponent>,
}

/// Resolve the BinaryName of the DaemonCli component from the
/// bundle manifest at the given path.
///
/// DEBT-3.f: replaces the hardcoded `"cognicode-mcp"` literal that
/// `install.rs::run_install` and `ide.rs::cmd_ide_install` used to
/// pass to `home.shim_path(...)`. The literal silently coupled
/// BinaryName to a specific component name; if a future bundle
/// shipped a different DaemonCli component (or no DaemonCli at
/// all), the runtime would write a shim that didn't exist.
///
/// Fails loudly when:
/// * the bundle manifest cannot be read or parsed;
/// * the bundle has no DaemonCli component (no source of truth
///   for the MCP server binary name).
pub fn daemon_cli_binary_name(manifest_path: &Path) -> Result<String> {
    let manifest = BundleManifest::from_path(manifest_path).with_context(|| {
        format!(
            "failed to read bundle manifest at {}",
            manifest_path.display()
        )
    })?;
    manifest
        .components_by_kind(ArtifactKind::DaemonCli)
        .first()
        .map(|c| c.name.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "bundle manifest at {} declares no DaemonCli component; \
                 cannot resolve MCP server binary name",
                manifest_path.display()
            )
        })
}

fn is_semver_like(s: &str) -> bool {
    let mut parts = s.splitn(2, '-');
    let numeric = parts.next().unwrap_or("");
    let mut nums = numeric.split('.');
    let a = nums.next().unwrap_or("");
    let b = nums.next().unwrap_or("");
    let c = nums.next().unwrap_or("");
    if nums.next().is_some() {
        return false;
    }
    a.parse::<u32>().is_ok() && b.parse::<u32>().is_ok() && c.parse::<u32>().is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A realistic-looking, non-placeholder digest.
    const DIGEST: &str = "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e";

    fn bundle_with(components: &str, profiles: &str) -> String {
        format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-09-17T00:00:00Z"
profiles:
{profiles}
components:
{components}
"#
        )
    }

    fn core_and_reviewer() -> String {
        "  - name: core\n    description: Daily CLI\n  - name: reviewer\n    description: Adds daemon\n"
            .to_string()
    }

    fn cognicode_component(profiles: &str) -> String {
        format!(
            r#"  - name: cognicode
    kind: cognicode
    version: "0.95.0"
    artifact: cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "{DIGEST}"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [{profiles}]
"#
        )
    }

    #[test]
    fn parse_valid_v2_bundle() {
        let yaml = bundle_with(&cognicode_component("core, reviewer"), &core_and_reviewer());
        let m = BundleManifest::from_str(&yaml).unwrap();
        assert_eq!(m.version, "0.95.0");
        assert_eq!(m.platform, Platform::LinuxX86_64);
        assert_eq!(m.profile_names(), vec!["core", "reviewer"]);
        assert_eq!(m.components.len(), 1);
        assert_eq!(m.components_for_profile("core").len(), 1);
        assert_eq!(m.components_for_profile("reviewer").len(), 1);
    }

    #[test]
    fn round_trips_through_yaml() {
        let yaml = bundle_with(&cognicode_component("core, reviewer"), &core_and_reviewer());
        let m = BundleManifest::from_str(&yaml).unwrap();
        let again = BundleManifest::from_str(&m.to_yaml().unwrap()).unwrap();
        assert_eq!(m, again);
    }

    // ---- WU17 adversarial cases ----

    #[test]
    fn reject_v1_apiversion() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace("cognicode.bundle/v2", "cognicode.bundle/v1");
        assert!(
            BundleManifest::from_str(&yaml)
                .unwrap_err()
                .to_string()
                .contains("apiVersion")
        );
    }

    #[test]
    fn reject_arbitrary_future_apiversion() {
        // v1 accepted any numeric `vN`. v2 supports exactly what it understands.
        for bad in [
            "cognicode.bundle/v3",
            "cognicode.bundle/v999",
            "cognicode.bundle/v2beta",
        ] {
            let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
                .replace("cognicode.bundle/v2", bad);
            assert!(
                BundleManifest::from_str(&yaml).is_err(),
                "apiVersion `{bad}` must be rejected"
            );
        }
    }

    #[test]
    fn reject_placeholder_digest() {
        for placeholder in [
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        ] {
            let yaml = bundle_with(&cognicode_component("core, reviewer"), &core_and_reviewer())
                .replace(DIGEST, placeholder);
            let err = format!("{:#}", BundleManifest::from_str(&yaml).unwrap_err());
            assert!(
                err.contains("placeholder"),
                "placeholder {placeholder} must be rejected as a placeholder, got: {err}"
            );
        }
    }

    #[test]
    fn reject_malformed_digest() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace(DIGEST, "not-hex-at-all");
        assert!(BundleManifest::from_str(&yaml).is_err());
    }

    #[test]
    fn reject_wrong_artifact_filename() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer()).replace(
            "cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz",
            "cognicode-0.95.0-linux-x86-64.tar.gz",
        );
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("not canonical"), "got: {err}");
    }

    #[test]
    fn reject_wrong_platform_token_in_filename() {
        // An artifact built for x86 listed in an aarch64 manifest.
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace("platform: linux-x86-64", "platform: linux-aarch64");
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("not canonical"), "got: {err}");
    }

    #[test]
    fn reject_non_canonical_url() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace("releases/download/v0.95.0/", "releases/download/");
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("url"), "got: {err}");
    }

    #[test]
    fn reject_version_mismatch() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer()).replace(
            "    version: \"0.95.0\"\n    artifact",
            "    version: \"0.94.0\"\n    artifact",
        );
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("!= bundle version"), "got: {err}");
    }

    #[test]
    fn reject_duplicate_component() {
        let one = cognicode_component("core");
        let yaml = bundle_with(&format!("{one}{one}"), &core_and_reviewer());
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("duplicate component"), "got: {err}");
    }

    #[test]
    fn reject_duplicate_profile_name() {
        let profiles = "  - name: core\n    description: a\n  - name: core\n    description: b\n";
        let yaml = bundle_with(&cognicode_component("core"), profiles);
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("duplicate profile"), "got: {err}");
    }

    #[test]
    fn reject_unknown_profile_reference() {
        let yaml = bundle_with(
            &cognicode_component("core, nonexistent"),
            &core_and_reviewer(),
        );
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("unknown profile"), "got: {err}");
    }

    #[test]
    fn reject_empty_component_profiles() {
        let yaml = bundle_with(&cognicode_component(""), &core_and_reviewer());
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("at least one profile"), "got: {err}");
    }

    #[test]
    fn reject_profile_that_resolves_to_zero_components() {
        // `core` is declared but nothing is in it. This was the v1 defect that
        // made the DEFAULT install path install nothing.
        let yaml = bundle_with(&cognicode_component("reviewer"), &core_and_reviewer());
        let err = BundleManifest::from_str(&yaml).unwrap_err().to_string();
        assert!(err.contains("zero components"), "got: {err}");
    }

    #[test]
    fn reject_layer0_and_meta_kinds_in_a_bundle() {
        for kind in ["cogh", "bundle-manifest", "release-inventory", "checksums"] {
            let c = format!(
                r#"  - name: {kind_to_test}
    kind: {kind}
    version: "0.95.0"
    artifact: "x"
    sha256: "{DIGEST}"
    url: "y"
    profiles: [core]
"#,
                kind_to_test = kind
            );
            let yaml = bundle_with(&c, &core_and_reviewer());
            assert!(
                BundleManifest::from_str(&yaml).is_err(),
                "kind `{kind}` must not be allowed inside a bundle"
            );
        }
    }

    #[test]
    fn reject_component_name_not_matching_kind() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace("- name: cognicode", "- name: cognicode-renamed");
        assert!(BundleManifest::from_str(&yaml).is_err());
    }

    #[test]
    fn reject_bad_version_string() {
        let yaml = bundle_with(&cognicode_component("core"), &core_and_reviewer())
            .replace("version: \"0.95.0\"", "version: \"not-semver\"");
        assert!(BundleManifest::from_str(&yaml).is_err());
    }

    #[test]
    fn reject_missing_profiles_section() {
        let yaml = bundle_with(&cognicode_component("core"), "  []\n");
        assert!(BundleManifest::from_str(&yaml).is_err());
    }

    #[test]
    fn wrong_platform_bundle_is_rejected() {
        let yaml = bundle_with(&cognicode_component("core, reviewer"), &core_and_reviewer());
        let m = BundleManifest::from_str(&yaml).unwrap();
        assert!(m.assert_host_platform(Platform::LinuxAarch64).is_err());
        assert!(m.assert_host_platform(Platform::LinuxX86_64).is_ok());
    }

    #[test]
    fn there_is_no_pkg_version_lockstep() {
        // e84's Layer 0 / Layer 1 split means the installed runtime version is
        // free to differ from the running cogh bootstrap version. A bundle at a
        // version other than this binary's must still validate; that coupling is
        // what v1's `assert_pkg_version` enforced and v2 removes.
        let yaml = bundle_with(&cognicode_component("core, reviewer"), &core_and_reviewer())
            .replace("0.95.0", "1.2.3");
        let m = BundleManifest::from_str(&yaml)
            .expect("a bundle version different from cogh's must still validate");
        assert_eq!(m.version, "1.2.3");
        assert_ne!(
            m.version,
            env!("CARGO_PKG_VERSION").to_string(),
            "the fixture must differ from this binary's version for the test to mean anything"
        );
    }

    // ========================================================================
    // DEBT-3.f — strict T2a + T2b for :67 elimination
    //
    // `daemon_cli_binary_name` replaces the hardcoded `"cognicode-mcp"`
    // literal at `install.rs:67`. The literal silently coupled BinaryName
    // to a specific component name; the helper reads it from the bundle
    // manifest's DaemonCli component.
    //
    // The manifest validator enforces `BundleComponent.name ==
    // kind.stem()` (e84 WU5). For ArtifactKind::DaemonCli, the stem
    // is "cognicode-mcp", so the helper's return value happens to
    // equal the legacy literal today. The strict tests therefore
    // cannot rely on string divergence — they pin the contract by:
    //
    //   T2a — bundle with a DaemonCli component returns the
    //         manifest-declared name (proves the helper READS the
    //         manifest, not a constant).
    //   T2b — bundle WITHOUT a DaemonCli component fails loudly
    //         (proves the helper does NOT silently fall back to a
    //         hardcoded literal).
    //
    // The pair rules out the regression mode where someone
    // "simplifies" the helper to `fn daemon_cli_binary_name(_: &Path)
    // -> Result<String> { Ok("cognicode-mcp".into()) }`.
    // ========================================================================

    fn bundle_with_daemon_cli(daemon_name: &str, version: &str) -> String {
        // Construct a valid v2 bundle whose only component is a
        // DaemonCli. The name MUST equal kind.stem() per the
        // validator, so daemon_name == "cognicode-mcp" today.
        // The test uses this to verify the helper follows the
        // manifest, not a literal.
        //
        // Note: ArtifactKind serializes as kebab-case in YAML
        // ("daemon-cli", not "DaemonCli"); the parser is strict
        // about variant names.
        let artifact = format!("{daemon_name}-{version}-x86_64-unknown-linux-gnu.tar.gz");
        let url = format!(
            "https://github.com/Rubentxu/CogniCode/releases/download/v{version}/{artifact}"
        );
        format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "{version}"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
  - name: {daemon_name}
    kind: daemon-cli
    version: "{version}"
    artifact: {artifact}
    sha256: "{DIGEST}"
    url: "{url}"
    profiles: [core]
"#
        )
    }

    fn bundle_without_daemon_cli() -> String {
        // A bundle whose only component is a non-DaemonCli kind
        // (Cognicode). The validator requires every declared
        // profile to resolve to ≥1 component, so we declare only
        // the profile the cognicode component uses ("core").
        // Used to verify the helper refuses to derive a name
        // when the manifest has no DaemonCli.
        format!(
            r#"
apiVersion: cognicode.bundle/v2
kind: Bundle
version: "0.95.0"
platform: linux-x86-64
released_at: "2026-09-18T00:00:00Z"
profiles:
  - name: core
    description: Daily CLI
components:
{cogh_comp}
"#,
            cogh_comp = cognicode_component("core"),
        )
    }

    /// DEBT-3.f strict T2a: `daemon_cli_binary_name` reads the
    /// declared `BundleComponent.name` for the DaemonCli kind, NOT
    /// a hardcoded literal. Plants a synthetic bundle on disk and
    /// asserts the helper returns the manifest-declared name.
    #[test]
    fn t_debt3f_strict_daemon_cli_binary_name_uses_manifest_decl() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("manifest.yaml");
        let yaml = bundle_with_daemon_cli("cognicode-mcp", "0.95.0");
        std::fs::write(&manifest_path, &yaml).expect("plant bundle manifest");

        let binary_name = daemon_cli_binary_name(&manifest_path)
            .expect("helper must resolve the DaemonCli component name");
        assert_eq!(
            binary_name, "cognicode-mcp",
            "DEBT-3.f strict T2a: helper must return the manifest-declared name; \
             got: {binary_name}"
        );
    }

    /// DEBT-3.f strict T2b: `daemon_cli_binary_name` fails loudly
    /// when the bundle has no DaemonCli component. The legacy
    /// inline form would have written
    /// `home.shim_path("cognicode-mcp")` regardless; the helper
    /// must refuse and surface the missing-identity error.
    #[test]
    fn t_debt3f_strict_daemon_cli_binary_name_fails_loudly_when_no_daemon_cli() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let manifest_path = tmp.path().join("manifest.yaml");
        let yaml = bundle_without_daemon_cli();
        std::fs::write(&manifest_path, &yaml).expect("plant bundle manifest");

        let result = daemon_cli_binary_name(&manifest_path);
        let err = result.expect_err(
            "DEBT-3.f strict T2b: helper MUST fail loudly when the bundle declares no DaemonCli; \
             silently returning a default would re-introduce the heuristic the cycle is eliminating",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("declares no DaemonCli component"),
            "DEBT-3.f strict T2b: error message must mention 'declares no DaemonCli component' \
             so the missing-identity is observable to the operator; got: {msg}"
        );
    }
}
