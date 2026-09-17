//! `cogh::profile` — Profile filtering for bundle components.
//!
//! Under schema v2, `components[].profiles` is the single authoritative encoding
//! of profile membership (e84 R7). v1 also carried a `profiles[].include_kinds`
//! field which was parsed but never read, and which disagreed with the component
//! side; it was removed in e85.

use crate::bundle_manifest::{BundleComponent, BundleManifest};

/// Filter components by profile.
///
/// Returns only the components that include the given profile in their
/// `profiles` list.
pub fn filter_by_profile<'a>(
    manifest: &'a BundleManifest,
    profile: &str,
) -> Vec<&'a BundleComponent> {
    manifest.components_for_profile(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEV_YAML: &str = r#"
apiVersion: cognicode.bundle/v2
version: "0.95.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
  - name: full
    description: full profile
components:
  - name: cognicode
    kind: cognicode
    version: "0.95.0"
    artifact: cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "9f2c1d4b7e0a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [core]
  - name: cognicode-mcp
    kind: daemon-cli
    version: "0.95.0"
    artifact: cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz
    sha256: "1a3f5c8d1b2e4a6f8c0d2e4b6a8c0e2f4a6b8c0d2e4f6a8b0c2d4e9f2c1d4b7e"
    url: "https://github.com/Rubentxu/CogniCode/releases/download/v0.95.0/cognicode-mcp-0.95.0-x86_64-unknown-linux-gnu.tar.gz"
    profiles: [full]
"#;

    #[test]
    fn filter_by_profile_returns_matching_components() {
        let manifest = BundleManifest::from_str(DEV_YAML).unwrap();

        let core = filter_by_profile(&manifest, "core");
        assert_eq!(core.len(), 1);
        assert_eq!(core[0].name, "cognicode");

        let full = filter_by_profile(&manifest, "full");
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].name, "cognicode-mcp");

        assert!(filter_by_profile(&manifest, "nonexistent").is_empty());
    }
}
