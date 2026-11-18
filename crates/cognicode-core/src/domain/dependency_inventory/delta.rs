//! The change between two inventories of the same unit.
//!
//! Packages are matched by name and source identity, not by id: a version
//! bump is an update of the same package, not a removal plus an addition.
//! The delta reads only the resolved layer, so it is exact whatever the
//! manifests did.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::model::{ResolvedPackage, WorkspaceInventory};

/// A package present on both sides with something different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageUpdate {
    /// The package name.
    pub name: String,
    /// The source kind and location, as in the lockfile (`path` for none).
    pub source: String,
    /// The versions on the base side (several when the crate was locked in
    /// more than one version).
    pub from: Vec<String>,
    /// The versions on the head side.
    pub to: Vec<String>,
    /// Whether the recorded checksum changed for an unchanged version set.
    pub checksum_changed: bool,
    /// The number of dependency edges before and after, summed over versions.
    pub edges: (usize, usize),
}

/// What changed in the resolved layer between a base and a head inventory.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InventoryDelta {
    /// Packages only the head resolves.
    pub added: Vec<String>,
    /// Packages only the base resolved.
    pub removed: Vec<String>,
    /// Packages both resolve, with a different version, checksum or edges.
    pub updated: Vec<PackageUpdate>,
    /// Whether the two resolved graphs are identical.
    pub unchanged: bool,
}

type Key = (String, String);

fn key(package: &ResolvedPackage) -> Key {
    (
        package.name.clone(),
        package
            .source
            .lock_string()
            .unwrap_or_else(|| "path".to_string()),
    )
}

fn group(packages: &[ResolvedPackage]) -> BTreeMap<Key, Vec<&ResolvedPackage>> {
    let mut groups: BTreeMap<Key, Vec<&ResolvedPackage>> = BTreeMap::new();
    for package in packages {
        groups.entry(key(package)).or_default().push(package);
    }
    groups
}

fn versions(packages: &[&ResolvedPackage]) -> Vec<String> {
    let mut versions: Vec<String> = packages.iter().map(|p| p.version.clone()).collect();
    versions.sort();
    versions
}

fn checksums(packages: &[&ResolvedPackage]) -> Vec<Option<String>> {
    let mut sums: Vec<_> = packages
        .iter()
        .map(|p| (p.version.clone(), p.checksum.clone()))
        .collect();
    sums.sort();
    sums.into_iter().map(|(_, sum)| sum).collect()
}

/// The crates each package depends on, by name: a bump of the depended-on
/// crate is reported on that crate, not on every package that uses it.
fn edges(packages: &[&ResolvedPackage]) -> Vec<(String, Vec<String>)> {
    let mut rows: Vec<_> = packages
        .iter()
        .map(|p| {
            let mut names: Vec<String> = p
                .dependencies
                .iter()
                .map(|id| id.split('@').next().unwrap_or(id).to_string())
                .collect();
            names.dedup();
            (p.version.clone(), names)
        })
        .collect();
    rows.sort();
    rows
}

fn label(key: &Key, packages: &[&ResolvedPackage]) -> String {
    format!("{}@{}", key.0, versions(packages).join(","))
}

/// Computes the delta from `base` to `head`.
pub fn diff_inventories(base: &WorkspaceInventory, head: &WorkspaceInventory) -> InventoryDelta {
    diff_packages(&base.packages, &head.packages)
}

/// Computes the delta between two sets of resolved packages. An empty side
/// stands for a unit that does not exist on that side.
pub fn diff_packages(base: &[ResolvedPackage], head: &[ResolvedPackage]) -> InventoryDelta {
    let before = group(base);
    let after = group(head);
    let mut delta = InventoryDelta::default();
    for (key, packages) in &after {
        let Some(old) = before.get(key) else {
            delta.added.push(label(key, packages));
            continue;
        };
        let same_versions = versions(old) == versions(packages);
        let checksum_changed = same_versions && checksums(old) != checksums(packages);
        if same_versions && !checksum_changed && edges(old) == edges(packages) {
            continue;
        }
        delta.updated.push(PackageUpdate {
            name: key.0.clone(),
            source: key.1.clone(),
            from: versions(old),
            to: versions(packages),
            checksum_changed,
            edges: (
                old.iter().map(|p| p.dependencies.len()).sum(),
                packages.iter().map(|p| p.dependencies.len()).sum(),
            ),
        });
    }
    for (key, packages) in &before {
        if !after.contains_key(key) {
            delta.removed.push(label(key, packages));
        }
    }
    delta.unchanged =
        delta.added.is_empty() && delta.removed.is_empty() && delta.updated.is_empty();
    delta
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dependency_inventory::reconcile::inventory_unit;
    use crate::domain::dependency_inventory::units::{SourceFile, UnitInput};

    const MANIFEST: &str =
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n";

    fn lock(serde_version: &str, extra: &str) -> String {
        format!(
            "version = 3\n\n[[package]]\nname = \"app\"\nversion = \"0.1.0\"\ndependencies = [\n \"serde\",\n]\n\n[[package]]\nname = \"serde\"\nversion = \"{serde_version}\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n{extra}",
            "a".repeat(64)
        )
    }

    fn inventory(lock_text: &str) -> WorkspaceInventory {
        inventory_unit(&UnitInput {
            root: String::new(),
            lock: Some(SourceFile {
                path: "Cargo.lock".into(),
                bytes: lock_text.as_bytes().to_vec(),
            }),
            manifests: vec![SourceFile {
                path: "Cargo.toml".into(),
                bytes: MANIFEST.as_bytes().to_vec(),
            }],
        })
    }

    #[test]
    fn identical_lockfiles_have_an_empty_delta() {
        let a = inventory(&lock("1.0.100", ""));
        let delta = diff_inventories(&a, &a.clone());
        assert!(delta.unchanged);
    }

    #[test]
    fn a_version_bump_is_an_update_and_not_a_removal_plus_an_addition() {
        let base = inventory(&lock("1.0.100", ""));
        let head = inventory(&lock("1.0.200", ""));
        let delta = diff_inventories(&base, &head);
        assert!(delta.added.is_empty() && delta.removed.is_empty());
        assert_eq!(delta.updated.len(), 1);
        assert_eq!(delta.updated[0].name, "serde");
        assert_eq!(delta.updated[0].from, vec!["1.0.100"]);
        assert_eq!(delta.updated[0].to, vec!["1.0.200"]);
        assert!(!delta.unchanged);
    }

    #[test]
    fn a_new_package_is_added_and_a_dropped_one_removed() {
        let extra = "\n[[package]]\nname = \"itoa\"\nversion = \"1.0.0\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"\n";
        let base = inventory(&lock("1.0.100", ""));
        let head = inventory(&lock("1.0.100", extra));
        assert_eq!(diff_inventories(&base, &head).added, vec!["itoa@1.0.0"]);
        assert_eq!(diff_inventories(&head, &base).removed, vec!["itoa@1.0.0"]);
    }

    #[test]
    fn a_changed_checksum_for_the_same_version_is_flagged() {
        let base = inventory(&lock("1.0.100", ""));
        let head_text = lock("1.0.100", "").replace(&"a".repeat(64), &"c".repeat(64));
        let delta = diff_inventories(&base, &inventory(&head_text));
        assert_eq!(delta.updated.len(), 1);
        assert!(delta.updated[0].checksum_changed);
    }
}
