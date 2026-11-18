//! Transitive dependents: who reaches a package through lockfile edges.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::model::{ResolvedPackage, WorkspaceInventory};

/// A package that depends, directly or not, on the queried one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Dependent {
    /// The dependent package id.
    pub id: String,
    /// The shortest number of edges from the dependent to the target.
    pub depth: usize,
    /// Whether the dependent is a workspace member.
    pub workspace_member: bool,
}

/// The answer to a dependents query over one inventory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependentsReport {
    /// The packages the query matched (a bare name matches every version).
    pub targets: Vec<String>,
    /// The packages that reach a target, nearest first.
    pub dependents: Vec<Dependent>,
    /// The workspace members among the dependents: what a change to the
    /// target can affect first.
    pub affected_members: Vec<String>,
}

/// Whether `query` (`name` or `name@version`) names the package.
fn matches(package: &ResolvedPackage, query: &str) -> bool {
    match query.split_once('@') {
        Some((name, version)) => package.name == name && package.version == version,
        None => package.name == query,
    }
}

/// Finds the packages that depend on `query`, transitively.
///
/// Returns `None` when the query matches no package of the inventory: an
/// unknown package is not the same answer as a package nothing depends on.
pub fn dependents_of(inventory: &WorkspaceInventory, query: &str) -> Option<DependentsReport> {
    let targets: BTreeSet<&str> = inventory
        .packages
        .iter()
        .filter(|package| matches(package, query))
        .map(|package| package.id.as_str())
        .collect();
    if targets.is_empty() {
        return None;
    }
    let mut parents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for package in &inventory.packages {
        for dependency in &package.dependencies {
            parents
                .entry(dependency.as_str())
                .or_default()
                .push(package.id.as_str());
        }
    }
    let by_id: BTreeMap<&str, &ResolvedPackage> = inventory
        .packages
        .iter()
        .map(|package| (package.id.as_str(), package))
        .collect();
    let mut depth: BTreeMap<&str, usize> = BTreeMap::new();
    let mut queue: VecDeque<&str> = targets.iter().copied().collect();
    let mut seen: BTreeSet<&str> = targets.clone();
    while let Some(node) = queue.pop_front() {
        let here = depth.get(node).copied().unwrap_or(0);
        for &parent in parents.get(node).into_iter().flatten() {
            if seen.insert(parent) {
                depth.insert(parent, here + 1);
                queue.push_back(parent);
            }
        }
    }
    let mut dependents: Vec<Dependent> = depth
        .iter()
        .filter(|(id, _)| !targets.contains(*id))
        .map(|(id, depth)| Dependent {
            id: id.to_string(),
            depth: *depth,
            workspace_member: by_id[id].workspace_member,
        })
        .collect();
    dependents.sort_by(|a, b| a.depth.cmp(&b.depth).then_with(|| a.id.cmp(&b.id)));
    let affected_members = dependents
        .iter()
        .filter(|dependent| dependent.workspace_member)
        .map(|dependent| dependent.id.clone())
        .collect();
    Some(DependentsReport {
        targets: targets.into_iter().map(str::to_string).collect(),
        dependents,
        affected_members,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::dependency_inventory::reconcile::inventory_unit;
    use crate::domain::dependency_inventory::units::{SourceFile, UnitInput};

    const LOCK: &str = r#"version = 3

[[package]]
name = "app"
version = "0.1.0"
dependencies = ["mid"]

[[package]]
name = "mid"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
dependencies = ["leaf 2.0.0"]

[[package]]
name = "leaf"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"

[[package]]
name = "leaf"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
"#;

    fn inventory() -> WorkspaceInventory {
        inventory_unit(&UnitInput {
            root: String::new(),
            lock: Some(SourceFile {
                path: "Cargo.lock".into(),
                bytes: LOCK.as_bytes().to_vec(),
            }),
            manifests: vec![SourceFile {
                path: "Cargo.toml".into(),
                bytes: b"[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nmid = \"1\"\n".to_vec(),
            }],
        })
    }

    #[test]
    fn transitive_dependents_are_found_with_their_depth() {
        let report = dependents_of(&inventory(), "leaf@2.0.0").expect("known package");
        let rows: Vec<_> = report
            .dependents
            .iter()
            .map(|d| (d.id.as_str(), d.depth))
            .collect();
        assert_eq!(rows, vec![("mid@1.0.0", 1), ("app@0.1.0", 2)]);
        assert_eq!(report.affected_members, vec!["app@0.1.0"]);
    }

    #[test]
    fn a_version_in_the_query_keeps_the_other_version_out() {
        let report = dependents_of(&inventory(), "leaf@1.0.0").expect("known package");
        assert!(report.dependents.is_empty());
    }

    #[test]
    fn a_bare_name_matches_every_version() {
        let report = dependents_of(&inventory(), "leaf").expect("known package");
        assert_eq!(report.targets, vec!["leaf@1.0.0", "leaf@2.0.0"]);
    }

    #[test]
    fn an_unknown_package_is_not_the_same_as_having_no_dependents() {
        assert!(dependents_of(&inventory(), "missing").is_none());
    }
}
