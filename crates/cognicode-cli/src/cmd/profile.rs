//! `cogh::profile` — Profile filtering for bundle components.
//!
//! Provides functions to filter components by install profile.

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
    use crate::bundle_manifest::{BundleManifest, ComponentKind, Platform};

    #[test]
    fn filter_by_profile_returns_matching_components() {
        let yaml = r#"
apiVersion: cognicode.bundle/v1
version: "0.94.0"
platform: linux-x86-64
profiles:
  - name: core
    description: core profile
  - name: full
    description: full profile
components:
  - name: cli
    kind: Cognicode
    version: "0.94.0"
    artifact: cli.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000001"
    url: "https://example.com/cli.tar.gz"
    profiles: [core]
  - name: daemon
    kind: Daemon
    version: "0.94.0"
    artifact: daemon.tar.gz
    sha256: "0000000000000000000000000000000000000000000000000000000000000002"
    url: "https://example.com/daemon.tar.gz"
    profiles: [full]
"#;
        let manifest = BundleManifest::from_str(yaml).unwrap();

        let core_components = filter_by_profile(&manifest, "core");
        assert_eq!(core_components.len(), 1);
        assert_eq!(core_components[0].name, "cli");

        let full_components = filter_by_profile(&manifest, "full");
        assert_eq!(full_components.len(), 1);
        assert_eq!(full_components[0].name, "daemon");

        let nonexistent = filter_by_profile(&manifest, "nonexistent");
        assert!(nonexistent.is_empty());
    }
}
